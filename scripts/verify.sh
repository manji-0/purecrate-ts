#!/usr/bin/env bash
# v0 acceptance: Rust tests (golden + TS/Rust equivalence via node), drift of
# each example's committed packages, then tsc over them (each carries its
# runtime; those with a schema find its library in examples/node_modules) and
# over the runtime and adapter sources that every package copies, under each
# supported TypeScript major (see TS_MAJORS in
# crates/cli/tests/it/support/mod.rs).
set -euo pipefail
cd "$(dirname "$0")/.."

TS_MAJORS=(6 7)

cargo test --offline -q
# Every example's committed output (scripts/examples.sh regenerates them).
examples=()
for dir in examples/*/; do
  dir=${dir%/}
  [ -f "$dir/src/lib.rs" ] || continue
  cargo run --offline -q -p purecrate-ts -- check "$dir" --out "$dir/ts/plain"
  examples+=("$dir/ts/plain")
  for lib in zod valibot arktype; do
    cargo run --offline -q -p purecrate-ts -- check "$dir" --out "$dir/ts/$lib" --schema "$lib"
    examples+=("$dir/ts/$lib")
  done
done
# The adapters import the runtime package, which exports `dist`; here its
# sources stand in for it (the purecrate-source condition).
for major in "${TS_MAJORS[@]}"; do
  for dir in packages/boundary packages/boundary-zod packages/boundary-valibot packages/boundary-arktype "${examples[@]}"; do
    (cd "$dir" && npx -y -p "typescript@$major" tsc -p . --customConditions purecrate-source) \
      || { echo "verify: tsc $major failed in $dir" >&2; exit 1; }
  done
done
echo "verify: ok"
