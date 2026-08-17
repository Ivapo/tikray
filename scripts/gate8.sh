#!/usr/bin/env bash
#
# tkr-001 Phase 3 — exit gate item 8, the human half.
#
#   "Convert a PNG, a JPEG and an SVG to each of PNG and JPEG, open the results
#    in a normal image viewer, and `tikray view` them — they show the same
#    picture, right way up, right colours. A transparent source converted to
#    JPEG comes out on white, not black, and the run says on stderr that alpha
#    was flattened."
#
# Self-contained: it builds tikray, writes its own transparent fixture, does
# every conversion from scratch, then shows you each result inline and asks.
# Nothing here writes into the repo.
#
# Run it in an iTerm2 window:   bash scripts/gate8.sh
#
# It reproduces the run recorded in specs/reviews/tkr-001.md under
# "Phase 3 exit gate". It is in the repo because a gate someone else could
# check is the point of a gate -- see CLAUDE.md.

set -uo pipefail

# The repo is wherever this script lives, so it works from any directory.
REPO="${REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
WORK="${WORK:-${TMPDIR:-/tmp}/tikray-gate8}"
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

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

bold "tkr-001 Phase 3 — gate item 8"
echo

if [ "${TERM_PROGRAM:-}" != "iTerm.app" ] && [ "${LC_TERMINAL:-}" != "iTerm2" ]; then
  warn "This does not look like iTerm2 (TERM_PROGRAM=${TERM_PROGRAM:-unset},"
  warn "LC_TERMINAL=${LC_TERMINAL:-unset}). The inline half of this gate cannot"
  warn "run here — open a real iTerm2 window and try again."
  exit 1
fi

if [ ! -d "$REPO" ]; then
  warn "No repo at $REPO — set REPO=... and re-run."
  exit 1
fi

dim "building…"
( cd "$REPO" && cargo build 2>&1 | tail -2 ) || { warn "build failed"; exit 1; }

rm -rf "$WORK"
mkdir -p "$WORK/out"

# A transparent source worth looking at. The repo's alpha fixtures are 2x2 and
# 4x4 — correct for the machine assertions, useless to a human eye. No
# background rect here: the page is genuinely transparent, which is the point.
cat > "$WORK/transparent.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320" viewBox="0 0 480 320">
  <circle cx="150" cy="160" r="110" fill="#e23b3b" fill-opacity="0.55"/>
  <circle cx="260" cy="160" r="110" fill="#2f7fd4" fill-opacity="0.55"/>
  <circle cx="205" cy="90"  r="70"  fill="#f0c419" fill-opacity="0.55"/>
  <rect x="330" y="40"  width="110" height="110" fill="#1a1a1a"/>
  <rect x="330" y="180" width="110" height="100" fill="#cccccc"/>
</svg>
SVG

# ---------------------------------------------------------------------------
# Convert
# ---------------------------------------------------------------------------

echo
bold "1. Converting"
dim "   Each command is shown with its exit code. Any non-zero here is a bug —"
dim "   every one of these is a supported pair."
echo

convert_one() {
  local src="$1" dst="$2"
  printf '   \033[2m$\033[0m tikray convert %s %s\n' "$(basename "$src")" "$(basename "$dst")"
  "$TIKRAY" convert "$src" "$dst"
  local code=$?
  if [ $code -ne 0 ]; then
    printf '\033[31m     exit=%s — unexpected\033[0m\n' "$code"
    fail=1
  fi
}

for pair in "wide.png:wide_png" "rgb.jpg:rgb_jpg" "wide.svg:wide_svg"; do
  src="${pair%%:*}"; base="${pair##*:}"
  for ext in png jpg; do
    convert_one "$REPO/tests/fixtures/$src" "$WORK/out/$base.$ext"
  done
done

echo
dim "   And the transparency case, which is what §2.13 is about:"
convert_one "$WORK/transparent.svg" "$WORK/out/transparent.jpg"
convert_one "$WORK/transparent.svg" "$WORK/out/transparent.png"

echo
bold "   The line above about flattening onto white is the one §2.13 mandates."
ask "Did 'flattening it onto white' appear on stderr for the JPEG runs?"

# ---------------------------------------------------------------------------
# Look at them in a normal image viewer
# ---------------------------------------------------------------------------

echo
bold "2. In a normal image viewer"
dim "   Opening $WORK/out in Preview. Arrow through all eight."
open "$WORK/out"
echo
ask "Does each PNG/JPEG pair show the SAME picture, right way up, right colours?"
ask "Is transparent.jpg on WHITE (not black)?"

# ---------------------------------------------------------------------------
# And inline, through tikray view
# ---------------------------------------------------------------------------

echo
bold "3. Inline, through tikray view"
dim "   The other half of the observable: these files have to be readable back"
dim "   by the tool that wrote them. Sorted so each PNG/JPEG pair is adjacent."
echo

for f in "$WORK"/out/*; do
  name="$(basename "$f")"
  printf '\033[1m   %s\033[0m\n' "$name"
  # transparent.png keeps its alpha, so iTerm2 composites it over the terminal
  # background rather than over white. That is correct, and is not the thing
  # this gate is asking about — say so, or it reads as a failure.
  case "$name" in
    transparent.png)
      dim "   (alpha is kept here, so this one shows through to your terminal"
      dim "    background — expected. The .jpg is the white-vs-black check.)" ;;
    rgb_jpg.*)
      dim "   (64x48, so it draws as a speck — never-upscale rule, OQ-8.)" ;;
  esac
  "$TIKRAY" view "$f" || { warn "   view failed for $f"; fail=1; }
  pause "$name"
done

echo
ask "Did every file draw inline, showing the same picture as in Preview?"

# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------

echo
if [ "$fail" -eq 0 ]; then
  printf '\033[32m\033[1mITEM 8 PASSES.\033[0m\n'
  echo
  dim "Report back and I will write commit 5: the shipped: 2026-08-16 date in"
  dim "the spec's phases[], and the exit-gate record in specs/reviews/tkr-001.md."
else
  printf '\033[31m\033[1mITEM 8 FAILED.\033[0m\n'
  echo
  dim "Tell me which question you answered no to and what you actually saw."
  dim "Nothing has been pushed, so Phase 3 is not shipped either way."
fi
echo
dim "Outputs left in $WORK/out for another look."
exit "$fail"
