# Wire boundary

Status: current (2026-10-16, 0.23.0)

<!-- constrained-by ./01-equivalence.md -->

## 1. Problem

Shared behavior is useless if the values it consumes still arrive through hand-written conversion: the drift this tool removes from logic would reappear at the edge. On the client, state and events usually come from the Rust server as serde JSON, and that JSON is not the generated value.

- **Shape.** Generated enums use an internal `kind` tag; serde's default is external tagging. Maps are JS `Map`s; serde writes a JSON object.
- **Numbers.** JSON numbers are unbranded; fields are `I32`, `F64`, `I64` (`bigint`). `JSON.parse` rounds integers above 2^53.
- **Attributes.** `#[serde(rename_all = "camelCase")]` would silently change field names, unless the reader follows it (§3.6).

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
| `BTreeMap<u32, V>` | `ReadonlyMap<U32, V>` | `{"9":v,"10":w}` |

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
| Core runtime `purecrate` | `packages/boundary`, copied into each package as `src/purecrate-runtime.ts` (design/03 §4.4) | The domain runtime (numeric brands and `Int`, `Result`, `Str`, `Char`, `Uuid`, `Ord`, `BTree`, `Slice`, ..) and the wire's own parts: `parseJson`, `JsonFloat`, `jsonKeys`, `structIssue`, `fromSequence`, `internalTag` / `adjacentTag`, and `Json`, the writers `toJson` calls (`object`, `withTag`, `tuple`, `array`, `map`, `set`, `int`, `bool`, `str`, and the floats), with `BTree.mapJson` for a map whose keys are not `String`s. It depends on no schema library |
| Adapters `purecrate-zod`, `-valibot`, `-arktype` | `packages/boundary-<lib>`, the one passed with `--schema` copied in as `src/purecrate-<lib>.ts` | Thin adapters, one per schema library: the scalar schemas (`i8` .. `u128`, `usize`, `f32`, `f64`, `bool`, `unit`, `str`, `char`, `uuid`, `uuidError`), `nullable`, `record` (and arktype's `sequence`) for a struct, `variant` / `unitVariant` / `unitEnum` for an enum, `btreeMap` / `btreeSet` / `btreeMapOf` / `btreeSetOf` with `intKey` / `boolKey` for maps and sets, `internallyTagged` / `adjacentlyTagged`; zod's also `optionalField`, arktype's also `Wire`, `memo`, `keyed`, `fail` |
| Wire module | `src/purecrate-wire.ts`, emitted by `--schema <lib>` | A schema for each public struct and enum that derives `Deserialize`, reading serde's JSON (with the attributes of §3.6 and §5) into the branded domain type; `fromJson.T(text)` for each, reading the text through `parseJson` and the schema; and `toJson.T(x)` for each that derives `Serialize`, writing it back (§6). See §3.2 |

Each adapter's library is a peer dependency of the generated package:

| Adapter | Targets |
| --- | --- |
| `purecrate-zod` | zod 4.6 (zod 3 up to purecrate-ts 0.2) |
| `purecrate-valibot` | valibot 1.1 |
| `purecrate-arktype` | arktype 2.1 |

### 3.2 serde in the input

- **Derives pass.** The input's types may derive `Serialize`/`Deserialize` (and `use serde::…`), so the server reads and writes the same types.
- **Only the derives make a wire form.** A schema reads a type that derives `Deserialize`, and `toJson` writes one that derives `Serialize`; a type with neither has no wire form, as in Rust. Without this, a closed type with no derive (signup's `Email`) got a schema that built it by shape, a value Rust has no way to read from JSON. `--schema` on a crate where no public type derives either is refused.
- **What the derive needs.** A type that derives one holds only types that derive it too, no std `Ordering`, which serde gives no form, and no array of more than 32 elements, for which serde implements neither trait; `check` refuses the rest (`item/serde-derive`), as the real derive does. With `#[serde(try_from = "T")]`, `Deserialize` needs it of `T` instead of the fields.
- **How `check` handles them.** `check` compiles the crate against a stand-in `serde` whose derives expand to nothing (`crates/cli/src/rustc.rs`); without it, a crate that derives serde fails `check` with `cannot find crate serde`. The translated code is unaffected, and the real derive is checked by the server's build.
- **Attributes: a fixed set, the rest refused.** `rename`, `rename_all`, `tag`, and `content` are modeled where serde reads them (§3.6), and `try_from` on a struct (§5). Any other `#[serde(...)]` (§4) is refused with its location and the list of what is modeled, so a wire format the schemas would not follow is never silently accepted.

### 3.3 Reading rules

<!-- constrained-by ./01-equivalence.md#726-keys-other-than-string -->
<!-- constrained-by ./01-equivalence.md#727-fixed-size-arrays -->

These follow serde's default behavior, as serde_json 1.0.145 reads and writes it. Other sections refer to them by number.

1. **Integers and parsing.** Input is an already-parsed value. `i64` / `u64` (and `i128` / `u128`) accept safe-integer numbers and `bigint`, not strings (serde_json refuses `"50"` for an integer); out-of-range values are schema failures, not throws. A rounded `JSON.parse` value always lies outside the safe range and is rejected, so a wrong value is never read. To read JSON text, use `parseJson` (`fromJson.T` does), which reads it as serde_json does where `JSON.parse` would not, from the reviver's source text (Node 21+):
   - an integer literal outside ±(2^53−1) becomes a `bigint` with its exact value;
   - an integral number written as a float (`50.0`, `5e1`, `-0`) becomes a `JsonFloat`, which the `f32` / `f64` schemas read and every integer schema refuses, as serde reads such a number as a float only (`-0` into an `i128` alone, as `0`, as serde_json reads it);
   - what `JSON.parse` loses about an object's keys is noted on the object (`jsonKeys`), not refused, for the schema that reads it to decide: a key held twice (`JSON.parse` keeps the last), a key with a lone surrogate, the keys written with an escape, and the keys' order in the text where `Object.keys` differs (it lists integer-like keys first). A struct or a variant's wrapper refuses the first two (`structIssue`): a struct for a field it has, a wrapper for any key, as serde refuses a field twice and a key it cannot decode only in an object it reads and reads past both in a field it ignores (`{"value":1,"x":1,"x":2}` is a `Chain`; `{"value":1,"value":2}` is not). A map reads the other two (rule 6).

   Without `parseJson` (an already-parsed value), `50.0` cannot be told from `50` and is read as an integer.
2. **`Option`.** A missing `Option` field and `null` are both `None`.
3. **`char` and `Uuid`.**
   - A `char` is a string of exactly one Unicode scalar value: `""`, `"ab"`, `"é"`, and a lone surrogate are rejected, as serde_json rejects them.
   - A `Uuid` is read from any string `Uuid::parse_str` accepts and becomes the canonical form; anything else, and a JSON array of bytes (which serde_json never passes to `Uuid`), is rejected. `toJson` writes the canonical form, as serde does.
   - `uuid::Error` has no JSON form in Rust; its schema rejects every value.
4. **Enum objects and struct fields.** With the default (external) tagging, the object wrapping a variant has exactly one key; extra keys are rejected. Tagged enums read as §3.6 says. Unknown fields inside structs are ignored. A unit variant accepts `"Dot"` and `{"Dot":null}`. So `{"Circle":1.5,"Rect":[1,2]}` is rejected, not read as `Circle`. A struct (and a struct variant's fields) is a JSON object, or its sequence form: an array of exactly its fields, in order, which serde reads too though serde_json never writes one (`[3,null]` is a `Chain { value: 3, next: None }`; `[3]` and `[3,null,1]` are refused, as serde refuses them, and so is an `Option` field left out of the array). The adapters' `record` (arktype's `sequence`) reads an array of that length as the object; one of another length is refused, where valibot's and arktype's object shapes would read every field as missing.
5. **Generic types.** A generic type's schema is a function of its parameters' schemas (`export const Page = <T>(wireT: z.ZodType<T>): z.ZodType<Page$<T>> => ..`, `Page(User)` where it is used), its writer a function of their writers (`toJson.Page(x, (v) => toJson.User(v))`), and `fromJson.Page(text, User)`. A recursive one is behind `lazy` inside the function. With arktype the parameter is any `Type` (`WT extends Type`, its value `WT["infer"]`, as the adapter's `nullable` takes one); inside, the schema is read as a plain `Type`, which arktype's type-level check of a definition takes where an unresolved `WT` fails it.
6. **`BTreeMap` and `BTreeSet`.** A map is a JSON object whose keys are each key's text; a set is an array of its keys' JSON values. `toJson` writes both in Rust's key order: by code point for `String`, `char`, and `Uuid` keys, by value for integers (`"9"` before `"10"`, `BTree.mapJson(m, Ord.cmp, write)`) and `bool`s.
   - **`String` keys** (`btreeMap`, `btreeSet`). Every own key of the object is a key, `__proto__` and `constructor` included (zod's record drops `__proto__`, valibot's `__proto__` and `constructor`, so the adapters read the object itself); a key with a lone surrogate is refused. A set is an array of strings.
   - **Other keys** ([01 §7.26](./01-equivalence.md#726-keys-other-than-string): an integer, `char`, `bool`, `Uuid`, or a newtype of one or of `String`), read by `btreeMapOf(key, value)` and `btreeSetOf(key)` as serde_json's map-key deserializer reads them. An integer key goes through `intKey(t)`: JSON's integer grammar only (`"01"`, `"+1"`, `" 1"`, `""`, `"1e2"`, `"1.0"` refused), a `number` where it is safe and a `bigint` past that, then `t`'s range (`u128` and `i128` too); `"-0"` only into an `i128`, as `0`. A `bool` key goes through `boolKey(t)`, exactly `"true"` or `"false"`. A `char`, `Uuid`, or `String` newtype key is read by its own schema, so a `Uuid` key in any form `parse_str` takes becomes the canonical form; a newtype key goes through the newtype's schema after its content's, as serde reads a newtype key. An integer or `bool` key written with an escape (`"1"`, `"1\/"`) is refused, as serde_json reads it from the bytes between its quotes; a `char`, `Uuid`, or `String` newtype key with an escape is decoded, as serde_json decodes it. A set of such keys is an array of their JSON values (`[9,10]`).
   - **A key twice.** A later text of one key keeps its value, as serde's map does: `JSON.parse` has kept the last of two equal texts, and `btreeMapOf` reads the keys in the order `parseJson` noted (rule 1), not `Object.keys`'s, so of two texts of one key (`"-0"` and `"0"` into an `i128`, a `Uuid` in two cases) the later in the text wins. A set keeps a repeated key once.
7. **Arrays `[T; N]`.** A JSON array of exactly `N` elements, as serde reads one: zod's `z.array(..).length(N)`, valibot's `v.pipe(v.array(..), v.length(N))`, arktype's `.array().exactlyLength(N)`; one of another length, nested too, is rejected. `toJson` writes it as a `Vec`. A longer array than serde implements its traits for is refused by `check` (§3.2).

### 3.4 Per-library notes

- **zod and valibot.** A struct is the object schema's own output, unknown keys dropped as serde drops them; it is built field by field only where that output is not the domain value: a closed struct (through its constructor), a `()` field (which inference makes optional), and with valibot an `Option` field (missing reads as `undefined`, made `null`). Schemas are printed dependencies first, and only a type in a cycle of references (a recursive type, or two that refer to each other) is behind `lazy`. The adapters' `unitVariant` and zod's `optionalField` spell §3.3 rules 4 and 2 once, and `unitEnum(["A", "B"])` reads an enum whose variants all are unit as one expression.
- **arktype.**
  - Enums try variants in turn, because arktype rejects a union of objects containing morphs; the adapter's `unitEnum("E", ["A", "B"])` does it for an enum whose variants all are unit.
  - Each schema is a morph from `unknown` typed `Wire<T>`, built lazily on first read so recursive and later-declared types resolve. (A `type.module` design hit a `ReferenceError` at import.)
  - A nested read that fails hands its errors to the morph's traversal (`fail`, through `ArkErrors.merge`), so they keep their path.
  - When no variant matches, an object keyed by a variant's name reports that variant's errors (`Confirm.outcome`), anything else one error for the enum.
  - A generic newtype's morph and its `fromJson` read are cast to their declared type (`as Wire<Wrap$<WT["infer"]>>`, `as Wrap$<WT["infer"]>`): arktype types a morph's output as `Exclude<Wrap$<T>, ArkErrors>` and `assert`'s as `_distill<Wrap$<T>, "out">`, which TS leaves open for `T & {brand}`, as `T` could be anything. The declared type is the value the inner schema has read.

### 3.5 Verification

For all three libraries the schemas pass `tsc --strict` on TS 6 and 7 and are run on node:

- `crates/cli/tests/it/wire.rs`: `fixtures/wire_shapes.rs`, which covers every type form, against serde-default JSON and malformed inputs.
- `crates/cli/tests/it/wire_write.rs`: each rule of §3.3 against the vendored serde_json, every text accepted or refused as serde_json does, and what is read written back byte for byte (§6): integers and floats, struct sequence forms, generic types, maps and sets of every key type (each key text in rule 6, escaped keys of every key type, two texts of one key in either order), and arrays at their length and at every wrong one.
- `crates/cli/tests/it/wire_attrs.rs`: the attributes of §3.6.

### 3.6 Names and tags

<!-- constrained-by #32-serde-in-the-input -->

The attributes a server most often puts on its wire types, modeled as serde_derive 1.0.228 reads and writes them. The TS domain types keep the Rust names; only the JSON changes.

| Attribute | On | JSON |
| --- | --- | --- |
| `rename = "x"` | a field, a variant | the field's key, or the variant's name, is `x` (any string; it is escaped as serde_json escapes it) |
| `rename_all = "camelCase"` (and serde's seven other rules) | a struct: its fields; an enum: its variants; a struct variant: its fields | each name converted as serde_derive converts it (`internals/case.rs`, ported with its tests); a field's own `rename` wins |
| `rename` | a struct or an enum | nothing on the wire: it renames the type only in serde's messages |
| `tag = "t"` | an enum | internally tagged: `{"t":"V", ..fields}` |
| `tag = "t", content = "c"` | an enum | adjacently tagged: `{"t":"V","c":content}` |

**Internally tagged** (`tag`). A unit variant is `{"t":"V"}`, a struct variant its fields beside the tag, a newtype variant the fields of the struct it holds beside the tag. Read as serde reads it: the tag anywhere in the object, other keys ignored, the tag twice refused, a tag that is not a string refused; the sequence form `["V", a, b]` too, where the rest of the array is the variant's fields in order (none for a unit variant). `check` refuses what serde cannot write: a tuple variant, a newtype of anything but a struct with named fields (serde's derive refuses the first; serialization fails at run time on the second), a newtype of `()` (serde writes the tag alone, and reads it back only from an object holding nothing else), and a field under the tag's name.

**Adjacently tagged** (`tag` and `content`). A unit variant is `{"t":"V"}` and reads with the content left out or `null`; any other variant needs its content, except a newtype of an `Option`, whose content left out is `None`. The content of a struct variant is an object, never the sequence form; a tuple variant's is an array. Other keys are ignored, either key twice is refused, and the sequence form is exactly `["V", content]`.

**`check` refuses** two fields of a struct (or of a struct variant) under one JSON name, two variants under one name, and a tag and a content with one name: serde's derive compiles each, and then reads only one of the two.

**How it is read and written.** The adapters' `internallyTagged` and `adjacentlyTagged` find the variant from the runtime's `internalTag` / `adjacentTag` and hand the rest to the variant's arm; the struct and variant schemas key their objects by the JSON names; `toJson` writes them, the tag first, and `Json.withTag` puts the tag into the object a newtype variant's struct writes.

**Verification.** `crates/cli/tests/it/wire_attrs.rs` compiles `fixtures/wire_attrs.rs` with the real derive (`serde` is a dev-dependency for this alone) and generates it with each library. On 2,953 JSON texts, 343 of which serde reads (edges written by hand: Rust names where the JSON names differ, keys twice, both sequence forms, tags that are not strings, content left out; and every key of each left out, renamed to every name the fixture uses, or doubled, objects as arrays), `fromJson.T` then `toJson.T` gives what `serde_json::from_str` then `to_string` gives, or both refuse. The case rules are tested against serde_derive's own table (`crates/syntax/src/serde_attrs.rs`).

## 4. Out of scope

- Parsing JSON inside transitions.
- Type declarations alone as a ts-rs replacement.
- Semantic checks transcribed into `.refine`: meaning comes only from Rust functions (§5).
- Of serde's attributes, everything §3.6 and §5 do not list: `untagged` (serde tries each variant in turn and keeps the first that reads, an order the schemas would have to copy exactly), `flatten` (the fields of one struct read from another's object, which serde buffers and the fixed object schemas cannot), `default` and `skip*` (a value made by a Rust function the generated code may not have), `alias`, `deny_unknown_fields`, `transparent`, and a `rename` split by direction. Each is refused with its location; a server that needs one keeps a separate wire type and converts in Rust.

## 5. Closed types on the wire

<!-- constrained-by ./01-equivalence.md#4-closed-types -->

serde's `#[derive(Deserialize)]` builds closed types by shape without calling the smart constructor. If the TS schema called the checked constructor, TS would reject JSON that Rust accepts.

1. **By shape.** A closed type that derives `Deserialize` without an attribute is read by shape and branded through its package-internal constructor (`unsafeMakeId` for `fixtures/wire_shapes.rs`'s `Id`). Same set as Rust; invariants are not upheld on the wire, as in Rust.
2. **Through the constructor.** `#[serde(try_from = "T")]` on a struct, with `impl TryFrom<T> for X` (translated as `X.tryFrom`; `check` requires it, since rustc sees only the stand-in serde). The schema reads `T` as serde would, then calls `X.tryFrom`; `Err` fails the read, as serde fails deserialization. Serializing is unchanged (by shape), so `toJson` writes what serde writes. `impl Display` for the error, which serde requires, becomes `toString` when its text is fixed per value, and is skipped otherwise. examples/payment reads `Amount`, `PaymentMethodId`, and `PaymentIntent` this way; the intent through `UncheckedIntent`, the same fields, so a status whose amounts disagree with its terms fails the read.

**Errors.** A refused value fails with an issue naming the type and what `try_from` returned: the error's text when its `impl Display` is translated (`toString`, [01 §7.11](./01-equivalence.md#711-impl-display-with-a-fixed-text)), the same words serde reports (`Amount: amount must be 50 to 99999999`), else the variant when the error type is an enum (`Amount: AmountOutOfRange`); zod's issue also carries the error value in `params.error`.

**Verification.** In `wire_write.rs`, for all three libraries: every schema reads a `PaymentMethodId`, an `Amount`, or an event carrying one exactly when `X::try_from` accepts the value, and gives the same value. The real serde on the same source (the `cargo test` in `bench/payment/wasm`) rejects the same inputs, with the `Display` text in its error.

**Remaining difference.** None in what the schemas read from `fromJson.T`. A schema handed an already-parsed value cannot tell `50.0` from `50` ([§3.3 rule 1](#33-reading-rules)); with `try_from` the value still passes through the constructor.

## 6. Reading and writing text

<!-- derived-from ./07-roadmap.md#2-evidence-from-examples -->

`toJson.T(x)` returns the JSON text serde_json writes for the Rust value of `x`, **byte for byte**. It needs no schema library; its helpers are the runtime's `Json`. A struct or a variant's wrapper is `Json.object([["a", ..], ..])`, a tuple `Json.tuple([..])`, so a long one reads one field per line.

`fromJson.T(text)` is its inverse for a type that derives `Deserialize`: the text through `parseJson` (§3.3 rule 1), then the schema. Malformed text and a refused value throw, as `JSON.parse` and the library's `parse` do (zod's `parse`, valibot's `v.parse`, arktype's `assert`). It is the one reading to use; `JSON.parse` and then the schema refuses an `i64` past 2^53.

| Value | Written as |
| --- | --- |
| integers, `i64` to `u128` included | the exact decimal (`bigint` is not rounded) |
| `f64` / `f32` | ryu's layout: `1.0`, `-0.0`, `0.001`, `1e16`, `1.5e-7`; `f32` with its own shortest digits, ties to even |
| non-finite floats | `null`, as serde_json writes them; neither side reads `null` back as a float |
| `String`, `char` | `JSON.stringify`, which escapes as serde_json does for well-formed strings |
| `Uuid` | its canonical form |
| `BTreeMap`, `BTreeSet` | an object and an array, in Rust's key order (§3.3 rule 6) |
| `[T; N]` | an array, as a `Vec` |
| structs, enums, `Option`, `Vec`, tuples, `()`, `Result` | serde's default shapes (§1), or §3.6's; struct fields in declaration order |

**Limits.**

- Unit structs are rejected ([02 §3.7](./02-authoring.md#37-types)): serde writes `struct S;` as `null` and `struct S {}` as `{}`, and the IR keeps only the fields.
- A string with a lone surrogate is not a Rust `String` (01 §2): a `str` schema refuses one, as serde_json refuses the escaped text `JSON.stringify` writes.

**Why.** In an optimistic update the client reads the server's state, runs `step`, and sends the event back.

**Verification.** `crates/cli/tests/it/wire_write.rs` checks against the vendored serde_json, over `Serialize` impls that `purecrate_canon::fixture!` generates with the data-model calls `#[derive(Serialize)]` makes:

- floats: 80,000 pseudo-random bit patterns and boundary values (2,000,000 once, by hand), text equal;
- shapes: every type form in `wire_shapes.rs`, generic types, maps and sets of every key type, and arrays, written by serde_json, read by the schema, and written back, text equal;
- payment: every state reachable in three events under each capture and confirmation method, times every event: the client's `toJson(step(read(state), read(event)))` equals the server's `serde_json::to_string(&step(state, event))`.

## 7. Open questions

- **Hermes.** Whether React Native's engine supports `JSON.parse` source text access is unconfirmed; if not, `parseJson` needs its own parser.
