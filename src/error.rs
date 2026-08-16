//! Every way Phase 1 can fail, as text a user can act on.
//!
//! One variant per failure named in tkr-001's Phase 1 exit gate. The split
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

    /// Stdout is not a terminal, so emitting would write escape bytes to a file (§2.7).
    NotATty,

    /// No iTerm2 signal in the environment (§2.7).
    NotIterm2,

    /// Re-encoding the buffer to PNG for the payload failed (§2.3).
    Encode { source: image::ImageError },

    /// Writing the escape sequence out failed.
    Output { source: std::io::Error },
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
                 supported input formats are PNG and JPEG",
                path.display(),
                format_name(*format),
            ),
            TikrayError::Decode { path, source } => {
                write!(f, "could not decode {}: {source}", path.display())
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
            TikrayError::Encode { source } => {
                write!(f, "could not encode the image for display: {source}")
            }
            TikrayError::Output { source } => {
                write!(f, "could not write the image to stdout: {source}")
            }
        }
    }
}

impl std::error::Error for TikrayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TikrayError::Io { source, .. } | TikrayError::Output { source } => Some(source),
            TikrayError::Decode { source, .. } | TikrayError::Encode { source } => Some(source),
            TikrayError::FormatUndetermined { .. }
            | TikrayError::FormatNotAllowed { .. }
            | TikrayError::NotATty
            | TikrayError::NotIterm2 => None,
        }
    }
}
