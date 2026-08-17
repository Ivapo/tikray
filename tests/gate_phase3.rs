//! tkr-001 Phase 3 exit gate, items 1-6.
//!
//! Item 7 is "Phases 1 and 2's gates still pass, unmodified" and is
//! `tests/gate.rs` and `tests/gate_phase2.rs` themselves — which is why nothing
//! here edits either. Item 8 is a human in iTerm2 and cannot live here: item 2
//! proves one pixel was composited, not that a person opening the file sees the
//! picture they expected.
//!
//! **Two fixtures are pinned to an exact layout, and the pin is load-bearing.**
//! `alpha.png` is 2x2 RGBA laid out `[255,0,0,128], [0,0,0,0], [255,0,0,128],
//! [0,0,0,0]` — the layout every number in §2.13's table was measured on — and
//! `deep16.png` is 2x2 16-bit RGB carrying `[65535, 1234, 7]` at (0,0). Round 2
//! measured a *correct* flatten -> JPEG on an 8x8 opaque-red-with-one-clear-
//! corner layout returning a min channel of 194, which fails item 2's `> 200`
//! bar: a gate item depending on a fixture nobody wrote down is one an
//! implementer can reproduce as broken.

use std::ffi::OsStr;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::{ColorType, DynamicImage, ImageFormat, ImageReader};
use tikray::convert::{Output, encode, flatten, resolve};
use tikray::error::TikrayError;
use tikray::load;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A clean directory to write into, per test.
///
/// `CARGO_TARGET_TMPDIR` is cargo's own scratch space for integration tests, so
/// this needs no `tempfile` dependency. It is cleared on entry rather than on
/// exit so that a failing run leaves its output behind to look at.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("gate_phase3")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable working directory");
    dir
}

fn run(args: &[&OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tikray"))
        .args(args)
        .output()
        .expect("the binary runs")
}

/// Read bytes back **by content**, never by extension (§2.8's construction).
///
/// Nothing is seeded from a path, so what comes back is what was actually
/// written rather than what the destination was named.
fn sniff(bytes: &[u8]) -> (ImageFormat, DynamicImage) {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .expect("the written bytes sniff");
    let format = reader.format().expect("a format was determined");
    (format, reader.decode().expect("the written bytes decode"))
}

fn reread(path: &Path) -> (ImageFormat, DynamicImage) {
    sniff(&std::fs::read(path).expect("the written file exists"))
}

// ---------------------------------------------------------------------------
// Item 1 — `resolve` reproduces §2.12.
// ---------------------------------------------------------------------------

#[test]
fn gate1_the_destination_extension_names_the_format_case_insensitively() {
    assert_eq!(resolve(Path::new("out.png"), None).unwrap(), Output::Png);
    assert_eq!(resolve(Path::new("out.PNG"), None).unwrap(), Output::Png);
    assert_eq!(resolve(Path::new("out.jpg"), None).unwrap(), Output::Jpeg);
    assert_eq!(resolve(Path::new("out.jpeg"), None).unwrap(), Output::Jpeg);
}

#[test]
fn gate1_the_format_override_beats_the_extension() {
    assert_eq!(
        resolve(Path::new("out.png"), Some("jpeg")).unwrap(),
        Output::Jpeg
    );
}

#[test]
fn gate1_svg_is_refused_by_name_naming_both_readings() {
    // The distinction this asserts is the evidence §2.12's own type was used
    // rather than `ImageFormat::from_path`, whose refusal reads "the file
    // extension `svg` was not recognized as an image format" — false, since
    // tikray reads SVG perfectly well.
    for dest in ["out.svg", "out.svgz"] {
        match resolve(Path::new(dest), None) {
            Err(err @ TikrayError::OutputSvg { .. }) => {
                let msg = err.to_string();
                assert!(!msg.contains("not recognized"), "{dest}: {msg}");
                assert!(msg.contains("traced"), "{dest} misses raster-in: {msg}");
                assert!(
                    msg.contains("wearing an .svg extension"),
                    "{dest} misses SVG-in: {msg}"
                );
            }
            other => panic!("expected OutputSvg for {dest}, got {other:?}"),
        }
    }
}

#[test]
fn gate1_an_ungated_extension_is_refused_naming_it() {
    match resolve(Path::new("out.gif"), None) {
        Err(err @ TikrayError::OutputNotAllowed { .. }) => {
            assert!(err.to_string().contains("GIF"), "message was {err}");
        }
        other => panic!("expected OutputNotAllowed, got {other:?}"),
    }
}

#[test]
fn gate1_no_extension_and_no_format_is_undetermined() {
    match resolve(Path::new("out"), None) {
        Err(TikrayError::OutputUndetermined { .. }) => {}
        other => panic!("expected OutputUndetermined, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Item 2 — alpha is composited, not dropped (§2.13).
// ---------------------------------------------------------------------------

#[test]
fn gate2_flatten_composites_onto_white_rather_than_dropping_alpha() {
    let img = load(&fixture("alpha.png")).expect("the fixture loads");
    let flat = flatten(&img);
    assert_eq!(flat.get_pixel(0, 0).0, [255, 127, 127]);
    assert_eq!(flat.get_pixel(1, 0).0, [255, 255, 255]);
}

#[test]
fn gate2_a_transparent_png_converts_to_jpeg_on_white_and_says_so() {
    let dir = workdir("gate2");
    let src = fixture("alpha.png");
    let out = dir.join("alpha.jpg");
    let got = run(&[OsStr::new("convert"), src.as_os_str(), out.as_os_str()]);

    // §2.13 mandates a line on stderr naming the flattening, and exit zero.
    // Silence would make this the fourth silent-and-plausible failure the spec
    // records rather than the fix for the third.
    assert!(got.status.success(), "must exit zero");
    assert!(!got.stderr.is_empty(), "must name the flattening on stderr");

    let (format, img) = reread(&out);
    assert_eq!(format, ImageFormat::Jpeg);

    // A threshold, not an equality, and deliberately so: JPEG is not byte-exact
    // — the composited pixel returns [255, 255, 243] — while the drop-alpha bug
    // returns [0, 1, 0]. The two are ~250 apart, so no equality is needed to
    // separate them, and a literal here would pin the encoder's rounding.
    let clear = img.to_rgb8().get_pixel(1, 0).0;
    assert!(
        clear.iter().all(|&c| c > 200),
        "the transparent pixel came back {clear:?} — black is the dropped-alpha bug"
    );
}

// ---------------------------------------------------------------------------
// Item 3 — the waist is not quantized on the way out.
// ---------------------------------------------------------------------------

#[test]
fn gate3_a_16_bit_png_survives_the_encode_edge_as_rgb16() {
    let img = load(&fixture("deep16.png")).expect("the fixture loads");
    let (_, back) = sniff(&encode(&img, Output::Png).expect("encodes"));

    // Colour type and pixel, never dimensions and format. Under a `to_rgba8()`
    // first — the one-liner an implementer reaches for at this edge — this
    // reads back Rgba8 with [65535, 1285, 0] compared in 16-bit space (the
    // stored pixel is the 8-bit [255, 5, 0, 255]; 1234/257 = 5, 5*257 = 1285),
    // and **both files report format = Png and dims = (2, 2)**. So a gate keyed
    // to those two passes identically under the bug — which is why §2.1's
    // defence of DynamicImage over RGBA8 could not be cashed until here.
    assert_eq!(
        back.color(),
        ColorType::Rgb16,
        "the buffer was quantized on the way out"
    );
    assert_eq!(back.to_rgb16().get_pixel(0, 0).0, [65535, 1234, 7]);
}

// ---------------------------------------------------------------------------
// Item 4 — every allowed (input, output) pair round-trips.
// ---------------------------------------------------------------------------

#[test]
fn gate4_every_allowed_input_output_pair_round_trips() {
    let dir = workdir("gate4");

    for input in ["rgb.png", "rgb.jpg", "icon24.svg"] {
        let src = fixture(input);
        let native = load(&src).expect("the fixture loads");

        for (ext, want) in [("png", ImageFormat::Png), ("jpg", ImageFormat::Jpeg)] {
            let out = dir.join(format!("{}.{ext}", input.replace('.', "_")));
            let got = run(&[OsStr::new("convert"), src.as_os_str(), out.as_os_str()]);
            assert!(
                got.status.success(),
                "{input} -> {ext} exited non-zero: {}",
                String::from_utf8_lossy(&got.stderr)
            );

            let (format, img) = reread(&out);
            assert_eq!(format, want, "{input} -> {ext} sniffed as {format:?}");
            assert_eq!(
                (img.width(), img.height()),
                (native.width(), native.height()),
                "{input} -> {ext} changed size"
            );

            // `load` accepting the file is what makes "and `tikray view`
            // displays it" checkable without a terminal: view's only remaining
            // step is `sequence`, which Phase 1's item 2 already gates.
            if let Err(err) = load(&out) {
                panic!("{input} -> {ext} is not loadable by tikray: {err}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Item 5 — the overwrite guard.
// ---------------------------------------------------------------------------

#[test]
fn gate5_converting_onto_an_existing_file_refuses_and_changes_nothing() {
    let dir = workdir("gate5_guard");
    let out = dir.join("taken.png");
    std::fs::write(&out, b"this must survive byte for byte").expect("the file writes");
    let before = std::fs::read(&out).expect("the file reads");

    let src = fixture("rgb.png");
    let got = run(&[OsStr::new("convert"), src.as_os_str(), out.as_os_str()]);
    assert!(!got.status.success(), "must exit non-zero");
    assert!(!got.stderr.is_empty(), "must say why on stderr");

    // The unchanged-bytes half is the one worth asserting: a guard that errored
    // *after* truncating would pass a weaker check.
    assert_eq!(
        std::fs::read(&out).expect("the file reads"),
        before,
        "the guard must refuse before touching the destination"
    );
}

#[test]
fn gate5_overwrite_replaces_the_destination() {
    let dir = workdir("gate5_overwrite");
    let out = dir.join("taken.png");
    std::fs::write(&out, b"this is about to be replaced").expect("the file writes");
    let before = std::fs::read(&out).expect("the file reads");

    let src = fixture("rgb.png");
    let got = run(&[
        OsStr::new("convert"),
        OsStr::new("--overwrite"),
        src.as_os_str(),
        out.as_os_str(),
    ]);
    assert!(
        got.status.success(),
        "must exit zero: {}",
        String::from_utf8_lossy(&got.stderr)
    );

    let after = std::fs::read(&out).expect("the file reads");
    assert_ne!(after, before, "the bytes must change");
    assert_eq!(sniff(&after).0, ImageFormat::Png);
}

// ---------------------------------------------------------------------------
// Item 6 — refusal writes no file and says why.
// ---------------------------------------------------------------------------

#[test]
fn gate6_a_refused_destination_writes_no_file_and_says_why() {
    let dir = workdir("gate6");
    let src = fixture("rgb.png");

    for name in ["out.svg", "out.gif", "out"] {
        let out = dir.join(name);
        let got = run(&[OsStr::new("convert"), src.as_os_str(), out.as_os_str()]);

        assert!(!got.status.success(), "{name} must exit non-zero");
        assert!(!got.stderr.is_empty(), "{name} must say why on stderr");
        assert!(
            !out.exists(),
            "{name} must leave no file at the destination"
        );

        if name == "out.svg" {
            // Both readings (§2.12): the destination is `.svg` either way and
            // only the source differs, so one message carries both.
            let msg = String::from_utf8_lossy(&got.stderr);
            assert!(msg.contains("traced"), "misses raster-in: {msg}");
            assert!(
                msg.contains("wearing an .svg extension"),
                "misses SVG-in: {msg}"
            );
        }
    }
}
