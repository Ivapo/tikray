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

use tikray::term::cell_size;

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
