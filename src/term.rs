//! What the terminal is, and how big it is. Both are queries, not UI.

use std::io::IsTerminal;

use crate::error::TikrayError;

/// Refuse to emit unless this is plausibly iTerm2 on a terminal (§2.7).
///
/// Two checks, failing in different places. The tty half stops
/// `tikray view x.png > out.txt` from filling a file with escape bytes; the env
/// half stops a non-iTerm2 terminal from printing them at a human. Both
/// variables are read because they survive different things: `TERM_PROGRAM` is
/// set by the local terminal, `LC_TERMINAL` is the one that survives an ssh hop.
/// Neither survives plain tmux.
///
/// `force` skips both. It exists because the checks have known false negatives,
/// and a detection rule with no override turns a false negative into an
/// unusable tool.
pub fn detect_iterm2(force: bool) -> Result<(), TikrayError> {
    if force {
        return Ok(());
    }
    if !std::io::stdout().is_terminal() {
        return Err(TikrayError::NotATty);
    }
    let iterm2 = std::env::var("TERM_PROGRAM").is_ok_and(|v| v == "iTerm.app")
        || std::env::var("LC_TERMINAL").is_ok_and(|v| v == "iTerm2");
    if iterm2 {
        Ok(())
    } else {
        Err(TikrayError::NotIterm2)
    }
}

/// The window's size in pixels, or [`None`] if the terminal does not report it.
///
/// `WindowSize` carries `width` and `height` in pixels alongside `rows` and
/// `columns`, and treating `0` as "unreported" is what crossterm's own
/// documentation prescribes rather than a hedge: the pixel fields "may not be
/// reliably implemented or default to 0", unix documents them as *unused*, and
/// on Windows they are not implemented at all. So a `0` in either axis, or an
/// error, is the unreported case — which [`crate::display::fit`] turns into
/// `auto` sizing (§2.6).
pub fn viewport() -> Option<(u32, u32)> {
    let size = crossterm::terminal::window_size().ok()?;
    if size.width == 0 || size.height == 0 {
        return None;
    }
    Some((u32::from(size.width), u32::from(size.height)))
}

/// The window in pixels *and* in cells, from one query (§2.14).
///
/// The reader half of the cell-geometry split. [`cell_size`] is the other half
/// and is pure, which is the same division Phase 1 made between this module's
/// [`viewport`] and [`crate::display::fit`] — and the only reason the gates run
/// with no terminal.
///
/// Both pairs come from **one** `window_size()` call rather than a second reader
/// beside [`viewport`]: two separate reads can straddle a resize and yield a
/// cell size that never existed. The pixel pair is widened to `u32` as
/// [`viewport`] already does; the cell pair stays `u16`, which is what ratatui's
/// `Rect` and [`crate::tui::pane_viewport`] take.
///
/// Zeroes are passed through rather than filtered here — [`cell_size`] is where
/// "unreported" is decided, so the rule lives in one place.
pub fn geometry() -> Option<((u32, u32), (u16, u16))> {
    let size = crossterm::terminal::window_size().ok()?;
    Some((
        (u32::from(size.width), u32::from(size.height)),
        (size.columns, size.rows),
    ))
}

/// How many pixels one character cell is, or [`None`] if that cannot be known.
///
/// Arithmetic §2.6 never needed, because it sizes to the whole window and so
/// never divided. Measured: 2528x1584 px over 158x44 cells is **16x36 px** per
/// cell, and a pane of `w` by `h` cells is the viewport `16w` by `36h`.
///
/// The division is integer and **truncates, which is the safe direction** — an
/// underestimate leaves the image slightly inside its pane, where an
/// overestimate spills it across the layout (§2.14's row 8). It divided evenly
/// on the machine that measurement came from, which will not always be true, so
/// the truncation is a live path rather than a formality.
///
/// A zero in **any** of the four is §2.6's unreported rule extended to the two
/// new fields, and so is a quotient that truncates to zero: neither can produce
/// a pane size, and `None` is what [`crate::tui::pane_sequence`] turns into
/// *draw the explanation and emit nothing*.
pub fn cell_size(px: (u32, u32), cells: (u16, u16)) -> Option<(u32, u32)> {
    let ((width, height), (columns, rows)) = (px, cells);
    if width == 0 || height == 0 || columns == 0 || rows == 0 {
        return None;
    }

    let cell = (width / u32::from(columns), height / u32::from(rows));
    if cell.0 == 0 || cell.1 == 0 {
        return None;
    }
    Some(cell)
}
