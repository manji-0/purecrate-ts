#!/usr/bin/env bash
# Builds both sides of the comparison and prints sizes, first-call times
# (median of 21 fresh node processes), and per-call costs. See README.md.
set -euo pipefail
cd "$(dirname "$0")"
root=../..

# Rust → WASM. The rustup toolchain's rust-lld may not find its libLLVM
# (a broken install); pointing dyld at the toolchain's lib fixes that.
toolchain_lib="$(dirname "$(dirname "$(rustup which rustc)")")/lib"
(cd wasm && RUSTC="$(rustup which rustc)" DYLD_FALLBACK_LIBRARY_PATH="$toolchain_lib" \
  "$(rustup which cargo)" build --release --target wasm32-unknown-unknown -q)
wasm=wasm/target/wasm32-unknown-unknown/release/payment_wasm.wasm
rm -rf pkg pkg-web out
wasm-bindgen --target nodejs --out-dir pkg "$wasm"
wasm-bindgen --target web --out-dir pkg-web "$wasm"

# Rust → TS.
(cd "$root" && cargo run --offline -q -p purecrate-ts -- build examples/payment --out bench/payment/ts --name payment)
mkdir -p ts/node_modules
ln -sfn "$(cd "$root" && pwd)/packages/boundary" ts/node_modules/purecrate
npx -y esbuild entry-ts.ts --bundle --format=esm --minify --conditions=purecrate-source --outfile=out/ts.min.mjs --log-level=warning
npx -y esbuild pkg-web/payment_wasm.js --format=esm --minify --outfile=out/wasm-glue.min.mjs --log-level=warning

size() { wc -c < "$1" | tr -d ' '; }
gz() { gzip -9c "$1" | wc -c | tr -d ' '; }
echo "## size (bytes: raw / gzip -9)"
echo "ts bundle:        $(size out/ts.min.mjs) / $(gz out/ts.min.mjs)"
echo "wasm module:      $(size pkg-web/payment_wasm_bg.wasm) / $(gz pkg-web/payment_wasm_bg.wasm)"
echo "wasm web glue:    $(size out/wasm-glue.min.mjs) / $(gz out/wasm-glue.min.mjs)"

# The smallest module a single-boundary app would ship: no serde_json,
# opt-level z, then binaryen's wasm-opt -Oz.
(cd wasm && RUSTC="$(rustup which rustc)" DYLD_FALLBACK_LIBRARY_PATH="$toolchain_lib" CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
  "$(rustup which cargo)" build --release --target wasm32-unknown-unknown -q --no-default-features --target-dir target-z)
wasm-bindgen --target web --out-dir out/pkg-z wasm/target-z/wasm32-unknown-unknown/release/payment_wasm.wasm
npx -y -p binaryen wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-reference-types --enable-multivalue out/pkg-z/payment_wasm_bg.wasm -o out/smallest.wasm
echo "wasm smallest:    $(size out/smallest.wasm) / $(gz out/smallest.wasm)"

echo "## first call (ms, median of 21 processes)"
for v in ts json swb handle; do
  for _ in $(seq 21); do node bench.mjs first "$v"; done \
    | node -e 'const xs=require("fs").readFileSync(0,"utf8").trim().split("\n").map(l=>JSON.parse(l).firstMs).sort((a,b)=>a-b); console.log(process.argv[1].padEnd(8), xs[10].toFixed(2))' "$v"
done

echo "## per call (median of 5 rounds)"
node bench.mjs calls
