//! Decode a file into the pipeline's one buffer, detecting format by content.
//!
//! The buffer is [`DynamicImage`] rather than a fixed RGBA8 raster (§2.1): it
//! keeps the source's channel layout and bit depth, where `to_rgba8()` would
//! quantize a 16-bit PNG to 8 on load. That is invisible for display and
//! material for Phase 3, whose gate reads a converted file back and compares it.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, ImageFormat, ImageReader};

use crate::error::TikrayError;

/// The formats Phase 1 supports, as opposed to the ones `image` happens to decode.
///
/// With default features `image` also reads GIF, BMP, TIFF, ICO, QOI and WebP
/// without being asked, so "unsupported format" is something Tikray decides
/// rather than something that happens to it (§2.8). A decodable-but-not-allowed
/// input is refused by name, so the failure is legible rather than a silent
/// success on a format no phase has gated.
const ALLOWED: [ImageFormat; 2] = [ImageFormat::Png, ImageFormat::Jpeg];

/// Read `path` into the pipeline's buffer.
///
/// Format comes from the bytes alone. The construction matters and is the one
/// §2.8's table exists to pin down: [`ImageReader::open`] seeds the format from
/// the path extension, and `with_guessed_format` is `format.or(self.format)` —
/// so a failed guess *keeps the extension's answer*, and a text file named
/// `.png` comes back as a corrupt PNG. Building the reader from an already-open
/// file seeds nothing, so there is nothing to fall back to.
pub fn load(path: &Path) -> Result<DynamicImage, TikrayError> {
    let file = File::open(path).map_err(|e| TikrayError::io(path, e))?;
    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|e| TikrayError::io(path, e))?;

    let format = reader
        .format()
        .ok_or_else(|| TikrayError::FormatUndetermined {
            path: path.to_path_buf(),
        })?;

    if !ALLOWED.contains(&format) {
        return Err(TikrayError::FormatNotAllowed {
            path: path.to_path_buf(),
            format,
        });
    }

    reader.decode().map_err(|source| TikrayError::Decode {
        path: path.to_path_buf(),
        source,
    })
}
