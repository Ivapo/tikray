//! Rasterize SVG into the pipeline's one buffer.
//!
//! This is the whole of §2.1's claim being cashed rather than asserted: `resvg`
//! yields a pixel buffer, and from that point SVG is indistinguishable from PNG
//! downstream. Nothing after [`rasterize`] knows the input was vector.
//!
//! The target size is the SVG's **own** resolved size, not a size derived from
//! the viewport (§2.11). Deriving it would buy nothing — §2.6's `scale` never
//! exceeds `1.0`, so the fit size is never larger than native and rasterizing at
//! a size you may not exceed cannot be sharper — and it would cost the
//! architecture, since [`crate::load::load`] takes no viewport and would have to
//! grow one or read the environment.

use std::path::Path;

use image::{DynamicImage, RgbaImage};
use resvg::tiny_skia;

use crate::error::TikrayError;

/// Rasterize `bytes` at the size `usvg` resolves for them.
///
/// Deterministic in `bytes`. `path` is read only to label errors — both
/// [`TikrayError::SvgParse`] and [`TikrayError::Rasterize`] carry one — which
/// follows [`TikrayError::io`]'s precedent and keeps this function a pure
/// function of the bytes, the seam that makes it testable without a terminal.
pub fn rasterize(path: &Path, bytes: &[u8]) -> Result<DynamicImage, TikrayError> {
    let mut options = usvg::Options::default();

    // `Options::default()` has an **empty** font database, and a `<text>`
    // element then renders zero non-transparent pixels with no error at all —
    // a silently blank image, which is exactly what the exit gate forbids. The
    // startup cost (~800 faces) is accepted for v1 over that correctness;
    // scanning the source bytes for `<text` first would avoid it in the common
    // case and is recorded as available, not taken.
    options.fontdb_mut().load_system_fonts();

    // `resources_dir` stays `None`, so an SVG referencing a raster by relative
    // href silently drops that element. A self-contained SVG is what is
    // supported; setting this from the input file's parent is a one-line change
    // available to any later phase that wants it.
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|source| TikrayError::SvgParse {
        path: path.to_path_buf(),
        source,
    })?;

    // `to_int_size()` *rounds* — `width="10mm" height="5mm"` resolves to
    // 37.795 x 18.898 and becomes 38 x 19. The mode is named because it changes
    // the `px` arguments §2.6 emits, and a gate keyed to 38x19 alone would pass
    // under either mode.
    let size = tree.size().to_int_size();
    let (width, height) = (size.width(), size.height());

    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or_else(|| TikrayError::Rasterize {
            path: path.to_path_buf(),
            reason: format!("{width}x{height} is too large to allocate a pixel buffer for"),
        })?;

    // Identity transform: the pixmap is already the tree's own size, so there
    // is nothing to scale.
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // `take_demultiplied()`, never `take()`. A `Pixmap` is **premultiplied**,
    // and the one-liner an implementer reaches for —
    // `RgbaImage::from_raw(w, h, pixmap.take())` — carries 50%-opacity red
    // through as [128, 0, 0, 128], compositing over white as (191,127,127)
    // instead of (255,127,127). Silent, plausible, and invisible to any check
    // that only looks at dimensions. `take_demultiplied` is exactly a
    // `PremultipliedColorU8::demultiply()` pass over every pixel.
    let rgba = RgbaImage::from_raw(width, height, pixmap.take_demultiplied()).ok_or_else(|| {
        TikrayError::Rasterize {
            path: path.to_path_buf(),
            reason: format!("the rasterized buffer does not match its {width}x{height} size"),
        }
    })?;

    Ok(DynamicImage::ImageRgba8(rgba))
}
