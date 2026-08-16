//! Turn the buffer into iTerm2's inline-image escape sequence, and print it.
//!
//! There is no cell-quantization, dithering or palette approximation anywhere
//! in Tikray: iTerm2's protocol carries a **complete encoded image file**, not
//! raw pixels, so the terminal does the rendering (§2.3). That is the single
//! largest reason this scope is affordable, and the single largest reason it is
//! iTerm2-only.

use std::io::{Cursor, Write};

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

/// Draw `img`, reading the viewport from the terminal.
///
/// The only part of Tikray that writes to the output stream. The trailing
/// newline is outside [`sequence`] — the gate requires the sequence itself to
/// end at `\x07` — and is here so the shell prompt does not land on the
/// image's last row.
pub fn display(img: &DynamicImage, out: &mut impl Write) -> Result<(), TikrayError> {
    let bytes = sequence(img, term::viewport())?;
    out.write_all(&bytes)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .map_err(|source| TikrayError::Output { source })
}
