//! tkr-001 Phase 6 exit gate, items 1-5.
//!
//! Item 6 is "Phases 1-4's gates still pass, unmodified" — all 52 assertions
//! green with no edits to any of the four files. It is `cargo test` plus an
//! empty diff, so it has nothing to add here.
//!
//! Item 7 is a human in iTerm2 and ships as `scripts/gate-phase4.sh`, which this
//! phase amends rather than adding a script beside: an assertion file is
//! evidence and a gate script is a procedure, and a procedure tracks the code.

use std::path::{Path, PathBuf};
use std::process::Command as Proc;

use clap::Parser as _;
use image::{DynamicImage, RgbImage};
use tikray::cli::Cli;
use tikray::tui::{centre_offset, pane_offset};

/// The pane §2.14 measured, in cells and in pixels-per-cell.
const PANE: (u16, u16) = (40, 20);
const CELL: (u32, u32) = (16, 36);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::new(w, h))
}

// ---------------------------------------------------------------------------
// Item 1 — `centre_offset` reproduces the arithmetic.
// ---------------------------------------------------------------------------

#[test]
fn gate1_centre_offset_splits_the_free_cells() {
    // Fills the width, and 427px is 12 rows of 36, leaving 8 to split.
    assert_eq!(centre_offset((640, 427), PANE, CELL), (0, 4));
}

#[test]
fn gate1_a_small_image_is_centred_in_both_axes() {
    // Two cells wide (24/16 = 1.5, rounded up) and one tall (24/36 = 0.67,
    // rounded up). The floor would call the height zero rows and answer
    // (19, 10) — a row low. This is what forces the rounding, not a spill.
    assert_eq!(centre_offset((24, 24), PANE, CELL), (19, 9));
}

#[test]
fn gate1_an_image_that_fills_its_pane_is_not_offset() {
    assert_eq!(centre_offset((640, 720), PANE, CELL), (0, 0));
}

#[test]
fn gate1_an_image_larger_than_its_pane_saturates() {
    // (0, 0) rather than an underflow. `fit` should make this unreachable, but
    // this function is public and pure.
    assert_eq!(centre_offset((800, 900), PANE, CELL), (0, 0));
}

// ---------------------------------------------------------------------------
// Item 2 — the rounding is up, asserted where the two rules disagree.
// ---------------------------------------------------------------------------

#[test]
fn gate2_the_footprint_rounds_up_not_down() {
    // 433/36 is 12.03, so the footprint is 13 rows, the free space 7, and its
    // half 3. The floor rule would say 12 rows, 8 free, offset 4 — so this is
    // the one input of the five where the two rules give different answers,
    // which is what makes it worth asserting. Item 1's (19, 9) is the other
    // half of the pin.
    assert_eq!(centre_offset((640, 433), PANE, CELL), (0, 3));
    assert_ne!(centre_offset((640, 433), PANE, CELL), (0, 4));
}

// ---------------------------------------------------------------------------
// Item 3 — `pane_offset` fits before it centres.
// ---------------------------------------------------------------------------

#[test]
fn gate3_pane_offset_centres_the_fitted_size_not_the_native_one() {
    // The phase's one real trap. `fit` returns 640x427 for a 1200x800 buffer in
    // this pane, and 427px is 12 rows -> (0, 4). Passing the buffer's NATIVE
    // size instead returns (0, 0), because ceil(1200/16) = 75 > 40 and
    // ceil(800/36) = 23 > 20 both saturate — so every real image would sit in
    // the corner while all four of item 1's assertions stayed green. This is
    // the only assertion that separates the two implementations.
    assert_eq!(pane_offset(&image(1200, 800), PANE, Some(CELL)), (0, 4));
    assert_eq!(
        centre_offset((1200, 800), PANE, CELL),
        (0, 0),
        "the premise"
    );
}

#[test]
fn gate3_pane_offset_is_zero_where_nothing_is_placed() {
    // Wherever `pane_sequence` returns Ok(None) there is nothing to place, which
    // is why this returns a bare pair rather than an Option.
    assert_eq!(pane_offset(&image(1200, 800), PANE, None), (0, 0));
    assert_eq!(pane_offset(&image(1200, 800), (0, 20), Some(CELL)), (0, 0));
}

// ---------------------------------------------------------------------------
// Item 4 — `--browse` parses, beside everything Phase 4 pinned.
// ---------------------------------------------------------------------------

#[test]
fn gate4_browse_parses_with_a_path() {
    let cli = Cli::try_parse_from(["tikray", "--browse", "a.png"]).unwrap();
    assert!(cli.browse);
    assert!(cli.command.is_none());
    assert_eq!(cli.path.as_deref(), Some(Path::new("a.png")));
}

#[test]
fn gate4_browse_without_a_path_is_the_bare_invocation() {
    // A flag that selects the default surface has nothing to complain about.
    let cli = Cli::try_parse_from(["tikray", "--browse"]).expect("legal");
    assert!(cli.browse);
    assert_eq!(cli.path, None);
}

#[test]
fn gate4_browse_is_not_views_flag() {
    // A surface selector on the invocation that already names its surface means
    // nothing, so it is an error rather than a no-op.
    assert!(Cli::try_parse_from(["tikray", "view", "--browse", "a.png"]).is_err());
}

// ---------------------------------------------------------------------------
// Item 5 — the dispatch itself, headlessly, through the binary.
// ---------------------------------------------------------------------------

/// Run the binary with stdout on a pipe and no iTerm2 signal in the environment.
///
/// Phase 1's harness, reused: the three dispatch branches give three *different*
/// refusals in this state, which is what makes them distinguishable to a test.
fn run_without_iterm2(args: &[&str]) -> std::process::Output {
    Proc::new(env!("CARGO_BIN_EXE_tikray"))
        .args(args)
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .output()
        .expect("the binary runs")
}

/// The assertions key on the half where the two refusals **diverge**.
///
/// `NotATty` and `NoScreen` share a 38-character prefix — both open "stdout is
/// not a terminal, so there is n…" — so an assertion on "not a terminal" passes
/// for either branch, and this item's whole property is that it fails if the
/// dispatch is wired backwards.
fn stderr_of(out: &std::process::Output) -> String {
    assert!(!out.status.success(), "every branch here refuses");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn gate5_a_bare_file_reaches_the_inline_surface() {
    // Its refusal is the inline one, which is also the evidence that this branch
    // runs §2.7's detection at all — without it, `tikray x.png > out.txt` fills
    // a file with escape bytes.
    let err = stderr_of(&run_without_iterm2(&[fixture("rgb.png").to_str().unwrap()]));
    assert!(
        err.contains("nowhere to draw an image"),
        "stderr was {err:?}"
    );
    assert!(
        !err.contains("no screen to browse in"),
        "stderr was {err:?}"
    );
}

#[test]
fn gate5_a_bare_directory_reaches_the_browser() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let err = stderr_of(&run_without_iterm2(&[dir.to_str().unwrap()]));
    assert!(err.contains("no screen to browse in"), "stderr was {err:?}");
    assert!(
        !err.contains("nowhere to draw an image"),
        "stderr was {err:?}"
    );
}

#[test]
fn gate5_browse_sends_a_file_to_the_browser_too() {
    // --browse overrides the stat rather than consulting it.
    let path = fixture("rgb.png");
    let err = stderr_of(&run_without_iterm2(&["--browse", path.to_str().unwrap()]));
    assert!(err.contains("no screen to browse in"), "stderr was {err:?}");
}

#[test]
fn gate5_a_missing_path_fails_at_the_stat_before_either_surface() {
    // The stat is the dispatch, so its failure is the dispatch's. Neither
    // surface's refusal appears, because neither surface started.
    let err = stderr_of(&run_without_iterm2(&["no_such_file_here.png"]));
    assert!(err.contains("could not read"), "stderr was {err:?}");
    assert!(
        !err.contains("nowhere to draw an image"),
        "stderr was {err:?}"
    );
    assert!(
        !err.contains("no screen to browse in"),
        "stderr was {err:?}"
    );
}
