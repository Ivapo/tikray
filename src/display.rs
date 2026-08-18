//! Turn the buffer into iTerm2's inline-image escape sequence, and print it.
//!
//! There is no cell-quantization, dithering or palette approximation anywhere
//! in Tikray: iTerm2's protocol carries a **complete encoded image file**, not
//! raw pixels, so the terminal does the rendering (§2.3). That is the single
//! largest reason this scope is affordable, and the single largest reason it is
//! iTerm2-only.

use std::io::{Cursor, IsTerminal, Write};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::{DynamicImage, ImageFormat};

use crate::error::TikrayError;
use crate::term;

/// Scale an image down to fit a viewport, never up (§2.6).
///
/// The protocol will size an image for us — `width=100%;height=100%` fills the
/// session "as much as possible without stretching" — and that is the wrong
/// default, because *filling* is not *fitting*: a 16x16 favicon fills the
/// window too, and comes out as a screenful of blurred squares.
///
/// ```text
/// scale = min(W / w, H / h, 1.0)          # the 1.0 clamp is the never-upscale rule
/// out_w = max(1, round(w * scale))
/// out_h = max(1, round(h * scale))
/// ```
///
/// One `scale` drives both axes, so the aspect ratio is preserved by
/// construction. [`None`] in means the viewport was unreported; [`None`] out
/// means [`sequence`] should emit `auto`, which never upscales either.
pub fn fit(native: (u32, u32), viewport: Option<(u32, u32)>) -> Option<(u32, u32)> {
    let (w, h) = native;
    let (vw, vh) = viewport?;
    if vw == 0 || vh == 0 || w == 0 || h == 0 {
        return None;
    }

    let scale = (f64::from(vw) / f64::from(w))
        .min(f64::from(vh) / f64::from(h))
        .min(1.0);

    // The max(1, ..) floor is not defensive: (10,3) into (1,100) computes
    // round(0.3) = 0, and a zero dimension is not a legal argument value.
    let out_w = ((f64::from(w) * scale).round() as u32).max(1);
    let out_h = ((f64::from(h) * scale).round() as u32).max(1);
    Some((out_w, out_h))
}

/// The complete escape sequence for `img`, as bytes.
///
/// Pure — viewport in, bytes out — which is the seam that makes the display
/// path testable without a terminal.
///
/// `inline=1` is mandatory and is the whole difference between the observable
/// and nothing. The argument defaults to `0`, and iTerm2's documentation is
/// explicit that the file then "will be downloaded with no visual
/// representation in the terminal session": a sequence omitting it is
/// well-formed, exits 0, and displays nothing.
///
/// The buffer is encoded at its **native** size and the computed dimensions are
/// passed as arguments; Tikray never resamples, because the terminal scales
/// better than a nearest-neighbour pass would and there is no reason to bake a
/// display decision into the payload.
pub fn sequence(img: &DynamicImage, viewport: Option<(u32, u32)>) -> Result<Vec<u8>, TikrayError> {
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|source| TikrayError::Encode { source })?;

    let size = match fit((img.width(), img.height()), viewport) {
        Some((w, h)) => format!("width={w}px;height={h}px"),
        None => "width=auto;height=auto".to_string(),
    };

    // preserveAspectRatio already defaults to 1; stating it is belt-and-braces
    // rather than load-bearing, since fit's arithmetic is what preserves it.
    Ok(format!(
        "\x1b]1337;File=inline=1;{size};preserveAspectRatio=1:{}\x07",
        BASE64.encode(&png)
    )
    .into_bytes())
}

/// How many character cells the inline draw is indented by.
pub const INDENT: u16 = 2;

/// The indent to emit, and the viewport left over after it (§2.6's note).
///
/// Returns both because they are one decision, not two: spaces without the
/// shrink push a window-width image past the right edge, where iTerm2 wraps or
/// scrolls it, and the shrink without the spaces silently narrows the image for
/// no visible reason.
///
/// Padding needs a cell size, since the indent is in cells and the viewport in
/// pixels — with no cell geometry there is neither. And where the indent would
/// leave no width at all, **the picture outranks it**: a terminal two cells wide
/// draws flush rather than drawing nothing.
pub fn indent(viewport: Option<(u32, u32)>, cell: Option<(u32, u32)>) -> (u16, Option<(u32, u32)>) {
    let (Some((width, height)), Some((cell_w, _))) = (viewport, cell) else {
        return (0, viewport);
    };

    let indent_px = u32::from(INDENT) * cell_w;
    if width <= indent_px {
        return (0, viewport);
    }
    (INDENT, Some((width - indent_px, height)))
}

/// [`sequence`] behind [`INDENT`] spaces, sized to what is left.
///
/// Pure, so the gate can assert the leading bytes and the shrunken sizing
/// together — which is the pairing [`indent`] exists to keep.
pub fn indented(
    img: &DynamicImage,
    viewport: Option<(u32, u32)>,
    cell: Option<(u32, u32)>,
) -> Result<Vec<u8>, TikrayError> {
    let (pad, viewport) = indent(viewport, cell);
    let mut bytes = vec![b' '; usize::from(pad)];
    bytes.extend_from_slice(&sequence(img, viewport)?);
    Ok(bytes)
}

/// Draw `img`, reading the viewport from the terminal.
///
/// The only part of Tikray that writes to the output stream. The trailing
/// newline is outside [`sequence`] — the gate requires the sequence itself to
/// end at `\x07` — and is here so the shell prompt does not land on the
/// image's last row.
///
/// **The geometry is read once**, through [`term::geometry`], and both values
/// come from that one pair: two reads can straddle a resize, which is why
/// §2.14 made `geometry` return both. [`term::viewport`]'s rule that a zero
/// pixel axis means *unreported* is reapplied here by hand.
///
/// **The indent is for a terminal.** A piped stdout gets the bare sequence,
/// because `--force` exists so a person can capture the *sequence* to a file or
/// a pipe, and two spaces prepended to that byte stream are corruption rather
/// than courtesy. The predicate is the process's stdout — §2.7's idiom, the same
/// call [`term::detect_iterm2`] makes — not a bound on `out`, which need not be
/// stdout at all.
pub fn display(img: &DynamicImage, out: &mut impl Write) -> Result<(), TikrayError> {
    let geometry = term::geometry();
    let viewport = geometry
        .map(|(px, _)| px)
        .filter(|&(w, h)| w != 0 && h != 0);
    let cell = geometry
        .filter(|_| std::io::stdout().is_terminal())
        .and_then(|(px, cells)| term::cell_size(px, cells));

    let bytes = indented(img, viewport, cell)?;
    out.write_all(&bytes)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .map_err(|source| TikrayError::Output { source })
}
