#!/usr/bin/env bash
# v0 acceptance: Rust tests (golden + TS/Rust equivalence via node), then tsc
# over the committed counter package.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo test --offline -q
(cd examples/counter-ts && npx -y -p typescript@5 tsc -p .)
echo "verify: ok"
