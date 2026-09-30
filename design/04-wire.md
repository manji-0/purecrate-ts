# Wire boundary

Status: current (2026-09-29)

<!-- constrained-by ./01-equivalence.md -->

## 1. Problem

Shared behavior is useless if the values it consumes still arrive through hand-written conversion: the drift this tool removes from logic would reappear at the edge. On the client, state and events usually come from the Rust server as serde JSON, and that JSON is not the generated value.

- **Shape.** Generated enums use an internal `kind` tag; serde's default is external tagging.
- **Numbers.** JSON numbers are unbranded; fields are `I32`, `F64`, `I64` (`bigint`). `JSON.parse` rounds integers above 2^53.
- **Attributes.** `#[serde(rename_all = "camelCase")]` would silently change field names.

| Rust | Generated value | serde_json default |
| --- | --- | --- |
| unit `Inc` | `{ kind: "Inc" }` | `"Inc"` |
| `Add(i32)` | `{ kind: "Add", content: [1] }` | `{"Add":1}` |
| `P(i32, i32)` | `{ kind: "P", content: [1, 2] }` | `{"P":[1,2]}` |
| `Move { dx }` | `{ kind: "Move", dx: 1 }` | `{"Move":{"dx":1}}` |
| `i64` | `bigint` | number |
| `Option<T>` | `T \| null` | `null` or value |
| `Result<T, E>` | `{ kind: "Ok", value }` | `{"Ok":v}` |
| `()` | `undefined` | `null` |

## 2. Decision: boundary schemas, not type declarations

Decided 2026-09-27. Of the options considered:

- **Leave it out of scope** — rejected: hand-written conversion undermines the central promise exactly where data enters.
- **Compete on type-only generation (ts-rs, specta)** — rejected: most DTO crates rely on generics, external types, and serde attributes, and chasing them pulls away from verified behavior.
- **Force `#[serde(tag = "kind")]`** — not required: serde rejects it for tuple variants and newtypes over primitives. It may become a recommended shape later.
- **Generate readers from the same source** — adopted. One Rust source yields types, behavior, and a validated reader, all differentially testable.

## 3. Current design

- The core runtime `purecrate` (`packages/boundary`) holds the numeric brands, `Int.*.of`, `Str`, and `parseJson`. It depends on no schema library.
- Thin adapters live in separate packages (`purecrate-zod`, `-valibot`, `-arktype`). Only the one passed with `--schema` is used. They target zod 4.6 (since 2026-09-30; zod 3 before), valibot 1.1, and arktype 2.1, each a peer dependency of the generated package.
- `--schema <lib>` emits `src/purecrate-wire.ts`: a schema for every public struct and enum, reading serde's default JSON (no attributes) into the branded domain type, and `toJson`, writing it back (§6).
- The input's types may derive `Serialize`/`Deserialize` (and `use serde::…`), so the server reads and writes the same types. `check` compiles the crate against a stand-in `serde` whose derives expand to nothing (`crates/cli/src/rustc.rs`): the translated code is unaffected, and the real derive is checked by the server's build. Before this (until 2026-09-29) a crate that derived serde failed `check` with `cannot find crate serde`.
- `#[serde(...)]` is rejected everywhere with its location, so a renamed wire format is never silently accepted. The one exception is `#[serde(try_from = "T")]` (§5).

Reading rules (serde's default behavior):

1. Input is an already-parsed value. `i64` / `u64` accept safe-integer numbers, `bigint`, and digit strings; out-of-range values are schema failures, not throws. A rounded `JSON.parse` value always lies outside the safe range and is rejected, so a wrong value is never read. To read large integers, parse the text with `parseJson`, which uses the `JSON.parse` reviver's source text (Node 21+) to turn only out-of-range integer literals into `bigint`. `f64` is unaffected because serde_json always writes `.` or an exponent.
2. A missing `Option` field and `null` are both `None`.
3. A `char` is a string of exactly one Unicode scalar value: `""`, `"ab"`, `"e\u0301"`, and a lone surrogate are rejected, as serde_json rejects them. A `Uuid` is read from any string `Uuid::parse_str` accepts and becomes the canonical form; anything else, and a JSON array of bytes (which serde_json never passes to `Uuid`), is rejected. `toJson` writes the canonical form, as serde does. `uuid::Error` has no JSON form in Rust; its schema rejects every value.
4. The object wrapping a variant has exactly one key; extra keys are rejected (`{"Circle":1.5,"Rect":[1,2]}` used to read as `Circle`; fixed 2026-09-28). Unknown fields inside structs are ignored. A unit variant accepts `"Dot"` and `{"Dot":null}`.

Library notes: zod and valibot build structs field by field, because inference makes `undefined`-valued (`()`) fields optional. Their schemas are printed dependencies first, and only a type in a cycle of references (a recursive type, or two that refer to each other) is behind `lazy`; the adapters' `unitVariant` and zod's `optionalField` spell rules 4 and 2 once (2026-09-30, before which every schema was `lazy` and printed on one line). arktype rejects a union of objects containing morphs, so enums try variants in turn; each schema is a morph from `unknown` typed `Wire<T>`, built lazily on first read so recursive and later-declared types resolve (an earlier `type.module` design hit a `ReferenceError` at import). A nested read that fails hands its errors to the morph's traversal (`fail`, through `ArkErrors.merge`), so they keep their path; before 2026-09-30 the morph replaced them with one error at its own root. When no variant matches, an object keyed by a variant's name reports that variant's errors (`Confirm.outcome`), anything else one error for the enum.

Verification: `fixtures/wire_shapes.rs` covers every type form; for all three libraries the schemas pass `tsc --strict` on TS 6 and 7 and are run on node against serde-default JSON and malformed inputs (`crates/cli/tests/it/wire.rs`).

## 4. Out of scope

Parsing JSON inside transitions; type declarations alone as a ts-rs replacement; existing APIs using `rename_all`; semantic checks transcribed into `.refine` (meaning comes only from Rust functions, §5).

## 5. Closed types on the wire

<!-- constrained-by ./01-equivalence.md#4-closed-types -->

serde's `#[derive(Deserialize)]` builds closed types by shape without calling the smart constructor. If the TS schema called the checked constructor, TS would reject JSON that Rust accepts.

1. **By shape.** Without an attribute, closed types are read by shape and branded through `Email$of`. Same set as Rust; invariants are not upheld on the wire, as in Rust.
2. **Through the constructor** (2026-09-29, examples/payment: `Amount` and `PaymentMethodId`). `#[serde(try_from = "T")]` on a struct, with `impl TryFrom<T> for X` (translated as `X.try_from`; `check` requires it, since rustc sees only the stand-in serde). The schema reads `T` as serde would, then calls `X.try_from`; `Err` fails the read, as serde fails deserialization. Serializing is unchanged (by shape), so `toJson` writes what serde writes. `impl Display` for the error, which serde requires, is skipped by the translator.

A refused value fails with an issue naming the type and, when the error type is an enum, the variant `try_from` returned (`Amount: AmountOutOfRange`); zod's issue also carries the error value in `params.error`.

Verified in `wire_write.rs` for all three libraries: every schema reads a `PaymentMethodId`, an `Amount`, or an event carrying one exactly when `X::try_from` accepts the value, and gives the same value. The real serde on the same source (the `cargo test` in `bench/payment/wasm`) rejects the same inputs, with the `Display` text in its error.

One difference remains, from §3 rule 1: `i64`/`u64` also read digit strings (`"50"`), which serde_json rejects. With `try_from` the value still passes through the constructor.

## 6. Writing domain values

<!-- derived-from ./07-roadmap.md#2-evidence-from-examples -->

Implemented 2026-09-29, when examples/payment needed it: in an optimistic update the client reads the server's state, runs `step`, and sends the event back.

`toJson.T(x)` returns the JSON text serde_json writes for the Rust value of `x`, **byte for byte**. It needs no schema library; its helpers are the runtime's `Json`.

| Value | Written as |
| --- | --- |
| integers, `i64`/`u64` included | the exact decimal (`bigint` is not rounded) |
| `f64` / `f32` | ryu's layout: `1.0`, `-0.0`, `0.001`, `1e16`, `1.5e-7`; `f32` with its own shortest digits, ties to even |
| non-finite floats | `null`, as serde_json writes them; neither side reads `null` back as a float |
| `String` | `JSON.stringify`, which escapes as serde_json does for well-formed strings |
| structs, enums, `Option`, `Vec`, tuples, `()`, `Result` | serde's default shapes (§1); struct fields in declaration order |

Unit structs are rejected ([02 §3.7](./02-authoring.md#37-types)): serde writes `struct S;` as `null` and `struct S {}` as `{}`, and the IR keeps only the fields.

Verification (`crates/cli/tests/it/wire_write.rs`) against the vendored serde_json, over `Serialize` impls that `purecrate_canon::fixture!` generates with the data-model calls `#[derive(Serialize)]` makes (the derive itself needs a newer `syn` than the vendored one):

- floats: 80,000 pseudo-random bit patterns and boundary values (2,000,000 once, by hand), text equal;
- shapes: every type form in `wire_shapes.rs`, written by serde_json, read by the schema, and written back, text equal;
- payment: every state reachable in three events under each capture and confirmation method, times every event: the client's `toJson(step(read(state), read(event)))` equals the server's `serde_json::to_string(&step(state, event))`.

A string with a lone surrogate is not a Rust `String` (01 §2); `JSON.stringify` escapes it and serde_json then rejects the text.

## 7. Open questions

- **Hermes.** Whether React Native's engine supports `JSON.parse` source text access is unconfirmed; if not, `parseJson` needs its own parser.
