//! Decode or rasterize a file into the pipeline's one buffer, detecting the
//! input type by content.
//!
//! The buffer is [`DynamicImage`] rather than a fixed RGBA8 raster (§2.1): it
//! keeps the source's channel layout and bit depth, where `to_rgba8()` would
//! quantize a 16-bit PNG to 8 on load. That is invisible for display and
//! material for Phase 3, whose gate reads a converted file back and compares it.

use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageFormat, ImageReader};

use crate::error::TikrayError;

/// How many bytes of the head the SVG rule may look through for `<svg`.
///
/// Public because [`previewable`] takes a head rather than a path, so the read
/// happens in the caller — and a caller that guesses this number can hide an SVG
/// whose `<svg` sits at byte 900.
pub const SVG_SNIFF_LIMIT: usize = 1024;

/// A UTF-8 byte-order mark, which an SVG is allowed to open with.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// What the bytes are, as far as Tikray is concerned (§2.10).
///
/// This type exists because SVG cannot be expressed as an [`ImageFormat`]:
/// `image` has no SVG variant at all, and its signature table returns `None`
/// for plain, XML-prolog, BOM-prefixed and gzipped SVG alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// A raster format `image` recognised from its signature.
    Raster(ImageFormat),
    /// Bytes that look like SVG. `usvg` remains the final arbiter.
    Svg,
}

/// What `head` is, or [`None`] if it is neither a known raster nor SVG.
///
/// Three steps, each **positive** — SVG is recognised, never fallen through to.
/// That distinction is the whole of §2.10: SVG bytes produce the *identical*
/// `None` a text file named `.png` produces, so "if `image` cannot sniff it,
/// hand the bytes to `usvg`" would turn Phase 1's shipped assertion about that
/// text file into an SVG parse error.
///
/// Step 1 is §2.8's third construction over the same bytes — nothing is seeded
/// from the path, so a PNG named `.txt` still loads and a text file named `.png`
/// is still undetermined.
pub fn detect(head: &[u8]) -> Option<Input> {
    if let Ok(reader) = ImageReader::new(Cursor::new(head)).with_guessed_format()
        && let Some(format) = reader.format()
    {
        return Some(Input::Raster(format));
    }

    if looks_like_svg(head) {
        return Some(Input::Svg);
    }

    None
}

/// The SVG rule, in bytes (§2.10).
///
/// After skipping a UTF-8 BOM and leading ASCII whitespace, the first byte must
/// be `<`, **and** a case-insensitive `<svg` must occur within the first
/// [`SVG_SNIFF_LIMIT`] bytes. Both conditions, because either alone is too
/// loose: the first admits any XML or HTML document, and the second admits any
/// file that merely mentions the string.
///
/// It is stated this precisely because it is what must keep a text file opening
/// `this is not an image` returning [`None`]. A gzipped `.svgz` opens `1f 8b`,
/// so it fails the first condition — a named non-goal rather than an oversight.
fn looks_like_svg(head: &[u8]) -> bool {
    let body = head.strip_prefix(&BOM).unwrap_or(head).trim_ascii_start();
    if body.first() != Some(&b'<') {
        return false;
    }

    let window = &head[..head.len().min(SVG_SNIFF_LIMIT)];
    window.windows(4).any(|w| w.eq_ignore_ascii_case(b"<svg"))
}

/// The inputs Phase 2 supports, as opposed to the ones `image` happens to decode.
///
/// A check over [`Input`], not over `[ImageFormat; 2]` — which is what SVG
/// joining the list costs, since `image::ImageFormat` cannot express it.
///
/// With default features `image` also decodes GIF, BMP, TIFF, ICO, QOI and WebP
/// without being asked, so "unsupported format" is something Tikray decides
/// rather than something that happens to it (§2.8).
const ALLOWED: [Input; 3] = [
    Input::Raster(ImageFormat::Png),
    Input::Raster(ImageFormat::Jpeg),
    Input::Svg,
];

/// Refuse an input no phase has gated, naming the format it turned out to be.
///
/// The failure is legible rather than a silent success. The only way to be
/// refused is a raster outside [`ALLOWED`], and that is the arm holding the
/// format to name — so `Input::Svg` needs no unreachable branch here.
fn allowed(input: Input, path: &Path) -> Result<(), TikrayError> {
    match input {
        Input::Raster(format) if !ALLOWED.contains(&input) => Err(TikrayError::FormatNotAllowed {
            path: path.to_path_buf(),
            format,
        }),
        Input::Raster(_) | Input::Svg => Ok(()),
    }
}

/// Whether `head` is something [`load`] would accept — the **allowlist**, not
/// the signature table.
///
/// The distinction is the whole of it. [`detect`] alone admits GIF, BMP, TIFF,
/// ICO, QOI and WebP, every one of which `load` then refuses by name, so a
/// browser filtered on `detect` would offer files that cannot be drawn — a worse
/// failure than no filter, because the refusal arrives after the choice. This
/// composes the same two steps `load` performs.
///
/// It is a byte rule, so it is not a promise the file will parse: an SVG that
/// `usvg` rejects passes here and fails at `load`, which is §2.10's "usvg
/// remains the final arbiter" seen from the listing side.
pub fn previewable(head: &[u8]) -> bool {
    detect(head).is_some_and(|input| ALLOWED.contains(&input))
}

/// Read `path` into the pipeline's buffer.
///
/// The signature is Phase 1's, unchanged — adding the vector branch behind it is
/// §2.1's claim being cashed rather than asserted (§2.11). The file is read
/// whole before sniffing because `usvg` needs the bytes anyway, and the reader
/// is then built over a cursor: [`ImageReader::open`] would seed the format from
/// the path extension, and `with_guessed_format` is `format.or(self.format)`, so
/// under that construction a failed guess *keeps the extension's answer* and a
/// text file named `.png` comes back as a corrupt PNG. Seeding nothing leaves
/// nothing to fall back to.
pub fn load(path: &Path) -> Result<DynamicImage, TikrayError> {
    let bytes = std::fs::read(path).map_err(|e| TikrayError::io(path, e))?;

    let input = detect(&bytes).ok_or_else(|| TikrayError::FormatUndetermined {
        path: path.to_path_buf(),
    })?;
    allowed(input, path)?;

    match input {
        Input::Raster(_) => ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|e| TikrayError::io(path, e))?
            .decode()
            .map_err(|source| TikrayError::Decode {
                path: path.to_path_buf(),
                source,
            }),
        Input::Svg => crate::svg::rasterize(path, &bytes),
    }
}
