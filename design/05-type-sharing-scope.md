# Can type sharing be brought into scope?

Created: 2026-09-27. Premise: design/04 (which redefined the objective as "sharing behaviour").

## 0. Conclusion

- **Yes, but as "boundary types".** Emitting type definitions alone (ts-rs territory) is not a goal. The goal is generating codecs that move the values shared behaviour takes and returns to and from the Rust server's JSON (serde).
- There are two reasons. First, even when behaviour is shared, TS-side values usually come from the server's JSON. Today that conversion must be hand-written, and the "drift between two implementations" this tool promises to eliminate remains there. Second, competing head-on with ts-rs on type-only generation would make an inferior product that handles neither generics, serde attributes, nor external types.
- **Depending on configuration, wrong types are emitted right now.** serde attributes such as `#[serde(rename_all = "camelCase")]` are silently ignored. Using the generated types as wire-format types makes field names disagree (§2.3). Whatever the policy, detecting and rejecting serde attributes should come first.

## 1. Three meanings of "type sharing"

| Meaning | Content | Representative | Relation to this tool |
| --- | --- | --- | --- |
| (a) Type definition generation | Emit TS type declarations from Rust types. No runtime checking | ts-rs, specta, tsify | Already possible as a by-product (§2.1) |
| (b) Consistency with the wire representation | Convert and validate between serde's JSON and TS values | zod_gen (schemas), hand-written decoders | **Unsupported. A practical gap in behaviour sharing** |
| (c) Behaviour and types from the same source | Emit transition functions and their argument types from the same Rust | Crux (via WASM) | Current target |

design/04's redefinition differentiated from (a). But without (b), the path for passing server data to what (c) generates remains hand-written.

## 2. Current facts (confirmed 2026-09-27)

### 2.1 Type-only crates already pass

A crate with only structs and enums is accepted, and types plus companions (`of` and variant constructors) are emitted. (a) "works" with no extra implementation. However, it is hardly usable for DTOs, for the following reasons.

- Generics (`Page<T>`), external types (`chrono`, `uuid`, `rust_decimal`), and `HashMap` are not accepted.
- `i64` becomes `bigint`. serde_json emits `i64` as a JSON number, which differs from the `number` ts-rs users expect.

### 2.2 The representation of generated values differs from serde's default JSON

Generated values are kamae-shaped (discriminated unions on `kind`), distinct from serde's default (external tagging).

| Rust | Generated TS value | serde_json default | With `#[serde(tag = "kind")]` |
| --- | --- | --- | --- |
| unit variant `Inc` | `{ kind: "Inc" }` | `"Inc"` | `{"kind":"Inc"}` (matches) |
| newtype `Add(i32)` | `{ kind: "Add", content: [1] }` | `{"Add":1}` | Only when the content is a struct. serde rejects `i32` |
| tuple `P(i32, i32)` | `{ kind: "P", content: [1, 2] }` | `{"P":[1,2]}` | serde rejects |
| struct `Move { dx }` | `{ kind: "Move", dx: 1 }` | `{"Move":{"dx":1}}` | `{"kind":"Move","dx":1}` (matches) |
| `i64` / `u64` | `bigint` | number | number |
| `Option<T>` | `T \| null` | `null` or the value | same |
| `Result<T, E>` | `{ kind: "Ok", value }` | `{"Ok":v}` | (attribute does not apply to Result) |
| `()` | `undefined` | `null` | `null` |

### 2.3 serde attributes are silently ignored

Given `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct State { pub total_count: i64 }`, `total_count: bigint` is emitted with no error. The server's JSON has `totalCount`. This violates v0's "never silently drop syntax" policy.

**Addressed (TODO 21)**: `#[serde(...)]` on items, variants, fields, and methods is rejected with its location. `#[cfg]` and `#[cfg_attr]` are also rejected (the output cannot reproduce conditional compilation, and `cfg_attr` can inject serde attributes). `#[cfg(test)]` items do not exist in a normal build, so they are skipped. `derive`, `doc`, and lint attributes pass.

### 2.4 64-bit integers lose precision in `JSON.parse`

`JSON.parse('{"a":9007199254740993}').a` becomes `9007199254740992`. In Node 24, the reviver's third argument exposes the original source text of a number, allowing lossless conversion to `BigInt`. `JSON.rawJSON` is available for writing. Both are ES2025 JSON.parse source text access. Whether Hermes (React Native) supports this is unconfirmed. If not, the output needs to include a small JSON parser.

### 2.5 serde is not vendored

`vendor/` contains only syn and its dependencies (proc-macro2, quote, unicode-ident). Writing differential tests against serde_json requires adding serde, serde_derive, and serde_json to vendor (a one-time online step).

## 3. Options

| Option | Content | Pros | Cons |
| --- | --- | --- | --- |
| A. Declare out of scope | Leave DTOs to ts-rs; users write conversions | No extra cost | Hand-written conversions remain a breeding ground for drift, and the central promise breaks at the edges. ts-rs types and generated types have different shapes (§2.2), so conversion between them is also needed |
| B. Make type-only generation an official feature | Extend support to serde attributes, generics, external types | Enters a large adjacent market (ts-rs recent downloads: 6.2M) | Competes head-on with ts-rs and specta. Incompatible with subset constraints (no generics). Dilutes the differentiator of behaviour sharing |
| C. Generate boundary codecs | For each public type, generate `decode` (JSON text → `Result<T, DecodeError>`) and `encode` (T → JSON text). Match serde_json's default representation and guarantee it with differential tests | ts-rs is types only with no runtime checking, and zod_gen's schemas are disconnected from behaviour. C emits "validated decoder + types + behaviour" from the same source. Matches kamae's "parse at the boundary" | Medium-sized implementation (lossless 64-bit integer parsing, external-tag conversion, error types). Requires vendoring serde |
| D. Converge representations | Require `#[serde(tag = "kind")]` on the Rust side so TS value representation and JSON representation coincide | Conversion becomes almost unnecessary; only validation and `bigint` handling remain | Forces changes to the server's API format. serde rejects tuple variants and newtype variants wrapping primitives |

## 4. Evaluation

- **B is not adopted.** Demand is largest for (a), but most DTO crates use generics, external types, and serde attributes, and are almost never accepted by the current subset. Widening acceptance would pull implementation in a direction unrelated to the core of "being able to verify it returns the same results as Rust".
- **A undermines the central promise.** Even in state-machine use, JSON is received for initial state and server sync. Making users hand-write that loses at the boundary the safety gained by generation.
- **C is the differentiator.** It solves the problem ts-rs users have, "the types match but runtime values differ", within the same differential-testing framework as behaviour. Differential tests can be built in this order: create a value in Rust, write it with serde_json, pass it through TS `decode`, the function call, and `encode`, read it back with Rust's serde_json, and compare. For malformed JSON, we can confirm that TS returns `Err` whenever serde errors.
- **D is a special case of C.** Types with `tag = "kind"` have matching representations, so the generated codec is smaller. It can be the recommended shape for new projects. Add after C.

## 5. Recommendation

**Decision (2026-09-27)**: adopt C (boundary codecs). Order of work: 1 (reject serde attributes), then design/04 §4-3 (`?`, `if let`, `Option`, `let mut`).

1. **Do now (small): detect and reject serde attributes.** Pass `#[derive(Serialize, Deserialize)]` alone. Reject `#[serde(...)]` with its location until supported. This removes the "silently wrong" state in §2.3 and is needed under any policy.
2. **Include DTO-style crates in the acceptance measurement (design/04 §4-2).** Use it for the investment decision on C. If most fail on generics and external types, narrow C's target to "State and Event of state machines".
3. **Implement C.** Targeting only serde's default representation, build `decode` and `encode`, lossless 64-bit integer parsing, and differential tests against serde_json.
4. **Depending on acceptance data, support `tag = "kind"` (D), `rename_all`, and `rename`.**

Broaden the objective statement in design/04 §1.2 as follows.

> Make it possible to map the **behaviour** of pure domain logic written in Rust, and the **boundary representation (serde JSON)** of the values it exchanges, to plain TypeScript without WASM, and to mechanically verify that both are observably equivalent to Rust.

Add the following success criteria.

- For every accepted public type, values match across the round trip serde_json → `decode` → `encode` → serde_json (differential test).
- 64-bit integer boundary values such as `2^53 + 1` are not lost.
- For JSON that serde rejects, `decode` also returns `Err`.

## 6. Open questions (as of 2026-09-27)

- Should `decode`'s input be JSON text or `unknown` (an already-parsed value)? Preserving 64-bit integer precision needs text. `unknown` fits better with existing fetch wrappers.
- Does Hermes support JSON.parse source text access? If not, the output carries its own JSON parser.
- Should non-finite floats (serde_json writes `NaN` as `null`) be rejected by type, or documented as a round-trip asymmetry?
- When an `Option` field is missing (serde treats it as `None`), should `decode` accept it the same way?

## 7. Breaking down the problem (2026-09-28)

No implementation yet. The problem of passing server JSON to generated functions splits into three layers. All are hand-written today.

### 7.1 What we want to pass

The target is the public states and events of newly written state machines. The client receives JSON from the server and passes it to `step(state, event)`. The event sequence is not accumulated inside the state (design/02 §1.1). Growing sequences is not a problem of this boundary.

`step` itself does not read JSON. In kamae, only values parsed at the boundary enter the domain.

### 7.2 The shapes differ

Generated values use an internal `kind` tag. serde's default JSON uses an external tag (§2.2). The result of `JSON.parse` is not the generated state type.

`#[serde(...)]` is rejected (§2.3). JSON under different names is never silently accepted. The codec targets only serde's default without attributes.

### 7.3 The numbers differ

JSON numbers are unbranded `number`. An `i32` field is `I32`, `f64` is `F64`, and `i64` is `I64` (`bigint`). Even if the shape matches, numbers from `JSON.parse` cannot be put into fields directly.

`i32` can be checked for range and integrality with `Int.i32.of`. For `i64`, `JSON.parse` loses precision above 2^53 (§2.4); it must be read from text (§7.6 item 1). For non-finite `f64` values, serde_json writes `NaN` as `null`.

### 7.4 What is not this problem

- Parsing JSON inside transition functions.
- Emitting type declarations alone as a replacement for ts-rs (option B in §4).
- Growing a `Vec` inside the state.
- Reading existing APIs with `#[serde(rename_all)]` as-is. The attributes are rejected.

### 7.5 Policy (2026-09-28)

Schema libraries are not part of the core. `packages/boundary` (package name `purecrate`) holds the branded types and `Int.*.of`. Generated packages import `I32` and so on from it.

zod, valibot, and arktype get thin adapters in separate packages. We do not depend on all three at once.

Only with `--schema zod|valibot|arktype` are wire schemas for public structs and enums emitted to `src/purecrate-wire.ts`. The shape is serde's default JSON, and the result is the domain's branded types. Schemas for libraries not specified are not emitted.

arktype rejects at runtime a single union made of objects containing morphs. Enums try variants one at a time. Every arktype schema is a morph from `unknown`, with its type annotated as `Wire<T>`. The JSON shape is built on first read (`memo`). This allows referring to recursive types and to schemas declared later in the file. Previously, recursive structs were placed in a single `type.module`, but it could not reference enums outside the module, and it read a struct inside `Box` before initialisation, causing a `ReferenceError` at import time (replaced on 2026-09-28).

Generated wire schemas pass `tsc --strict` on TypeScript 6 and 7. A fixture covering all type shapes (`crates/cli/tests/fixtures/wire_shapes.rs`) is generated for all three libraries, and type checking plus reading and rejecting serde's default JSON are verified in node (`crates/cli/tests/wire.rs`). zod and valibot structs do not return the parsed object as-is but build it field by field, because schema inference makes fields whose value may be `undefined` (`()`) optional.

### 7.6 Decisions made in this implementation

1. Schema input is an already-parsed value. `i64` / `u64` accept JSON numbers that are safe integers (within ±(2^53−1)), `bigint`, and digit strings. Out-of-range values are returned as schema failures, not exceptions. Numbers above 2^53 have already been rounded by `JSON.parse`, so they are rejected. A rounded value always falls outside the safe range, so a wrong value is never silently read. To read large integers written by serde_json without loss, read the JSON text with `purecrate`'s `parseJson(text)`. From the source text passed to the `JSON.parse` reviver (Node 21+), only integer literals outside the safe range become `bigint`. serde_json always writes `f64` with a `.` or an exponent, so `f64` values do not change (2026-09-28).
2. The target is all public structs and enums.
3. A missing `Option` field is `None`. JSON `null` is also `None`. Same as serde's default.
4. The object wrapping an enum variant (`{"Circle": 1.5}`) has exactly one key. Extra keys are rejected, same as serde_json. Previously extra keys were discarded, and `{"Circle": 1.5, "Rect": [1, 2]}` was read as `Circle` (fixed 2026-09-28). zod uses `.strict()`, valibot `strictObject`, and arktype `"+": "reject"` to make only the wrapper strict. Unknown fields inside structs and struct variants are ignored, per serde's default. Unit variants accept `"Dot"` and also, like serde_json, `{"Dot": null}`.

### 7.7 Reading closed types (2026-09-29)

<!-- constrained-by ./04-objective-means-demand.md#16-sharing-validation-and-public-constructors -->

Even for closed types with private fields (design/04 §1.6), Rust's `#[derive(Deserialize)]` builds values by looking only at the shape. The smart constructor is not invoked. So if the wire schema called the checked constructor on the TS side only, TS would reject JSON that serde_json accepts. That is a non-equivalence at the boundary.

Decisions:

1. Closed types without attributes are read by shape only, as before. The schema goes through the package-internal construction function (`Email$of`) and returns the value with the closed type's brand (implemented; `Id`, `Label`, `Sealed` in `crates/cli/tests/wire.rs`). This yields the same set of values as Rust's derive, so it is equivalent. In exchange, values coming in over the wire do not uphold invariants. This is also the same as Rust.
2. To uphold invariants on the wire, write `#[serde(try_from = "T")]` on the Rust side. This is serde's canonical way to express this composition. The plan is to accept only this form within `#[serde(...)]`, and only when the subset contains `impl TryFrom<T> for X`. The schema reads the shape of `T` and calls the generated `try_from`. `Err` is returned as a schema failure. The wire tests confirm that serde rejects the same inputs.
3. Item 2 is not implemented. Currently all `#[serde(...)]` are rejected (§2.3). Trait impls are also rejected (`trait impls are not in v0`). Adding 2 also requires accepting exactly one `impl TryFrom<T> for X` as the companion function `X.tryFrom`. To be added when a validation example actually needs invariants upheld on the wire.

Schemas handle only the shape layer (the three layers in design/04 §1.6). Semantic checks enter only by calling Rust-authored functions via `try_from`. Checks are not transcribed into zod's `.refine` or the like.
