#!/usr/bin/env bash
# Regenerates every example's committed output, beside its source:
# examples/<name>/ts/plain without a schema, and, when the example derives
# serde (only those types have a wire form), examples/<name>/ts/<lib> with
# each schema library. scripts/verify.sh fails when one is out of date.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --offline -q -p purecrate-ts
for dir in examples/*/; do
  dir=${dir%/}
  [ -f "$dir/src/lib.rs" ] || continue
  target/debug/purecrate-ts build "$dir" --out "$dir/ts/plain"
  # Nested module files and multi-line derives are visible to check, not to
  # a `src/*.rs` grep. `--schema` refuses when there is no wire form.
  wired=
  for lib in zod valibot arktype; do
    if err=$(target/debug/purecrate-ts build "$dir" --out "$dir/ts/$lib" --schema "$lib" 2>&1); then
      wired=1
      continue
    fi
    if [ -z "$wired" ] && printf '%s\n' "$err" | grep -q 'no public struct or enum derives'; then
      break
    fi
    printf '%s\n' "$err" >&2
    exit 1
  done
done
