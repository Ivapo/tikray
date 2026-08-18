//! `tikray` — the CLI caller of the library core.

use std::path::Path;
use std::process::ExitCode;

use clap::Parser as _;
use tikray::cli::{Cli, Command};
use tikray::convert::{self, Output};
use tikray::{TikrayError, display, load, term, tui};

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
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        // No subcommand dispatches on what the path *is* (§2.9's corrected
        // note): a file draws inline, a directory browses, and --browse forces
        // the browser. The stat is the dispatch, so its failure is the
        // dispatch's — a missing path is Io here, before either surface starts.
        let Some(path) = cli.path else {
            return tui::run(None);
        };
        let meta = std::fs::metadata(&path).map_err(|e| TikrayError::io(&path, e))?;

        return if cli.browse || meta.is_dir() {
            tui::run(Some(&path))
        } else {
            // Not a second inline path beside `view`'s: the same one, with
            // force off. That is what keeps §2.7's detection on this branch —
            // without it `tikray x.png > out.txt` fills a file with escape
            // bytes.
            view(false, &path)
        };
    };

    match command {
        Command::View { force, path } => view(force, &path),

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

/// Draw `path` inline and return to the prompt.
///
/// One function with two callers — the `view` subcommand and a bare path that
/// turned out to be a file — rather than two inline paths that have to be kept
/// in step. Detection comes first, and before anything reaches stdout: a refused
/// run must write zero bytes there (§2.7, Phase 1's gate item 4).
fn view(force: bool, path: &Path) -> Result<(), TikrayError> {
    term::detect_iterm2(force)?;
    let img = load(path)?;
    display::display(&img, &mut std::io::stdout().lock())
}
