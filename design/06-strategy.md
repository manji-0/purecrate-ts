# Strategy: means, demand, and criteria

Status: analysis, revised 2026-10-16 (0.23.0)

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
| Workflows, state machines | medium | **high** (order, payment, raft, ssh) | payment at 2.3× idiomatic lines, for its checked constructor |
| Optimistic UI, offline-first | medium–high | high | no `&mut`: a transition returns the new state, and a nested value is rebuilt from the root |
| Turn-based game rules | medium | medium | recursion depth is JS's stack |
| **Input validation** | **high** | **medium** (signup, iban, semver) | bytes via `as_bytes`, classes via `matches!`; no regex; ≈1.4–1.9× lines |
| Pricing, fees, tax | medium | high (invoice) | integer newtypes, no decimal; rounding written as integer division; ≈1.8× lines |
| Edge functions | low–medium | high | none (WASM works too) |

**Demand and capability are misaligned**: validation has the most signals and is the hardest to write; state machines are the easiest and have only indirect signals. This is the main strategic risk. Closed types, `as_bytes`, and range `for` were added to narrow the gap, then the string methods, `format!`, and keyed collections (0.13.1 to 0.23.0).

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

Types holds for every example and fixture. The seeded generators hold it too: every generated function of seeds 1 to 120 of the five generators of 0.10.3 type-checks ([CHANGELOG 0.10.3](../CHANGELOG.md#rationale-the-rest-of-what-tsc-refused); 60 of the bodies did before narrowing followed TS's control flow, [0.10.2](../CHANGELOG.md#rationale-narrowing-as-ts-does-it)); with 0.19.0 every generator's did but statements seed 119 ([CHANGELOG 0.19.0](../CHANGELOG.md#0190--2026-10-14)), which 0.23.0 fixed ([CHANGELOG 0.23.0](../CHANGELOG.md#0230--2026-10-15)). There are eight generators now (`scripts/gen-sweep.sh`).

Not a criterion: [07 §2.5](./07-roadmap.md#25-distance-to-the-ideal) estimates the distance to the ideal subset, 94% with 0.23.0 (93% counting the shapes no document had decided, 88% counting what [07 §6](./07-roadmap.md#6-not-doing) leaves out on purpose). It orders the work; it neither proves nor withdraws the approach.

### 4.2 External criteria

The author's own examples cannot validate the constraints: the author writes around them. External criteria were set 2026-09-28; the numbers are to be revisited. On 2026-10-13 the 2× line count moved from the withdrawal conditions to an indicator of what to improve first (item 4).

1. **Write from third-party specifications**, reading only [02](./02-authoring.md) and [07](./07-roadmap.md). Record rewrites forced by rejection, and rejections the documents could not explain.
2. **Compare with idiomatic code**: the same spec in unconstrained Rust and in kamae-style TS. Compare line counts, forced style changes, and readability.
3. **Withdraw or switch (to B or C) if any holds:**
   - more than half of third-party specs cannot be written;
   - silent wrong values on accepted input keep appearing (guide: one or more per new example);
   - no real use replacing a dual implementation is ever obtained.
4. **Improve first where constrained Rust exceeds 2×** the lines of idiomatic Rust. This is not a withdrawal condition: a capability an example's excess names goes first among the candidates if it helps code in general, not one domain ([07 §1](./07-roadmap.md#1-how-additions-are-chosen), [07 §3](./07-roadmap.md#3-candidates)). An excess that is no missing capability, such as a reference that does less work, or that only a narrow addition would remove, is named and raises nothing.

### 4.3 Status

| Withdrawal condition | Status |
| --- | --- |
| More than half of third-party specs cannot be written | Not met. 16 of the 18 examples are from third-party specs (all but counter and order, [00 §4.1](./00-overview.md#41-examples)); all 16 could be written |
| Silent wrong values keep appearing | Not met in the examples: none appeared in any of them. The generator is not as clean as that row once read: audits found output that disagreed with Rust on accepted input in 0.8.1 (a `?` in a `for` range's end ran before its start), 0.9.0 (a `?` inside `ok_or(..)?` left only an inline function), and 0.9.1 (`r.ok().is_some()` gave 0 for `Ok(0)`; a struct pattern's guard read a name its closure hid), each in a shape no fixture spelled. Since then the generators build such shapes from seeds ([CHANGELOG 0.10.0 to 0.10.3](../CHANGELOG.md#0103--2026-10-04)); seeds 1 to 120 of the five generators of 0.10.3 agreed on every value (26,400 functions, 316,800 calls), and with 0.19.0 every generator's did; each release since runs the generators it extends over the same seeds. The hole hunt after 0.13.1 (four generators over its additions and a review of its diff) found wrong values with no error again, in shapes no example writes (`const M: i128 = !0;` folded as unsigned, `!x.is_infinite()` always `false`, a struct holding NaN equal to its `clone()`), among about 40 holes fixed in 0.13.2 ([CHANGELOG](../CHANGELOG.md#0132--2026-10-11)). The statements generator's new forms in 0.17.0 found three more released through 0.16.0 (a variant inside `Some` beside a last `_`, and `map_err(f)?` in two places where its `return` left only an inline function), fixed in 0.17.0 ([CHANGELOG](../CHANGELOG.md#0170--2026-10-14)) |
| No real use | Open. No real-world replacement yet; the Oxide `Name` proposal is the one exception to the no-outreach decision, with a deadline (§5.2) |

**Over 2× (item 4).** Two of the eleven examples the script measures are over it (2026-10-16, table below): jsonpatch 3.9× and payment 2.3×; iban is at 1.9×. What each excess names ([07 §2.2](./07-roadmap.md#22-line-counts-against-idiomatic-rust)):

- **jsonpatch.** Most of it is the parser and serializer, which the reference leaves to serde_json; the patch layer alone is 2.8×, and its excess is `edit_at` rebuilding the path from the root for want of `&mut` into a nested value. It raises nothing: `&mut` stays out ([07 §6](./07-roadmap.md#6-not-doing)), and an addition for JSON alone would help one domain.
- **payment.** No missing capability: the checked constructor an intent read from JSON goes through (`consistent`, the constructors, the accessors), which the reference, a `step` over plain values, does not do. It raises nothing.
- **semver**, over it until 0.16.0 (2.3×), named the decimal digits of a `u64` built by hand, as no number turned into text in the subset. That raised `format!` on integers in 07 §3, taken in 0.16.0 (2.0×); 0.18.0's `map_or`, `zip`, and `fold` took it to 1.7×.

Line counts against idiomatic Rust, logic only, both sides formatted with rustfmt at width 120, as `scripts/line-counts.py` printed them on 2026-10-16 (the same as on 2026-10-14, after 0.18.0). signup is counted by hand (its test has no idiomatic module); counter and order are the author's own, and ssh and raft are checked rule by rule instead of against a second implementation, so none of the four is measured. How each ratio got here, release by release, is in [07 §2.2](./07-roadmap.md#22-line-counts-against-idiomatic-rust).

| Example | Idiomatic | Constrained | Ratio | As first written |
| --- | --- | --- | --- | --- |
| signup | 28, 17 | 42, 24 | 1.5× (email), 1.4× (password) | 2.8× (email) |
| iban | 24 | 46 | 1.9× | 2.4× |
| payment | 52 | 122 | 2.3× | 2.1× |
| invoice | 54 | 96 | 1.8× | 2.2× |
| oidc | 390 | 568 | 1.5× | 1.65× |
| semver | 72 | 124 | 1.7× | 2.8× |
| punycode | 152 | 250 | 1.6× | 2.0× |
| calendar | 439 | 729 | 1.7× | 1.8× |
| jsonpatch | 159 | 621 | 3.9× (patch layer 2.8×) | 789 lines |
| geo | 135 | 222 | 1.6× | 2.2× |
| ulid | 59 | 79 | 1.3× | 3.4× |
| negotiate | 188 | 282 | 1.5× | 2.6× |

## 5. Validating demand next

### 5.1 More third-party specs

Done: fourteen examples from third-party specifications, from Stripe's PaymentIntent lifecycle (payment), invoice (NTA), and oidc (OIDC Core, PKCE, TOTP) to raft ([00 §4.1](./00-overview.md#41-examples)); results in [07 §2](./07-roadmap.md#2-evidence-from-examples).

### 5.2 One real use

- **Goal.** Replace a dual implementation; record lines removed and what the tests guarantee.
- **Status.** Still open, and the only withdrawal criterion not yet answered.
- **Target** ([91](./91-real-use-candidates.md)): Oxide's resource `Name` rule (omicron's Rust, console's hand-written TS, which already disagree), then Stoat's permission calculator.
- **Done locally**, on local branches ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)): the console runs omicron's rule, generated, and agrees with it on 106,000 names where the hand-written copy disagreed on 39,852. Not yet proposed upstream, so the criterion stays open until it is used there.
- **No outreach, with one exception.** Decided with the 0.1.0 release: no outreach and no requests to adopt ([07 §8](./07-roadmap.md#8-releases)). Waiting for an adoption nobody asked for gave the criterion no way to close, so two things were added on 2026-10-13:
  - **Exception.** Proposing the Oxide `Name` change to omicron and console, from the local branches, is allowed: one proposal per repository, made by the author. It answers the criterion if either repository merges it. Nothing else changes: no other project is approached, and Stoat stays a candidate only.
  - **Deadline.** If by 2027-04-13 there is still no real use (the Oxide proposal merged, or an adoption that happens without being asked for), the condition counts as holding, and §4.2's withdraw-or-switch decision is taken then.
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
