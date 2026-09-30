# Generated TS vs. wasm-bindgen: the payment transition

<!-- derived-from ../../design/06-strategy.md#5-validating-demand-next -->

Numbers for [design/06 §2](../../design/06-strategy.md#2-alternatives): the same Rust source, [examples/payment](../../examples/payment/src/lib.rs), called from JS as the generated TS package (F) and as WASM built with wasm-bindgen (B).

`./measure.sh` builds both and prints everything below. The WASM crate is outside the workspace and fetches its dependencies (from outside the repository's directory, so the workspace's vendored sources do not apply) (serde, serde-wasm-bindgen, wasm-bindgen 0.2.118 to match the CLI) from crates.io. `build.rs` copies the example unchanged: it already derives serde, reading `Amount` and `PaymentMethodId` through `#[serde(try_from)]`. `cargo test` in `wasm/` checks those derives with the real serde. Both sides keep debug-build integer semantics: the release profile sets `overflow-checks = true`.

## Variants

| Variant | What crosses per call | Caller's state |
| --- | --- | --- |
| `ts` | nothing (plain JS call) | plain `Readonly` values |
| `json` | `JSON.stringify` → serde_json → `JSON.parse` | plain objects |
| `swb` | serde-wasm-bindgen converts JS objects both ways | plain objects |
| `handle` | the event only; state stays in WASM memory | an opaque handle (not React state, not `structuredClone`-able) |

A run is `create` plus four events: attach a card, confirm (3D Secure required), the action succeeds (manual capture holds the funds), capture 1500 with a 100 fee. Every result is checked against the expected final state, including inside the timing loop, so no run can be optimized away.

## Results

2026-10-01, purecrate-ts after 0.4.0 (payment rewritten with a tuple `match`, guards, and the `Option` methods; the runtime with the 0.4.0 methods), Apple M5 Max, Node 24.20, rustc 1.98.1, wasm-bindgen 0.2.118, esbuild 0.28.

Size, in bytes (raw / gzip -9):

| Artifact | Size |
| --- | --- |
| TS bundle (esbuild, minified; `create`, `step`, constructors, runtime) | 9,383 / 3,101 |
| WASM, all three boundaries, opt-level 3 | 235,252 / 80,195 |
| WASM, serde-wasm-bindgen only, opt-level z, `wasm-opt -Oz` | 72,296 / 32,772 |
| wasm-bindgen web glue (minified) | 7,708 / 2,959 |

The smallest WASM plus glue is about 11.5× the TS bundle, gzipped.

Time from script start to the first `step` result, including module load and WASM compilation (median of 21 fresh Node processes):

| Variant | ms |
| --- | --- |
| `ts` | 0.72 |
| `json` | 2.30 |
| `swb` | 1.84 |
| `handle` | 1.64 |

Per-call cost after warm-up (median of 5 rounds of 200,000 runs; one run is five calls):

| Variant | ns per run | ns per call | vs. `ts` |
| --- | --- | --- | --- |
| `ts` | 74 | 15 | 1× |
| `handle` | 1,734 | 347 | 23× |
| `swb` | 5,670 | 1,134 | 76× |
| `json` | 7,128 | 1,426 | 95× |

Against the first measurement (2026-09-29, before 0.4.0): the TS bundle grew from 6,036 / 1,795 bytes, mostly the runtime's integer methods and slicing, which every package carries whether it calls them or not; the first call went from 0.46 to 0.72 ms for the same reason. A 1.18 ms first call measured the same day came from the runtime's Unicode-property regular expression, compiled at module load; it is now built when first used. The WASM numbers did not move.

## Reading

- For small, frequent transitions, the boundary dominates. Keeping state in WASM (`handle`) is the cheapest WASM option, but the event still pays serde-wasm-bindgen, and the state stops being a plain value.
- The logic itself is not what is being measured: `step` does a handful of comparisons. A computation-heavy function would narrow the gap or reverse it; this project targets transitions, not kernels.
- Not measured: browsers (streaming compilation changes first-call time), React Native / Hermes, memory. Node compiles the WASM module synchronously here (`--target nodejs`); a browser's `instantiateStreaming` overlaps compilation with download.
