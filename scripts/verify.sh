#!/usr/bin/env bash
# v0 acceptance: Rust tests (golden + TS/Rust equivalence via node), drift of
# the committed counter package, then tsc over it and the runtime packages
# under each supported TypeScript major (see TS_MAJORS in
# crates/cli/tests/support/mod.rs).
set -euo pipefail
cd "$(dirname "$0")/.."

TS_MAJORS=(6 7)

cargo test --offline -q
cargo run --offline -q -p purecrate-ts -- check examples/counter --out examples/counter-ts
cargo run --offline -q -p purecrate-ts -- check examples/order
mkdir -p examples/counter-ts/node_modules
ln -sfn "$(pwd)/packages/boundary" examples/counter-ts/node_modules/purecrate
# The runtime packages export `dist`; in this repository their sources stand
# in for it (the purecrate-source condition).
for major in "${TS_MAJORS[@]}"; do
  for dir in packages/boundary packages/boundary-zod packages/boundary-valibot packages/boundary-arktype examples/counter-ts; do
    (cd "$dir" && npx -y -p "typescript@$major" tsc -p . --customConditions purecrate-source) \
      || { echo "verify: tsc $major failed in $dir" >&2; exit 1; }
  done
done
echo "verify: ok"
