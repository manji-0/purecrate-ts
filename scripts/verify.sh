#!/usr/bin/env bash
# v0 acceptance: Rust tests (golden + TS/Rust equivalence via node), drift of
# the committed counter package, then tsc over it.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo test --offline -q
cargo run --offline -q -p purecrate-ts -- check examples/counter --out examples/counter-ts
mkdir -p examples/counter-ts/node_modules
ln -sfn "$(pwd)/packages/boundary" examples/counter-ts/node_modules/purecrate
(cd packages/boundary && npx -y -p typescript@5 tsc -p .)
(cd examples/counter-ts && npx -y -p typescript@5 tsc -p .)
echo "verify: ok"
