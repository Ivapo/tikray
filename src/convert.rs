//! Encode the buffer back out as a file — the pipeline's second output edge.
//!
//! Nothing here knows how the buffer was filled. A rasterized SVG and a decoded
//! PNG arrive identically, which is why every input the load path already
//! supports is convertible on arrival rather than one at a time (§2.1).
//!
//! [`encode`] is this phase's **pure seam** — buffer plus target in, bytes out,
//! no filesystem — the role [`crate::display::sequence`] plays for the display
//! edge and [`crate::svg::rasterize`] for the vector one. It is what lets the
//! exit gate assert on encoded bytes without writing a file, and it is why the
//! stderr note §2.13 mandates is emitted by `src/main.rs:run` instead: a
//! function that prints is not a pure seam.

use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

use crate::error::TikrayError;

/// The formats tikray writes (§2.12).
///
/// **This type is the output allowlist.** §2.8 makes "support is an explicit
/// allowlist, per phase" a standing obligation, and this discharges it
/// structurally rather than in prose: a two-variant enum cannot name a format no
/// phase has gated. That was worth doing because the encode edge was *wider* than
/// the decode edge — an RGBA8 buffer encoded happily to PNG, JPEG, GIF, WebP,
/// BMP, TIFF, ICO, QOI and TGA, so "whatever `image` will write" was nine
/// formats, not two.
///
/// **Phase 9 made it two**, by linking `png` and `jpeg` alone. That does not
/// retire this type, it vindicates it: the enum is *why* narrowing the
/// dependency changed no behaviour, and relinking a codec would widen the edge
/// again without touching what tikray supports.
///
/// It exists for the same reason [`crate::load::Input`] does: `image` has no SVG
/// variant, so `ImageFormat` cannot express the refusal Tikray needs to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Png,
    Jpeg,
}

/// What to write `dest` as: `over` when `--format` was given, else its extension.
///
/// Deliberately **not** `ImageFormat::from_path`, whose refusal for `.svg` reads
/// *"the file extension `svg` was not recognized as an image format"* — false,
/// since tikray reads SVG perfectly well. That distinction is the evidence this
/// type was used rather than the crate's, and `tests/gate_phase3.rs` asserts it.
///
/// Both sources are read case-insensitively, so `OUT.PNG` resolves like
/// `out.png`, and `--format` beats the extension where the two disagree.
pub fn resolve(dest: &Path, over: Option<&str>) -> Result<Output, TikrayError> {
    let token = match over {
        Some(name) => name.to_string(),
        None => dest
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    };

    if token.is_empty() {
        return Err(TikrayError::OutputUndetermined {
            path: dest.to_path_buf(),
        });
    }

    match token.to_ascii_lowercase().as_str() {
        "png" => Ok(Output::Png),
        "jpg" | "jpeg" => Ok(Output::Jpeg),
        // Both readings of "convert to SVG" are refused, by name (OQ-2, §2.12):
        // raster-to-SVG is tracing, a different and much larger project, and
        // SVG-to-SVG passthrough is the case §2.1's pixel-buffer waist excludes.
        "svg" | "svgz" => Err(TikrayError::OutputSvg {
            path: dest.to_path_buf(),
        }),
        _ => Err(TikrayError::OutputNotAllowed {
            path: dest.to_path_buf(),
            name: token,
        }),
    }
}

/// Composite `img` onto white, dropping the alpha channel (§2.13).
///
/// The library's default here is silently and materially wrong, which is why
/// this is its own function rather than a line inside [`encode`].
/// `DynamicImage::write_to(_, Jpeg)` on an RGBA8 buffer **succeeds** by calling
/// `to_rgb8()`, which drops alpha with no compositing at all: a fully
/// transparent pixel lands on **black** (`[0, 0, 0, 0]` → `[0, 1, 0]`), exit
/// zero, correct dimensions, correct format, no error.
///
/// White because it is what browsers and image viewers do, so the result matches
/// what the user last saw the file look like.
pub fn flatten(img: &DynamicImage) -> RgbImage {
    let src = img.to_rgba8();
    RgbImage::from_fn(src.width(), src.height(), |x, y| {
        let [r, g, b, a] = src.get_pixel(x, y).0;
        Rgb([over_white(r, a), over_white(g, a), over_white(b, a)])
    })
}

/// One channel of `out = src·α + 255·(1−α)`, in integers so it cannot drift.
///
/// With `α = a/255` that is `(c·a + 255·(255−a)) / 255`, and the `+ 127` rounds
/// to nearest rather than truncating. The numerator peaks at `255·255`, so the
/// result is always in range.
fn over_white(channel: u8, alpha: u8) -> u8 {
    let c = u32::from(channel);
    let a = u32::from(alpha);
    ((c * a + 255 * (255 - a) + 127) / 255) as u8
}

/// `img` as the bytes of a `target` file.
///
/// Pure — no filesystem, no stdout, no stderr — which is what lets the gate
/// assert on the encoded bytes directly.
///
/// The PNG branch hands the buffer over **as it is**, deliberately: a
/// `to_rgba8()` first — the one-liner an implementer reaches for at this edge —
/// would quantize a 16-bit PNG to 8 on the way out, and both files would still
/// report `format = Png` at the same dimensions. That is the assertion §2.1 has
/// owed the `DynamicImage` waist since Phase 1.
///
/// The JPEG branch flattens only when the buffer *carries* an alpha channel, not
/// when some pixel is actually transparent: a coarser rule, and one that cannot
/// be got subtly wrong. Quality and compression stay at the library defaults
/// (JPEG 75) with no flags — §1.1 makes editing a non-goal, and `--quality` is
/// the thin end of exactly the operator set this project refuses to grow.
pub fn encode(img: &DynamicImage, target: Output) -> Result<Vec<u8>, TikrayError> {
    let mut bytes = Vec::new();

    let written = match target {
        Output::Png => img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png),
        Output::Jpeg if img.color().has_alpha() => {
            flatten(img).write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
        }
        Output::Jpeg => img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg),
    };

    written.map_err(|source| TikrayError::Encode { source })?;
    Ok(bytes)
}
