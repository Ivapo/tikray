//! tkr-001 Phase 7 exit gate, items 1-6.
//!
//! Item 7 is "Phases 1-4 and 6's gates still pass, unmodified" — all 66
//! assertions green with no edits to any of the five files. It is `cargo test`
//! plus an empty diff, so it has nothing to add here.
//!
//! Item 8 is a human in iTerm2, and this phase's gate is unusual in leaning on
//! it: **nothing below can tell part 1 wired in from part 1 never called.** An
//! implementation shipping `indent` and `indented` as pure functions and never
//! calling them from `src/display.rs:display` passes every item in this file,
//! because items 1-3 exercise the pure functions directly, item 4 asserts the
//! *absence* of spaces, and items 5-6 are part 2. The indent appears only on a
//! tty, and `cargo test` gives the binary a pipe.

use std::path::{Path, PathBuf};
use std::process::Command as Proc;

use image::{DynamicImage, RgbImage};
use tikray::display::{INDENT, indent, indented};
use tikray::load::previewable;
use tikray::tui::entries;

/// The cell §2.14 measured.
const CELL: (u32, u32) = (16, 36);

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn head(name: &str) -> Vec<u8> {
    let bytes = std::fs::read(fixtures().join(name)).expect("fixture reads");
    bytes[..bytes.len().min(1024)].to_vec()
}

fn image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::new(w, h))
}

/// The argument segment of a sequence, skipping any leading indent.
fn args(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).expect("ASCII");
    let body = text
        .split_once("\x1b]1337;File=")
        .expect("the escape is in there")
        .1;
    body.split_once(':').expect("args : payload").0.to_string()
}

fn labels(dir: &Path, all: bool) -> Vec<String> {
    entries(dir, all)
        .expect("the fixtures directory lists")
        .into_iter()
        .map(|e| e.label)
        .collect()
}

// ---------------------------------------------------------------------------
// Item 1 — `indent` reproduces the arithmetic.
// ---------------------------------------------------------------------------

#[test]
fn gate1_indent_takes_two_cells_off_the_width_only() {
    assert_eq!(
        indent(Some((1600, 1200)), Some(CELL)),
        (INDENT, Some((1568, 1200)))
    );
}

#[test]
fn gate1_no_cell_geometry_means_no_padding() {
    // The indent is in cells and the viewport in pixels; with no conversion
    // there is neither a shrink nor a space.
    assert_eq!(indent(Some((1600, 1200)), None), (0, Some((1600, 1200))));
}

#[test]
fn gate1_the_auto_branch_draws_flush() {
    assert_eq!(indent(None, None), (0, None));
}

#[test]
fn gate1_the_picture_outranks_the_indent_when_there_is_no_room() {
    // 30px is under two 16px cells, so a terminal that narrow draws flush
    // rather than drawing nothing.
    assert_eq!(indent(Some((30, 1200)), Some(CELL)), (0, Some((30, 1200))));
}

// ---------------------------------------------------------------------------
// Item 2 — the shrink is real, not just spaces.
// ---------------------------------------------------------------------------

#[test]
fn gate2_the_indent_comes_out_of_the_viewport() {
    // The item part 1's decision 1 exists for. An implementation that emits the
    // spaces and forgets the shrink passes every other item here and overflows
    // the terminal on exactly the images most likely to be viewed:
    // scale = min(1568/1600, 1200/1200, 1.0) = 0.98, so 1200 -> 1176 and the
    // ratio survives.
    let bytes = indented(&image(1600, 1200), Some((1600, 1200)), Some(CELL)).unwrap();
    let args = args(&bytes);
    assert!(args.contains("width=1568px"), "args were {args:?}");
    assert!(args.contains("height=1176px"), "args were {args:?}");
    assert!(!args.contains("width=1600px"), "args were {args:?}");
}

// ---------------------------------------------------------------------------
// Item 3 — `indented` frames the output.
// ---------------------------------------------------------------------------

#[test]
fn gate3_indented_opens_with_exactly_two_spaces() {
    let bytes = indented(&image(64, 48), Some((1600, 1200)), Some(CELL)).unwrap();
    assert!(bytes.starts_with(b"  \x1b]1337;File="), "not indented");
    assert_eq!(bytes[2], 0x1b, "exactly two, not three");
}

#[test]
fn gate3_without_a_cell_size_it_opens_with_the_escape() {
    let bytes = indented(&image(64, 48), Some((1600, 1200)), None).unwrap();
    assert!(bytes.starts_with(b"\x1b]1337;File="), "no leading spaces");
}

// ---------------------------------------------------------------------------
// Item 4 — a piped run emits no spaces, through the binary.
// ---------------------------------------------------------------------------

#[test]
fn gate4_a_piped_force_run_writes_the_bare_sequence() {
    // `tests/gate.rs:gate4_force_emits_anyway` restated here on purpose. That
    // shipped assertion is what an unconditional indent would break, and it
    // breaks only where window_size() resolves — from a real terminal, not from
    // CI. A coupling that silent belongs written down in the phase that made it.
    let out = Proc::new(env!("CARGO_BIN_EXE_tikray"))
        .args(["view", "--force"])
        .arg(fixtures().join("rgb.png"))
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .output()
        .expect("the binary runs");

    assert!(out.status.success(), "--force exits zero");
    assert!(
        out.stdout.starts_with(b"\x1b]1337;File="),
        "a pipe gets the bare sequence, first byte {:?}",
        out.stdout.first()
    );
}

// ---------------------------------------------------------------------------
// Item 5 — `previewable` is the allowlist, not the signature table.
// ---------------------------------------------------------------------------

#[test]
fn gate5_the_three_supported_inputs_are_previewable() {
    assert!(previewable(&head("rgb.png")));
    assert!(previewable(&head("rgb.jpg")));
    assert!(previewable(&head("icon24.svg")));
}

#[test]
fn gate5_a_decodable_but_unallowed_format_is_not_previewable() {
    // The item's point. GIF sniffs as a raster and `load` refuses it by name, so
    // a filter keyed to `detect` alone would list a file that cannot be drawn.
    assert!(!previewable(&head("still.gif")));
}

#[test]
fn gate5_a_non_image_and_a_gzipped_svg_are_not_previewable() {
    assert!(!previewable(&head("not_an_image.png")));
    assert!(!previewable(&head("icon24.svgz")));
}

// ---------------------------------------------------------------------------
// Item 6 — the listing is content-filtered.
// ---------------------------------------------------------------------------

#[test]
fn gate6_the_filtered_listing_keeps_images_including_a_png_named_txt() {
    // `rgb_png.txt` being IN is §2.8's property surviving into the browser: the
    // filter reads bytes, not extensions. Asserting only the obvious three
    // would pass under an extension filter too.
    let shown = labels(&fixtures(), false);
    for name in ["rgb.png", "rgb.jpg", "icon24.svg", "rgb_png.txt"] {
        assert!(
            shown.iter().any(|l| l == name),
            "{name} missing from {shown:?}"
        );
    }
}

#[test]
fn gate6_the_filtered_listing_drops_what_load_would_refuse() {
    // And `not_an_image.png` being OUT is the same property from the other side.
    // Either assertion alone can pass for the wrong reason.
    let shown = labels(&fixtures(), false);
    for name in ["still.gif", "not_an_image.png", "icon24.svgz"] {
        assert!(!shown.iter().any(|l| l == name), "{name} should be hidden");
    }
}

#[test]
fn gate6_showing_all_brings_the_refused_ones_back() {
    // Keyed to the named files, never to a count: `+3` would be falsified by the
    // next refused-format fixture, and refused-format fixtures are exactly what
    // Phase 2 added.
    let all = labels(&fixtures(), true);
    for name in ["still.gif", "not_an_image.png", "icon24.svgz", "rgb.png"] {
        assert!(all.iter().any(|l| l == name), "{name} missing from {all:?}");
    }
}
