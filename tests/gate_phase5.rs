//! tkr-001 Phase 5 exit gate, items 1-4.
//!
//! Item 5 is "Phases 1-4, 6 and 7's gates still pass, unmodified" — all 80
//! assertions green with no edits to any of the six files. It is `cargo test`
//! plus an empty diff, so it has nothing to add here.
//!
//! Item 6 is a human in iTerm2 and ships as `scripts/gate-phase4.sh`. The
//! confirm state machine — first press refuses, second press writes — lives in
//! the private `Browser` and is reachable only there; item 2 below pins
//! `convert_to`'s `force` parameter in both directions, which is the half a
//! test can hold.
//!
//! `convert_to` touches the filesystem and never the terminal, which is what
//! puts items 2-4 in the machine half at all.

use std::path::{Path, PathBuf};

use tikray::TikrayError;
use tikray::convert::Output;
use tikray::tui::{convert_to, destination};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Phase 3's idiom: cargo's own scratch space, so no `tempfile` dependency.
/// Cleared on entry rather than exit, so a failing run leaves its output behind.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("gate_phase5")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable working directory");
    dir
}

/// Copy a fixture into a working directory, so a test can write beside it.
fn staged(dir: &Path, fixture_name: &str, as_name: &str) -> PathBuf {
    let dest = dir.join(as_name);
    std::fs::copy(fixture(fixture_name), &dest).expect("the fixture copies");
    dest
}

// ---------------------------------------------------------------------------
// Item 1 — `destination` reproduces decision 2.
// ---------------------------------------------------------------------------

#[test]
fn gate1_destination_swaps_the_extension() {
    assert_eq!(
        destination(Path::new("a/photo.svg"), Output::Png),
        PathBuf::from("a/photo.png")
    );
}

#[test]
fn gate1_jpeg_spells_itself_jpg_and_lands_on_its_own_source() {
    // `resolve` accepts both spellings inbound, so which comes back out is a
    // choice. This is also decision 4's input: the destination IS the source.
    assert_eq!(
        destination(Path::new("a/photo.jpg"), Output::Jpeg),
        PathBuf::from("a/photo.jpg")
    );
}

#[test]
fn gate1_only_the_final_extension_component_is_replaced() {
    // The corner an implementer meets immediately, and the same answer
    // `tikray convert archive.tar.gz out.png` gives — so the two surfaces agree.
    assert_eq!(
        destination(Path::new("a/archive.tar.gz"), Output::Png),
        PathBuf::from("a/archive.tar.png")
    );
}

#[test]
fn gate1_a_source_with_no_extension_gains_one() {
    assert_eq!(
        destination(Path::new("a/photo"), Output::Png),
        PathBuf::from("a/photo.png")
    );
}

// ---------------------------------------------------------------------------
// Item 2 — the overwrite guard leaves the destination byte-for-byte unchanged.
// ---------------------------------------------------------------------------

#[test]
fn gate2_refusing_leaves_the_destination_untouched() {
    // Phase 3's item 5 restated at a surface with no --overwrite flag. The
    // unchanged-bytes half is the one worth asserting: a guard that errors
    // *after* truncating would pass a weaker check.
    let dir = workdir("overwrite");
    let source = staged(&dir, "rgb.jpg", "photo.jpg");
    let dest = dir.join("photo.png");
    std::fs::write(&dest, b"not a png, and must stay that way").unwrap();
    let before = std::fs::read(&dest).unwrap();

    match convert_to(&source, Output::Png, false) {
        Err(TikrayError::OutputExists { .. }) => {}
        other => panic!("expected OutputExists, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(&dest).unwrap(),
        before,
        "bytes must not change"
    );
}

#[test]
fn gate2_forcing_replaces_the_destination() {
    let dir = workdir("force");
    let source = staged(&dir, "rgb.jpg", "photo.jpg");
    let dest = dir.join("photo.png");
    std::fs::write(&dest, b"not a png").unwrap();

    let written = convert_to(&source, Output::Png, true).expect("force writes");
    assert_eq!(written.path, dest);
    assert!(
        std::fs::read(&dest).unwrap().starts_with(b"\x89PNG"),
        "the destination is a PNG now"
    );
}

// ---------------------------------------------------------------------------
// Item 3 — converting onto the source is refused by name.
// ---------------------------------------------------------------------------

#[test]
fn gate3_a_file_onto_itself_is_output_is_source_not_output_exists() {
    // The distinction decision 4 exists for, and the only evidence from outside
    // that the two cases were separated at all. `force` does not change it:
    // the second press exists to overwrite a *different* file.
    let dir = workdir("self");
    let source = staged(&dir, "rgb.png", "photo.png");

    for force in [false, true] {
        match convert_to(&source, Output::Png, force) {
            Err(TikrayError::OutputIsSource { .. }) => {}
            other => panic!("force={force}: expected OutputIsSource, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Item 4 — the notice fires on the channel, not on the pixels.
// ---------------------------------------------------------------------------

#[test]
fn gate4_an_opaque_rgba_source_still_reports_a_flatten_for_jpeg() {
    // §2.13's coarse rule, asserted where it is easy to "improve" into a
    // per-pixel scan. `opaque.svg` has no transparent pixel at all, and
    // `rasterize` always yields Rgba8 — so the buffer HAS an alpha channel.
    let dir = workdir("flatten_jpeg");
    let source = staged(&dir, "opaque.svg", "pic.svg");
    let written = convert_to(&source, Output::Jpeg, false).expect("writes");
    assert!(written.flattened, "a JPEG target flattens an alpha channel");
}

#[test]
fn gate4_the_same_source_reports_no_flatten_for_png() {
    // The half that stops the flag being hard-wired to the source's colour type.
    let dir = workdir("flatten_png");
    let source = staged(&dir, "opaque.svg", "pic.svg");
    let written = convert_to(&source, Output::Png, false).expect("writes");
    assert!(
        !written.flattened,
        "PNG carries alpha, so nothing is flattened"
    );
}
