#!/usr/bin/env bash
# v0 acceptance: Rust tests (golden + TS/Rust equivalence via node), drift of
# the committed counter package, then tsc over it (it carries its runtime, so
# it needs nothing installed) and over the runtime and adapter sources that
# every package copies, under each supported TypeScript major (see TS_MAJORS
# in crates/cli/tests/support/mod.rs).
set -euo pipefail
cd "$(dirname "$0")/.."

TS_MAJORS=(6 7)

cargo test --offline -q
cargo run --offline -q -p purecrate-ts -- check examples/counter --out examples/counter-ts
cargo run --offline -q -p purecrate-ts -- check examples/order
cargo run --offline -q -p purecrate-ts -- check examples/signup
cargo run --offline -q -p purecrate-ts -- check examples/iban
cargo run --offline -q -p purecrate-ts -- check examples/payment
cargo run --offline -q -p purecrate-ts -- check examples/invoice
cargo run --offline -q -p purecrate-ts -- check examples/oidc
# The adapters import the runtime package, which exports `dist`; here its
# sources stand in for it (the purecrate-source condition).
for major in "${TS_MAJORS[@]}"; do
  for dir in packages/boundary packages/boundary-zod packages/boundary-valibot packages/boundary-arktype examples/counter-ts; do
    (cd "$dir" && npx -y -p "typescript@$major" tsc -p . --customConditions purecrate-source) \
      || { echo "verify: tsc $major failed in $dir" >&2; exit 1; }
  done
done
echo "verify: ok"
