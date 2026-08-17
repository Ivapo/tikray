#!/usr/bin/env bash
#
# tkr-001 Phase 4 — exit gate items 6 and 7, the human half.
#
#   6. "`tikray` opens the browser; arrow-key navigation redraws the highlighted
#       image; `tikray <path>` opens at that path's directory with it
#       highlighted; `tikray view <path>` still draws inline and exits. Quitting
#       restores the terminal — no residual alternate screen, no swallowed
#       cursor, no leaked escape state. And the named visual property: the image
#       sits inside its pane and does not cross the border, including while the
#       window is resized."
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

bold "tkr-001 Phase 4 — gate items 6 and 7"
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
dim "     · land on README.md — no image, and a line saying why"
dim "     · RESIZE THE WINDOW, a few times, with an image showing"
dim "     · press q to quit"
echo
bold "   Watch the border. The whole question is whether the image ever crosses"
bold "   it — into the file list, or past the bottom edge, at any window size."
pause "ready"

snapshot
( cd "$REPO/samples" && "$TIKRAY" )
echo
check_stty
ask "Did each highlighted image draw inside the preview pane?"
ask "Did the image stay INSIDE its border at every size you tried?"
ask "Did a non-image (README.md) show a line saying why, instead of an image?"
ask "After quitting: normal prompt, cursor visible, no leftover screen?"

# ---------------------------------------------------------------------------
# Item 6b — a path argument opens the browser there, not a single-image view
# ---------------------------------------------------------------------------

echo
bold "2. tikray <path> — the browser at that path's directory"
dim "   Running: tikray samples/landscape.svg"
dim "   It should open the BROWSER on samples/, with landscape.svg already"
dim "   highlighted and previewed — not a single-image view. Press q to quit."
pause "ready"

snapshot
( cd "$REPO" && "$TIKRAY" samples/landscape.svg )
echo
check_stty
ask "Did it open the browser on samples/ with landscape.svg highlighted?"

# ---------------------------------------------------------------------------
# Item 6c — view still does what it did
# ---------------------------------------------------------------------------

echo
bold "3. tikray view <path> — still inline, still gives the prompt back"
dim "   The second caller must not have moved the first one."
echo
( cd "$REPO" && "$TIKRAY" view samples/landscape.png )
echo
ask "Did it draw inline and return to the prompt, taking no screen?"

# ---------------------------------------------------------------------------
# Item 7a — Ctrl-C, which raw mode makes a keypress
# ---------------------------------------------------------------------------

echo
bold "4. Ctrl-C inside the TUI — a KeyEvent, not a signal"
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
bold "5. kill -INT — the other mechanism entirely"
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
