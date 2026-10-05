# Wire boundary

Status: current (2026-10-05, 0.12.0)

<!-- constrained-by ./01-equivalence.md -->

## 1. Problem

Shared behavior is useless if the values it consumes still arrive through hand-written conversion: the drift this tool removes from logic would reappear at the edge. On the client, state and events usually come from the Rust server as serde JSON, and that JSON is not the generated value.

- **Shape.** Generated enums use an internal `kind` tag; serde's default is external tagging.
- **Numbers.** JSON numbers are unbranded; fields are `I32`, `F64`, `I64` (`bigint`). `JSON.parse` rounds integers above 2^53.
- **Attributes.** `#[serde(rename_all = "camelCase")]` would silently change field names.

| Rust | Generated value | serde_json default |
| --- | --- | --- |
| unit `Inc` | `{ kind: "Inc" }` | `"Inc"` |
| `Add(i32)` | `{ kind: "Add", value: 1 }` | `{"Add":1}` |
| `P(i32, i32)` | `{ kind: "P", content: [1, 2] }` | `{"P":[1,2]}` |
| `Move { dx }` | `{ kind: "Move", dx: 1 }` | `{"Move":{"dx":1}}` |
| `i64` | `bigint` | number |
| `Option<T>` | `T \| null` | `null` or value |
| `Result<T, E>` | `{ kind: "Ok", value }` | `{"Ok":v}` |
| `()` | `undefined` | `null` |

## 2. Decision: boundary schemas, not type declarations

Of the options considered:

- **Leave it out of scope** — rejected: hand-written conversion undermines the central promise exactly where data enters.
- **Compete on type-only generation (ts-rs, specta)** — rejected: most DTO crates rely on generics, external types, and serde attributes, and chasing them pulls away from verified behavior.
- **Force `#[serde(tag = "kind")]`** — not required: serde rejects it for tuple variants and newtypes over primitives. It may become a recommended shape later.
- **Generate readers from the same source** — adopted. One Rust source yields types, behavior, and a validated reader, all differentially testable.

## 3. Current design

### 3.1 Components

| Part | Where | What |
| --- | --- | --- |
| Core runtime `purecrate` | `packages/boundary`, copied into each package as `src/purecrate-runtime.ts` (design/03 §4.4) | The numeric brands and `Int`, `Result`, `Str`, `Char`, `Uuid`, `parseJson`, and `Json` (`object`, `tuple`, `array`, and the scalar writers `toJson` calls). It depends on no schema library |
| Adapters `purecrate-zod`, `-valibot`, `-arktype` | `packages/boundary-<lib>`, the one passed with `--schema` copied in as `src/purecrate-<lib>.ts` | Thin adapters, one per schema library: the scalar schemas, `nullable`, `unitVariant`, `unitEnum`, and zod's `optionalField`; arktype's also `Wire`, `memo`, `keyed`, `fail` |
| Wire module | `src/purecrate-wire.ts`, emitted by `--schema <lib>` | A schema for each public struct and enum that derives `Deserialize`, reading serde's default JSON (no attributes) into the branded domain type, with `fromJson.T(text)` reading the text through `parseJson` and the schema, and `toJson` for each that derives `Serialize`, writing it back (§6); see §3.2 |

Each adapter's library is a peer dependency of the generated package:

| Adapter | Targets |
| --- | --- |
| `purecrate-zod` | zod 4.6 (zod 3 up to purecrate-ts 0.2) |
| `purecrate-valibot` | valibot 1.1 |
| `purecrate-arktype` | arktype 2.1 |

### 3.2 serde in the input

- **Derives pass.** The input's types may derive `Serialize`/`Deserialize` (and `use serde::…`), so the server reads and writes the same types.
- **Only the derives make a wire form.** A schema reads a type that derives `Deserialize`, and `toJson` writes one that derives `Serialize`; a type with neither has no wire form, as in Rust. Without this, a closed type with no derive (signup's `Email`) got a schema that built it by shape, a value Rust has no way to read from JSON. `--schema` on a crate where no public type derives either is refused.
- **What the derive needs.** A type that derives one holds only types that derive it too, and no std `Ordering`, which serde gives no form; `check` refuses the rest (`item/serde-derive`), as the real derive does. With `#[serde(try_from = "T")]`, `Deserialize` needs it of `T` instead of the fields.
- **How `check` handles them.** `check` compiles the crate against a stand-in `serde` whose derives expand to nothing (`crates/cli/src/rustc.rs`). The translated code is unaffected, and the real derive is checked by the server's build.
- **Attributes are rejected.** `#[serde(...)]` is rejected everywhere with its location, so a renamed wire format is never silently accepted. The one exception is `#[serde(try_from = "T")]` (§5).
- **Why a stand-in.** Without it, a crate that derives serde fails `check` with `cannot find crate serde`.

### 3.3 Reading rules

These follow serde's default behavior. Other sections refer to them by number.

1. **Integers and parsing.** Input is an already-parsed value. `i64` / `u64` accept safe-integer numbers, `bigint`, and digit strings; out-of-range values are schema failures, not throws. A rounded `JSON.parse` value always lies outside the safe range and is rejected, so a wrong value is never read. To read large integers, parse the text with `parseJson`, which uses the `JSON.parse` reviver's source text (Node 21+) to turn only out-of-range integer literals into `bigint`. `f64` is unaffected because serde_json always writes `.` or an exponent.
2. **`Option`.** A missing `Option` field and `null` are both `None`.
3. **`char` and `Uuid`.**
   - A `char` is a string of exactly one Unicode scalar value: `""`, `"ab"`, `"e\u0301"`, and a lone surrogate are rejected, as serde_json rejects them.
   - A `Uuid` is read from any string `Uuid::parse_str` accepts and becomes the canonical form; anything else, and a JSON array of bytes (which serde_json never passes to `Uuid`), is rejected. `toJson` writes the canonical form, as serde does.
   - `uuid::Error` has no JSON form in Rust; its schema rejects every value.
4. **Enum objects and struct fields.** The object wrapping a variant has exactly one key; extra keys are rejected. Unknown fields inside structs are ignored. A unit variant accepts `"Dot"` and `{"Dot":null}`. So `{"Circle":1.5,"Rect":[1,2]}` is rejected, not read as `Circle`.

### 3.4 Per-library notes

- **zod and valibot.** A struct is the object schema's own output, unknown keys dropped as serde drops them; it is built field by field only where that output is not the domain value: a closed struct (through its constructor), a `()` field (which inference makes optional), and with valibot an `Option` field (missing reads as `undefined`, made `null`). Schemas are printed dependencies first, and only a type in a cycle of references (a recursive type, or two that refer to each other) is behind `lazy`. The adapters' `unitVariant` and zod's `optionalField` spell §3.3 rules 4 and 2 once, and `unitEnum(["A", "B"])` reads an enum whose variants all are unit as one expression.
- **arktype.**
  - Enums try variants in turn, because arktype rejects a union of objects containing morphs; the adapter's `unitEnum("E", ["A", "B"])` does it for an enum whose variants all are unit.
  - Each schema is a morph from `unknown` typed `Wire<T>`, built lazily on first read so recursive and later-declared types resolve. (A `type.module` design hit a `ReferenceError` at import.)
  - A nested read that fails hands its errors to the morph's traversal (`fail`, through `ArkErrors.merge`), so they keep their path.
  - When no variant matches, an object keyed by a variant's name reports that variant's errors (`Confirm.outcome`), anything else one error for the enum.

### 3.5 Verification

`fixtures/wire_shapes.rs` covers every type form. For all three libraries the schemas pass `tsc --strict` on TS 6 and 7 and are run on node against serde-default JSON and malformed inputs (`crates/cli/tests/it/wire.rs`).

## 4. Out of scope

Parsing JSON inside transitions; type declarations alone as a ts-rs replacement; existing APIs using `rename_all`; semantic checks transcribed into `.refine` (meaning comes only from Rust functions, §5).

## 5. Closed types on the wire

<!-- constrained-by ./01-equivalence.md#4-closed-types -->

serde's `#[derive(Deserialize)]` builds closed types by shape without calling the smart constructor. If the TS schema called the checked constructor, TS would reject JSON that Rust accepts.

1. **By shape.** A closed type that derives `Deserialize` without an attribute is read by shape and branded through its package-internal constructor (`unsafeMakeId` for `fixtures/wire_shapes.rs`'s `Id`). Same set as Rust; invariants are not upheld on the wire, as in Rust.
2. **Through the constructor.** `#[serde(try_from = "T")]` on a struct, with `impl TryFrom<T> for X` (translated as `X.tryFrom`; `check` requires it, since rustc sees only the stand-in serde). The schema reads `T` as serde would, then calls `X.tryFrom`; `Err` fails the read, as serde fails deserialization. Serializing is unchanged (by shape), so `toJson` writes what serde writes. `impl Display` for the error, which serde requires, becomes `toString` when its text is fixed per value, and is skipped otherwise. examples/payment reads `Amount` and `PaymentMethodId` this way.

**Errors.** A refused value fails with an issue naming the type and what `try_from` returned: the error's text when its `impl Display` is translated (`toString`, [01 §7.11](./01-equivalence.md#711-impl-display-with-a-fixed-text)), the same words serde reports (`Amount: amount must be 50 to 99999999`), else the variant when the error type is an enum (`Amount: AmountOutOfRange`); zod's issue also carries the error value in `params.error`.

**Verification.** In `wire_write.rs`, for all three libraries: every schema reads a `PaymentMethodId`, an `Amount`, or an event carrying one exactly when `X::try_from` accepts the value, and gives the same value. The real serde on the same source (the `cargo test` in `bench/payment/wasm`) rejects the same inputs, with the `Display` text in its error.

**Remaining difference.** One difference remains, from [§3.3 rule 1](#33-reading-rules): `i64`/`u64` also read digit strings (`"50"`), which serde_json rejects. With `try_from` the value still passes through the constructor.

## 6. Reading and writing text

<!-- derived-from ./07-roadmap.md#2-evidence-from-examples -->

`toJson.T(x)` returns the JSON text serde_json writes for the Rust value of `x`, **byte for byte**. It needs no schema library; its helpers are the runtime's `Json`. A struct or a variant's wrapper is `Json.object([["a", ..], ..])`, a tuple `Json.tuple([..])`, so a long one reads one field per line.

`fromJson.T(text)` is its inverse for a type that derives `Deserialize`: the text through `parseJson` (§3.3 rule 1), then the schema. Malformed text and a refused value throw, as `JSON.parse` and the library's `parse` do (zod's `parse`, valibot's `v.parse`, arktype's `assert`). It is the one reading to use; `JSON.parse` and then the schema refuses an `i64` past 2^53.

| Value | Written as |
| --- | --- |
| integers, `i64`/`u64` included | the exact decimal (`bigint` is not rounded) |
| `f64` / `f32` | ryu's layout: `1.0`, `-0.0`, `0.001`, `1e16`, `1.5e-7`; `f32` with its own shortest digits, ties to even |
| non-finite floats | `null`, as serde_json writes them; neither side reads `null` back as a float |
| `String` | `JSON.stringify`, which escapes as serde_json does for well-formed strings |
| structs, enums, `Option`, `Vec`, tuples, `()`, `Result` | serde's default shapes (§1); struct fields in declaration order |

**Limits.**

- Unit structs are rejected ([02 §3.7](./02-authoring.md#37-types)): serde writes `struct S;` as `null` and `struct S {}` as `{}`, and the IR keeps only the fields.
- A string with a lone surrogate is not a Rust `String` (01 §2); `JSON.stringify` escapes it and serde_json then rejects the text.

**Why.** In an optimistic update the client reads the server's state, runs `step`, and sends the event back.

**Verification.** `crates/cli/tests/it/wire_write.rs` checks against the vendored serde_json, over `Serialize` impls that `purecrate_canon::fixture!` generates with the data-model calls `#[derive(Serialize)]` makes:

- floats: 80,000 pseudo-random bit patterns and boundary values (2,000,000 once, by hand), text equal;
- shapes: every type form in `wire_shapes.rs`, written by serde_json, read by the schema, and written back, text equal;
- payment: every state reachable in three events under each capture and confirmation method, times every event: the client's `toJson(step(read(state), read(event)))` equals the server's `serde_json::to_string(&step(state, event))`.

## 7. Open questions

- **Hermes.** Whether React Native's engine supports `JSON.parse` source text access is unconfirmed; if not, `parseJson` needs its own parser.
