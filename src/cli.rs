//! The command line, as a type.
//!
//! It lives in the library rather than in `src/main.rs` so the grammar can be
//! parsed without running anything — `tikray` and `tikray <path>` open a TUI,
//! and a gate that had to launch one could not assert on what they parse to.
//!
//! **The bare-argument rule**: the first argument is a subcommand when it
//! exactly matches one and a **path** otherwise, so `tikray view` is a
//! missing-argument error rather than a browser opened on a file named `view`.
//! `./view` is the escape hatch for the file that really is called that.
//!
//! **A bare path then dispatches on what it is** (§2.9's corrected note, Phase
//! 6): a file draws inline and a directory browses, and `--browse` forces the
//! browser either way. So a path argument does flip the output mode — which
//! §2.9 first rejected and then reversed after the tool had been used, because
//! the common case is *show me this image* and `view` was the ceremony rather
//! than the affordance.
//!
//! **The two flags sit where they do for a reason**: `--force` modifies a
//! surface (§2.7's detection bypass) so it stays on `view`, and `--browse`
//! selects one, which is the single thing the bare form must be able to say now
//! that its meaning depends on what the path turns out to be.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Every way tikray can be invoked (§2.9).
#[derive(Parser)]
#[command(
    name = "tikray",
    version,
    about = "View and convert images from the terminal"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// An image to draw inline, or a directory to browse.
    ///
    /// With neither this nor a subcommand, the browser opens in the working
    /// directory.
    pub path: Option<PathBuf>,

    /// Browse instead of drawing: open the browser at PATH's directory with
    /// PATH highlighted.
    ///
    /// Only meaningful for a file — a directory browses either way — and it is
    /// the spelling of what a bare path meant before Phase 6.
    #[arg(long)]
    pub browse: bool,
}

/// The two one-shot surfaces. Their absence is the TUI.
#[derive(Subcommand)]
pub enum Command {
    /// Draw an image inline in iTerm2.
    View {
        /// Emit the escape sequence even where this does not look like iTerm2.
        #[arg(long)]
        force: bool,

        /// The PNG, JPEG or SVG to draw.
        path: PathBuf,
    },

    /// Write an image out in another format.
    Convert {
        /// The output format, overriding whatever the destination's extension
        /// names. One of: png, jpg, jpeg.
        ///
        /// No short form, deliberately: `-f` is the obvious abbreviation for
        /// this and for --overwrite alike, and `view --force` already means
        /// something else entirely (§2.12).
        #[arg(long)]
        format: Option<String>,

        /// Replace the destination if it already exists.
        #[arg(long)]
        overwrite: bool,

        /// The PNG, JPEG or SVG to read.
        input: PathBuf,

        /// Where to write it. Its extension picks the format unless --format does.
        output: PathBuf,
    },
}
