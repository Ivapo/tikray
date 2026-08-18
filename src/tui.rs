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

use crate::display;
use crate::error::TikrayError;
use crate::term;

/// How often the loop wakes to notice a signal (see [`Interrupt`]).
const POLL: Duration = Duration::from_millis(100);

/// The keys, in the footer. Short enough that it needs no second row.
const KEYS: &str = " ↑/↓ move   ⏎ open   ← up   q quit ";

/// Why there is no preview, for each standing condition.
///
/// Both are *standing*: they hold for every entry, not for the highlighted one,
/// which is why they are reported before any per-entry reason. A user who reads
/// "not an image" on file after file has been told the wrong thing.
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
        let placement = match browser.preview.as_ref() {
            Some(img) => {
                pane_sequence(img, pane, cell)?.map(|bytes| (pane_offset(img, pane, cell), bytes))
            }
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
struct Entry {
    label: String,
    path: PathBuf,
    is_dir: bool,
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

        let entries = read_dir(&dir)?;
        let index = index_of(&entries, selected.as_deref());

        let mut browser = Self {
            dir,
            entries,
            list: ListState::default().with_selected(Some(index)),
            preview: None,
            reason: String::new(),
            previews,
        };
        browser.reload_preview();
        Ok(browser)
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
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.list.select_previous();
                self.reload_preview();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.list.select_next();
                self.reload_preview();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.list.select_first();
                self.reload_preview();
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.list.select_last();
                self.reload_preview();
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.descend(),
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => self.ascend(),
            _ => {}
        }
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
        match read_dir(dir) {
            Err(err) => {
                self.preview = None;
                self.reason = err.to_string();
            }
            Ok(entries) => {
                let index = index_of(&entries, focus.as_deref());
                self.dir = dir.to_path_buf();
                self.entries = entries;
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
        let [main, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                .areas(main);

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

        let block = Block::bordered().title(" preview ");
        let inner = block.inner(right);
        frame.render_widget(block, right);

        let [status_area, image_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
        frame.render_widget(Paragraph::new(status), status_area);
        frame.render_widget(Paragraph::new(KEYS), footer);

        image_area
    }
}

/// The rows of `dir`: the directories first, then the files, each by name.
///
/// `..` is not synthesised — [`Browser::ascend`] is the way up, and a fake row
/// that is not a real entry would have to be special-cased in every place an
/// entry is used.
fn read_dir(dir: &Path) -> Result<Vec<Entry>, TikrayError> {
    let mut entries: Vec<Entry> = std::fs::read_dir(dir)
        .map_err(|e| TikrayError::io(dir, e))?
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            let is_dir = path.is_dir();
            let mut label = entry.file_name().to_string_lossy().into_owned();
            if is_dir {
                label.push('/');
            }
            Entry {
                label,
                path,
                is_dir,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    Ok(entries)
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
