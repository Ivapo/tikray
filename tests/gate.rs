//! tkr-001 Phase 1 exit gate, items 1-4.
//!
//! Item 5 is a human in iTerm2 and cannot live here — no assertion in this file
//! can tell a drawn image from a well-formed sequence that displays nothing.
//! What these four do is catch a *wrong* implementation, which is the other half
//! of the gate's job.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::{DynamicImage, ImageReader, RgbImage};
use tikray::display::{fit, sequence};
use tikray::error::TikrayError;
use tikray::load;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::new(w, h))
}

/// Split a sequence into its argument segment and its base64 payload.
///
/// The protocol is `ESC ] 1337 ; File=<args> : <payload> BEL`, and base64's
/// alphabet excludes `:`, so the first colon after the header is unambiguous.
fn parts(seq: &[u8]) -> (String, String) {
    let text = String::from_utf8(seq.to_vec()).expect("the sequence is ASCII");
    let body = text
        .strip_prefix("\x1b]1337;File=")
        .and_then(|b| b.strip_suffix('\x07'))
        .expect("header and terminator");
    let (args, payload) = body.split_once(':').expect("args : payload");
    (args.to_string(), payload.to_string())
}

// ---------------------------------------------------------------------------
// Item 1 — `fit` reproduces §2.6.
// ---------------------------------------------------------------------------

#[test]
fn gate1_fit_clamps_at_native_size_rather_than_upscaling() {
    // The never-upscale rule. Not (1600, 1200).
    assert_eq!(fit((800, 600), Some((1600, 1200))), Some((800, 600)));
}

#[test]
fn gate1_fit_scales_down_on_the_binding_axis() {
    // scale = min(0.25, 1.0, 1.0)
    assert_eq!(fit((4000, 1000), Some((1000, 1000))), Some((1000, 250)));
}

#[test]
fn gate1_fit_floors_a_dimension_that_rounds_to_zero() {
    // scale = 0.1, so the height computes round(0.3) = 0 and the max(1, ..)
    // floor is what rescues it — a zero dimension is not a legal argument value.
    assert_eq!(fit((10, 3), Some((1, 100))), Some((1, 1)));
}

#[test]
fn gate1_fit_reports_none_for_an_unreported_viewport() {
    assert_eq!(fit((800, 600), None), None);
}

#[test]
fn gate1_fit_reads_a_zero_axis_as_unreported() {
    assert_eq!(fit((800, 600), Some((0, 1200))), None);
    assert_eq!(fit((800, 600), Some((1600, 0))), None);
}

// ---------------------------------------------------------------------------
// Item 2 — `sequence` emits the protocol.
// ---------------------------------------------------------------------------

#[test]
fn gate2_sequence_is_framed_by_the_protocol() {
    let seq = sequence(&image(64, 48), Some((1600, 1200))).unwrap();
    assert!(
        seq.starts_with(b"\x1b]1337;File="),
        "must open the OSC 1337 File sequence"
    );
    assert_eq!(seq.last(), Some(&0x07), "must terminate at BEL");
}

#[test]
fn gate2_sequence_carries_inline_1() {
    // Not ceremony. Without it the sequence is still well-formed, exits 0, and
    // displays nothing — iTerm2 downloads the file instead. This is the one
    // byte the machine half of the gate exists to pin.
    let (args, _) = parts(&sequence(&image(64, 48), Some((1600, 1200))).unwrap());
    assert!(
        args.split(';').any(|a| a == "inline=1"),
        "args were {args:?}"
    );
}

#[test]
fn gate2_sequence_carries_exactly_the_dimensions_fit_returned() {
    let img = image(4000, 1000);
    let viewport = Some((1000, 1000));
    let (w, h) = fit((img.width(), img.height()), viewport).unwrap();
    assert_eq!((w, h), (1000, 250), "the premise of this assertion");

    let (args, _) = parts(&sequence(&img, viewport).unwrap());
    assert!(args.contains(&format!("width={w}px")), "args were {args:?}");
    assert!(
        args.contains(&format!("height={h}px")),
        "args were {args:?}"
    );
}

#[test]
fn gate2_sequence_falls_back_to_auto_when_the_viewport_is_unreported() {
    let (args, _) = parts(&sequence(&image(64, 48), None).unwrap());
    assert!(args.contains("width=auto"), "args were {args:?}");
    assert!(args.contains("height=auto"), "args were {args:?}");
    assert!(
        !args.contains("px"),
        "no px sizing without a viewport: {args:?}"
    );
}

#[test]
fn gate2_payload_decodes_to_a_png_at_the_source_buffers_dimensions() {
    // The payload is a complete encoded image file, not raw pixels, and it is
    // encoded at native size — the protocol does the scaling, Tikray never
    // resamples.
    let img = image(4000, 1000);
    let (_, payload) = parts(&sequence(&img, Some((1000, 1000))).unwrap());
    let png = BASE64.decode(payload).expect("payload is valid base64");

    let reader = ImageReader::new(Cursor::new(&png))
        .with_guessed_format()
        .unwrap();
    assert_eq!(reader.format(), Some(image::ImageFormat::Png));
    assert_eq!(
        reader.into_dimensions().unwrap(),
        (img.width(), img.height())
    );
}

// ---------------------------------------------------------------------------
// Item 3 — `load` honours the allowlist.
// ---------------------------------------------------------------------------

#[test]
fn gate3_load_decodes_png_and_jpeg_to_their_known_dimensions() {
    assert_eq!(
        load(&fixture("rgb.png")).unwrap().into_rgb8().dimensions(),
        (64, 48)
    );
    assert_eq!(
        load(&fixture("rgb.jpg")).unwrap().into_rgb8().dimensions(),
        (64, 48)
    );
}

#[test]
fn gate3_load_detects_by_content_not_by_extension() {
    // A PNG named .txt still loads.
    let img = load(&fixture("rgb_png.txt")).expect("content sniffing beats the extension");
    assert_eq!((img.width(), img.height()), (64, 48));
}

#[test]
fn gate3_a_text_file_named_png_is_undetermined_not_corrupt() {
    // The whole evidence that §2.8's third construction was used and not its
    // second: under `ImageReader::open(p)?.with_guessed_format()?` the format
    // is seeded from the extension and a failed guess keeps it, so this same
    // file comes back as "Invalid PNG signature" — a corrupt PNG rather than a
    // non-image. Asserting the variant is what makes the two distinguishable
    // from outside.
    match load(&fixture("not_an_image.png")) {
        Err(TikrayError::FormatUndetermined { .. }) => {}
        other => panic!("expected FormatUndetermined, got {other:?}"),
    }
}

#[test]
fn gate3_a_decodable_but_unallowed_format_is_refused_by_name() {
    match load(&fixture("still.gif")) {
        Err(err @ TikrayError::FormatNotAllowed { .. }) => {
            assert!(err.to_string().contains("GIF"), "message was {err}");
        }
        other => panic!("expected FormatNotAllowed, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Item 4 — refusal is not silent.
// ---------------------------------------------------------------------------

/// Run the binary with stdout on a pipe and no iTerm2 signal in the environment.
fn run_without_iterm2(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tikray"))
        .args(args)
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .output()
        .expect("the binary runs")
}

#[test]
fn gate4_refusing_writes_zero_bytes_to_stdout() {
    let out = run_without_iterm2(&["view", fixture("rgb.png").to_str().unwrap()]);
    assert!(!out.status.success(), "must exit non-zero");
    assert!(
        out.stdout.is_empty(),
        "must write nothing to stdout, wrote {} bytes",
        out.stdout.len()
    );
    assert!(!out.stderr.is_empty(), "must say why on stderr");
}

#[test]
fn gate4_force_emits_anyway() {
    let out = run_without_iterm2(&["view", "--force", fixture("rgb.png").to_str().unwrap()]);
    assert!(out.status.success(), "must exit zero");

    // Prefix-only, deliberately. window_size() reads /dev/tty and only falls
    // back to stdout, so a piped run under a real terminal can still resolve a
    // viewport and emit px, while an environment with no controlling terminal
    // emits auto. Asserting sizing here would pass in one and fail in the
    // other; item 1 is where sizing is pinned, with the viewport injected
    // rather than discovered.
    assert!(out.stdout.starts_with(b"\x1b]1337;File="));
    assert!(String::from_utf8_lossy(&out.stdout).contains("inline=1"));
    assert!(
        out.stdout.contains(&0x07),
        "must contain the BEL terminator"
    );
}
