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
- Thin adapters live in separate packages (`purecrate-zod`, `-valibot`, `-arktype`). Only the one passed with `--schema` is used.
- `--schema <lib>` emits `src/purecrate-wire.ts`: a schema for every public struct and enum, reading serde's default JSON (no attributes) into the branded domain type.
- `#[serde(...)]` is rejected everywhere with its location, so a renamed wire format is never silently accepted.

Reading rules (serde's default behavior):

1. Input is an already-parsed value. `i64` / `u64` accept safe-integer numbers, `bigint`, and digit strings; out-of-range values are schema failures, not throws. A rounded `JSON.parse` value always lies outside the safe range and is rejected, so a wrong value is never read. To read large integers, parse the text with `parseJson`, which uses the `JSON.parse` reviver's source text (Node 21+) to turn only out-of-range integer literals into `bigint`. `f64` is unaffected because serde_json always writes `.` or an exponent.
2. A missing `Option` field and `null` are both `None`.
3. The object wrapping a variant has exactly one key; extra keys are rejected (`{"Circle":1.5,"Rect":[1,2]}` used to read as `Circle`; fixed 2026-09-28). Unknown fields inside structs are ignored. A unit variant accepts `"Dot"` and `{"Dot":null}`.

Library notes: zod and valibot build structs field by field, because inference makes `undefined`-valued (`()`) fields optional. arktype rejects a union of objects containing morphs, so enums try variants in turn; each schema is a morph from `unknown` typed `Wire<T>`, built lazily on first read so recursive and later-declared types resolve (an earlier `type.module` design hit a `ReferenceError` at import).

Verification: `fixtures/wire_shapes.rs` covers every type form; for all three libraries the schemas pass `tsc --strict` on TS 6 and 7 and are run on node against serde-default JSON and malformed inputs (`crates/cli/tests/wire.rs`).

## 4. Out of scope

Parsing JSON inside transitions; type declarations alone as a ts-rs replacement; existing APIs using `rename_all`; semantic checks transcribed into `.refine` (meaning comes only from Rust functions, §5).

## 5. Closed types on the wire

<!-- constrained-by ./01-equivalence.md#4-closed-types -->

serde's `#[derive(Deserialize)]` builds closed types by shape without calling the smart constructor. If the TS schema called the checked constructor, TS would reject JSON that Rust accepts.

1. **Implemented.** Closed types are read by shape and branded through `Email$of`. Same set as Rust; invariants are not upheld on the wire, as in Rust.
2. **Planned, when an example needs it.** `#[serde(try_from = "T")]` would be the one accepted serde attribute, together with exactly one `impl TryFrom<T> for X` as `X.tryFrom`. The schema reads `T` and calls the generated `tryFrom`; `Err` becomes a schema failure.

## 6. Open questions

- **Encoding.** Only reading exists. Writing the same JSON from domain values needs `bigint` written as a JSON number. Waits for an example where the client sends domain values back.
- **Non-finite `f64`.** serde_json writes `NaN` as `null`. Reject by type, or document as a round-trip asymmetry? Decide with encoding.
- **Hermes.** Whether React Native's engine supports `JSON.parse` source text access is unconfirmed; if not, `parseJson` needs its own parser.
