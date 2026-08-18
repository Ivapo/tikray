#!/usr/bin/env bash
#
# tkr-001 Phase 9 — the exit gate, all three items.
#
# This gate is a script rather than a `cargo test` for a reason the phase had to
# discover: **cargo unifies features across normal and dev-dependencies**, and
# `tests/` declares `image` with defaults. So under `cargo test` the library is
# built with all fifteen codecs no matter what Cargo.toml's normal dependency
# says, and the 97 shipped assertions would pass while proving nothing at all
# about the shipped artifact.
#
# Item 1 is the only one that can fail if the phase is never done -- items 2 and
# 3 assert that behaviour and source are unchanged, which is exactly what the
# phase promises, so they pass on an untouched tree too.
#
#   bash scripts/gate-phase9.sh
#
# Needs no terminal and asks no questions.

set -uo pipefail
REPO="${REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$REPO" || exit 1

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
dim()  { printf '\033[2m%s\033[0m\n' "$*"; }
pass() { printf '\033[32m  pass\033[0m  %s\n' "$*"; }
fail=0
bad()  { printf '\033[31m  FAIL\033[0m  %s\n' "$*"; fail=1; }

WORK="${TMPDIR:-/tmp}/tikray-gate9"; rm -rf "$WORK"; mkdir -p "$WORK"
TIKRAY="$REPO/target/release/tikray"

bold "tkr-001 Phase 9 — gate"
dim "building --release (the artifact is what this gate is about)…"
cargo build --release --locked 2>&1 | tail -1

# ---------------------------------------------------------------------------
bold "1. the artifact is configured as this phase claims"
# `no-dev` is load-bearing. Without it the dev-dependency drags the defaults
# back in and this prints eighteen features on a correctly reduced tree --
# looking thorough while asserting nothing.
FEATS="$(cargo tree -e features,no-dev -i image 2>/dev/null || true)"
for want in png jpeg; do
  grep -q "image feature \"$want\"" <<<"$FEATS" \
    && pass "links $want" || bad "does not link $want"
done
for deny in gif avif rayon dds exr tiff webp bmp ico qoi pnm hdr tga ff; do
  grep -q "image feature \"$deny\"" <<<"$FEATS" \
    && bad "still links $deny" || true
done
grep -qE 'image feature "(gif|avif|rayon)"' <<<"$FEATS" || pass "links none of gif, avif, rayon"

# ---------------------------------------------------------------------------
bold "2. the shipped binary behaves identically"
F="$REPO/tests/fixtures"

out="$("$TIKRAY" convert "$F/still.gif" "$WORK/g.png" 2>&1)"
[[ "$out" == *GIF* && "$out" != *"could not be determined"* ]] \
  && pass "a GIF is refused BY NAME, without the GIF codec" \
  || bad "GIF refusal changed: $out"

for f in not_an_image.png icon24.svgz; do
  out="$("$TIKRAY" convert "$F/$f" "$WORK/u.png" 2>&1)"
  [[ "$out" == *"could not be determined"* ]] \
    && pass "$f is still undetermined" || bad "$f: $out"
done

# Distinct destinations per case: a shared one hits the overwrite guard and
# reports a failure that is this script's, not the tool's.
for f in rgb.png rgb.jpg icon24.svg bom.svg wide.svg alpha.png deep16.png; do
  "$TIKRAY" convert "$F/$f" "$WORK/${f//./_}.png" 2>/dev/null \
    && pass "$f converts" || bad "$f failed to convert"
done

python3 - "$WORK/deep16_png.png" <<'PY' && pass "deep16 is still 16-bit" || bad "deep16 was quantized"
import struct, sys
d = open(sys.argv[1], 'rb').read()
sys.exit(0 if d[24] == 16 and d[25] == 2 else 1)
PY

# JPEG *encode* -- the jpeg feature gates decode and encode together, so
# rgb.jpg above proves only half of it. This is §2.13's path.
out="$("$TIKRAY" convert "$F/alpha.png" "$WORK/a.jpg" 2>&1)"
[[ -s "$WORK/a.jpg" && "$out" == *flatten* ]] \
  && pass "JPEG encode works and still announces the flattening" \
  || bad "JPEG encode path: $out"

# The display path, without needing iTerm2.
n="$("$TIKRAY" view --force "$F/rgb.png" 2>/dev/null | wc -c | tr -d ' ')"
[[ "$n" -gt 1000 ]] && pass "the escape sequence still emits ($n bytes)" \
  || bad "display path emitted $n bytes"

# ---------------------------------------------------------------------------
bold "3. Phases 1-8's gates still pass, unmodified"
dim "   (they prove the SOURCE is unchanged; item 2 proves the artifact behaves,"
dim "    item 1 proves it is the artifact this phase describes)"
n="$(cargo test 2>&1 | grep -cE '^test result: ok')"
[[ "$n" -ge 8 ]] && pass "$n test binaries green" || bad "only $n green"

echo
if [ "$fail" -eq 0 ]; then
  printf '\033[32m\033[1mPHASE 9 GATE PASSES.\033[0m\n'
  ls -l "$TIKRAY" | awk '{printf "  binary: %.2f MiB\n", $5/1048576}'
else
  printf '\033[31m\033[1mPHASE 9 GATE FAILED.\033[0m\n'
fi
exit "$fail"
