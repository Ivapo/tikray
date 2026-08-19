#!/usr/bin/env bash
# Check that scripts/gate-phase4.sh's section selector actually selects.
#
# This exists because the selector shipped broken (5ed8809, repaired in
# 927b7d5): the `if want; then` guards were nested rather than sequential, so
# the counter under-ran and `gate-phase4.sh 9` ran zero sections, asked zero
# questions, and exited 0 under a PASS banner. A gate that asks nothing and
# reports success is the worst failure available to a gate, and it is invisible
# to anyone running it -- which is why this is a script and not a habit.
#
# Needs no terminal, asks nothing, launches no binary and signals nothing:
#
#   bash scripts/verify-sections.sh                  # the real gate script
#   bash scripts/verify-sections.sh /tmp/broken.sh   # or any copy, to prove it bites
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="${1:-$REPO/scripts/gate-phase4.sh}"   # an argument lets this be tested against a broken copy
fail=0
ok()   { printf '\033[32m  ok\033[0m   %s\n' "$1"; }
bad()  { printf '\033[31m  FAIL\033[0m %s\n' "$1"; fail=1; }

# 1. The guards are sequential. Reading top to bottom, `if want; then` and `fi`
#    must alternate -- a second `if` before its `fi` is a nested guard.
#
#    This walks every `^fi$`, not only the guards', so a top-level `if ...; then
#    ... fi` written INSIDE a section would decrement the depth and could mask a
#    real nesting. That is why it is the cheap pre-check and check 2 is the
#    authoritative one: check 2 drives the file and cannot be fooled by
#    punctuation.
depth=0; nested=0
while read -r line; do
  case "$line" in
    "if want; then") depth=$((depth + 1)); [ "$depth" -gt 1 ] && nested=1 ;;
    "fi")            [ "$depth" -gt 0 ] && depth=$((depth - 1)) ;;
  esac
done < <(grep -E '^(if want; then|fi)$' "$GATE")
if [ "$nested" -eq 0 ] && [ "$depth" -eq 0 ]; then
  ok "guards are sequential, none nested"
else
  bad "guards nest (nested=$nested, unbalanced depth=$depth) -- selectors past the first will under-run"
fi

# 2. Selector N runs section N, driven against the real file. The copy stubs the
#    preflight and the reads; the control flow is untouched, which is the point.
probe="$(mktemp)"; trap 'rm -f "$probe"' EXIT
python3 - "$GATE" "$probe" <<'PY'
import sys, re
lines = open(sys.argv[1]).read().split("\n")
out, skip, done = [], False, False
for l in lines:
    if l.startswith('if [ "${TERM_PROGRAM:-}"'): skip = True
    if skip and not done and l == "if want; then": skip, done = False, True
    out.append("" if skip else l)
s = "\n".join(out)
s = s.replace('  read -r _\n', '  :\n').replace('  read -r reply\n', '  reply=y\n')
# Neutralise anything that launches or signals the real binary. The stub must
# match how the gate actually spells it -- `( cd "$REPO" && "$TIKRAY" ... )`,
# never a bare `$TIKRAY` at the start of a line. An earlier version regexed for
# the latter, matched nothing, and so drove the real TUI 15 times and fired
# `pkill -INT` at whatever tikray was newest on the machine.
s = re.sub(r'^.*"\$TIKRAY".*$', '  :', s, flags=re.M)
s = re.sub(r'^.*\bpkill\b.*$', '( : ) &', s, flags=re.M)
s = re.sub(r'^\s*open .*$', '  :', s, flags=re.M)
s = re.sub(r'^(bold "(\d+)\..*)$', r'echo "RAN \2"\n\1', s, flags=re.M)
open(sys.argv[2], "w").write(s)
PY

total=$(grep -cE '^bold "[0-9]+\.' "$GATE")
ran_all=$(bash "$probe" 2>/dev/null | grep -oE '^RAN [0-9]+' | awk '{print $2}' | paste -sd, -)
want_all=$(seq 1 "$total" | paste -sd, -)
[ "$ran_all" = "$want_all" ] && ok "no argument runs all $total" \
  || bad "no argument ran [$ran_all], expected [$want_all]"

each_ok=1
for n in $(seq 1 "$total"); do
  got=$(bash "$probe" "$n" 2>/dev/null | grep -oE '^RAN [0-9]+' | awk '{print $2}' | paste -sd, -)
  [ "$got" = "$n" ] || { bad "selector $n ran [${got:-none}], expected [$n]"; each_ok=0; }
done
[ "$each_ok" -eq 1 ] && ok "each selector 1..$total runs exactly its own section"

got=$(bash "$probe" "1,$total" 2>/dev/null | grep -oE '^RAN [0-9]+' | awk '{print $2}' | paste -sd, -)
[ "$got" = "1,$total" ] && ok "a comma list runs exactly those" \
  || bad "selector 1,$total ran [${got:-none}]"

# 3. A selector naming nothing must fail loudly, not pass silently.
range_ok=1
for bad_sel in 0 "$((total + 1))" abc "1,$((total + 1))"; do
  bash "$GATE" "$bad_sel" >/dev/null 2>&1
  [ "$?" -eq 2 ] || { bad "selector '$bad_sel' did not exit 2"; range_ok=0; }
done
[ "$range_ok" -eq 1 ] && ok "out-of-range and non-numeric selectors exit 2"

echo
if [ "$fail" -eq 0 ]; then
  printf '\033[32m\033[1mSELECTOR OK\033[0m — %s sections, each reachable.\n' "$total"
else
  printf '\033[31m\033[1mSELECTOR BROKEN\033[0m — do not trust a passing gate run.\n'
fi
exit "$fail"
