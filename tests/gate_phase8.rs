//! tkr-001 Phase 8 exit gate, items 1-4.
//!
//! Item 5 is "Phases 1-7's gates still pass, unmodified" — all 89 assertions
//! green with no edits to any of the seven files. Item 6 is a human in iTerm2,
//! shipped in `scripts/gate-phase4.sh`.
//!
//! Items 2 and 3 sweep rather than sample, and that is not thoroughness for its
//! own sake: **both of this phase's arithmetic blockers were found by sweeping
//! and neither by reasoning.** A single literal hid the binary64 crop because
//! 1200×800 happens to divide exactly.

use image::{DynamicImage, RgbImage};
use tikray::display::fit;
use tikray::tui::{LEVELS, pane_offset, pane_sequence, pane_view, zoom_view};

/// The pane §2.14 measured, in pixels and in cells.
const PANE: (u32, u32) = (640, 720);
const CELLS: (u16, u16) = (40, 20);
const CELL: (u32, u32) = (16, 36);

fn image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::new(w, h))
}

// ---------------------------------------------------------------------------
// Item 1 — `zoom_view` reproduces the arithmetic.
// ---------------------------------------------------------------------------

#[test]
fn gate1_a_small_source_is_never_cropped_and_grows_past_its_own_size() {
    // (48,48) is the first emitted size in this project that EXCEEDS its source:
    // decision 1 — the never-upscale clamp is a default, not an invariant —
    // visible in a literal.
    for (level, emit) in [(1, (24, 24)), (2, (48, 48)), (4, (96, 96))] {
        assert_eq!(
            zoom_view((24, 24), PANE, level),
            Some(((0, 0, 24, 24), emit)),
            "level {level}"
        );
    }
}

#[test]
fn gate1_a_large_source_is_cropped_to_the_centre() {
    assert_eq!(
        zoom_view((1200, 800), PANE, 2),
        Some(((300, 62, 600, 675), (640, 720)))
    );
    assert_eq!(
        zoom_view((1200, 800), PANE, 4),
        Some(((450, 231, 300, 337), (640, 719)))
    );
}

// ---------------------------------------------------------------------------
// Item 2 — `L=1` is exactly what ships today, swept rather than sampled.
// ---------------------------------------------------------------------------

#[test]
fn gate2_level_one_is_the_shipped_path_for_every_source_size() {
    // Tautological by design: decision 3 makes `zoom_view` RETURN `fit`'s pair
    // at level 1, so this asserts the special case is still there rather than
    // that two computations agree. A later refactor that "simplifies" it away
    // turns this red — which is the whole point, because everything shipped
    // runs through the level-1 path.
    //
    // Swept because one literal cannot carry it: 1200×800 divides exactly and
    // hides the binary64 cases. 48–55 × 1003 is the family that does not.
    let mut checked = 0;
    for w in (1..=1600).step_by(7).chain(48..=55) {
        for h in (1..=1600).step_by(11).chain(1000..=1030) {
            let expected = fit((w, h), Some(PANE));
            let got = zoom_view((w, h), PANE, 1);
            assert_eq!(
                got,
                expected.map(|f| ((0, 0, w, h), f)),
                "level 1 must be fit() at {w}x{h}"
            );
            checked += 1;
        }
    }
    assert!(checked > 20_000, "swept {checked} sizes");
}

// ---------------------------------------------------------------------------
// Item 3 — no zero dimension, and nothing exceeds its bounds.
// ---------------------------------------------------------------------------

#[test]
fn gate3_no_emitted_dimension_is_ever_zero_or_out_of_bounds() {
    // The property both arithmetic blockers violated. `4000x1` is named because
    // it is the one that emitted `height=0px` — round(1 * 0.16) — against a
    // floor Phase 1's gate pins as "not a legal argument value".
    let panes = [(640, 720), (16, 36), (1, 1), (2000, 40), (37, 1999)];
    for pane in panes {
        for w in (1..=4000).step_by(37) {
            for h in (1..=4000).step_by(53) {
                for level in LEVELS {
                    let Some(((x, y, cw, ch), (ew, eh))) = zoom_view((w, h), pane, level) else {
                        continue;
                    };
                    assert!(
                        ew >= 1 && eh >= 1,
                        "{w}x{h} L{level} in {pane:?}: zero emit"
                    );
                    assert!(ew <= pane.0 && eh <= pane.1, "{w}x{h} L{level}: spill");
                    assert!(x + cw <= w && y + ch <= h, "{w}x{h} L{level}: crop escapes");
                    assert!(cw >= 1 && ch >= 1, "{w}x{h} L{level}: empty crop");
                }
            }
        }
    }
}

#[test]
fn gate3_the_named_zero_dimension_case_emits_one_pixel_not_none() {
    for level in LEVELS {
        let (_, emit) = zoom_view((4000, 1), PANE, level).expect("a viewport resolves");
        assert_eq!(emit, (640, 1), "level {level}");
    }
}

// ---------------------------------------------------------------------------
// Item 4 — `pane_view` agrees with the shipped pair at level 1.
// ---------------------------------------------------------------------------

#[test]
fn gate4_pane_view_at_level_one_equals_the_shipped_pair() {
    // Ties the composed path to the two functions Phases 4 and 6 gated, rather
    // than leaving them to drift. Two equalities, not one tuple: the signatures
    // do not line up.
    let img = image(1200, 800);
    let (offset, bytes) = pane_view(&img, CELLS, Some(CELL), 1)
        .unwrap()
        .expect("a preview is drawn");

    assert_eq!(offset, pane_offset(&img, CELLS, Some(CELL)));
    assert_eq!(
        Some(bytes),
        pane_sequence(&img, CELLS, Some(CELL)).unwrap(),
        "byte-identical: a full-rect crop preserves the buffer"
    );
}

#[test]
fn gate4_pane_view_emits_nothing_wherever_pane_sequence_does() {
    let img = image(1200, 800);
    for level in LEVELS {
        assert!(pane_view(&img, CELLS, None, level).unwrap().is_none());
        assert!(
            pane_view(&img, (0, 20), Some(CELL), level)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn gate4_zooming_fills_the_pane_and_stays_flush() {
    // At 2x the emitted size IS the pane, so the offset is (0,0) — and this is
    // the assertion that would have caught the spill: the old path answered
    // (0,4) from `fit`, putting 20 rows four rows down a 20-row pane.
    let img = image(1200, 800);
    let (offset, _) = pane_view(&img, CELLS, Some(CELL), 2)
        .unwrap()
        .expect("a preview is drawn");
    assert_eq!(offset, (0, 0), "a pane-filling image is flush, not offset");
}
