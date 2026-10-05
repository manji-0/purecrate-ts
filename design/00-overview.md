# Overview

Status: current (2026-10-05, release 0.12.0). Replaces the old `00-foundations.md` as the entry point.

## 1. Claim

> Pure domain logic written in Rust (ADTs and transitions) can be shared with TypeScript **as behavior, not just as types**: translated into plain TS values and functions, without WASM or a serialization boundary, and **mechanically verified to return the same results as a Rust debug build** for every accepted input.

Three commitments follow from it and shape every other decision.

1. **Rust is the source of truth.** The TS package is derived output. It is never edited; drift is detected by byte comparison (`check --out`).
2. **Equivalence is the product.** The moment the output silently disagrees with Rust, this tool is worse than a hand-written dual implementation, because nobody notices. So anything whose meaning cannot be preserved is **rejected before generation**, never partially translated.
3. **The output is idiomatic TS.** Values are `Readonly` objects, `kind` unions, and branded numbers that can live directly in React state or pass through `structuredClone`. No classes, no runtime initialization, no async loading.

## 2. Focus

**Target: new domain code written within the PureCrate constraints.** It is not a compiler for existing Rust. Measurements showed existing crates are almost never accepted as-is (1 of 956 public functions; [90-acceptance-survey](./90-acceptance-survey.md)), and chasing that number would mean maintaining the semantics of most of `std`. Instead, the author pays a bounded cost in notation, and in return gets verified behavior sharing.

The canonical shape is a transition:

```rust
pub fn step(state: State, event: Event) -> Result<State, Error>
```

The use cases in focus, and how well they fit today ([06-strategy §3](./06-strategy.md#3-demand)):

| Use case | Fit |
| --- | --- |
| Workflows and state machines (order lifecycle, approvals, payment status) | High. examples/order, examples/payment, examples/oidc, examples/ssh |
| Optimistic UI / offline-first (same transition on client and server) | High |
| Pricing, fees, tax (integer newtypes in the smallest unit) | High. examples/invoice |
| Input validation (checked constructors) | Medium. examples/signup, examples/iban |
| Turn-based game rules | Medium (recursion depth) |

Non-goals:

- compiling arbitrary Rust;
- I/O, async, threads, `unsafe`, trait objects;
- binding to JS ecosystem types;
- emitting type declarations alone as a ts-rs competitor;
- a WASM backend (the IR leaves room for one, but it is not planned).

## 3. How it holds together

```
Rust crate ──parse (syn)──▶ subset check ──▶ rustc (pass/fail) ──▶ IR ──▶ TS printer ──▶ npm package
                              │                                                           │
                              └── reject with path:line:col + reason, write nothing       └── optional wire schemas
                                                                                              (zod / valibot / arktype)
```

- **Constraints on the author** ([02](./02-authoring.md)) keep the input inside the region where meaning can be preserved.
- **The definition of equivalence** ([01](./01-equivalence.md)) says exactly what "same result" means and where it stops.
- **The output contract** ([03](./03-output.md)) says what TS callers get and must observe.
- **The wire boundary** ([04](./04-wire.md)) gets server JSON into domain values without hand-written conversion.
- **Differential tests** run the same inputs through Rust and the generated TS and compare whole values; they are the evidence behind every "matches" in these documents.

## 4. What works today

| Area | What works |
| --- | --- |
| Types | structs, enums (`kind` unions), newtypes (brands), `Option`, `Result`; sequences as `Vec`s a function builds (a local grown by `push`, `collect`) or recursive enums (`Box` erased) |
| Closed types | structs with private fields keep their invariants (no public `of`) |
| Patterns and `match` | `?`, `if let`, exhaustive `match` (with `A \| B`, a last `_`, guards, struct patterns, and on tuples); byte literals, integer, `char`, `bool`, range and string literal patterns, `matches!`; tuple patterns in `let`, closure parameters, and `for` |
| Bindings | local `let mut`, local `const`, local closures over immutable bindings, struct update `S { a, ..base }` |
| Loops | integer-range `for i in a..b`; `for` over a `Vec` or slice, `s.chars()`, `s.bytes()`, and `s.split(c)`, with `.enumerate()`; `while` with `break` and `continue`; lazy `map` / `filter` and `all` / `any` / `position` / `count` / `sum` / `collect` on them |
| Numbers | integer arithmetic with debug-build semantics (truncation, overflow and division-by-zero throw); `i64` / `u64` as `bigint`; bitwise operators and shifts; lossless widening via `i64::from(x)`; `min` / `max` / `abs` / `pow` and the checked, saturating, and wrapping forms |
| Constants | crate-level `const` items folded at check time; enum discriminants read with `as` |
| `Option` | read with `is_some` / `is_none` / `unwrap_or` / `ok_or` / `map` / `as_ref` / `as_deref`; `clone` |
| `Vec` | read by index, `len`, slices, and `cmp`; built as `vec![a, b]`, by `collect`, by `clone`, or grown in a function by `push` on a `let mut` local |
| Strings | `String::from("…")`, string `==`, `len` / `is_empty` / `starts_with` / `ends_with` / `contains` / `strip_prefix` / `strip_suffix` / `split_once`, slices at byte positions, string contents via `s.as_bytes()` |
| `char` | a branded string: literals, ordering by code point, `u32::from` / `char::from` / `char::from_u32`, the ASCII methods |
| `uuid::Uuid` | a branded canonical string: `parse_str` / `try_parse` exactly as the `uuid` crate, `nil`, `==` and ordering, serde's JSON form |
| serde | serde-ready crates: the types may derive `Serialize`/`Deserialize` for the server; `#[serde(try_from = "T")]` keeps a closed type's invariant on the wire in Rust and TS alike |
| Wire schemas | `--schema zod\|valibot\|arktype` reads serde's default JSON into domain values, and `toJson` writes them back byte for byte as serde_json does |

### 4.1 Examples

Eight examples are written within the constraints and differentially tested:

| Example | Source |
| --- | --- |
| [counter](../examples/counter/src/lib.rs) | author's own |
| [order](../examples/order/src/lib.rs) | author's own |
| [signup](../examples/signup/src/lib.rs) | third-party specification |
| [iban](../examples/iban/src/lib.rs) | third-party specification |
| [payment](../examples/payment/src/lib.rs) | third-party specification |
| [invoice](../examples/invoice/src/lib.rs) | third-party specification |
| [oidc](../examples/oidc/src/lib.rs) | third-party specification |
| [semver](../examples/semver/src/lib.rs) | third-party specification |
| [ssh](../examples/ssh/src/lib.rs) | third-party specification |
| [calendar](../examples/calendar/src/lib.rs) | third-party specification |
| [punycode](../examples/punycode/src/lib.rs) | third-party specification |

Against wasm-bindgen on the same source, the payment transition is 24–93× cheaper per call and about 16× smaller gzipped (measured 2026-10-01) ([bench/payment](../bench/payment/README.md)).

## 5. Documents

| Document | Question it answers |
| --- | --- |
| [00-overview](./00-overview.md) | What is claimed, and what is in focus |
| [01-equivalence](./01-equivalence.md) | What "returns the same result" means, how it is verified, and where it stops |
| [02-authoring](./02-authoring.md) | What the Rust author can write, and how |
| [03-output](./03-output.md) | The shape of the generated TS, and what callers must observe |
| [04-wire](./04-wire.md) | Getting serde JSON into domain values |
| [05-architecture](./05-architecture.md) | Pipeline, crates, IR, type inference |
| [06-strategy](./06-strategy.md) | Why this means over alternatives, demand, success and withdrawal criteria |
| [07-roadmap](./07-roadmap.md) | How additions are chosen, evidence from examples, what comes next, open questions |
| [90-acceptance-survey](./90-acceptance-survey.md) | Archive: measurement of existing crates. Not a metric |
| [91-real-use-candidates](./91-real-use-candidates.md) | Record: dual Rust/TS implementations found in public projects, their fit to the subset, and the first real-use target |

Reading order: 00 → 02 → 03 for users; 00 → 01 → 06 → 07 for evaluating the approach.
