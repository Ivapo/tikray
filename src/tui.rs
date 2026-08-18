//! The browsing surface — the second caller of the core §2.2 exists for.
//!
//! **The image is not in ratatui's model.** Ratatui diffs and writes *cells*; it
//! has no notion of a region it must leave alone, so an image survives exactly
//! as long as the cells under it stay blank between frames and the diff
//! therefore says nothing about them. It is not an image widget, it is an image
//! behind a hole in the layout. Measured, 2026-08-17: it survived an identical
//! frame and a counter changing outside the pane, and died the moment a widget
//! drew text *inside* the pane (§2.14).
//!
//! So the rule is: **reserve a pane, render nothing into it, and re-emit
//! whenever the frame is invalidated.**
//!
//! [`crate::display::display`] is not reusable here and
//! [`crate::display::sequence`] is. `display` reads the whole-window viewport
//! from [`crate::term::viewport`], and a pane is not the window: at natural size
//! the image spilled over the border and across the neighbouring pane, because
//! OSC 1337 draws at the cursor and iTerm2 clips it to nothing. This module
//! calls `sequence` with pane-relative pixels instead — Phase 1's pure seam
//! paying off a second time.

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::queue;
use crossterm::style::ResetColor;
use image::DynamicImage;
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::convert::{self, Output};
use crate::display;
use crate::error::TikrayError;
use crate::term;

/// How often the loop wakes to notice a signal (see [`Interrupt`]).
const POLL: Duration = Duration::from_millis(100);

/// The keys, in the footer. Short enough that it needs no second row.
///
/// The hidden count joins them here rather than in the list pane's border title,
/// which is the obvious home and the wrong one: that title is 30% of the window
/// wide, [`compact`] exists only because it is already tight enough to need
/// `$HOME` rewritten to `~`, and ratatui truncates — so the count would vanish on
/// exactly the narrow window where it matters. The footer spans the window.
fn keys(all: bool, hidden: usize, zoom: u32) -> String {
    // Three states. Off, the key re-hides both kinds, so "images only" would be
    // half the story; on with nothing hidden, a key advertising a filter that
    // holds nothing back is noise and the listing is its own evidence.
    let filter = if all {
        "  . filtered".to_string()
    } else if hidden > 0 {
        format!("  . all ({hidden} hidden)")
    } else {
        String::new()
    };
    let level = if zoom > 1 {
        format!("  {zoom}x")
    } else {
        String::new()
    };
    format!(" ↑/↓ move   ⏎ open   ← up{filter}   +/- zoom{level}   P/J convert   q quit ")
}

/// Why there is no preview, for each standing condition.
///
/// Both are *standing*: they hold for every entry, not for the highlighted one,
/// which is why they are reported before any per-entry reason. A user who reads
/// "not an image" on file after file has been told the wrong thing.
/// The scroll thumb, drawn over the list border so it costs no width.
const SCROLL_THUMB: &str = "\u{2590}";

const NOT_ITERM2: &str = "no preview: this does not look like iTerm2";
const NO_PIXELS: &str = "no preview: this terminal reports no pixel size";

/// A pane's size in pixels, or [`None`] where no image may be emitted (§2.14).
///
/// `cell` is [`crate::term::cell_size`]'s answer, and [`None`] there means the
/// terminal reported no pixel geometry — §2.6's `auto` fallback is *unusable*
/// in a pane, since `width=auto` is precisely the spill this exists to prevent.
///
/// **A zero in either pane axis is the same answer**, and is asserted rather
/// than inherited: a pane shrunk to nothing under a bordered layout otherwise
/// multiplies out to `(0, …)`, [`crate::display::fit`] returns [`None`] for a
/// zero axis, and `sequence` then emits `width=auto;height=auto` — §2.14's row 8
/// reached from the other side, and the one path by which the spill can still
/// get in.
pub fn pane_viewport(pane: (u16, u16), cell: Option<(u32, u32)>) -> Option<(u32, u32)> {
    let (cell_w, cell_h) = cell?;
    let (cols, rows) = pane;
    if cols == 0 || rows == 0 {
        return None;
    }
    Some((u32::from(cols) * cell_w, u32::from(rows) * cell_h))
}

/// The escape sequence for `img` sized to `pane`, or [`None`] to emit nothing.
///
/// `Ok(None)` means *draw the explanation and emit nothing*: the TUI runs and
/// shows one line saying why there is no preview. Refusing to launch would be
/// worse — the file list is useful on its own — and `auto` would be worse still,
/// because it spills across the layout.
///
/// One function covers both branches, so there is exactly one place that decides
/// whether bytes reach the terminal. That matters because the failure it guards
/// is a spilled image, which no assertion can see.
pub fn pane_sequence(
    img: &DynamicImage,
    pane: (u16, u16),
    cell: Option<(u32, u32)>,
) -> Result<Option<Vec<u8>>, TikrayError> {
    match pane_viewport(pane, cell) {
        None => Ok(None),
        Some(viewport) => display::sequence(img, Some(viewport)).map(Some),
    }
}

/// A rectangle of source pixels to crop: `(x, y, width, height)`.
pub type CropRect = (u32, u32, u32, u32);

/// What a zoom level shows: the rect to crop, and the size to emit.
pub type ZoomView = (CropRect, (u32, u32));

/// Where in the pane an image starts, in cells, and the bytes that draw it.
pub type Placement = ((u16, u16), Vec<u8>);

/// The layout, as one pure function so the split can be asserted.
///
/// Extracted in Phase 12 because Phases 6, 7 and 8 all key their arithmetic to
/// `image` at runtime while their gates use synthetic constants — so nothing
/// asserted the real rectangle, and widening the split is exactly the change
/// that could move the picture while claiming to move only a border.
pub struct Panes {
    pub list: Rect,
    pub status: Rect,
    pub image: Rect,
    pub footer: Rect,
}

/// Where everything goes, for a whole-window `area`.
///
/// The list takes **30%**: enough for 44 characters of filename at 158 columns
/// and 21 at 80, once two borders and the highlight symbol come off. A
/// max-width list (`Max(40)` + `Min(0)`) is better on a wide window and worse on
/// a narrow one, where it takes half the screen — recorded as available, not
/// taken.
pub fn panes(area: Rect) -> Panes {
    let [main, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]).areas(main);
    let inner = Block::bordered().inner(right);
    let [status, image] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    Panes {
        list: left,
        status,
        image,
        footer,
    }
}

/// The next selection, wrapping at both ends.
///
/// ratatui's own movement does not wrap — `ListState::select_next` is
/// `saturating_add` and `select_previous` is `saturating_sub`, and today's
/// behaviour only *looks* clamped because the list widget resets an
/// out-of-range selection at draw time. `g`/`G` keep `select_first`/`select_last`
/// and do not come through here: they are absolute jumps, and a wrapping jump is
/// a contradiction.
///
/// An empty listing answers 0 rather than dividing by zero.
pub fn step(current: usize, len: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if forward {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    }
}

/// Where the scroll thumb sits in a track of `viewport` rows: `(start, height)`.
///
/// [`None`] when everything fits — an indicator always present indicates
/// nothing.
///
/// The arithmetic is `PanEx`'s `render_scroll_thumb`, including its **round to
/// nearest**, which is what lands the thumb flush at the bottom instead of a row
/// short. Truncating instead agrees at both ends — the product is exact there —
/// and differs mid-scroll, which is why `tests/gate_phase12.rs` asserts a
/// mid-scroll offset rather than the two ends.
///
/// One adaptation: `PanEx` takes the track and the viewport separately, because
/// one of its panes has a capacity that differs from its row count. Collapsing
/// them is expressible here only because tikray's track *is* its viewport — the
/// list block's inner height.
pub fn scroll_thumb(len: usize, viewport: usize, offset: usize) -> Option<(usize, usize)> {
    if viewport == 0 || len <= viewport {
        return None;
    }
    let thumb = (viewport * viewport / len).clamp(1, viewport);
    let travel = viewport - thumb;
    let max_offset = len - viewport;
    let start = (offset.min(max_offset) * travel + max_offset / 2) / max_offset;
    Some((start, thumb))
}

/// The zoom levels, in multiples of the fit size.
///
/// Multiples of *fit* rather than absolute ratios, because fit already varies
/// with the pane: "twice the size of what I am looking at" is predictable where
/// "100%" is not. Three because the ask was a few, and because a fourth step of
/// a centre-only zoom shows so little that it argues for panning instead.
pub const LEVELS: [u32; 3] = [1, 2, 4];

/// What a zoom level shows: the source rect to crop, and the size to emit.
///
/// [`None`] exactly where [`crate::display::fit`] is `None`, at every level.
///
/// **Level 1 returns the whole source and `fit`'s pair by construction**, not by
/// arithmetic. The rule below is stated over a real-valued scale, and
/// `floor(pane / s)` lands on the wrong side whenever the ratio is not exactly
/// representable in binary64 — measured, 32 of 240 sampled sizes cropped a row
/// they should not, including every 48-55 x 1003 source. Everything shipped runs
/// through level 1, so it is made true structurally rather than hoped for.
///
/// Above that: the visible region is `floor(pane / (s*L))`, floored at 1 and
/// clamped to the source, centred; the emitted size is `round(region * s*L)`,
/// floored at 1 and clamped to the pane. Both floors restore `fit`'s own
/// `max(1, ..)` — a 4000x1 source computes `round(1 * 0.16) = 0` otherwise,
/// which is not a legal argument value. The clamp is defence in depth and does
/// fire in one corner the floor creates: a pane axis under 4 pixels, which real
/// cell geometry never produces.
pub fn zoom_view(native: (u32, u32), pane: (u32, u32), level: u32) -> Option<ZoomView> {
    let fitted = display::fit(native, Some(pane))?;
    if level <= 1 {
        return Some(((0, 0, native.0, native.1), fitted));
    }

    let target = display::scale(native, pane) * f64::from(level);
    let region = |px: u32, src: u32| -> u32 {
        let want = (f64::from(px) / target).floor() as u32;
        want.max(1).min(src)
    };
    let (vw, vh) = (region(pane.0, native.0), region(pane.1, native.1));

    let emit =
        |v: u32, limit: u32| -> u32 { ((f64::from(v) * target).round() as u32).max(1).min(limit) };

    Some((
        ((native.0 - vw) / 2, (native.1 - vh) / 2, vw, vh),
        (emit(vw, pane.0), emit(vh, pane.1)),
    ))
}

/// The offset **and** the bytes for a pane at a zoom level, from one call.
///
/// One call because the alternative spills. [`pane_offset`] computes `fit`'s
/// pair internally, so at 2x a 1200x800 preview emits 20 cell-rows while that
/// function still answers `(0, 4)` — twenty rows placed four rows down ends four
/// rows past a twenty-row pane, which is §2.14's spill produced by the one
/// feature deliberately pushing against the border.
///
/// `Ok(None)` wherever [`pane_sequence`] returns it, meaning *draw the
/// explanation and emit nothing*.
pub fn pane_view(
    img: &DynamicImage,
    pane: (u16, u16),
    cell: Option<(u32, u32)>,
    level: u32,
) -> Result<Option<Placement>, TikrayError> {
    let Some(viewport) = pane_viewport(pane, cell) else {
        return Ok(None);
    };
    let Some(cell) = cell else { return Ok(None) };
    let Some(((x, y, w, h), emit)) = zoom_view((img.width(), img.height()), viewport, level) else {
        return Ok(None);
    };

    let cropped = img.crop_imm(x, y, w, h);
    let bytes = display::sequence_at(&cropped, emit)?;
    Ok(Some((centre_offset(emit, pane, cell), bytes)))
}

/// Where inside its pane a `image`-sized picture starts, in **cells**.
///
/// `image` is the pair [`crate::display::fit`] returned — the size the terminal
/// is told — and **never the buffer's native size**. That distinction is the one
/// trap in this arithmetic: almost every real image is larger than its pane, so
/// a native pair makes the footprint exceed the pane, the offset saturate to
/// `(0, 0)`, and every picture sit in the corner while every assertion below
/// stays green. [`pane_offset`] exists so the wrong call cannot be made.
///
/// The footprint rounds **up**: a 24×24 image is 0.67 of a 36px row, and a floor
/// would call that zero rows and centre it as though it occupied nothing. (It is
/// not spill prevention — `fit` already guarantees `ceil(h/cell) ≤ pane`, so the
/// floor never pushes an image out of its pane. The reason is smaller and exact.)
///
/// Saturating, because this is public and pure even though `fit` clamps first.
pub fn centre_offset(image: (u32, u32), pane: (u16, u16), cell: (u32, u32)) -> (u16, u16) {
    let footprint = |px: u32, per_cell: u32| -> u16 {
        if per_cell == 0 {
            return u16::MAX;
        }
        u16::try_from(px.div_ceil(per_cell)).unwrap_or(u16::MAX)
    };

    let free_x = pane.0.saturating_sub(footprint(image.0, cell.0));
    let free_y = pane.1.saturating_sub(footprint(image.1, cell.1));
    (free_x / 2, free_y / 2)
}

/// [`centre_offset`] with the fitting done for you, over the same arguments
/// [`pane_sequence`] takes.
///
/// `(0, 0)` wherever `pane_sequence` returns `Ok(None)`: there is nothing to
/// place in that state, which is why this returns a bare pair where its sibling
/// returns an [`Option`].
pub fn pane_offset(img: &DynamicImage, pane: (u16, u16), cell: Option<(u32, u32)>) -> (u16, u16) {
    let Some(cell) = cell else { return (0, 0) };
    let Some(viewport) = pane_viewport(pane, Some(cell)) else {
        return (0, 0);
    };
    match display::fit((img.width(), img.height()), Some(viewport)) {
        Some(fitted) => centre_offset(fitted, pane, cell),
        None => (0, 0),
    }
}

/// What a conversion wrote, and whether it had to flatten to do it.
///
/// Carried out of [`convert_to`] rather than printed, because §2.13 put the
/// notice in `src/main.rs:run` as an `eprintln!` and stderr inside the
/// alternate screen paints straight over the display. Public fields: the gate
/// reads `flattened` from another crate.
#[derive(Debug, PartialEq, Eq)]
pub struct Written {
    pub path: PathBuf,
    pub flattened: bool,
}

/// Where `P` or `J` would write, given the highlighted file.
///
/// `set_extension` on the source, so `photo.svg` + [`Output::Png`] is
/// `photo.png`. It replaces only the final component, so `archive.tar.gz`
/// becomes `archive.tar.png` — the same answer `tikray convert` gives, which is
/// why the two surfaces agree.
pub fn destination(source: &Path, target: Output) -> PathBuf {
    let mut dest = source.to_path_buf();
    dest.set_extension(match target {
        Output::Png => "png",
        // `resolve` accepts both spellings inbound, so which one comes back out
        // is a choice rather than a default.
        Output::Jpeg => "jpg",
    });
    dest
}

/// Write the highlighted file out in `target`, beside itself.
///
/// Touches the filesystem and **never the terminal**, which is what puts it in
/// the machine half of the gate.
///
/// The order is **source check, exists check, write**, and `force` skips only
/// the second. Letting a confirm through the first would perform in place
/// exactly the destructive re-encode that check exists to prevent — on the
/// second press of a key whose first press said the wrong thing.
///
/// The source test is not string equality. On a case-insensitive filesystem a
/// source named `photo.PNG` derives `photo.png`: a different string naming the
/// same file, which string equality misses and a confirm would then overwrite.
/// Both sides must resolve for the answer to be *same file* — a double failure
/// is a race, and falls through to the exists check, which refuses anyway.
///
/// It loads from disk rather than taking the browser's decoded buffer, which is
/// `None` on a terminal that is not iTerm2 — a convert key that worked only
/// where previews do is a surprise nobody could predict from its name.
pub fn convert_to(source: &Path, target: Output, force: bool) -> Result<Written, TikrayError> {
    let dest = destination(source, target);

    let same_file = dest == source
        || (dest.exists()
            && match (std::fs::canonicalize(&dest), std::fs::canonicalize(source)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            });
    if same_file {
        return Err(TikrayError::OutputIsSource { path: dest });
    }
    if !force && dest.exists() {
        return Err(TikrayError::OutputExists { path: dest });
    }

    let img = crate::load(source)?;
    // §2.13's condition, unchanged on this surface: the buffer *having* an alpha
    // channel, not any pixel being transparent. `encode` composites on exactly
    // the same test, so this reports what it did rather than doing it twice.
    let flattened = target == Output::Jpeg && img.color().has_alpha();
    let bytes = convert::encode(&img, target)?;
    std::fs::write(&dest, bytes).map_err(|source| TikrayError::Write {
        path: dest.clone(),
        source,
    })?;

    Ok(Written {
        path: dest,
        flattened,
    })
}

/// Browse from `start`, previewing whatever is highlighted.
///
/// `None` starts in the working directory. A file starts in **its directory,
/// with the file highlighted**, and a directory starts there with its first
/// entry highlighted — not a single-image view, because that is `view`'s job and
/// §2.9's whole point is that the two surfaces stay distinguishable.
///
/// **A tty is required; iTerm2 is not.** The two halves of §2.7's check fail
/// differently here: with no terminal there is nothing to run a TUI in, so that
/// is a refusal, while "this is not iTerm2" costs only the preview — the same
/// argument decision 3 makes for a terminal that reports no pixel geometry, and
/// the file list is useful on its own.
pub fn run(start: Option<&Path>) -> Result<(), TikrayError> {
    let previews = match term::detect_iterm2(false) {
        Ok(()) => true,
        Err(TikrayError::NotIterm2) => false,
        // The same condition §2.7 refuses on, refused with different advice:
        // `--force` is `view`'s escape hatch and buys a browser nothing.
        Err(TikrayError::NotATty) => return Err(TikrayError::NoScreen),
        Err(err) => return Err(err),
    };

    let mut browser = Browser::open(start, previews)?;
    let interrupt = Interrupt::register()?;

    // try_init installs a panic hook that restores the terminal; the guard
    // covers the normal exit and every `?` below. Neither covers a signal,
    // which is what `interrupt` is for.
    let terminal = ratatui::try_init().map_err(|source| TikrayError::Tui { source })?;
    let mut session = Session {
        terminal,
        placed: None,
    };
    let result = session.browse(&mut browser, &interrupt);
    drop(session);
    result
}

// ---------------------------------------------------------------------------
// The terminal session
// ---------------------------------------------------------------------------

/// The terminal, and what is currently drawn on it outside ratatui's model.
///
/// `placed` is the whole of the invalidation rule. Ratatui cannot erase an image
/// it does not know is there, so the bytes on screen are tracked here and
/// rewritten only when they or their rectangle change.
struct Session {
    terminal: DefaultTerminal,
    /// The pane, where in it the picture starts, and the bytes that drew it.
    placed: Option<(Rect, (u16, u16), Vec<u8>)>,
}

/// Restoring the terminal is this type's only job, so an early `?` cannot skip it.
impl Drop for Session {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

impl Session {
    fn browse(&mut self, browser: &mut Browser, interrupt: &Interrupt) -> Result<(), TikrayError> {
        let mut dirty = true;
        loop {
            if interrupt.fired() {
                return Ok(());
            }
            if dirty {
                self.frame(browser)?;
                dirty = false;
            }

            match event::poll(POLL) {
                Ok(false) => continue,
                // A signal interrupts the underlying wait. That is not a
                // failure — it is the loop's other exit, checked at the top.
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(source) => return Err(TikrayError::Tui { source }),
                Ok(true) => {}
            }

            match event::read().map_err(|source| TikrayError::Tui { source })? {
                Event::Resize(_, _) => {
                    // Ratatui repaints its whole buffer on a resize, which
                    // destroys the image without touching `placed` — so the
                    // record of what is on screen has to be dropped by hand.
                    self.placed = None;
                    dirty = true;
                }
                Event::Key(key) if key.is_press() => {
                    if quits(key.code, key.modifiers) {
                        return Ok(());
                    }
                    browser.key(key.code);
                    dirty = true;
                }
                _ => {}
            }
        }
    }

    /// Draw the frame, **then** place the image. The order is load-bearing.
    fn frame(&mut self, browser: &mut Browser) -> Result<(), TikrayError> {
        let cell = term::geometry().and_then(|(px, cells)| term::cell_size(px, cells));
        let status = browser.status(cell);

        // The image area comes back out of the closure rather than being
        // computed beside it: bytes sized to a pane the frame did not actually
        // draw are how an image spills.
        let mut image_area = Rect::ZERO;
        self.terminal
            .draw(|frame| image_area = browser.render(frame, &status))
            .map_err(|source| TikrayError::Tui { source })?;

        let pane = (image_area.width, image_area.height);
        // One call for both, so the offset and the bytes cannot disagree about
        // how big the image is — which is how a zoomed image lands past the
        // bottom of its pane.
        let placement = match browser.preview.as_ref() {
            Some(img) => pane_view(img, pane, cell, LEVELS[browser.zoom])?,
            None => None,
        };
        self.place(image_area, placement)
    }

    /// Write the image outside ratatui's writer, after its own flush has landed.
    ///
    /// `area` is the **whole** pane and `offset` is where inside it the picture
    /// starts. Both are kept: the offset is where the bytes go, and the pane is
    /// what has to be erased — wherever the previous image sat, which since
    /// centring is no longer the corner.
    fn place(
        &mut self,
        area: Rect,
        placement: Option<((u16, u16), Vec<u8>)>,
    ) -> Result<(), TikrayError> {
        let want = placement.map(|(offset, bytes)| (area, offset, bytes));
        if want == self.placed {
            return Ok(());
        }

        let mut out = std::io::stdout().lock();
        if let Some((old, _, _)) = self.placed.take() {
            blank(&mut out, old)?;
        }
        if let Some((area, (dx, dy), bytes)) = &want {
            queue!(out, MoveTo(area.x + dx, area.y + dy))
                .map_err(|source| TikrayError::Tui { source })?;
            out.write_all(bytes)
                .map_err(|source| TikrayError::Tui { source })?;
        }
        out.flush().map_err(|source| TikrayError::Tui { source })?;

        self.placed = want;
        Ok(())
    }
}

/// Paint `area` blank, cell by cell, without going through ratatui.
///
/// Ratatui's diff writes only cells whose *buffer* contents changed, and the
/// image area's cells are blank in that buffer the whole time — so the image
/// outlives any frame unless it is overwritten deliberately. Attributes are
/// reset first: whatever the last widget left set would otherwise colour the
/// spaces.
fn blank(out: &mut impl Write, area: Rect) -> Result<(), TikrayError> {
    let row = " ".repeat(usize::from(area.width));
    queue!(out, ResetColor).map_err(|source| TikrayError::Tui { source })?;
    for y in area.y..area.y.saturating_add(area.height) {
        queue!(out, MoveTo(area.x, y)).map_err(|source| TikrayError::Tui { source })?;
        out.write_all(row.as_bytes())
            .map_err(|source| TikrayError::Tui { source })?;
    }
    Ok(())
}

/// A `SIGINT` that arrived as a signal rather than as a keypress.
///
/// The two interruptions are different mechanisms and need different code.
/// crossterm's raw mode goes through `cfmakeraw`, which clears `ISIG`, so
/// **Ctrl-C inside the TUI is a [`KeyEvent`](crossterm::event::KeyEvent)** and
/// is handled as quit. A real `kill -INT` *is* a signal: it default-terminates
/// without unwinding, so no `Drop` runs and the terminal would be left in raw
/// mode on the alternate screen. Registering the flag replaces that default with
/// a clean exit through the loop.
struct Interrupt(Arc<AtomicBool>);

impl Interrupt {
    fn register() -> Result<Self, TikrayError> {
        let flag = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&flag))
            .map_err(|source| TikrayError::Tui { source })?;
        Ok(Self(flag))
    }

    fn fired(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Ctrl-C is in this list because raw mode makes it a keypress, not a signal.
fn quits(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('q') | KeyCode::Esc)
        || (modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c'))
}

// ---------------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------------

/// One row of the file list.
///
/// Public, fields and all, so [`entries`] can be asserted on without a terminal.
pub struct Entry {
    pub label: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Where we are, what is highlighted, and what that decoded to.
struct Browser {
    dir: PathBuf,
    entries: Vec<Entry>,
    list: ListState,
    /// The highlighted entry decoded, or [`None`] with `reason` saying why.
    preview: Option<DynamicImage>,
    reason: String,
    /// False when the terminal is not iTerm2, in which case nothing is decoded.
    previews: bool,
    /// `.` turns the filter off — both halves of it, dot-entries and non-images
    /// alike. It persists across directory changes: one that silently reset on
    /// entering a directory would need re-applying at exactly the moment the
    /// user is looking for something.
    all: bool,
    /// How many entries the filter is holding back, for the footer.
    hidden: usize,
    /// A convert that refused because the destination exists, waiting for the
    /// same key again. Keyed to *(path, format)* so a confirm that outlives its
    /// keypress still matches the file it was pressed on.
    pending: Option<(PathBuf, Output)>,
    /// What the last convert did, shown until the highlighted entry changes.
    result: Option<String>,
    /// The zoom level, an index into [`LEVELS`]. Resets when the highlighted
    /// entry changes, and **only** then — `refresh` runs after a convert, where
    /// the selection has not moved and the zoom has nothing to do with it.
    zoom: usize,
}

impl Browser {
    fn open(start: Option<&Path>, previews: bool) -> Result<Self, TikrayError> {
        let (dir, selected) = match start {
            None => {
                let cwd =
                    std::env::current_dir().map_err(|e| TikrayError::io(Path::new("."), e))?;
                (cwd, None)
            }
            Some(path) => {
                let meta = std::fs::metadata(path).map_err(|e| TikrayError::io(path, e))?;
                if meta.is_dir() {
                    (path.to_path_buf(), None)
                } else {
                    // A file starts in its directory with itself highlighted.
                    // A bare filename has no parent component, so "" becomes ".".
                    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
                    (
                        parent.unwrap_or(Path::new(".")).to_path_buf(),
                        path.file_name().map(OsStr::to_os_string),
                    )
                }
            }
        };

        let all = false;
        // The named entry is exempted from the filter: see `entries_with`.
        let listing = entries_with(&dir, all, selected.as_deref())?;
        let hidden = hidden_count(&dir, &listing, all);
        let index = index_of(&listing, selected.as_deref());

        let mut browser = Self {
            dir,
            entries: listing,
            list: ListState::default().with_selected(Some(index)),
            preview: None,
            reason: String::new(),
            previews,
            all,
            hidden,
            pending: None,
            result: None,
            zoom: 0,
        };
        browser.reload_preview();
        Ok(browser)
    }

    /// Move the highlight one row, wrapping at both ends.
    ///
    /// Returns early where `step` lands on the row it started from — a listing of
    /// one — because re-running `reload_preview` for an unchanged selection would
    /// clear the result and reset the zoom, contradicting the invariant that
    /// those happen *only* when the highlighted entry changes.
    fn move_by(&mut self, forward: bool) {
        let current = self.list.selected().unwrap_or(0);
        let next = step(current, self.entries.len(), forward);
        if next == current {
            return;
        }
        self.list.select(Some(next));
        self.result = None;
        self.zoom = 0;
        self.reload_preview();
    }

    /// Turn the filter off or back on, keeping the highlight on the same file.
    ///
    /// **By name, never by index.** Toggling changes how many rows precede the
    /// highlighted one, so a kept index lands on a different file — silently, in
    /// a UI whose whole job is to say which file you are looking at.
    fn toggle_all(&mut self) {
        let focus = self
            .selected()
            .and_then(|e| e.path.file_name().map(OsStr::to_os_string));
        self.all = !self.all;
        let Ok(listing) = entries(&self.dir, self.all) else {
            self.all = !self.all;
            return;
        };
        self.hidden = hidden_count(&self.dir, &listing, self.all);
        let index = index_of(&listing, focus.as_deref());

        // Phase 8's invariant is that the zoom resets when the highlighted entry
        // changes and only then. Toggling moved the highlight rarely before —
        // only off a non-previewable file — and moves it on every dot-entry now,
        // so the comparison has to be made rather than assumed.
        let moved = listing
            .get(index)
            .and_then(|e| e.path.file_name().map(OsStr::to_os_string))
            != focus;
        if moved {
            self.zoom = 0;
            self.result = None;
        }

        self.entries = listing;
        self.list.select(Some(index));
        self.reload_preview();
    }

    fn selected(&self) -> Option<&Entry> {
        self.entries.get(self.list.selected().unwrap_or(0))
    }

    /// Decode the highlighted entry, or record why there is nothing to draw.
    ///
    /// A refusal from [`crate::load`] is a message in the pane, never the end of
    /// the session: a browser lands on directories and non-images constantly,
    /// and its job is to keep browsing.
    fn reload_preview(&mut self) {
        self.preview = None;
        self.reason = match self.selected() {
            // "empty" would be false for a directory that is merely filtered,
            // which this phase makes an ordinary state rather than a corner.
            None if self.hidden > 0 => {
                format!(
                    "nothing to show — {} hidden, press . to reveal",
                    self.hidden
                )
            }
            None => "empty directory".to_string(),
            Some(entry) if entry.is_dir => "directory".to_string(),
            Some(entry) => {
                if !self.previews {
                    String::new()
                } else {
                    match crate::load(&entry.path) {
                        Ok(img) => {
                            self.preview = Some(img);
                            String::new()
                        }
                        Err(err) => err.to_string(),
                    }
                }
            }
        };
    }

    /// The one line above the image: what is highlighted, or why nothing is.
    fn status(&self, cell: Option<(u32, u32)>) -> String {
        // The result outranks the standing conditions, which is a change to this
        // precedence rather than an addition to it. `NOT_ITERM2` fires
        // unconditionally when previews are off — and that is exactly the
        // terminal the convert keys go out of their way to keep working on, so
        // appending a branch below would write a file and say nothing about it
        // on the one surface with no picture to confirm it by.
        if let Some(result) = &self.result {
            return result.clone();
        }
        if !self.previews {
            return NOT_ITERM2.to_string();
        }
        if cell.is_none() {
            return NO_PIXELS.to_string();
        }
        if !self.reason.is_empty() {
            return self.reason.clone();
        }
        match (&self.preview, self.selected()) {
            (Some(img), Some(entry)) => {
                format!("{}  —  {}×{}", entry.label, img.width(), img.height())
            }
            _ => String::new(),
        }
    }

    fn key(&mut self, code: KeyCode) {
        // The pending confirm is a licence to overwrite, so any key that is not
        // the one that set it clears it. The result line is looser and clears
        // when the highlighted entry changes: a message about a file you just
        // wrote has to survive the arrow key you press to go and look at it.
        if !matches!(code, KeyCode::Char('P') | KeyCode::Char('J')) {
            self.pending = None;
        }

        match code {
            KeyCode::Up | KeyCode::Char('k') => self.move_by(false),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(true),
            KeyCode::Home | KeyCode::Char('g') => {
                self.list.select_first();
                self.result = None;
                self.zoom = 0;
                self.reload_preview();
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.list.select_last();
                self.result = None;
                self.zoom = 0;
                self.reload_preview();
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.descend(),
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => self.ascend(),
            KeyCode::Char('.') => self.toggle_all(),
            // Uppercase because `j` is Down and `g`/`G` are Home/End. The shift
            // also suits a key that writes to disk.
            KeyCode::Char('P') => self.convert(Output::Png),
            KeyCode::Char('J') => self.convert(Output::Jpeg),
            // Saturating at both ends: three levels, and `0` is the way back.
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.zoom = (self.zoom + 1).min(LEVELS.len() - 1);
            }
            KeyCode::Char('-') => self.zoom = self.zoom.saturating_sub(1),
            KeyCode::Char('0') => self.zoom = 0,
            _ => {}
        }
    }

    /// Write the highlighted file out in `target`, beside itself.
    ///
    /// The order is **write, refresh, then set the result**, and it is
    /// load-bearing: refreshing goes through the same listing call a navigation
    /// makes, and navigation is what clears the result line — so setting the
    /// result first would destroy the message inside the keypress that produced
    /// it, and the pane would say nothing about a file it had just written.
    fn convert(&mut self, target: Output) {
        let Some(entry) = self.selected() else { return };
        if entry.is_dir {
            return;
        }
        let source = entry.path.clone();
        let dest = destination(&source, target);

        // A second press of the same key on the same destination is the confirm.
        let force = self.pending.as_ref() == Some(&(dest.clone(), target));
        self.pending = None;

        match convert_to(&source, target, force) {
            Err(err @ TikrayError::OutputExists { .. }) => {
                self.pending = Some((dest, target));
                self.result = Some(format!("{err} — press again to replace it"));
            }
            Err(err) => self.result = Some(err.to_string()),
            Ok(written) => {
                self.refresh();
                let name = written.path.file_name().map_or_else(
                    || written.path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                // §2.13's line, on the surface where stderr would paint over
                // the display. The condition is the buffer *having* an alpha
                // channel, so an opaque SVG announces it too — the coarse rule
                // working as written, which Phase 3's exit gate recorded.
                self.result = Some(if written.flattened {
                    format!("wrote {name} — alpha flattened onto white")
                } else {
                    format!("wrote {name}")
                });
            }
        }
    }

    /// Re-list the current directory, keeping the highlight, without touching
    /// the result line.
    fn refresh(&mut self) {
        let focus = self
            .selected()
            .and_then(|e| e.path.file_name().map(OsStr::to_os_string));
        let Ok(listing) = entries(&self.dir, self.all) else {
            return;
        };
        self.hidden = hidden_count(&self.dir, &listing, self.all);
        self.list.select(Some(index_of(&listing, focus.as_deref())));
        self.entries = listing;
        self.reload_preview();
    }

    /// Enter the highlighted directory. A file is left alone — opening it is
    /// what the preview already did.
    fn descend(&mut self) {
        let Some(entry) = self.selected() else { return };
        if !entry.is_dir {
            return;
        }
        let target = entry.path.clone();
        self.enter(&target, None);
    }

    fn ascend(&mut self) {
        let Some(parent) = self.dir.parent().map(Path::to_path_buf) else {
            return;
        };
        let here = self.dir.file_name().map(OsStr::to_os_string);
        self.enter(&parent, here);
    }

    /// Move to `dir`, highlighting `focus` if it is there.
    ///
    /// A directory that cannot be read leaves the browser where it is and says
    /// so, for the same reason an undecodable file does.
    fn enter(&mut self, dir: &Path, focus: Option<std::ffi::OsString>) {
        match entries(dir, self.all) {
            Err(err) => {
                self.preview = None;
                self.reason = err.to_string();
            }
            Ok(listing) => {
                let index = index_of(&listing, focus.as_deref());
                self.hidden = hidden_count(dir, &listing, self.all);
                self.result = None;
                self.zoom = 0;
                self.dir = dir.to_path_buf();
                self.entries = listing;
                self.list.select(Some(index));
                self.reload_preview();
            }
        }
    }

    /// Draw the frame and return the rectangle the image goes in.
    ///
    /// **Nothing is rendered into that rectangle.** The image is not a widget;
    /// it survives because the cells under it stay blank between frames and
    /// ratatui's diff therefore says nothing about them. The explanation line
    /// lives in its own row above, which is what keeps the rectangle free to be
    /// blanked directly.
    fn render(&mut self, frame: &mut ratatui::Frame, status: &str) -> Rect {
        let Panes {
            list: left,
            status: status_area,
            image: image_area,
            footer,
        } = panes(frame.area());
        let right = Rect {
            x: image_area.x.saturating_sub(1),
            y: status_area.y.saturating_sub(1),
            width: image_area.width + 2,
            height: image_area.height + status_area.height + 2,
        };

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|e| ListItem::new(e.label.as_str()))
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title(compact(&self.dir)))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▸");
        frame.render_stateful_widget(list, left, &mut self.list);

        frame.render_widget(Block::bordered().title(" preview "), right);

        // After `render_stateful_widget`, which is what writes `state.offset` —
        // computed before, the thumb lags a frame.
        if let Some((start, height)) = scroll_thumb(
            self.entries.len(),
            left.height.saturating_sub(2) as usize,
            self.list.offset(),
        ) {
            let x = left.x + left.width - 1;
            let buf = frame.buffer_mut();
            for i in 0..height {
                let y = left.y + 1 + (start + i) as u16;
                if y >= left.y + left.height - 1 {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(SCROLL_THUMB);
                }
            }
        }

        frame.render_widget(Paragraph::new(status), status_area);
        let footer_text = keys(self.all, self.hidden, LEVELS[self.zoom]);
        frame.render_widget(Paragraph::new(footer_text), footer);

        image_area
    }
}

/// The rows of `dir`: the directories first, then the files, each by name.
///
/// With `all` false, files are kept only when [`crate::load::previewable`]
/// accepts their first [`crate::load::SVG_SNIFF_LIMIT`] bytes — **by content,
/// never by extension**. That costs an open and a short read per entry, and it
/// is the only rule consistent with what `tests/gate.rs` has asserted since
/// Phase 1: a PNG named `.txt` is a PNG. Filtering on the extension would hide
/// exactly that file while `load` still drew it, leaving the list disagreeing
/// with the loader. Directories are filtered only by the dot rule: Phase 7 exempted
/// them from *previewability* because hiding one stops you descending into it,
/// and that reason does not carry to a leading `.` — leaving is `←`, not a row.
///
/// An entry that cannot be read is not previewable, and so is hidden rather than
/// raised: a browser lands on unreadable files, and its job is to keep browsing.
///
/// `..` is not synthesised — [`Browser::ascend`] is the way up, and a fake row
/// that is not a real entry would have to be special-cased in every place an
/// entry is used.
pub fn entries(dir: &Path, all: bool) -> Result<Vec<Entry>, TikrayError> {
    entries_with(dir, all, None)
}

/// [`entries`], with one entry exempted from the filter.
///
/// `keep` is the name a person put on the command line, and it is listed even
/// when the filter would hide it — **naming a path is a request, a listing is a
/// heuristic**, the principle that also makes `./view` the escape hatch for a
/// file called `view`. Without it `tikray --browse <a hidden file>` opens the
/// right directory and highlights the wrong row, because
/// [`Browser::open`] lists before [`index_of`] looks.
///
/// One entry, one directory, at startup. Navigating away drops it, and so does a
/// `.` round trip — [`Browser::toggle_all`] relists without it. The filter's
/// *state* is untouched, which is why this beats starting with the filter off.
pub fn entries_with(
    dir: &Path,
    all: bool,
    keep: Option<&OsStr>,
) -> Result<Vec<Entry>, TikrayError> {
    let mut listing: Vec<Entry> = std::fs::read_dir(dir)
        .map_err(|e| TikrayError::io(dir, e))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_dir = path.is_dir();
            let named = keep.is_some_and(|k| entry.file_name() == k);
            if !all && !named {
                // Dot-entries go, directories included. Phase 7 exempted
                // directories from the *previewability* filter because hiding
                // one stops you descending into it; that reason does not carry
                // here, since leaving is `←` rather than a row.
                if entry.file_name().as_encoded_bytes().starts_with(b".") {
                    return None;
                }
                if !is_dir && !previewable_file(&path) {
                    return None;
                }
            }
            let mut label = entry.file_name().to_string_lossy().into_owned();
            if is_dir {
                label.push('/');
            }
            Some(Entry {
                label,
                path,
                is_dir,
            })
        })
        .collect();

    listing.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    Ok(listing)
}

/// Sniff one file's head, reading no more than the SVG rule can look through.
fn previewable_file(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = vec![0u8; crate::load::SVG_SNIFF_LIMIT];
    match std::io::Read::read(&mut file, &mut head) {
        Ok(n) => crate::load::previewable(&head[..n]),
        Err(_) => false,
    }
}

/// How many rows the filter is holding back, for the footer.
///
/// Counted by relisting rather than tracked, because the alternative is a number
/// that drifts from what is on screen the moment anything else changes.
fn hidden_count(dir: &Path, shown: &[Entry], all: bool) -> usize {
    if all {
        return 0;
    }
    entries(dir, true)
        .map(|full| full.len().saturating_sub(shown.len()))
        .unwrap_or(0)
}

/// Where `name` sits in `entries`, or the first row when it is gone.
///
/// Falling back to the first row rather than to nothing is what makes
/// `tikray <path>` and going back up a directory the same operation: the entry
/// looked for may have been deleted, and the browser still has to land
/// somewhere.
fn index_of(entries: &[Entry], name: Option<&OsStr>) -> usize {
    name.and_then(|name| {
        entries
            .iter()
            .position(|e| e.path.file_name() == Some(name))
    })
    .unwrap_or(0)
}

/// A directory path short enough for a border title, with `$HOME` as `~`.
fn compact(dir: &Path) -> String {
    let shown = std::env::var("HOME")
        .ok()
        .and_then(|home| {
            dir.strip_prefix(&home).ok().map(|rest| {
                let rest = rest.to_string_lossy();
                if rest.is_empty() {
                    "~".to_string()
                } else {
                    format!("~/{rest}")
                }
            })
        })
        .unwrap_or_else(|| dir.to_string_lossy().into_owned());
    format!(" {shown} ")
}
