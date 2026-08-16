//! `tikray` — the CLI caller of the library core.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
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

        /// The PNG or JPEG to draw.
        path: PathBuf,
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
    }
}
