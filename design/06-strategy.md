# Strategy: means, demand, and criteria

Status: analysis, revised 2026-10-04

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

### 2.1 Where F is stronger

- **Calls are small and frequent.** WASM pays encoding and copying at each crossing; many small calls can be slower than JS (wasm-bindgen #2355). F has no boundary. Measured on examples/payment ([bench/payment](../bench/payment/README.md), Node 24):

  | Measure | F | WASM |
  | --- | --- | --- |
  | Per `step` | 15 ns | 357 ns with the state kept in WASM; 1.2–1.4 µs with plain objects crossing |
  | To the first result | 0.5 ms | 1.6–2.3 ms |
  | Size, gzipped | 2.3 KB | 36 KB for the smallest WASM module with its glue |

  Measured with 0.4.1 (the runtime cut to what the package uses). The first measurement, before 0.4.0, had F at 12 ns, 0.5 ms, and 1.8 KB ([bench/payment](../bench/payment/README.md)).

- **Values should be plain TS.** Crux's web shell serializes with bincode; F's values go straight into React state.
- **The output should be readable.** Generated TS can be reviewed and stepped through.
- *WASM is unavailable* — no longer a main argument: React Native 0.84's Hermes V1 (2026-02) supports WASM, though wasm-bindgen output is unconfirmed there.

### 2.2 Where F is weaker

- **Expressiveness.** Only the subset. The author pays in notation ([02](./02-authoring.md)).
- **Burden of correctness.** Under B, rustc guarantees semantics. Under F, every semantic hole is ours. This is why equivalence is defined narrowly and tested per operator ([01](./01-equivalence.md)); the early `7 / 2 = 3.5` bug showed what happens otherwise.
- **Maintaining a small compiler.** Type inference, strings, and iteration each raise the maintenance load.

### 2.3 Design decisions judged sound after v0

- a target-independent IR;
- rejection over partial generation (mandatory when equivalence is the product);
- `kind` unions and companions;
- `i64` as `bigint`;
- goldens, differential tests, and drift detection from day one;
- building type inference and numeric semantics before widening syntax.

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
| Workflows, state machines | medium | **high** (order, payment) | payment at 1.6× idiomatic lines |
| Optimistic UI, offline-first | medium–high | high | sequences rebuilt per transition (`Vec` or recursive enums) |
| Turn-based game rules | medium | medium | loops as recursion; recursion-depth limit |
| **Input validation** | **high** | **medium** (signup, iban) | bytes via `as_bytes`, classes via `matches!`; no regex; ≈1.4–1.8× lines |
| Pricing, fees, tax | medium | high (invoice) | integer newtypes, no decimal; rounding written as integer division; ≈1.4× lines |
| Edge functions | low–medium | high | none (WASM works too) |

**Demand and capability are misaligned**: validation has the most signals and is the hardest to write; state machines are the easiest and have only indirect signals. This is the main strategic risk. Closed types, `as_bytes`, and range `for` were added to narrow the gap.

Expected users: small to mid-sized teams with a Rust backend and a TS frontend (web or React Native), already sharing types, now fighting duplicated logic, avoiding WASM builds. Tauri apps need it less (IPC to Rust), except for synchronous frontend-only checks.

## 4. Success and withdrawal criteria

<!-- constrained-by ./02-authoring.md#1-what-constraint-costs -->

### 4.1 Internal criteria

| Metric | Criterion |
| --- | --- |
| Equivalence | Differential tests (boundary values, exhaustive sequences where feasible) agree on whole values for every example |
| Types | 0 errors under `tsc --strict`, TS 6 and 7 |
| Determinism | same input, same bytes |
| Rejection quality | `path:line:col` + reason, no partial output |
| Capability | transitions keep ADTs, exhaustiveness, `Result`, debug integer semantics |
| Idiomaticity | no serialization, initialization, or async loading for TS users |

Types holds for every example and fixture. Generated function bodies type-check on 60 of seeds 1 to 120: 47 of the others stop only where TS narrows across joins and loops and the printer does not, 13 also at a `??` on a known `null`, unreachable code, or a temporary TS types from itself in a loop ([07 §3](./07-roadmap.md#3-candidates), [§8.15](./07-roadmap.md#815-unreleased-generated-function-bodies)).

### 4.2 External criteria

The author's own examples cannot validate the constraints: the author writes around them. External criteria were set 2026-09-28; the numbers are to be revisited.

1. **Write from third-party specifications**, reading only [02](./02-authoring.md) and [07](./07-roadmap.md). Record rewrites forced by rejection, and rejections the documents could not explain.
2. **Compare with idiomatic code**: the same spec in unconstrained Rust and in kamae-style TS. Compare line counts, forced style changes, and readability.
3. **Withdraw or switch (to B or C) if any holds:**
   - more than half of third-party specs cannot be written;
   - constrained Rust exceeds **2×** the lines of idiomatic Rust;
   - silent wrong values on accepted input keep appearing (guide: one or more per new example);
   - no real use replacing a dual implementation is ever obtained.

### 4.3 Status

| Withdrawal condition | Status |
| --- | --- |
| More than half of third-party specs cannot be written | Not met. signup, iban, payment, invoice, oidc, and semver are from third-party specs; all six could be written |
| Constrained Rust exceeds 2× | Not met. semver, the last example over 2×, is 1.8× by the script after `split_once`, `collect`, and `Some((a, b))` (0.7.0), 1.3× leaving out the closed type's accessors and counting `impl Ord` as logic. Its first draft was 2.8× ([07 §2.2](./07-roadmap.md#22-line-counts-against-idiomatic-rust)) |
| Silent wrong values keep appearing | Not met in the examples: none appeared in any of them. The generator is not as clean as that row once read: audits found output that disagreed with Rust on accepted input in 0.8.1 (a `?` in a `for` range's end ran before its start), 0.9.0 (a `?` inside `ok_or(..)?` left only an inline function), and 0.9.1 (`r.ok().is_some()` gave 0 for `Ok(0)`; a struct pattern's guard read a name its closure hid), each in a shape no fixture spelled. Since then `generated_equivalence.rs` builds such shapes from seeds: no value disagreed on any seed whose output reached node, and a `?` in `ok_or`'s argument, printed in an inline function, was found and fixed before it could (tsc refused its type). With the runtime's `Result` defaults and the narrowing folds of 0.10.0, seeds 1 to 120 reach node and agree (7,200 functions, 86,400 calls); one fold, wrong where an arm reassigned the place, was caught by an existing fixture before release. Generated function bodies (statements, loops, jumps, `?` in a range's end or a loop's test; `generated_statements_equivalence.rs`) then found one more: `match o.ok_or(x.ok_or(e)?)` returned from an inline function only, and took the `Err` arm. Fixed; seeds 1 to 120 of bodies (4,800 functions, 57,600 calls) and of expressions agree on every value. The generator's domain is still `i32`, `bool`, `Option<i32>`, and `Result<i32, i32>`: no crate types, patterns on them, strings, or `Vec` yet |
| No real use | Open. No real-world replacement yet (§5.2) |

Line counts against idiomatic Rust, logic only, both sides formatted with rustfmt at width 120 (`scripts/line-counts.py`); the history column was taken as written. Per-capability detail is in [07 §2.2](./07-roadmap.md#22-line-counts-against-idiomatic-rust).

| Example | Ratio | History |
| --- | --- | --- |
| signup | 1.5× (email), 1.4× (password) | 2.8× at first; under 2× after range `for`; measured by hand, no idiomatic module in its test |
| iban | 1.8× | 2.4× at first; 1.9× after range `for` |
| payment | 1.6× | 2.1× at first, 1.8× after `_` and `A \| B` arms; one tuple `match` in 0.4.0 |
| invoice | 1.6× | 2.2× for the first draft, 1.4× restructured (as written) |
| oidc | 1.3× | 1.65× written from the authoring skill alone; 0.4.0 rewrite |
| semver | 1.8× (1.3× counting `impl Ord` as logic and leaving out the closed type's accessors) | 2.8× written from the authoring skill alone, 2.6× restructured; 2.2× with ordering (0.5.0); 1.8× with `split_once`, `collect`, and `Some((a, b))` (0.7.0) |

## 5. Validating demand next

### 5.1 More third-party specs

Done: Stripe's PaymentIntent lifecycle (payment), then invoice (NTA) and oidc (OIDC Core, PKCE, TOTP); results in [07 §2](./07-roadmap.md#2-evidence-from-examples).

### 5.2 One real use

- **Goal.** Replace a dual implementation; record lines removed and what the tests guarantee.
- **Status.** Still open, and the only withdrawal criterion not yet answered.
- **Target** ([91](./91-real-use-candidates.md)): Oxide's resource `Name` rule (omicron's Rust, console's hand-written TS, which already disagree), then Stoat's permission calculator.
- **Done locally**, on local branches ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)): the console runs omicron's rule, generated, and agrees with it on 106,000 names where the hand-written copy disagreed on 39,852. Not yet proposed upstream, so the criterion stays open until it is used there.
- **No outreach.** Decided with the 0.1.0 release: no outreach and no requests to adopt ([07 §8](./07-roadmap.md#8-releases)). The local branches stay as evidence, and the criterion is answered only by an adoption that happens without being asked for.
- **Packaging.** Generated packages carry their runtime, so nothing but the schema library is installed beside them ([03](./03-output.md)).
- **Finding.** Dual implementations with drift turned out common in public Rust + TS projects, and mostly validation rules.

### 5.3 Numbers for §2

Done on Node for payment ([bench/payment](../bench/payment/README.md)); see §2.1. Browsers and Hermes are not measured.

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
