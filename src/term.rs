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
