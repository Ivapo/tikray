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

use image::DynamicImage;

use crate::display;
use crate::error::TikrayError;

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
