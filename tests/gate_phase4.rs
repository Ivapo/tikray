//! tkr-001 Phase 4 exit gate, items 1-3 and 5.
//!
//! Item 4 is "Phases 1-3's gates still pass, unmodified" — all 39 assertions in
//! `tests/gate.rs`, `tests/gate_phase2.rs` and `tests/gate_phase3.rs` green with
//! no edits to any of the three. It is `cargo test` plus an empty diff on those
//! files, so it has nothing to add here.
//!
//! Items 6 and 7 are a human in iTerm2 and ship as `scripts/gate-phase4.sh`. The
//! hazard §2.14 names — an image spilling across its pane border — is not
//! something an assertion can see, which is why item 5 below pins the *pure*
//! function that decides whether bytes are emitted at all.

use std::path::Path;

use clap::Parser as _;
use image::{DynamicImage, RgbImage};
use tikray::cli::{Cli, Command};
use tikray::display::sequence;
use tikray::term::cell_size;
use tikray::tui::{pane_sequence, pane_viewport};

fn image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::new(w, h))
}

/// The argument segment of a sequence — `ESC ] 1337 ; File=<args> : <payload> BEL`.
fn args(seq: &[u8]) -> String {
    let text = String::from_utf8(seq.to_vec()).expect("the sequence is ASCII");
    let body = text
        .strip_prefix("\x1b]1337;File=")
        .and_then(|b| b.strip_suffix('\x07'))
        .expect("header and terminator");
    body.split_once(':').expect("args : payload").0.to_string()
}

// ---------------------------------------------------------------------------
// Item 1 — `cell_size` reproduces §2.14's arithmetic.
// ---------------------------------------------------------------------------

#[test]
fn gate1_cell_size_divides_the_window_into_cells() {
    // The measurement §2.14 recorded, re-derived: 2528/158 = 16, 1584/44 = 36.
    assert_eq!(cell_size((2528, 1584), (158, 44)), Some((16, 36)));
}

#[test]
fn gate1_cell_size_reads_a_zero_in_any_of_the_four_as_unreported() {
    // §2.6's unreported rule extended to the two new fields. The pixel pair is
    // the one crossterm documents as unreliable, and the cell pair is what a
    // division by zero would otherwise reach.
    assert_eq!(cell_size((0, 1584), (158, 44)), None);
    assert_eq!(cell_size((2528, 0), (158, 44)), None);
    assert_eq!(cell_size((2528, 1584), (0, 44)), None);
    assert_eq!(cell_size((2528, 1584), (158, 0)), None);
}

#[test]
fn gate1_cell_size_truncates_toward_the_safe_direction() {
    // Integer division, and it does not divide evenly on every machine: 100/158
    // is zero cells' worth of pixels, which cannot produce a pane size either.
    assert_eq!(cell_size((2540, 1590), (158, 44)), Some((16, 36)));
    assert_eq!(cell_size((100, 1584), (158, 44)), None);
}

// ---------------------------------------------------------------------------
// Item 2 — `pane_viewport` turns cells into pixels.
// ---------------------------------------------------------------------------

#[test]
fn gate2_pane_viewport_multiplies_cells_by_the_cell_size() {
    assert_eq!(pane_viewport((40, 20), Some((16, 36))), Some((640, 720)));
}

#[test]
fn gate2_fits_to_the_pane_rather_than_to_the_window() {
    // The one place Phase 4 touches shipped arithmetic: `fit`'s clamp applied to
    // a pane. scale = min(640/1200, 720/800, 1.0) = 0.5333..., so 800 rounds to
    // 427 -- and 427 is what the terminal is told, not 800.
    let viewport = pane_viewport((40, 20), Some((16, 36)));
    let args = args(&sequence(&image(1200, 800), viewport).unwrap());
    assert!(args.contains("width=640px"), "args were {args:?}");
    assert!(args.contains("height=427px"), "args were {args:?}");
}

#[test]
fn gate2_pane_viewport_reports_none_without_a_cell_size() {
    // Item 5's input: no cell geometry, so there is no pane size to emit.
    assert_eq!(pane_viewport((40, 20), None), None);
}

#[test]
fn gate2_a_degenerate_pane_is_none_rather_than_auto() {
    // Asserted rather than inherited. A pane shrunk to nothing under a bordered
    // layout multiplies out to (0, ..); `fit` returns None for a zero axis, and
    // `sequence` would then emit width=auto;height=auto -- §2.14's row 8, the
    // spill, reached from the other side.
    assert_eq!(pane_viewport((0, 20), Some((16, 36))), None);
    assert_eq!(pane_viewport((40, 0), Some((16, 36))), None);
}

// ---------------------------------------------------------------------------
// Item 3 — the clap tree parses all four invocations.
// ---------------------------------------------------------------------------
//
// Parsed, never run: `tikray` and `tikray a.png` open a TUI, and a gate that
// had to launch one could not assert on what they mean. That is the whole
// reason `Cli` moved out of `src/main.rs`.

#[test]
fn gate3_bare_tikray_is_the_tui_with_no_path() {
    let cli = Cli::try_parse_from(["tikray"]).expect("a bare invocation is legal");
    assert!(cli.command.is_none(), "no subcommand");
    assert_eq!(cli.path, None, "and nowhere in particular to start");
}

#[test]
fn gate3_a_bare_path_is_the_tui_starting_there() {
    let cli = Cli::try_parse_from(["tikray", "a.png"]).expect("a bare path is legal");
    assert!(cli.command.is_none(), "a path is not a subcommand");
    assert_eq!(cli.path.as_deref(), Some(Path::new("a.png")));
}

#[test]
fn gate3_the_two_subcommands_still_parse() {
    let cli = Cli::try_parse_from(["tikray", "view", "a.png"]).unwrap();
    assert!(matches!(cli.command, Some(Command::View { .. })));

    let cli = Cli::try_parse_from(["tikray", "convert", "a.png", "b.jpg"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Convert { .. })));
}

#[test]
fn gate3_view_with_no_path_is_an_error_not_a_file_named_view() {
    // Decision 1 pinned rather than described: a bare first argument is a
    // subcommand when it *exactly* matches one, so this is `view` missing its
    // required argument — not the browser opened on a file called "view".
    // `./view` is the escape hatch for the file that really is called that.
    assert!(Cli::try_parse_from(["tikray", "view"]).is_err());

    let cli = Cli::try_parse_from(["tikray", "./view"]).expect("the escape hatch parses");
    assert!(cli.command.is_none(), "./view is a path, not a subcommand");
    assert_eq!(cli.path.as_deref(), Some(Path::new("./view")));
}

// ---------------------------------------------------------------------------
// Item 5 — the no-pixel fallback emits nothing.
// ---------------------------------------------------------------------------

#[test]
fn gate5_pane_sequence_emits_zero_bytes_without_a_cell_size() {
    // Not `auto`, and not a shorter sequence: nothing at all. The pane draws the
    // explanation instead.
    assert_eq!(
        pane_sequence(&image(1200, 800), (40, 20), None).unwrap(),
        None
    );
}

#[test]
fn gate5_pane_sequence_carries_the_pane_arguments_with_one() {
    // The same call with a cell size returns item 2's arguments, so one function
    // covers both branches -- which is what makes it the single place deciding
    // whether bytes reach the terminal at all.
    let bytes = pane_sequence(&image(1200, 800), (40, 20), Some((16, 36)))
        .unwrap()
        .expect("a cell size means bytes");
    let args = args(&bytes);
    assert!(args.contains("width=640px"), "args were {args:?}");
    assert!(args.contains("height=427px"), "args were {args:?}");
    assert!(args.contains("inline=1"), "args were {args:?}");
}
