//! The command line, as a type.
//!
//! It lives in the library rather than in `src/main.rs` so the grammar can be
//! parsed without running anything — `tikray` and `tikray <path>` open a TUI,
//! and a gate that had to launch one could not assert on what they parse to.
//!
//! **The bare-argument rule** (§2.9's grammar, settled by Phase 4): the first
//! argument is a subcommand when it exactly matches one and a **path**
//! otherwise, so `tikray view` is a missing-argument error rather than a
//! browser opened on a file named `view`. `./view` is the escape hatch for the
//! file that really is called that. Adding an argument does not flip the output
//! mode: `tikray` and `tikray <path>` both reach the browser, which is the
//! `vim` / `vim file` shape a path argument already implies.

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

    /// A file or directory to open the browser at.
    ///
    /// With neither this nor a subcommand, the browser opens in the working
    /// directory.
    pub path: Option<PathBuf>,
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
