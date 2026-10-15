# Overview

Status: current (2026-10-16, 0.23.0). Replaces the old `00-foundations.md` as the entry point.

## 1. Claim

> Pure domain logic written in Rust (ADTs and transitions) can be shared with TypeScript **as behavior, not just as types**: translated into plain TS values and functions, without WASM or a serialization boundary, and **mechanically verified to return the same results as a Rust debug build** for every accepted input.

Three commitments follow from it and shape every other decision.

1. **Rust is the source of truth.** The TS package is derived output. It is never edited; drift is detected by byte comparison (`check --out`).
2. **Equivalence is the product.** The moment the output silently disagrees with Rust, this tool is worse than a hand-written dual implementation, because nobody notices. So anything whose meaning cannot be preserved is **rejected before generation**, never partially translated.
3. **The output is idiomatic TS.** Values are `Readonly` objects, `kind` unions, and branded numbers that can live directly in React state or pass through `structuredClone`. No classes, no runtime initialization, no async loading.

## 2. Focus

**Target: new domain code written within the PureCrate constraints.** It is not a compiler for existing Rust. Measurements showed existing crates are almost never accepted as-is (1 of 956 public functions when first measured; 188 with 0.15.0, 183 of them one crate's short validators, and 5 of 126 outside it; [90-acceptance-survey](./90-acceptance-survey.md#8-re-measured-with-0150-2026-10-13)), and chasing that number would mean maintaining the semantics of most of `std`. Instead, the author pays a bounded cost in notation, and in return gets verified behavior sharing.

The canonical shape is a transition:

```rust
pub fn step(state: State, event: Event) -> Result<State, Error>
```

The use cases in focus, and how well they fit today ([06-strategy §3](./06-strategy.md#3-demand)):

| Use case | Fit |
| --- | --- |
| Workflows and state machines (order lifecycle, approvals, payment status) | High. examples/order, examples/payment, examples/oidc, examples/ssh, examples/raft |
| Optimistic UI / offline-first (same transition on client and server) | High |
| Pricing, fees, tax (integer newtypes in the smallest unit) | High. examples/invoice |
| Input validation (checked constructors) | Medium. examples/signup, examples/iban, examples/semver |
| Turn-based game rules | Medium (recursion depth) |

Non-goals:

- compiling arbitrary Rust;
- I/O, async, threads, `unsafe`, trait objects;
- binding to JS ecosystem types;
- emitting type declarations alone as a ts-rs competitor;
- a WASM backend (the IR leaves room for one, but it is not planned).

## 3. How it holds together

```
Rust crate ──parse (syn)──▶ IR ──▶ subset check, typing ──▶ rustc (pass/fail) ──▶ TS printer ──▶ npm package
                                       │                                                          │
                                       └── reject with path:line:col + reason, write nothing      └── optional wire schemas
                                                                                                      (zod / valibot / arktype)
```

- **Constraints on the author** ([02](./02-authoring.md)) keep the input inside the region where meaning can be preserved.
- **The definition of equivalence** ([01](./01-equivalence.md)) says exactly what "same result" means and where it stops.
- **The output contract** ([03](./03-output.md)) says what TS callers get and must observe.
- **The wire boundary** ([04](./04-wire.md)) gets server JSON into domain values without hand-written conversion.
- **Differential tests** run the same inputs through Rust and the generated TS and compare whole values; they are the evidence behind every "matches" in these documents.

## 4. What works today

In brief; [02 §2](./02-authoring.md#2-what-can-be-written) lists each form with the TS it becomes, and [01](./01-equivalence.md) where each stops.

| Area | What works |
| --- | --- |
| Types | structs, enums (`kind` unions), newtypes (brands), `Option`, `Result`, tuples, `Vec`, fixed-size arrays `[T; N]`, `BTreeMap` / `BTreeSet` keyed by `String`, integers, `char`, `bool`, `Uuid`, or a newtype of one; recursive enums (`Box` erased); type parameters on types, `impl` blocks, and functions |
| Closed types | structs with private fields keep their invariants (no public `of`) |
| Traits | user traits with default bodies and consts, `impl Tr for X` on the crate's types, bounds as a dictionary parameter; `PartialEq`, `Ord`, and `Default` derived or written by hand; `Display` and `From` (so `?` converts an error) written by hand |
| Patterns and `match` | `?`, exhaustive `match` (with `A \| B`, a last `_`, guards, struct patterns, tuples, slice patterns), `if let`, `let`-`else`, `matches!`; byte, integer, `char`, `bool`, range, and string literal patterns; tuple patterns in `let`, closure parameters, and `for` |
| Bindings | local `let mut`, local and associated `const`, local closures over immutable bindings, struct update `S { a, ..base }`, paths through modules |
| Loops | `for` over integer ranges (`a..b`, `a..=b`, `rev`, `step_by`), a `Vec`, slice, array, map, or set, and a string's `chars`, `bytes`, `split`, `lines`; `while`, `while let`, `loop` with a value; `break` and `continue` |
| Sequences | lazy `map`, `filter`, `filter_map`, `flat_map`, `flatten`, `take`, `skip`, `take_while`, `skip_while`, `rev`, `zip`, `enumerate`, `windows`, `chunks`; consumers `all`, `any`, `find`, `position`, `count`, `sum`, `fold`, `max` / `min` and their `_by` / `_by_key` forms, `collect`, `partition` |
| `Vec` | built by `vec![..]`, `collect`, `clone`, or on a `let mut` local by `push`, `insert`, `remove`, `v[i] = x`, and stable `sort*`; read by index, slices, `len`, `get`, `first`, `last`, `contains`, `==`, `cmp` |
| Numbers | integer arithmetic with debug-build semantics (overflow and division by zero panic); `i64` to `u128` as `bigint`; bitwise operators and shifts; `T::from` widening and `as` narrowing; `min` / `max` / `abs` / `pow` and the checked, saturating, and wrapping forms; `f32` / `f64` with their rounding methods |
| `Option`, `Result` | the std combinators (`map`, `and_then`, `unwrap_or_else`, `ok_or`, `filter`, `map_or`, `is_some_and`, ..), `Option::unwrap` / `expect` |
| Strings | `String::from`, `==` and `<`, `len`, search (`starts_with`, `contains`, `find`, `strip_prefix`, `split_once`, ..), `trim*`, case (ASCII and Unicode), slices at byte positions, `split` on a `char`, a `&str`, or a closure, `split_whitespace`, `lines`, `join`, `repeat`, `parse` into an integer; built with `push` / `push_str`, `collect`, `to_string`, or `format!` with widths and padding over integers, `bool`, `char`, strings, `Uuid`, and the crate's types with `Display` |
| `char`, `Uuid` | `char` a branded string ordered by code point, with the ASCII methods; `uuid::Uuid` a branded canonical string, `parse_str` / `try_parse` exactly as the `uuid` crate |
| Panics | `panic!`, `unreachable!`, `assert!` with Rust's messages |
| serde | the types may derive `Serialize`/`Deserialize` for the server; `rename`, `rename_all`, and internal or adjacent `tag` shape the JSON as serde does; `#[serde(try_from = "T")]` keeps a closed type's invariant on the wire in Rust and TS alike |
| Wire schemas | `--schema zod\|valibot\|arktype` reads serde's JSON into domain values, and `toJson` writes them back byte for byte as serde_json does ([04](./04-wire.md)) |
| Checking a crate | `purecrate-ts test` calls every public function in Rust and in the generated TS on the same drawn arguments and names each call that differs ([01 §8.4](./01-equivalence.md#84-a-users-crate-purecrate-ts-test)) |

Not accepted, by design: `&mut`, `HashMap` (write `BTreeMap`), `dyn Trait`, an enum, struct, or tuple as a map key, and the effects of §2's non-goals ([07 §6](./07-roadmap.md#6-not-doing)).

### 4.1 Examples

Sixteen examples are written within the constraints and differentially tested: two are the author's own, fourteen follow a third-party specification.

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
| [jsonpatch](../examples/jsonpatch/src/lib.rs) | third-party specification |
| [geo](../examples/geo/src/lib.rs) | third-party specification |
| [ulid](../examples/ulid/src/lib.rs) | third-party specification |
| [negotiate](../examples/negotiate/src/lib.rs) | third-party specification |
| [raft](../examples/raft/src/lib.rs) | third-party specification |

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
| [06-strategy](./06-strategy.md) | Analysis: why this means over alternatives, demand, success and withdrawal criteria and where they stand |
| [07-roadmap](./07-roadmap.md) | How additions are chosen, evidence from examples, what comes next, open questions |
| [90-acceptance-survey](./90-acceptance-survey.md) | Archive: measurement of existing crates. Not a metric |
| [91-real-use-candidates](./91-real-use-candidates.md) | Record: dual Rust/TS implementations found in public projects, their fit to the subset, and the first real-use target |

Reading order: 00 → 02 → 03 for users; 00 → 01 → 06 → 07 for evaluating the approach.
