//! Every way tikray can fail, as text a user can act on.
//!
//! One variant per failure named in tkr-001's exit gates. The split
//! between [`TikrayError::FormatUndetermined`] and [`TikrayError::Decode`] is
//! load-bearing rather than tidy: it is the externally visible evidence that
//! the load path sniffs content instead of trusting the path extension (§2.8).

use std::fmt;
use std::path::{Path, PathBuf};

use image::ImageFormat;

/// A failure that reaches the user as a message, never as a panic or a `Debug` dump.
#[derive(Debug)]
pub enum TikrayError {
    /// The file could not be opened or read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// The bytes matched no known image signature.
    ///
    /// Distinct from [`TikrayError::Decode`] on purpose: a text file named
    /// `.png` lands here, not on "corrupt PNG".
    FormatUndetermined { path: PathBuf },

    /// The bytes are an image, of a format no phase has gated yet (§2.8).
    FormatNotAllowed { path: PathBuf, format: ImageFormat },

    /// The format is allowed, and the file is damaged.
    Decode {
        path: PathBuf,
        source: image::ImageError,
    },

    /// The bytes detected as SVG, and `usvg` refused them (§2.10).
    ///
    /// `usvg` is the final arbiter of the detection rule: a false positive — an
    /// HTML file carrying an inline `<svg` — lands here as a legible parse
    /// error rather than as a wrong render.
    SvgParse { path: PathBuf, source: usvg::Error },

    /// The SVG parsed, and turning it into pixels failed.
    Rasterize { path: PathBuf, reason: String },

    /// Stdout is not a terminal, so emitting would write escape bytes to a file (§2.7).
    NotATty,

    /// No iTerm2 signal in the environment (§2.7).
    NotIterm2,

    /// Stdout is not a terminal, and the TUI needs one to be a screen.
    ///
    /// Separate from [`TikrayError::NotATty`] because the advice differs, not
    /// because the condition does: `--force` is `view`'s escape hatch, and it
    /// buys the browser nothing — there is still no screen. The *other* half of
    /// §2.7's check is not a refusal here at all; a non-iTerm2 terminal browses
    /// fine, it just shows no previews.
    NoScreen,

    /// Encoding the buffer failed, on the display edge or the convert one.
    Encode { source: image::ImageError },

    /// Writing the escape sequence out failed.
    Output { source: std::io::Error },

    /// The destination names no output format, and `--format` did not either (§2.12).
    OutputUndetermined { path: PathBuf },

    /// The destination names a format no phase has gated as an *output* (§2.12).
    ///
    /// Its own variant rather than [`TikrayError::FormatNotAllowed`], whose
    /// message ends "supported **input** formats are …" and would report about
    /// the wrong side of the pipeline.
    OutputNotAllowed { path: PathBuf, name: String },

    /// The destination is SVG, which tikray does not write (OQ-2, §2.12).
    OutputSvg { path: PathBuf },

    /// The destination exists and `--overwrite` was not given.
    OutputExists { path: PathBuf },

    /// The destination *is* the source (§2.13's TUI surface).
    ///
    /// Reachable only from the browser, where the format is a key rather than a
    /// path: pressing the PNG key on a PNG asks for a re-encode of the file in
    /// place, which is destructive and achieves nothing. A CLI user typing
    /// `convert a.png a.png` meets [`TikrayError::OutputExists`] first, and it
    /// is a different mistake — so this says so rather than reporting that the
    /// file it is about to destroy already exists.
    OutputIsSource { path: PathBuf },

    /// Writing the encoded bytes to the destination failed.
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Driving the terminal from the TUI failed.
    ///
    /// Its own variant rather than [`TikrayError::Output`]: that one is the
    /// display path writing a sequence to stdout, and this one covers raw mode,
    /// the alternate screen, event reads and ratatui's own writes — whose
    /// backend error type is [`std::io::Error`] too, which is why one variant
    /// carries all of them.
    Tui { source: std::io::Error },
}

impl TikrayError {
    /// Attach a path to an [`std::io::Error`] at the point the path is still known.
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        TikrayError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// The detected format as a name a user recognises.
///
/// `ImageFormat` is `#[non_exhaustive]`, so this goes through the crate's own
/// extension table rather than a match that would need updating per release.
fn format_name(format: ImageFormat) -> String {
    match format.extensions_str().first() {
        Some(ext) => ext.to_uppercase(),
        None => format!("{format:?}"),
    }
}

impl fmt::Display for TikrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TikrayError::Io { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            TikrayError::FormatUndetermined { path } => write!(
                f,
                "the image format could not be determined for {}: \
                 its contents match no image format tikray knows",
                path.display()
            ),
            TikrayError::FormatNotAllowed { path, format } => write!(
                f,
                "{} is a {} image, which tikray does not support yet — \
                 supported input formats are PNG, JPEG and SVG",
                path.display(),
                format_name(*format),
            ),
            TikrayError::Decode { path, source } => {
                write!(f, "could not decode {}: {source}", path.display())
            }
            TikrayError::SvgParse { path, source } => {
                write!(f, "could not parse {} as SVG: {source}", path.display())
            }
            TikrayError::Rasterize { path, reason } => {
                write!(f, "could not rasterize {}: {reason}", path.display())
            }
            TikrayError::NotATty => write!(
                f,
                "stdout is not a terminal, so there is nowhere to draw an image — \
                 redirect to a file only with --force, which emits raw escape bytes"
            ),
            TikrayError::NotIterm2 => write!(
                f,
                "this does not look like iTerm2 (neither TERM_PROGRAM=iTerm.app nor \
                 LC_TERMINAL=iTerm2 is set) — tikray's inline display is iTerm2-only; \
                 use --force to emit anyway"
            ),
            TikrayError::NoScreen => write!(
                f,
                "stdout is not a terminal, so there is no screen to browse in — \
                 use `tikray view <path>` or `tikray convert <in> <out>` instead"
            ),
            TikrayError::Encode { source } => {
                write!(f, "could not encode the image: {source}")
            }
            TikrayError::Output { source } => {
                write!(f, "could not write the image to stdout: {source}")
            }
            TikrayError::OutputUndetermined { path } => write!(
                f,
                "the output format could not be determined for {}: it has no \
                 extension naming one — give it a .png or .jpg extension, or \
                 pass --format",
                path.display()
            ),
            TikrayError::OutputNotAllowed { path, name } => write!(
                f,
                "cannot write {} as {}: supported output formats are PNG and JPEG",
                path.display(),
                name.to_uppercase(),
            ),
            // Both readings, in one message, because the destination is `.svg`
            // either way and only the source differs (§2.12).
            TikrayError::OutputSvg { path } => write!(
                f,
                "cannot write SVG ({}): tikray converts through a pixel buffer, \
                 so a raster in would have to be traced (a different tool), and \
                 an SVG in would come back out a raster wearing an .svg \
                 extension — supported output formats are PNG and JPEG",
                path.display()
            ),
            TikrayError::OutputExists { path } => write!(
                f,
                "{} already exists — pass --overwrite to replace it",
                path.display()
            ),
            TikrayError::OutputIsSource { path } => write!(
                f,
                "{} is the file itself — converting it in place would re-encode \
                 it over itself and change nothing",
                path.display()
            ),
            TikrayError::Write { path, source } => {
                write!(f, "could not write {}: {source}", path.display())
            }
            TikrayError::Tui { source } => {
                write!(f, "could not drive the terminal: {source}")
            }
        }
    }
}

impl std::error::Error for TikrayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TikrayError::Io { source, .. }
            | TikrayError::Output { source }
            | TikrayError::Write { source, .. }
            | TikrayError::Tui { source } => Some(source),
            TikrayError::Decode { source, .. } | TikrayError::Encode { source } => Some(source),
            TikrayError::SvgParse { source, .. } => Some(source),
            TikrayError::FormatUndetermined { .. }
            | TikrayError::FormatNotAllowed { .. }
            | TikrayError::Rasterize { .. }
            | TikrayError::NotATty
            | TikrayError::NotIterm2
            | TikrayError::NoScreen
            | TikrayError::OutputUndetermined { .. }
            | TikrayError::OutputNotAllowed { .. }
            | TikrayError::OutputSvg { .. }
            | TikrayError::OutputExists { .. }
            | TikrayError::OutputIsSource { .. } => None,
        }
    }
}
