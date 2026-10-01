#!/usr/bin/env bash
# Regenerates every example's committed output, beside its source:
# examples/<name>/ts/plain without a schema, and examples/<name>/ts/<lib> with
# each schema library. scripts/verify.sh fails when one is out of date.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --offline -q -p purecrate-ts
for dir in examples/*/; do
  dir=${dir%/}
  [ -f "$dir/src/lib.rs" ] || continue
  target/debug/purecrate-ts build "$dir" --out "$dir/ts/plain"
  for lib in zod valibot arktype; do
    target/debug/purecrate-ts build "$dir" --out "$dir/ts/$lib" --schema "$lib"
  done
done
