#!/usr/bin/env bash
# Regenerates the committed output of every example from examples/<name>:
# examples/<name>-ts without a schema, and examples/<name>-<lib>-ts with each
# schema library. scripts/verify.sh fails when one is out of date.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --offline -q -p purecrate-ts
for dir in examples/*/; do
  dir=${dir%/}
  [ -f "$dir/src/lib.rs" ] || continue
  target/debug/purecrate-ts build "$dir" --out "$dir-ts"
  for lib in zod valibot arktype; do
    target/debug/purecrate-ts build "$dir" --out "$dir-$lib-ts" --schema "$lib"
  done
done
