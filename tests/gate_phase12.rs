//! tkr-001 Phase 12 exit gate, items 1-3.
//!
//! Item 4 is "Phases 1-10's gates still pass, unmodified" — 104 assertions
//! across nine files. Item 5 is a human in iTerm2, in `scripts/gate-phase4.sh`.
//!
//! Item 3 pins the rectangle `panes` computes; that `Browser::render` *uses* it
//! is item 5's, by eye, because `Browser` is private and this project gates only
//! from `tests/`. Decision 1's early return is likewise ungated — stated here so
//! nobody assumes coverage that is not present.

use ratatui::layout::Rect;
use tikray::tui::{panes, scroll_thumb, step};

// ---------------------------------------------------------------------------
// Item 1 — `step` wraps at both ends and nowhere else.
// ---------------------------------------------------------------------------

#[test]
fn gate1_step_wraps_at_both_ends() {
    // ratatui's own movement sticks: select_next is saturating_add and
    // select_previous is saturating_sub.
    assert_eq!(step(4, 5, true), 0, "past the end wraps to the first");
    assert_eq!(step(0, 5, false), 4, "before the start wraps to the last");
}

#[test]
fn gate1_step_is_ordinary_in_the_middle() {
    assert_eq!(step(2, 5, true), 3);
    assert_eq!(step(2, 5, false), 1);
}

#[test]
fn gate1_a_single_row_wraps_onto_itself() {
    // Also decision 1's early-return input: the caller must not reload a preview
    // for a selection that did not change.
    assert_eq!(step(0, 1, true), 0);
    assert_eq!(step(0, 1, false), 0);
}

#[test]
fn gate1_an_empty_listing_does_not_divide_by_zero() {
    assert_eq!(step(0, 0, true), 0);
    assert_eq!(step(0, 0, false), 0);
}

// ---------------------------------------------------------------------------
// Item 2 — the thumb, asserted where the two rounding rules diverge.
// ---------------------------------------------------------------------------

#[test]
fn gate2_nothing_to_indicate_when_everything_fits() {
    assert_eq!(scroll_thumb(10, 10, 0), None);
    assert_eq!(scroll_thumb(3, 10, 0), None);
    assert_eq!(scroll_thumb(100, 0, 0), None);
}

#[test]
fn gate2_the_thumb_is_flush_at_both_ends() {
    // len 100 in a 10-row track: a 1-row thumb with 9 rows of travel.
    assert_eq!(scroll_thumb(100, 10, 0), Some((0, 1)), "flush at the top");
    assert_eq!(
        scroll_thumb(100, 10, 90),
        Some((9, 1)),
        "flush at the bottom"
    );
}

#[test]
fn gate2_mid_scroll_is_rounded_not_truncated() {
    // THE item. The two ends above do NOT discriminate: truncating division
    // gives 0 and 9 there too, because offset*travel is exact at both. They
    // differ only here — rounded 5, truncated (45*9)/90 = 4 — so an item
    // asserting the ends alone goes green on the regression it names.
    assert_eq!(scroll_thumb(100, 10, 45), Some((5, 1)));
    assert_ne!(scroll_thumb(100, 10, 45), Some((4, 1)));
}

// ---------------------------------------------------------------------------
// Item 3 — `panes` puts the split where decision 2 says.
// ---------------------------------------------------------------------------

#[test]
fn gate3_the_split_is_thirty_seventy() {
    let p = panes(Rect::new(0, 0, 80, 24));
    assert_eq!(p.list.x, 0);
    assert_eq!(p.list.width, 24, "30% of 80");
    // Not the x=29, w=50 that 35% produced — the move is the point.
    assert_eq!((p.image.x, p.image.width), (25, 54));
    assert_ne!((p.image.x, p.image.width), (29, 50), "the old split");
}

#[test]
fn gate3_the_status_row_sits_above_the_image_and_the_footer_below() {
    let p = panes(Rect::new(0, 0, 80, 24));
    assert_eq!(p.status.height, 1, "one row, as Phase 4 reserved");
    assert_eq!(p.image.y, p.status.y + 1, "the image starts under it");
    assert_eq!(p.footer.height, 1);
    assert_eq!(p.footer.y, 23, "the last row of the window");
}

#[test]
fn gate3_the_image_rect_is_inside_the_preview_border() {
    // What keeps Phases 6-8's arithmetic honest: the rectangle handed to
    // pane_view must not touch the border it is drawn inside.
    let p = panes(Rect::new(0, 0, 158, 44));
    assert!(p.image.x > p.list.x + p.list.width, "past the list");
    assert_eq!(
        p.image.width, 109,
        "the preview's inner width at 158 columns"
    );
    assert!(
        p.image.y + p.image.height < p.footer.y,
        "clear of the footer"
    );
}
