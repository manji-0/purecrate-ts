#!/usr/bin/env bash
# Generates every example and every test fixture, without a schema and with
# each schema library, surveys every crate, and prints one digest of all the
# bytes written (and of which inputs were rejected). A refactor that must not change the output
# keeps this digest: run it before and after.
#
#   scripts/output-snapshot.sh [<out-dir>]   # default: a temporary directory
#
# The per-file hashes go to <out-dir>/manifest.txt, to diff two runs, and the
# bytes of each input's generated code (no schema) to <out-dir>/sizes.txt,
# without and with the copied runtime; the examples' sizes are printed too.
set -euo pipefail
cd "$(dirname "$0")/.."

out="${1:-$(mktemp -d)}"
rm -rf "$out/gen"
mkdir -p "$out/gen"
cargo build --offline -q -p purecrate-ts 2>/dev/null || cargo build --offline -q -p purecrate-ts
bin=target/debug/purecrate-ts

inputs=()
for dir in examples/*/; do
  [ -f "$dir/src/lib.rs" ] && inputs+=("${dir%/}")
done
for file in crates/cli/tests/fixtures/*.rs; do
  inputs+=("$file")
done

for input in "${inputs[@]}"; do
  name=$(echo "$input" | tr '/.' '__')
  for schema in none zod valibot arktype; do
    dest="$out/gen/$name/$schema"
    args=(build "$input" --out "$dest")
    [ "$schema" != none ] && args+=(--schema "$schema")
    # A fixture meant to be rejected, or one that needs a driver, records
    # the rejection itself: its diagnostics are output too.
    if ! "$bin" "${args[@]}" >/dev/null 2>"$out/gen/$name.$schema.err"; then
      mkdir -p "$dest"
      sed "s|$PWD/||g" "$out/gen/$name.$schema.err" >"$dest/REJECTED"
    fi
    rm -f "$out/gen/$name.$schema.err"
  done
done

# `survey` reports, first cause and every cause, over each crate it can read.
mkdir -p "$out/gen/survey"
for dir in examples/*/ crates/cli/tests/fixtures/survey/; do
  [ -f "$dir/src/lib.rs" ] || continue
  name=$(echo "${dir%/}" | tr '/.' '__')
  "$bin" survey "$dir" --json | sed "s|$PWD/||g" >"$out/gen/survey/$name.json"
  "$bin" survey "$dir" --json --all-causes | sed "s|$PWD/||g" >"$out/gen/survey/$name.all.json"
done

(cd "$out/gen" && find . -type f | LC_ALL=C sort | xargs shasum -a 256) >"$out/manifest.txt"
for input in "${inputs[@]}"; do
  src="$out/gen/$(echo "$input" | tr '/.' '__')/none/src"
  [ -d "$src" ] || continue
  bytes=$(find "$src" -name '*.ts' ! -name purecrate-runtime.ts -exec cat {} + | wc -c | tr -d ' ')
  all=$(find "$src" -name '*.ts' -exec cat {} + | wc -c | tr -d ' ')
  echo "$bytes $all $input"
done >"$out/sizes.txt"
digest=$(shasum -a 256 <"$out/manifest.txt" | cut -d' ' -f1)
echo "$digest  ($(wc -l <"$out/manifest.txt" | tr -d ' ') files, manifest in $out/manifest.txt)"
grep ' examples/' "$out/sizes.txt" | sed 's/^/  bytes /'
