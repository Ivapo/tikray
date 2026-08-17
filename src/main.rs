//! `tikray` — the CLI caller of the library core.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tikray::convert::{self, Output};
use tikray::{TikrayError, display, load, term};

#[derive(Parser)]
#[command(
    name = "tikray",
    version,
    about = "View and convert images from the terminal"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("tikray: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), TikrayError> {
    match Cli::parse().command {
        Command::View { force, path } => {
            // Detection comes first, and before anything reaches stdout: a
            // refused run must write zero bytes there (§2.7, gate item 4).
            term::detect_iterm2(force)?;
            let img = load(&path)?;
            display::display(&img, &mut std::io::stdout().lock())
        }

        // No tty check here: convert writes a file, not escape bytes, so
        // §2.7's detection has nothing to protect.
        //
        // The order is resolve, guard, load, encode, write. Both refusals are
        // cheap and come before the decode, so a run that cannot possibly
        // succeed does no work and touches no file. Its one visible
        // consequence is deliberate: `convert missing.png existing.png`
        // reports OutputExists rather than Io.
        Command::Convert {
            format,
            overwrite,
            input,
            output,
        } => {
            let target = convert::resolve(&output, format.as_deref())?;

            if !overwrite && output.exists() {
                return Err(TikrayError::OutputExists { path: output });
            }

            let img = load(&input)?;

            // §2.13 puts this line here rather than in `encode`, which is the
            // phase's pure seam. It fires on the buffer *having* an alpha
            // channel, not on any pixel actually being transparent: coarser,
            // and not something an implementer can get subtly wrong.
            if target == Output::Jpeg && img.color().has_alpha() {
                eprintln!(
                    "tikray: {} has an alpha channel and JPEG cannot carry one — \
                     flattening it onto white",
                    input.display()
                );
            }

            let bytes = convert::encode(&img, target)?;
            std::fs::write(&output, bytes).map_err(|source| TikrayError::Write {
                path: output,
                source,
            })
        }
    }
}
