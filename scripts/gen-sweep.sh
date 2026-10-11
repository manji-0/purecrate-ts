#!/usr/bin/env bash
# Runs the generated equivalence tests over a range of seeds, one process per
# seed and generator, in parallel, and reports per generator how many seeds
# agree with Rust and type-check, agree but are refused by tsc, or fail.
#
#   scripts/gen-sweep.sh [-s FROM-TO] [-j JOBS] [-g GEN[,GEN..]] [-o DIR] [-k]
#
#   -s  seeds, inclusive (default 1-120)
#   -j  parallel runs (default: half the CPUs; each run uses rustc, tsc, node)
#   -g  generators (default all): expressions, statements, patterns,
#       widths, text, floats, collections, strings
#   -o  where the logs go (default: a temporary directory, printed)
#   -k  keep going on a refusal: exit 0 when every value agrees, even where
#       tsc refuses (by default, a refusal fails the sweep as a mismatch does)
#
# Each run sets PURECRATE_GEN_SEED and PURECRATE_GEN_TYPES=report, so the
# values are compared even where tsc refuses the package; its log is
# <dir>/<generator>-<seed>.log. Rerun one with the line the summary prints,
# and reduce it with scripts/gen-reduce.py. Seeds 1 to 120 of all five
# generators take about 25 minutes with 6 runs at a time on an M-series Mac.
# Run it when changing what the printer or the fold produces
# (crates/emit_ts, crates/check, crates/syntax), and before a release.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

seeds=1-120
cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)
jobs=$(( cpus > 1 ? cpus / 2 : 1 ))
gens=expressions,statements,patterns,widths,text,floats,collections,strings
out=
keep=
while getopts "s:j:g:o:k" opt; do
  case $opt in
    s) seeds=$OPTARG ;;
    j) jobs=$OPTARG ;;
    g) gens=$OPTARG ;;
    o) out=$OPTARG ;;
    k) keep=1 ;;
    *) sed -n '2,24p' "$0" >&2; exit 2 ;;
  esac
done
from=${seeds%-*}
to=${seeds#*-}
[ -n "$out" ] || out=$(mktemp -d "${TMPDIR:-/tmp}/gen-sweep.XXXXXX")
mkdir -p "$out"

test_of() {
  case $1 in
    expressions) echo generated_equivalence::generated_expressions_match_rust ;;
    statements) echo generated_statements_equivalence::generated_statements_match_rust ;;
    patterns) echo generated_patterns_equivalence::generated_patterns_match_rust ;;
    widths) echo generated_widths_equivalence::generated_widths_match_rust ;;
    text) echo generated_text_equivalence::generated_text_matches_rust ;;
    floats) echo generated_floats_equivalence::generated_floats_match_rust ;;
    collections) echo generated_collections_equivalence::generated_collections_match_rust ;;
    strings) echo generated_strings_equivalence::generated_strings_match_rust ;;
    *) echo "gen-sweep: no generator '$1'" >&2; exit 2 ;;
  esac
}

# One test binary for every run, copied out of target/ so a rebuild while
# the sweep runs does not change it.
echo "gen-sweep: building the test binary"
bin=$(cargo test --offline -q -p purecrate-ts --test it --no-run --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*\/deps\/it-[^"]*\)".*/\1/p' | tail -1)
[ -x "$bin" ] || { echo "gen-sweep: no test binary built" >&2; exit 1; }
cp "$bin" "$out/it"

runs="$out/runs.txt"
: > "$runs"
IFS=, read -ra list <<<"$gens"
for g in "${list[@]}"; do
  test_of "$g" >/dev/null
  for s in $(seq "$from" "$to"); do echo "$g $s"; done >> "$runs"
done
total=$(wc -l < "$runs" | tr -d ' ')
echo "gen-sweep: $total runs (seeds $from-$to; $gens), $jobs at a time; logs in $out"

one() {
  local g=$1 s=$2 log="$out/$1-$2.log" t
  t=$(test_of "$g")
  if (cd "$root/crates/cli" && PURECRATE_GEN_SEED=$s PURECRATE_GEN_TYPES=report \
      "$out/it" --exact "$t" --nocapture) > "$log" 2>&1; then
    if grep -q 'tsc refuses the package' "$log"; then echo "$g $s types"; else echo "$g $s ok"; fi
  else
    echo "$g $s FAIL"
  fi
}
export -f one test_of
export out root
xargs -P "$jobs" -L 1 bash -c 'one "$0" "$1"' < "$runs" > "$out/status.txt"

echo
printf '%-12s %6s %8s %6s\n' generator ok "tsc-only" fail
bad=0
for g in "${list[@]}"; do
  ok=$(grep -c "^$g [0-9]* ok$" "$out/status.txt" || true)
  ty=$(grep -c "^$g [0-9]* types$" "$out/status.txt" || true)
  fl=$(grep -c "^$g [0-9]* FAIL$" "$out/status.txt" || true)
  printf '%-12s %6s %8s %6s\n' "$g" "$ok" "$ty" "$fl"
  if [ "$fl" -gt 0 ] || { [ -z "$keep" ] && [ "$ty" -gt 0 ]; }; then bad=1; fi
done

problems=$(grep -E ' (types|FAIL)$' "$out/status.txt" | sort -k1,1 -k2,2n || true)
if [ -n "$problems" ]; then
  echo
  echo "gen-sweep: seeds to look at (first lines of each log):"
  while read -r g s st; do
    echo "== $g seed $s: $st"
    grep -m3 -E 'rust .* / ts|error TS|rejected|message:|SyntaxError|panicked' "$out/$g-$s.log" | cut -c1-200 || true
    echo "   rerun: (cd crates/cli && PURECRATE_GEN_SEED=$s PURECRATE_GEN_TYPES=report $out/it --exact $(test_of "$g") --nocapture)"
    echo "   reduce: scripts/gen-reduce.py $g $s"
  done <<<"$problems"
fi
echo
if [ "$bad" -eq 0 ]; then echo "gen-sweep: ok"; else echo "gen-sweep: failed"; fi
exit "$bad"
