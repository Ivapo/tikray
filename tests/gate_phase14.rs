//! tkr-001 Phase 14 exit gate, items 1-4.
//!
//! Item 5 — "Phases 1-12's gates still pass, unmodified" — is `cargo test` over
//! the ten shipped gate files, which this phase does not edit, plus
//! `bash scripts/verify-sections.sh`, which proves item 6 is reachable at all
//! before a human answers anything. Item 6 is `scripts/gate-phase4.sh` §12: the
//! panel opening and closing over a drawn picture, in iTerm2, by a person.
//!
//! **Item 3 pins a list against itself, and that is its known limit.** It cannot
//! fail on a row that *lies* about what its key does, because `Browser::key` is
//! private and this project gates from `tests/`. That correspondence is item 6's,
//! by hand. This item is a regression check: it fails on a row deleted, a
//! description emptied or a glyph mistyped.

use ratatui::layout::Rect;

use tikray::tui::{BRAND, Entry, footer_left, footer_split, help_area, help_rows, image_count};

/// One row of a hand-built listing, since `entries` needs a real directory.
fn entry(label: &str, is_dir: bool, previewable: bool) -> Entry {
    Entry {
        label: label.to_string(),
        path: std::path::PathBuf::from(label),
        is_dir,
        previewable,
    }
}

// ---------------------------------------------------------------------------
// Item 1 — the footer's left half renders its states, and `images` counts images
// ---------------------------------------------------------------------------

#[test]
fn gate1_footer_left_renders_its_four_states() {
    assert_eq!(footer_left(6, 0, 1), " 6 images", "nothing hidden, no zoom");
    assert_eq!(
        footer_left(6, 3, 1),
        " 6 images, 3 hidden",
        "the hidden count"
    );
    assert_eq!(footer_left(6, 0, 4), " 6 images  4x", "the zoom level");
    assert_eq!(
        footer_left(6, 3, 4),
        " 6 images, 3 hidden  4x",
        "both, in that order"
    );
}

#[test]
fn gate1_one_image_is_singular() {
    // A count that says "1 images" is the kind of thing that survives forever
    // once shipped.
    assert_eq!(footer_left(1, 0, 1), " 1 image");
}

#[test]
fn gate1_the_zoom_vanishes_at_level_one() {
    // Today's behaviour unchanged: it renders at 2 and 4 and not at 1, because a
    // level that is the default indicates nothing.
    assert_eq!(footer_left(6, 0, 1), " 6 images");
    assert_eq!(footer_left(6, 0, 2), " 6 images  2x");
}

#[test]
fn gate1_image_count_counts_images_and_not_rows() {
    let listing = vec![
        entry("alpha/", true, false),
        entry("beta/", true, false),
        entry("one.png", false, true),
        entry("two.svg", false, true),
        entry("notes.txt", false, false),
    ];
    assert_eq!(image_count(&listing), 2, "two previewable files");
    // The two wrong implementations, pinned as wrong. `entries.len()` is what a
    // footer written without `Entry::previewable` reaches for, and a directory of
    // four subdirectories then reads " 4 images".
    assert_ne!(image_count(&listing), listing.len(), "not every row");
    assert_ne!(image_count(&listing), 3, "not every file either");
}

// ---------------------------------------------------------------------------
// Item 2 — `footer_split` puts the brand flush right, and does not underflow
// ---------------------------------------------------------------------------

#[test]
fn gate2_the_brand_is_flush_right_at_eighty_columns() {
    let footer = Rect {
        x: 0,
        y: 23,
        width: 80,
        height: 1,
    };
    let brand_w = u16::try_from(BRAND.chars().count()).unwrap();
    let (text, brand) = footer_split(footer);

    assert_eq!(brand.x, 80 - brand_w, "flush against the right edge");
    assert_eq!(brand.width, brand_w, "exactly as wide as it needs");
    assert_eq!(text.x, 0, "the text starts at the left edge");
    assert_eq!(text.width, 80 - brand_w, "and takes the rest");
}

#[test]
fn gate2_a_row_narrower_than_the_brand_does_not_underflow() {
    // Six, not twenty: at 20 columns the subtraction is still positive and the
    // case proves nothing, whereas below 9 a literal `width - count` wraps `u16`
    // and puts the brand at column 65533.
    let footer = Rect {
        x: 0,
        y: 23,
        width: 6,
        height: 1,
    };
    let (text, brand) = footer_split(footer);

    assert_eq!(text.x, 0, "no nonsense origin");
    assert!(brand.x <= 6, "and none on the brand either: {}", brand.x);
    assert_eq!(text.width + brand.width, 6, "the two tile the row");
    assert_eq!(brand.x, text.x + text.width, "and do not overlap");
}

// ---------------------------------------------------------------------------
// Item 3 — `help_rows` lists every action, and each row says something
// ---------------------------------------------------------------------------

#[test]
fn gate3_the_panel_lists_every_action() {
    let keys: Vec<&str> = help_rows()
        .iter()
        .flat_map(|(_, rows)| rows.iter().map(|(key, _)| *key))
        .collect();

    // Actions, not keycodes. `Browser::key` also binds `j`/`k`, `h`/`l`, `→`,
    // `Backspace`, `Home`/`End` and `=`, and `Ctrl-C` is bound in `quits` rather
    // than in the browser — a panel spelling every one of those would be a worse
    // panel, so the rule maintained by hand is one row per action.
    assert_eq!(
        keys,
        vec![
            "↑/↓", "g/G", "⏎", "←", ".", "+/-", "0", "P", "J", "?", "q/Esc"
        ],
        "eleven actions, in the order the panel shows them"
    );
}

#[test]
fn gate3_every_row_says_something() {
    for (title, rows) in help_rows() {
        assert!(!title.is_empty(), "a section with no title");
        assert!(!rows.is_empty(), "an empty section: {title}");
        for (key, desc) in rows.iter() {
            assert!(!key.is_empty(), "a row with no key under {title}");
            assert!(!desc.is_empty(), "{key} under {title} says nothing");
        }
    }
}

// ---------------------------------------------------------------------------
// Item 4 — `help_area` is centred, which is arithmetic and so is not asked of
// a human
// ---------------------------------------------------------------------------

#[test]
fn gate4_the_panel_is_centred_in_eighty_by_twenty_four() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    let panel = help_area(area);

    // Stated as the offset, not as the symmetric `x + width == 80 - x`, which is
    // false for any odd `80 - width`: at width 61 it asserts 70 == 71 and fails
    // on a rectangle as centred as integer division allows.
    assert_eq!(panel.x, (80 - panel.width) / 2, "centred horizontally");
    assert_eq!(panel.y, (24 - panel.height) / 2, "centred vertically");
    assert!(
        panel.width > 0 && panel.height > 0,
        "and is a real rectangle"
    );
}

#[test]
fn gate4_the_panel_fits_inside_its_area() {
    for (w, h) in [(80u16, 24u16), (200, 60), (40, 12), (10, 4)] {
        let area = Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        };
        let panel = help_area(area);
        assert!(
            panel.x + panel.width <= w && panel.y + panel.height <= h,
            "{panel:?} escapes a {w}x{h} area"
        );
    }
}

#[test]
fn gate4_centring_is_relative_to_the_area_not_the_screen() {
    // The whole-window area starts at the origin today, so an implementation that
    // ignored `area.x`/`area.y` would pass every assertion above.
    let area = Rect {
        x: 10,
        y: 5,
        width: 80,
        height: 24,
    };
    let panel = help_area(area);
    assert_eq!(panel.x, 10 + (80 - panel.width) / 2);
    assert_eq!(panel.y, 5 + (24 - panel.height) / 2);
}
