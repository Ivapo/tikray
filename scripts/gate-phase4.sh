#!/usr/bin/env bash
#
# tkr-001 Phase 4 — exit gate items 6 and 7, the human half,
#   AMENDED by Phase 6 — which also carries its item 7 here.
#
# Phase 6 reversed what `tikray <path>` does and centred the preview, so two of
# the questions below had their right answers changed and one invocation is new.
# The amendment is deliberate and is argued in Phase 6's scope: an assertion file
# is *evidence* and a gate script is a *procedure*. Phase 4's evidence —
# tests/gate_phase4.rs and its exit-gate entry in specs/reviews/tkr-001.md — is
# untouched and still true. A procedure tracks the code, as a rules/ file does,
# and one left stale would fail for the wrong reason.
#
#   6. "`tikray` opens the browser; arrow-key navigation redraws the highlighted
#       image; [`tikray <path>` opens at that path's directory with it
#       highlighted; — SUPERSEDED by Phase 6: a bare path now draws a file
#       inline, and `--browse` is what opens the browser at it] `tikray view
#       <path>` still draws inline and exits. Quitting restores the terminal — no
#       residual alternate screen, no swallowed cursor, no leaked escape state.
#       And the named visual property: the image sits inside its pane and does
#       not cross the border, including while the window is resized."
#
#   Phase 6 item 7 adds to that: the image **sits in the middle of its pane**,
#       and `tikray --browse <file>` opens the browser at that file's directory
#       with the file highlighted — the assertion that the affordance Phase 4
#       shipped was restored rather than approximated.
#
#   7. "The terminal survives both interruptions, which are two different
#       mechanisms." Raw mode goes through `cfmakeraw`, which clears ISIG, so
#       Ctrl-C inside the TUI is a KeyEvent. A real `kill -INT` is a signal that
#       default-terminates without unwinding and runs no Drop at all.
#
# The spill is why item 6 is human: §2.14's headline hazard is an image crossing
# its pane, gate item 5 says outright that no test can see one, and this is the
# only place it is caught.
#
# Run it in an iTerm2 window:   bash scripts/gate-phase4.sh
#
# Nothing here writes into the repo. Following scripts/gate8.sh — a gate someone
# else could check is the point of a gate, see CLAUDE.md.

set -uo pipefail

REPO="${REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TIKRAY="$REPO/target/debug/tikray"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
dim()  { printf '\033[2m%s\033[0m\n' "$*"; }
warn() { printf '\033[33m%s\033[0m\n' "$*"; }
fail=0

pause() {
  printf '\033[2m  ── %s ── [enter] ──\033[0m' "$1"
  read -r _
  echo
}

ask() {
  # ask "question"  -> sets `fail` if the answer is not y
  local reply
  printf '\033[1m  ? %s \033[0m[y/N] ' "$1"
  read -r reply
  case "$reply" in
    y|Y|yes|YES) printf '\033[32m    pass\033[0m\n' ;;
    *) printf '\033[31m    FAIL — say what you saw when you report back\033[0m\n'; fail=1 ;;
  esac
}

# The terminal's own answer about its own state, so "it survived" is measured
# rather than eyeballed. `stty -g` is the whole termios struct: raw mode left on
# would change it, and so would a swallowed ECHO.
before_stty=""
snapshot() { before_stty="$(stty -g 2>/dev/null)"; }
check_stty() {
  local after
  after="$(stty -g 2>/dev/null)"
  if [ -n "$before_stty" ] && [ "$after" = "$before_stty" ]; then
    printf '\033[32m    termios unchanged — raw mode was restored\033[0m\n'
  else
    printf '\033[31m    TERMIOS CHANGED — the terminal did not come back clean\033[0m\n'
    dim "    before: $before_stty"
    dim "    after:  $after"
    fail=1
  fi
}

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

bold "tkr-001 Phases 4-8 and 10 — the human gate items"
echo

if [ "${TERM_PROGRAM:-}" != "iTerm.app" ] && [ "${LC_TERMINAL:-}" != "iTerm2" ]; then
  warn "This does not look like iTerm2 (TERM_PROGRAM=${TERM_PROGRAM:-unset},"
  warn "LC_TERMINAL=${LC_TERMINAL:-unset}). The browser would run here and show"
  warn "no previews, which is correct behaviour and not what this gate is for —"
  warn "open a real iTerm2 window and try again."
  exit 1
fi

if [ ! -t 0 ] || [ ! -t 1 ]; then
  warn "This needs a terminal on both stdin and stdout. Do not pipe it."
  exit 1
fi

dim "building…"
( cd "$REPO" && cargo build 2>&1 | tail -2 ) || { warn "build failed"; exit 1; }

# ---------------------------------------------------------------------------
# Item 6a — the browser, and the property no test can see
# ---------------------------------------------------------------------------

echo
bold "1. tikray — the browser, opened where you are"
dim "   Opening the repo's samples/ directory so there is something to look at."
echo
dim "   While you are in there, please do all four:"
dim "     · arrow up and down — each highlighted image redraws in the right pane"
dim "     · press '.' — everything appears: README.md, and any dot-entries"
dim "     · press '.' again — back to filtered, same file still highlighted"
dim "     · RESIZE THE WINDOW, a few times, with an image showing"
dim "     · press q to quit"
echo
bold "   Two things to watch, and they pull against each other."
bold "   The image should sit in the MIDDLE of its pane — Phase 6 centred it —"
bold "   and it must still never cross the border, at any window size."
pause "ready"

snapshot
( cd "$REPO/samples" && "$TIKRAY" )
echo
check_stty
ask "Did each highlighted image draw inside the preview pane?"
ask "Was it CENTRED in the pane, horizontally and vertically?"
ask "Did the image stay INSIDE its border at every size you tried?"
ask "After quitting: normal prompt, cursor visible, no leftover screen?"

echo
dim "   Phase 7 added the list filter, so README.md is no longer listed at all —"
dim "   which is why the question above about a non-image is gone. Check the"
dim "   filter instead: samples/ holds one README.md and one .gitignore-ish"
dim "   file or two that are not images."
ask "Were ONLY visible images and directories listed, with a hidden count?"
ask "Did '.' show everything, and '.' again put the filter back?"
ask "After toggling, was the SAME file still highlighted?"

# ---------------------------------------------------------------------------
# Phase 6 — a bare path dispatches on what the path IS
# ---------------------------------------------------------------------------
#
# This section is what Phase 6 changed. Phase 4 asked whether `tikray <path>`
# opened the browser at that path's directory; the right answer is now no.

echo
bold "2. tikray <file> — draws inline, indented, and gives the prompt back"
dim "   Running: tikray samples/landscape.svg"
dim "   No browser, no alternate screen — the picture, then your prompt."
echo
bold "   This is the ONLY check that Phase 7's indent was wired in at all:"
bold "   every machine assertion passes even if display() never calls it,"
bold "   because the indent appears only on a tty and cargo test gets a pipe."
echo
( cd "$REPO" && "$TIKRAY" samples/landscape.svg )
echo
ask "Did it draw inline and return to the prompt, taking no screen?"
ask "Is it INDENTED — a small gap between the left edge and the picture?"

echo
dim "   And the half that matters more: the indent must come out of the WIDTH,"
dim "   not be added to it. This image is 1200px wide — narrow your window until"
dim "   it is the binding dimension, then re-run. It must still fit on one"
dim "   screen, with no wrapping and no scrolling."
pause "ready — narrow the window first"
( cd "$REPO" && "$TIKRAY" samples/landscape.png )
echo
ask "Did it still fit on one screen, unwrapped, with the indent intact?"

echo
bold "3. tikray <dir> — browses there"
dim "   Running: tikray samples/    (q to quit)"
pause "ready"

snapshot
( cd "$REPO" && "$TIKRAY" samples )
echo
check_stty
ask "Did it open the browser on samples/?"

echo
bold "4. tikray --browse <file> — the browser, that file highlighted"
dim "   Running: tikray --browse samples/landscape.svg    (q to quit)"
dim "   This is the spelling of what a bare path meant before Phase 6, and the"
dim "   question is whether it lands on the same file rather than merely the"
dim "   same directory."
pause "ready"

snapshot
( cd "$REPO" && "$TIKRAY" --browse samples/landscape.svg )
echo
check_stty
ask "Did it open the browser on samples/ with landscape.svg HIGHLIGHTED?"

# ---------------------------------------------------------------------------
# Item 6c — view still does what it did
# ---------------------------------------------------------------------------

echo
bold "5. tikray view <path> — unchanged"
dim "   Two callers reach the inline draw now; neither may have moved it."
echo
( cd "$REPO" && "$TIKRAY" view samples/landscape.png )
echo
ask "Did it draw inline and return to the prompt, taking no screen?"

# ---------------------------------------------------------------------------
# Phase 8 — zoom
# ---------------------------------------------------------------------------
#
# Zoom is the first feature that deliberately pushes against the pane border, so
# the border property Phases 4, 6 and 7 each asserted is what this section is
# really testing.

echo
bold "6. + and - — zoom, in three centred steps"
dim "   Opening samples/ again. Please do all of these:"
dim "     · highlight icon.svg — a 24x24 speck — and press + twice"
dim "     · then - and 0, which step back down and return to fit"
dim "     · highlight landscape.svg and press + twice: it crops to the middle,"
dim "       because there is no panning"
dim "     · arrow to another file — the zoom must return to fit on its own"
dim "     · press q"
echo
bold "   THE question: at every level, does the image stay inside its border?"
bold "   A zoomed image that crosses into the file list or past the bottom edge"
bold "   is the one failure this phase was most likely to introduce."
pause "ready"

snapshot
( cd "$REPO/samples" && "$TIKRAY" )
echo
check_stty
ask "Did + enlarge the image and - shrink it back, with 0 returning to fit?"
ask "Did it stay INSIDE its border at every level, on every image you tried?"
ask "Is the 24x24 icon actually usable at 4x, where it used to be a speck?"
ask "Did moving to another file reset the zoom to fit?"

# ---------------------------------------------------------------------------
# Phase 5 — converting from inside the browser
# ---------------------------------------------------------------------------
#
# The phase exists because §2.13's flattening notice is an eprintln! and stderr
# inside the alternate screen paints over the TUI. This section is where that
# line has to appear in the pane instead.

echo
bold "7. P and J — convert from inside the browser"
dim "   Working in a scratch copy of samples/, so nothing in the repo changes."
CONV="${TMPDIR:-/tmp}/tikray-gate-phase5"
rm -rf "$CONV"; mkdir -p "$CONV"
cp "$REPO/samples/translucent.svg" "$CONV/" 2>/dev/null
cp "$REPO/samples/landscape.png" "$CONV/" 2>/dev/null
echo
dim "   In the browser, please do all four:"
dim "     · highlight translucent.svg and press J — it writes translucent.jpg"
dim "     · press J again — it refuses, because that file now exists"
dim "     · press J a third time — the refusal offered a confirm, so it writes"
dim "     · highlight landscape.png and press P — refused: that IS the file"
echo
bold "   The line to watch is the flattening notice. It must appear IN THE PANE."
bold "   If it lands on the terminal as stray text over the layout, that is the"
bold "   exact failure this phase was split out of Phase 4 to prevent."
pause "ready"

snapshot
( cd "$CONV" && "$TIKRAY" )
echo
check_stty
ask "Did J write translucent.jpg, with the pane naming the file?"
ask "Did the pane say alpha was flattened onto white — IN THE PANE, not over it?"
ask "Did the second press refuse, and the third replace it?"
ask "Did P on landscape.png refuse as 'the file itself', not 'already exists'?"
ask "Did the new file appear in the list without navigating away?"
echo
dim "   Opening $CONV so you can check the written files are real images."
open "$CONV" 2>/dev/null
ask "Do the written files open in Preview and look right?"

# ---------------------------------------------------------------------------
# Phase 10 — dot-entries, and the one check that its exemption was wired in
# ---------------------------------------------------------------------------

echo
bold "8. hidden entries, and --browse on one"
dim "   First your home directory, which is the case that prompted the phase:"
dim "   47 dot-entries against 14 visible, and they sort FIRST."
dim "   Press '.' to reveal, '.' again to hide, then q."
pause "ready"
snapshot
( cd "$HOME" && "$TIKRAY" )
echo
check_stty
ask "Did ~ open with NO dot-entries — no .cache/, .cargo/, .config/?"
ask "Did '.' reveal them and '.' hide them again?"

echo
dim "   Now the one thing no test can check: --browse on a HIDDEN file must"
dim "   highlight it. entries_with can be shipped and never called, and every"
dim "   machine assertion still passes -- this is the only place that shows."
HID="${TMPDIR:-/tmp}/tikray-gate10"; rm -rf "$HID"; mkdir -p "$HID"
cp "$REPO/samples/landscape.png" "$HID/.hidden-pic.png" 2>/dev/null
dim "   Running: tikray --browse $HID/.hidden-pic.png    (q to quit)"
pause "ready"
snapshot
"$TIKRAY" --browse "$HID/.hidden-pic.png"
echo
check_stty
ask "Was .hidden-pic.png HIGHLIGHTED, not just the directory opened?"
ask "And a directory whose contents are all hidden — did it say how many?"

# ---------------------------------------------------------------------------
# Item 7a — Ctrl-C, which raw mode makes a keypress
# ---------------------------------------------------------------------------

echo
bold "9. Ctrl-C inside the TUI — a KeyEvent, not a signal"
dim "   crossterm's raw mode goes through cfmakeraw, which clears ISIG, so the"
dim "   terminal never turns Ctrl-C into SIGINT here. tikray handles it as quit."
echo
dim "   The browser will open. Press Ctrl-C — nothing else."
pause "ready"

snapshot
( cd "$REPO/samples" && "$TIKRAY" )
echo
check_stty
ask "Did Ctrl-C quit cleanly, leaving a usable terminal?"

# ---------------------------------------------------------------------------
# Item 7b — a real SIGINT, which runs no Drop at all
# ---------------------------------------------------------------------------

echo
bold "10. kill -INT — the other mechanism entirely"
dim "   A real signal default-terminates without unwinding, so no Drop runs and"
dim "   nothing would restore the terminal. signal-hook is pinned for this one"
dim "   case, and turns it into the loop's other exit."
echo
dim "   The browser will open and a SIGINT will be sent to it in 8 seconds."
dim "   Press NOTHING — just watch it exit on its own."
warn "   (It signals the newest process named tikray. Quit any other one first.)"
pause "ready"

snapshot
( sleep 8; pkill -INT -n -x tikray ) &
signaller=$!
( cd "$REPO/samples" && "$TIKRAY" )
wait "$signaller" 2>/dev/null
echo
check_stty
ask "Did it exit by itself, leaving a usable terminal and a visible cursor?"

# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------

echo
dim "One last check, since escape state leaks silently: this line should be"
dim "plain text, and your prompt below it should behave normally."
echo

if [ "$fail" -eq 0 ]; then
  printf '\033[32m\033[1mITEMS 6 AND 7 PASS.\033[0m\n'
  echo
  dim "Report back and I will write the last commit: the shipped: date in the"
  dim "spec's phases[], the exit-gate record in specs/reviews/tkr-001.md, and"
  dim "the rules/ and README reconciliation."
else
  printf '\033[31m\033[1mPHASE 4 GATE FAILED.\033[0m\n'
  echo
  dim "Tell me which question you answered no to and what you actually saw."
  dim "Nothing has been pushed, so Phase 4 is not shipped either way."
fi
echo
exit "$fail"
