//! tkr-001 Phase 2 exit gate, items 1-5.
//!
//! Item 6 is "Phase 1's gate still passes, unmodified" and is `tests/gate.rs`
//! itself — which is why nothing here edits that file. Item 7 is a human in
//! iTerm2 and cannot live here, for the same reason Phase 1's item 5 could not:
//! no assertion in this file can tell a drawn image from a well-formed sequence
//! that displays nothing.
//!
//! The blast radius of this phase is the shared `load` entry point, so several
//! of these assertions are Phase 1's restated at the new seam rather than new
//! ground.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::ImageFormat;
use tikray::error::TikrayError;
use tikray::load;
use tikray::load::{Input, detect};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).expect("the fixture exists")
}

// ---------------------------------------------------------------------------
// Item 1 — `detect` reproduces §2.10.
// ---------------------------------------------------------------------------

#[test]
fn gate1_detect_recognises_the_raster_formats_by_signature() {
    assert_eq!(
        detect(&bytes("rgb.png")),
        Some(Input::Raster(ImageFormat::Png))
    );
    assert_eq!(
        detect(&bytes("rgb.jpg")),
        Some(Input::Raster(ImageFormat::Jpeg))
    );
}

#[test]
fn gate1_detect_recognises_svg_bare_with_a_prolog_and_with_a_bom() {
    // Three spellings of the same thing. `image` returns None for all of them,
    // so each has to pass the SVG rule rather than the signature table.
    assert_eq!(detect(&bytes("icon24.svg")), Some(Input::Svg));
    assert_eq!(detect(&bytes("prolog.svg")), Some(Input::Svg));
    assert_eq!(detect(&bytes("bom.svg")), Some(Input::Svg));
}

#[test]
fn gate1_detect_refuses_a_gzipped_svgz() {
    // A named non-goal, not an oversight: `.svgz` opens `1f 8b`, so it fails the
    // SVG rule's first condition and lands on FormatUndetermined.
    assert_eq!(detect(&bytes("icon24.svgz")), None);
}

#[test]
fn gate1_a_text_file_named_png_is_still_undetermined_at_the_new_seam() {
    // Phase 1's shipped assertion restated where §2.10 exists to protect it.
    // This fixture begins `this is not an image`: it fails the raster sniff, and
    // it must also fail the SVG rule rather than being handed to usvg — which is
    // what a fall-through dispatch would do, turning gate item 3 into an SVG
    // parse error.
    assert_eq!(detect(&bytes("not_an_image.png")), None);

    match load(&fixture("not_an_image.png")) {
        Err(TikrayError::FormatUndetermined { .. }) => {}
        other => panic!("expected FormatUndetermined, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Item 2 — `load` rasterizes into the waist at `usvg`'s resolved size.
// ---------------------------------------------------------------------------

#[test]
fn gate2_a_viewbox_with_no_width_or_height_resolves_to_the_viewbox() {
    let img = load(&fixture("icon24.svg")).expect("a well-formed SVG loads");
    assert_eq!((img.width(), img.height()), (24, 24));
}

#[test]
fn gate2_physical_units_resolve_through_dpi_and_round() {
    // 10mm x 5mm at 96 dpi is 37.795 x 18.898. `to_int_size()` *rounds*, so this
    // is 38 x 19 — and the rounding mode matters because it changes the `px`
    // arguments §2.6 emits.
    let img = load(&fixture("mm.svg")).expect("a well-formed SVG loads");
    assert_eq!((img.width(), img.height()), (38, 19));
}

#[test]
fn gate2_neither_attribute_nor_viewbox_falls_back_to_the_content_bbox() {
    let img = load(&fixture("bbox10.svg")).expect("a well-formed SVG loads");
    assert_eq!((img.width(), img.height()), (10, 10));
}

// ---------------------------------------------------------------------------
// Item 3 — alpha survives the premultiplied boundary.
// ---------------------------------------------------------------------------

#[test]
fn gate3_alpha_is_demultiplied_on_the_way_out_of_the_pixmap() {
    // A tiny_skia::Pixmap is premultiplied. `RgbaImage::from_raw(w, h,
    // pixmap.take())` — the one-liner an implementer reaches for — carries
    // 50%-opacity red through as [128, 0, 0, 128], which composites over white
    // as (191,127,127) instead of (255,127,127). Dimensions alone cannot catch
    // that, which is why this is a pixel assertion.
    let img = load(&fixture("alpha.svg")).expect("a well-formed SVG loads");
    let rgba = img.into_rgba8();
    assert_eq!(rgba.get_pixel(1, 1).0, [255, 0, 0, 128]);
}

// ---------------------------------------------------------------------------
// Item 4 — text renders.
// ---------------------------------------------------------------------------

#[test]
fn gate4_text_renders_a_non_zero_number_of_pixels() {
    // Zero is what an empty font database produces, silently and with no error.
    //
    // This item is *knowingly* environment-dependent: it passes because the
    // machine running it has system fonts, and would fail in a fontless
    // container where `load_system_fonts()` finds none. That is the property
    // Phase 1's gate item 4 was reshaped to avoid, accepted here because the
    // alternative — bundling a font — is a heavier decision than the assertion
    // is worth, and because a fontless build genuinely cannot render text.
    let img = load(&fixture("text.svg")).expect("a well-formed SVG loads");
    let drawn = img.into_rgba8().pixels().filter(|p| p.0[3] > 0).count();
    assert!(drawn > 0, "an empty font database renders zero pixels");
}

// ---------------------------------------------------------------------------
// Item 5 — a malformed SVG is refused, not blanked.
// ---------------------------------------------------------------------------

#[test]
fn gate5_bytes_that_detect_as_svg_but_usvg_rejects_are_a_parse_error() {
    assert_eq!(detect(&bytes("malformed.svg")), Some(Input::Svg));
    match load(&fixture("malformed.svg")) {
        Err(TikrayError::SvgParse { .. }) => {}
        other => panic!("expected SvgParse, got {other:?}"),
    }
}

#[test]
fn gate5_the_cli_refuses_a_malformed_svg_writing_zero_bytes() {
    // --force is required to reach the failure being tested: without it this
    // run is refused by detect_iterm2 for want of a tty, which proves nothing
    // about the SVG path. ("Not a blank image" is unreachable by construction —
    // `run` returns before `display` writes — so the assertion is on the
    // refusal, not on the absence of a draw.)
    let out = Command::new(env!("CARGO_BIN_EXE_tikray"))
        .args(["view", "--force"])
        .arg(fixture("malformed.svg"))
        .output()
        .expect("the binary runs");

    assert!(!out.status.success(), "must exit non-zero");
    assert!(
        out.stdout.is_empty(),
        "must write nothing to stdout, wrote {} bytes",
        out.stdout.len()
    );
    assert!(!out.stderr.is_empty(), "must say why on stderr");
}
