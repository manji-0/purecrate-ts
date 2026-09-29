# Strategy: means, demand, and criteria

Status: analysis, revised 2026-09-29

<!-- derived-from ./00-overview.md#1-claim -->

## 1. The problem

Teams with a Rust backend and a TS frontend want the same decisions (a transition, a validation, a price) on both sides. Today they either implement twice and keep the copies aligned with parity tests, or ship the Rust as WASM. Types are already shared routinely (ts-rs and friends); behavior is not.

## 2. Alternatives

| Means | Equivalence | TS ergonomics | Runtime needs | Cost | Expressiveness |
| --- | --- | --- | --- | --- | --- |
| A. Dual implementation + parity tests | as good as the tests | best | none | continuously high | unlimited |
| B. Rust → WASM (wasm-bindgen, Crux) | near complete | serialization at the boundary, async init | WASM | medium | nearly all Rust |
| C. Types only (ts-rs, specta, tsify) + hand-written logic | types only | good | none | low | types only |
| D. A language targeting both (Gleam, Kotlin MP) | by the language | varies | none | language migration | whole language |
| E. LLM translation | none | good | none | low, poor upkeep | arbitrary |
| **F. This: translate a pure subset** | **by the translator, checked by differential tests** | **best (plain values)** | **none** | **low to medium** | **subset** |

F is stronger when:

- **Calls are small and frequent.** WASM pays encoding and copying at each crossing; many small calls can be slower than JS (wasm-bindgen #2355). F has no boundary. Measured on examples/payment ([bench/payment](../bench/payment/README.md), Node 24): 12 ns per `step` for F against 345 ns with the state kept in WASM and 1.1–1.4 µs with plain objects crossing; 0.5 ms to the first result against 1.7–2.2 ms; 1.8 KB gzipped against 36 KB for the smallest WASM module with its glue.
- **Values should be plain TS.** Crux's web shell serializes with bincode; F's values go straight into React state.
- **The output should be readable.** Generated TS can be reviewed and stepped through.
- *WASM is unavailable* — no longer a main argument: React Native 0.84's Hermes V1 (2026-02) supports WASM, though wasm-bindgen output is unconfirmed there.

F is weaker in:

- **Expressiveness.** Only the subset. The author pays in notation ([02](./02-authoring.md)).
- **Burden of correctness.** Under B, rustc guarantees semantics. Under F, every semantic hole is ours. This is why equivalence is defined narrowly and tested per operator ([01](./01-equivalence.md)); the early `7 / 2 = 3.5` bug showed what happens otherwise.
- **Maintaining a small compiler.** Type inference, strings, and iteration each raise the maintenance load.

Design decisions judged sound after v0: a target-independent IR; rejection over partial generation (mandatory when equivalence is the product); `kind` unions and companions; `i64` as `bigint`; goldens, differential tests, and drift detection from day one; building type inference and numeric semantics before widening syntax.

## 3. Demand

### 3.1 Signals (crates.io, 2026-09)

| Crate | Total DL | Recent DL | Nature |
| --- | --- | --- | --- |
| ts-rs | 15.93M | 6.2M | Rust → TS types |
| tsify | 9.75M | 1.76M | types for wasm-bindgen |
| specta | 2.61M | 1.23M | type introspection → TS |

These measure type sharing, include transitive downloads, and are not user counts; they do show a large population that keeps TS aligned with Rust as the source of truth.

For behavior sharing: **Crux** shares a pure Rust core via WASM with the same transition-centered structure; a **zod_gen** report removed 1,300 lines of triple-maintained structs, interfaces, and schemas; **r/rust** threads on sharing validation split between WASM, dual implementation, and LLM translation; **jsonstat/validator** runs cross-implementation parity tests in CI. No mature tool translating function bodies was found; tamusjroyce/rust-to-ts is a PoC that leaves unsupported syntax as comments, the opposite design choice.

Reading: **the adjacent market is large; direct demand is unverified.** Because the target is new code written within constraints, the question is whether users will pay that authoring cost, not how much existing code passes.

### 3.2 Use cases

| Use case | Demand (est.) | Writable | Author's cost |
| --- | --- | --- | --- |
| Workflows, state machines | medium | **high** (order, payment) | one function per state; `_ =>` since 2026-09-29 |
| Optimistic UI, offline-first | medium–high | high | sequences as recursive enums |
| Turn-based game rules | medium | medium | loops as recursion; recursion-depth limit |
| **Input validation** | **high** | **medium** (signup, iban) | bytes via `as_bytes`, classes via `matches!`; no regex; ≈1.4–1.8× lines |
| Pricing, fees, tax | medium | high (invoice) | integer newtypes, no decimal; rounding written as integer division; ≈1.4× lines |
| Edge functions | low–medium | high | none (WASM works too) |

**Demand and capability are misaligned**: validation has the most signals and is the hardest to write; state machines are the easiest and have only indirect signals. This is the main strategic risk. Closed types, `as_bytes`, and range `for` were added to narrow the gap.

Expected users: small to mid-sized teams with a Rust backend and a TS frontend (web or React Native), already sharing types, now fighting duplicated logic, avoiding WASM builds. Tauri apps need it less (IPC to Rust), except for synchronous frontend-only checks.

## 4. Success and withdrawal criteria

<!-- constrained-by ./02-authoring.md#1-what-constraint-costs -->

| Metric | Criterion |
| --- | --- |
| Equivalence | Differential tests (boundary values, exhaustive sequences where feasible) agree on whole values for every example |
| Types | 0 errors under `tsc --strict`, TS 6 and 7 |
| Determinism | same input, same bytes |
| Rejection quality | `path:line:col` + reason, no partial output |
| Capability | transitions keep ADTs, exhaustiveness, `Result`, debug integer semantics |
| Idiomaticity | no serialization, initialization, or async loading for TS users |

The author's own examples cannot validate the constraints: the author writes around them. External criteria (set 2026-09-28; numbers to be revisited):

1. **Write from third-party specifications**, reading only [02](./02-authoring.md) and [07](./07-roadmap.md). Record rewrites forced by rejection, and rejections the documents could not explain.
2. **Compare with idiomatic code**: the same spec in unconstrained Rust and in kamae-style TS. Compare line counts, forced style changes, and readability.
3. **Withdraw or switch (to B or C) if any holds:**
   - more than half of third-party specs cannot be written;
   - constrained Rust exceeds **2×** the lines of idiomatic Rust;
   - silent wrong values on accepted input keep appearing (guide: one or more per new example);
   - no real use replacing a dual implementation is ever obtained.

Status: signup, iban, payment, and invoice are from third-party specs; all four could be written. signup and iban first exceeded 2× (2.8×, 2.4×) and returned under it (1.9×) after range `for`; payment's transitions first measured 2.1× and returned to 1.8× after `_` and `A | B` arms ([07 §2](./07-roadmap.md#2-evidence-from-examples)). invoice's first draft measured 2.2× and a restructured one 1.4× with no new capability. No silent wrong value appeared in any of them. No real-world replacement yet.

## 5. Validating demand next

1. More third-party specs. Done for one outside validation (Stripe's PaymentIntent lifecycle, 2026-09-29).
2. **One real use**: replace a dual-implemented state machine; record lines removed and what the tests guarantee. Still open, and the only withdrawal criterion not yet answered. The packages are not on npm yet.
3. **Numbers for §2**: done on Node for payment ([bench/payment](../bench/payment/README.md)). Browsers and Hermes are not measured.

## References

- React Native 0.84: <https://reactnative.dev/blog/2026/02/11/react-native-0.84>; Hermes and wasm-bindgen: <https://github.com/paritytech/verifiablejs/issues/25>
- WASM boundary cost: <https://rs4ts.dev/19-wasm/09-performance/>, <https://github.com/rustwasm/wasm-bindgen/issues/2355>
- Crux: <https://github.com/redbadger/crux>, <https://redbadger.github.io/crux/part-2/shell/react.html>
- crates.io: <https://crates.io/crates/ts-rs>, <https://crates.io/crates/tsify>, <https://crates.io/crates/specta>
- zod_gen: <https://kamil.chm.ski/bridging-rust-and-typescript-with-zod_gen>
- Sharing validation: <https://www.reddit.com/r/rust/comments/1m0x8k2/share_validation_schemas_between_backend_and/>
- Parity tests: <https://github.com/jsonstat/validator>
- Partial translation PoC: <https://github.com/tamusjroyce/rust-to-ts>
- Gleam multi-target: <https://gleam.run/news/v0.34-multi-target-projects/>
