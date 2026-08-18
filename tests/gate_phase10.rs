//! tkr-001 Phase 10 exit gate, items 1-4.
//!
//! Item 5 is a human in iTerm2 and ships in `scripts/gate-phase4.sh`. It is
//! where the keys are checked — they live in the private `Browser`, as Phases 4,
//! 5 and 8's did — and it is **the only check that decision 3 was wired in at
//! all**: `entries_with` shipped as an uncalled function passes every item here.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use tikray::tui::{entries, entries_with};

/// A tree with both kinds of hidden thing, plus one that is both.
///
/// Built here rather than added to `tests/fixtures/`, precisely so item 2 stays
/// true: a dot-entry there would silently change what Phase 7's shipped listing
/// assertions prove.
///
/// Per-test subdirectory, following Phase 3's `workdir(name)` idiom — cargo runs
/// tests in parallel, and a shared path means one test's teardown races
/// another's setup. Caught by the full suite after a single-file run passed by
/// luck, which is the usual way.
fn tree(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("gate_phase10")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::create_dir_all(dir.join(".cache")).unwrap();
    std::fs::create_dir_all(dir.join("visible")).unwrap();
    let svg = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/icon24.svg");
    std::fs::copy(&svg, dir.join("pic.svg")).unwrap();
    // Previewable AND hidden: the case that proves the two filters are
    // independent rather than one being a special case of the other.
    std::fs::copy(&svg, dir.join(".secret.svg")).unwrap();
    std::fs::write(dir.join(".DS_Store"), b"junk").unwrap();
    std::fs::write(dir.join("notes.txt"), b"not an image").unwrap();
    dir
}

fn labels(dir: &Path, all: bool) -> Vec<String> {
    entries(dir, all)
        .unwrap()
        .into_iter()
        .map(|e| e.label)
        .collect()
}

// ---------------------------------------------------------------------------
// Item 1 — dot-entries are hidden, files and directories alike.
// ---------------------------------------------------------------------------

#[test]
fn gate1_the_filtered_listing_holds_only_visible_images_and_directories() {
    let dir = tree("filtered");
    let mut shown = labels(&dir, false);
    shown.sort();
    assert_eq!(shown, vec!["pic.svg".to_string(), "visible/".to_string()]);
}

#[test]
fn gate1_a_hidden_directory_is_filtered_which_phase_7_had_exempted() {
    // Phase 7 exempted directories from the *previewability* filter because
    // hiding one stops you descending into it. That reason does not carry to the
    // dot rule: leaving is `←`, not a row.
    let shown = labels(&tree("dirs"), false);
    for name in [".git/", ".cache/"] {
        assert!(
            !shown.contains(&name.to_string()),
            "{name} should be hidden"
        );
    }
}

#[test]
fn gate1_a_previewable_but_hidden_file_is_still_hidden() {
    // The case that proves the filters are independent: `.secret.svg` passes
    // `previewable` and is hidden anyway.
    let dir = tree("both");
    assert!(!labels(&dir, false).contains(&".secret.svg".to_string()));
    assert!(labels(&dir, true).contains(&".secret.svg".to_string()));
}

#[test]
fn gate1_showing_all_brings_back_all_seven() {
    let mut all = labels(&tree("all"), true);
    all.sort();
    assert_eq!(
        all,
        vec![
            ".DS_Store",
            ".cache/",
            ".git/",
            ".secret.svg",
            "notes.txt",
            "pic.svg",
            "visible/"
        ]
    );
}

// ---------------------------------------------------------------------------
// Item 2 — Phase 7's own listing assertions are untouched.
// ---------------------------------------------------------------------------

#[test]
fn gate2_the_shipped_fixtures_contain_no_dot_entries() {
    // Asserted rather than assumed: a fixture added later with a leading dot
    // would silently change what `tests/gate_phase7.rs`'s three membership
    // assertions prove, without failing them.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in std::fs::read_dir(&fixtures).unwrap() {
        let name = entry.unwrap().file_name();
        assert!(
            !name.as_encoded_bytes().starts_with(b"."),
            "{name:?} is a dot-entry, so Phase 7's gate no longer means what it meant"
        );
    }
}

// ---------------------------------------------------------------------------
// Item 3 — the named entry is exempted, and nothing else is.
// ---------------------------------------------------------------------------

#[test]
fn gate3_entries_with_lists_the_named_entry_the_filter_would_hide() {
    let dir = tree("keep");
    let shown: Vec<String> = entries_with(&dir, false, Some(OsStr::new(".secret.svg")))
        .unwrap()
        .into_iter()
        .map(|e| e.label)
        .collect();

    assert!(
        shown.contains(&".secret.svg".to_string()),
        "the named entry"
    );
    // One exemption, not a filter that gave up.
    for still_hidden in [".git/", ".cache/", ".DS_Store"] {
        assert!(
            !shown.contains(&still_hidden.to_string()),
            "{still_hidden} must stay hidden"
        );
    }
}

#[test]
fn gate3_entries_is_unchanged_by_the_sibling_existing() {
    // What keeps Phase 7's gate item 6 green.
    let dir = tree("delegate");
    let plain: Vec<String> = entries(&dir, false)
        .unwrap()
        .into_iter()
        .map(|e| e.label)
        .collect();
    let delegated: Vec<String> = entries_with(&dir, false, None)
        .unwrap()
        .into_iter()
        .map(|e| e.label)
        .collect();
    assert_eq!(plain, delegated);
}
