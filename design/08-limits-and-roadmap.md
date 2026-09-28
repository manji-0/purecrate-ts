# Limits and roadmap

Date: 2026-09-28
Status: current overview

<!-- constrained-by ./07-authored-constraints.md -->
<!-- derived-from ./04-objective-means-demand.md -->
<!-- constrained-by ./05-type-sharing-scope.md -->
<!-- constrained-by ./01-surface-flatten-roadmap.md -->

## 0. Where this document stands

The target is new code written within [PureCrate's constraints](./07-authored-constraints.md). This is not a plan to raise the acceptance rate of existing crates. Measurements stay in [design/06](./06-acceptance-survey.md) and are not used as a metric.

We have reached the point where the constraints the Rust side accepts when writing pure transitions, and the constraints on callers of the generated TS, can be stated on a single page. The individual decisions are in [design/07](./07-authored-constraints.md), [design/04](./04-objective-means-demand.md), [design/01](./01-surface-flatten-roadmap.md), and [design/05](./05-type-sharing-scope.md). This document is the whole picture, plus the order in which to add things beyond it.

There is one condition for adding something. Add a capability only when a new example written within the constraints can no longer be written without it. The number of rejections in a corpus does not set the order.

## 1. What can be written today

Transitions that write state and events as ADTs and return the next value, like `fn step(state, event) -> State`. The counter example is the acceptance criterion ([design/00](./00-foundations.md) §15).

What can be written:

- structs, enums (`kind` discriminated unions), single-element tuple structs (newtypes)
- `Option`, `Result`, `?`, `if let`, exhaustive `match`
- Local `let mut`. Updates return a new value
- Local closures that capture only immutable bindings
- Struct update `S { a: e, ..base }`
- Sequences that grow and shrink are recursive enums. Owned trees are written with `Box<T>`
- `Vec` reads a sequence whose length was fixed outside, via index and `len`
- String contents are read by indexing the UTF-8 byte sequence (`&[u8]`) from `s.as_bytes()`
- Integer-range `for i in a..b`. The body updates `let mut` and exits with early `return` and `?`
- Integer and floating-point widths are distinguished by TS brand types
- Integer widening for which std has `From`, written `i64::from(x)`
- Fixed strings are built with `String::from("…")`
- `--schema zod|valibot|arktype` reads serde's default JSON into domain values

## 2. Constraints on the Rust author

So that the output produces the same results as a Rust debug build, the way code is written is bent toward us. The definition of capability is [design/07](./07-authored-constraints.md) §0.

### 2.1 Return state as the next value

A transition takes `state` and returns a new `State`. `&mut self`, assignment to fields, and `mut` parameters cannot be written. A local `let mut` can be used only inside that function.

Events are not accumulated in the state. If the past sequence is needed, keep it outside the transition ([design/02](./02-kamae-ts-emit.md) §1.1).

A sequence that grows and shrinks is a recursive enum, like `Lines::Cons(line, Box::new(lines))`, returned as a new list. Do not `push` onto a `Vec`. A `Vec` is a sequence whose length is fixed outside the function, built with `[a, b]` and read with `xs[i]` and `xs.len()`. `map` / `filter` / `collect` and `vec!` are not included.

### 2.2 Numbers are only sized integers and floats

No decimal type is included. Neither `Decimal` nor `rust_decimal` is accepted. Money is written as an integer newtype in the smallest unit (`struct Yen(i64)`).

`i32` and `f64` are both `number` at runtime, but at the type level they are `I32` and `F64` and do not mix. A numeric literal whose type is not determined is pinned with a suffix or `let x: T`. The result of a bare `+` loses its brand, so computations that go back into the domain are written with width-specific operations such as `Int.i32.add`.

Integers of different widths can be converted with `to::from(x)` only for widenings for which std has `From`: from `u8` / `u16` / `u32` to wider unsigned and to wider signed, and from `i8` / `i16` / `i32` to wider signed. As in std, `usize` accepts only `u8` and `u16`. The value does not change. In TS, `BigInt(x)` is emitted only when going from `number` to `bigint`. Narrowing, `as`, `.into()`, and `try_from` are rejected. [examples/order](../examples/order/src/lib.rs) holds quantities as `u32` and multiplies by the unit price with `i64::from(line.qty)`.

The reference for equivalence is the Rust debug build. Overflow and division by zero throw. We do not match release-mode wrapping. `usize` at or above 2^53, deep recursion (about 10,000 levels), and the like are outside equivalence (§4, [design/04](./04-objective-means-demand.md) §1.3.1).

Invariants protected by private fields and smart constructors survive into TS (2026-09-29). A struct with non-`pub` fields becomes a closed type: it gets a brand and no `of` is emitted. To obtain a value in TS, call a Rust public function (`pub fn new(..) -> Result<Self, E>` becomes `Percent.new`). This is the same as in Rust ([design/04](./04-objective-means-demand.md) §1.6, [design/07](./07-authored-constraints.md) §4.7). A struct whose fields are all `pub` still gets `of` as before.

### 2.3 Names are unique across the crate

Module paths do not remain in generated names. `billing::State` and `shipping::State` cannot both be written. The same applies to free function names ([design/01](./01-surface-flatten-roadmap.md) §2). Types and functions also collide if their kebab-case file names coincide. The type `Command` and the function `command` both become `command.ts`, so they cannot both be written.

User-written type parameters are rejected. The only allowed type constructors are `Option`, `Result`, `Vec`, and the erased `Box` / `Arc` / `Mutex`. `HashMap` and `BTreeMap` are rejected, because key equality differs between Rust and JS.

`#[serde(...)]` is rejected. JSON with renamed fields is not silently accepted. Field names are the Rust names as-is.

### 2.4 Sharing and interior mutability are not collapsed to values

`Rc`, `Cell`, and `RefCell` are rejected. Even single-threaded, collapsing writes through shared references or interior mutability into values changes the result.

`Box<T>`, `Arc<T>`, and `Mutex<T>` are erased to `T`. `::new(v)` becomes `v`. The output keeps a comment stating what it is used for in Rust and that TS ignores it because it is single-threaded. `lock` and `clone` do not become methods. Owned recursive data is written with `Box`. `Box` and `Arc` can be read through with `*x`. `Mutex` cannot be read without `lock`, so it can only be created and held.

Closures are used only within a function. They do not capture `let mut`. They are not placed in parameters, return values, or fields.

### 2.5 Each `match` arm names one variant

An arm may only be an enum variant, `Some` / `None`, or `Ok` / `Err`. The following cannot be written:

- A tuple scrutinee like `match (state, event)`. Split into one function per state and `match` on the event inside it
- `_ =>` and `A | B =>`. For transitions that are not accepted, write an arm per variant. The number of arms grows as the product of the number of states and events
- Binding-only arms (`lines => ...`). Name the variant and rebuild the value

### 2.6 Build strings with `String::from`, compare with `==`

The type of a string literal is `&str`. It cannot be placed in a `String` position. Write `String::from("a")`. In TS it becomes the literal itself. `"a".to_string()`, `.to_owned()`, and `.into()` are rejected, to keep one way of writing it.

A `String` can be placed in a `&str` position. `==` and `!=` compare `String` and `&str` in either order. `clone` is not available, so to place one `String` in two places, build it again with `String::from`.

### 2.7 Specified but not yet writable

The following are decided, but the implementation still rejects them. They are added when an example can no longer be written without them (§5).

- `char`, `String::len`, byte-position slicing, ordering comparison of `String`. Specification in [design/04](./04-objective-means-demand.md) §1.5. String contents are read by index and recursion over the byte sequence from `as_bytes()` (2026-09-29). If ordering is needed, use an enum or an integer.
- `isize`
- `while`, `loop`, `break` / `continue`, `a..=b` and iterator `for`, literal patterns in `match`
- The std method allow-list. Only `Vec::len`, indexing, and `str::as_bytes` are in, as a preview of it
- Byte literals `b'@'`. For now write `64u8`
- `const` / `static`

`==` on structs and enums is rejected. JS structural comparison does not match Rust. Write comparisons as an `eq` method.

## 3. Constraints on the TS caller

What callers must observe for functions that passed translation. Details are in [design/07](./07-authored-constraints.md) §4.

- Methods are `State.bump(state)`. No `this` is emitted. Structs and arrays are `Readonly`, and updates happen via return values.
- Expected failures are `Result` values. Only overflow, division by zero, out-of-bounds indexing, and `assertNever` throw.
- Numbers are brands. When bringing values in from outside, check them in with e.g. `Int.i32.of`. The result of arithmetic on raw `number` cannot be passed back to a branded parameter.
- `i64` / `u64` are `bigint`. When reading serde_json JSON, use `parseJson`, not `JSON.parse`. If the result of `JSON.parse` is passed, the schema rejects values above 2^53.
- The in-memory shape of an enum is `kind`. This differs from serde's default JSON. When passing JSON to a function, go through the wire schema emitted by `--schema` ([design/05](./05-type-sharing-scope.md) §7.5).
- A missing `Option` field and JSON `null` are `None`. `undefined` is `()`, not absence.
- In Rust, the original `state` cannot be used after the call. Generated TS does not mutate arguments, so the original object remains. No `Object.freeze` is applied. A retained original value, or mutation that strips the type, is outside the equivalence guarantee.
- Equivalence is guaranteed only when passing TS values that are images of Rust values ([design/04](./04-objective-means-demand.md) §1.3.1). No guarantee is made for values that break invariants via `of`, or values that skip checks via `as I32`. Closed types (§2.2) have no `of`, so the former cannot be constructed. The latter, `as`, remains outside the guarantee.
- Do not edit generated files. To change them, change the Rust and regenerate.

## 4. Remaining gaps in equivalence

For accepted inputs, return values, expected `Result`s, and debug integer arithmetic match Rust. The domain is the image of Rust values mapped into TS ([design/04](./04-objective-means-demand.md) §1.3.1). The following are outside it.

| Gap | Handling |
| --- | --- |
| `usize` at or above 2^53 | Explicit non-equivalence. Lengths and indices do not reach this range |
| Recursion depth | TS (Node 24 default stack) hits `RangeError` at about 12,000 levels. Rust debug passes 50,000 levels on the main thread and aborts at 100,000. Outside equivalence, and the failure modes differ too |
| JSON nesting depth | serde_json rejects nesting deeper than 128 levels. The wire schema has no limit. Boundary equivalence holds up to 128 levels |
| Private-field invariants | Closed types (§2.2, [design/04](./04-objective-means-demand.md) §1.6) have no `of`; the image is limited to return values of public functions. Types attached with `as` are outside the image. Values read from the wire are checked for shape only, like serde's derive ([design/05](./05-type-sharing-scope.md) §7.7) |
| Scope of differential-test comparison | Return values are compared in full, via a normal form built on both sides from the IR type. Cases run through a driver function that returns a scalar do not compare fields the driver does not read. The counter acceptance test compares only `State.n` ([design/04](./04-objective-means-demand.md) §1.4.1) |
| Release-build wrapping | Not matched. The reference is debug |
| `i64` JSON numbers | The schema accepts safe-integer numbers, `bigint`, and digit strings. Large values are read from text via `parseJson`. With the result of `JSON.parse`, values above 2^53 are rejected ([design/05](./05-type-sharing-scope.md) §7.6) |
| Non-finite `f64` | serde_json writes `NaN` as `null`. Whether to reject it or document it as a round-trip asymmetry is undecided ([design/05](./05-type-sharing-scope.md) §6) |
| String `.length` and `[i]` | The output does not emit these operations. Callers must not take JS UTF-16 units to be Rust byte lengths |
| Type inference implementation | The typing that determines output is a home-grown bidirectional inference. Whether to switch to rustc's type information is undecided ([design/04](./04-objective-means-demand.md) §5) |
| Inputs that do not compile | `check` and `build` run rustc after the subset check, report errors in the form `path:line:col: [rustc/E0382]`, and reject. The subset check erases borrows and does not track moves or lifetimes, so rustc closes that gap. Running `check` requires rustc (overridable with `RUSTC`). It fails if rustc cannot be launched |

## 5. Roadmap

Order is decided by where a new example got stuck, not by which syntax has the most rejections.

### 5.1 Done

Counter-style transitions, recursive sequences, newtypes, `Result`, numeric brands, reading from JSON into domain values via `--schema`. Closed types with private fields (2026-09-29, §2.2).

The second example written within the constraints is an order lifecycle ([examples/order](../examples/order/src/lib.rs)). Five states: draft, confirmed, paid, shipped, cancelled; line items are a recursive enum, and amounts are `Yen(i64)`. `Yen` and `Sku` are closed types, constructible only via `Yen::new` (rejects negatives) and `Sku::new` (rejects empty). All 4-step sequences, plus these two rejections, are compared between Rust and the generated TS (`crates/cli/tests/order_equivalence.rs`).

### 5.2 What examples got stuck on

Writing examples/order got stuck on the following two. Both were added.

1. **Lossless integer widening** (§2.2). Quantities could go back to `u32`. The 20 widenings are differentially tested at both ends of the source type (`crates/cli/tests/widen_equivalence.rs`). Narrowing (`try_from`) and `as` stay rejected until an example demands them.
2. **`String` from a string literal** (§2.6). This was stuck not in a domain transition but in a differential-test driver function that builds a `Command` from code. At the same time, inputs that place a literal in a `String` position were rejected. This was a hole where `check` accepted what rustc rejects.

examples/signup (2026-09-29) is the first example written from a third-party specification (item 1 of [design/04](./04-objective-means-demand.md) §1.4.2). It implements WHATWG HTML's "valid e-mail address" and the password length rules of NIST SP 800-63B-4, written by looking only at design/07 and design/08. It got stuck on the following:

1. **Could not read string contents** (capability). `char`, `len`, `find`, `split`, `chars`, and slicing were all rejected; the only thing readable from a string was `==`. `str::as_bytes` was added (the UTF-8 byte sequence, per the specification in [design/04](./04-objective-means-demand.md) §1.5). Both rules can be written with byte indexing, recursion, and `u8` comparison. The WHATWG rule is determined by ASCII alone, and the NIST length counts code points, so it counts bytes excluding continuation bytes (0x80–0xBF).
2. **`Ok(())` was rejected** (hole). Expression `()` was lowered as an empty tuple, so it did not match type `()` and was rejected with `expected (), found ()`. Even when the types matched, TS emitted `[]`. Fixed to `Lit::Unit`, with differential tests added (`unit_ok` and `unit_some` in `crates/cli/tests/fixtures/control.rs`).
3. **File-name collision between driver function `email` and type `Email`** (a known constraint, §2.3). The driver function was renamed.

Rewrites due to notational loss: `char` to `u8`, `find` and `chars().all` to index recursion, `split('.')` to recursion over label ends, character classes to numeric range comparisons, and `b'@'` to `64u8`. Line counts (excluding blank lines and comments) are as follows.

| Part | Idiomatic Rust | Rust within the constraints | Ratio |
| --- | --- | --- | --- |
| Email (WHATWG) | 28 | 79 | 2.8x |
| Password (NIST length) | 17 | 28 | 1.6x |

**Email exceeded the withdrawal threshold (2x)** (the second of item 3 in [design/04](./04-objective-means-demand.md) §1.4.2). The increase comes from unrolling iterator adapters into recursive functions and from spelling out character classes as range comparisons. Both are notational loss; the capability is sufficient. One example is not grounds to revisit the approach, but if the next validation example shows the same ratio, add `chars` and `for` (items 3 and 4 of §5.3) or re-compare approaches.

As a semantic check, the acceptance and rejection of Rust's `Email::parse` are matched against running the regular expression given by WHATWG in node (`crates/cli/tests/signup_equivalence.rs`). The Rust-vs-generated-TS differential test compares down to the payload of `Err` and the closed types inside `Ok`.

As a second validation example, examples/iban (2026-09-29) was written: the ISO 13616-1 electronic format and ISO 7064 MOD 97-10. It was written under the current constraints without getting stuck. As a semantic check, it is matched against a version of the same rules written in idiomatic Rust, using published valid IBANs, those with one character changed, and malformed inputs (`crates/cli/tests/iban_equivalence.rs`).

Line counts exceeded the threshold, following Email. In both examples, most of the increase comes from unrolling `all`, `fold`, and `chain` traversals into recursive functions. Both were rewritten assuming integer-range `for i in a..b` (in the form using `let mut` updates and early `return` in the body), run with rustc, and line-counted.

| Example | Idiomatic Rust | Current constraints | With integer-range `for` |
| --- | --- | --- | --- |
| Email (WHATWG) | 28 | 79 (2.8x) | 52 (1.9x) |
| IBAN (ISO 13616) | 24 | 57 (2.4x) | 45 (1.9x) |

With `for`, both fall inside the threshold. Much of the remaining difference comes from spelling out character classes as numeric comparisons, because there are no literal/range patterns in `match` and no byte literals (§2.7).

So integer-range `for` was added (`crates/cli/tests/loops_equivalence.rs`) and the two examples were rewritten. The differential tests, and the matching against the WHATWG regular expression and idiomatic Rust, pass unchanged after the rewrite.

| Example | Idiomatic Rust | Recursion only | After adding `for` |
| --- | --- | --- | --- |
| Email (WHATWG) | 28 | 79 (2.8x) | 52 (1.9x) |
| Password (NIST length) | 17 | 28 (1.6x) | 24 (1.4x) |
| IBAN (ISO 13616) | 24 | 57 (2.4x) | 45 (1.9x) |

All three fall inside the threshold, but the margin is small. If the threshold is hit next, notation for writing character classes (literal/range patterns in `match`, byte literals) is the first candidate.

Notational loss (the arm constraint of §2.5) does not reduce capability, so it is not listed here. If the number of arms due to no `_ =>` grows enough to make transition tables unreadable, it will be dealt with then.

### 5.3 Add when an example needs it

1. **Boundary encoding.** Reading exists. The side that writes the same JSON from the domain (encode) does not yet. An example where a server returns JSON in the same shape as the output is needed first. Reading `i64` was solved with `parseJson` (2026-09-28). Encoding requires writing `bigint` as a JSON number. `NaN` handling will close [design/05](./05-type-sharing-scope.md) §6 at that time.
2. **Validation examples.** Sharing input validation has the most visible demand yet the lowest writability ([design/04](./04-objective-means-demand.md) §3.2). Closed types were added first (§5.1). Since that change closes a hole in the domain of equivalence rather than adding a capability, it did not wait for §0's "add only when an example gets stuck". examples/signup was written and `as_bytes` was added (§5.2). The second example (examples/iban) also exceeded 2x in line count; integer-range `for` from item 4 was added, bringing both back inside. Differential tests can now compare down to the payload of `Err` (2026-09-29, [design/04](./04-objective-means-demand.md) §1.4.1). `#[serde(try_from)]`, which protects invariants on the wire, will be added when an example demands it ([design/05](./05-type-sharing-scope.md) §7.7).
3. **Characters and strings.** When an example needs to write validation within this subset. Reproduce UTF-8 byte units and do not map to JS `.length`. Methods are added one at a time via an allow-list with differential tests.
4. **Iteration.** Integer-range `for i in a..b` has been added (2026-09-29, §5.2). `while`, `break` / `continue`, and iterator `for` will be added once an example appears that cannot be written with range `for` and recursion.
5. **The std methods that example calls.** Those that cannot be made to match stay rejected. Iterator `map` / `filter` / `collect` are not added, because they are a way of growing a state's sequence as an array.

### 5.4 When type expressiveness runs out (v1)

As in [design/01](./01-surface-flatten-roadmap.md).

- Unbounded type parameters. The TS side is also emitted as generics. `where` and associated types are not included.
- Only `HashMap` / `BTreeMap` with `String` keys become `ReadonlyMap<string, V>`. Insertion order is not made to match Rust.

Both wait until an example appears that cannot be expressed without type parameters or without Map.

### 5.5 Not doing

- An allow-list aimed at getting existing crates through. Demand and success are measured by examples written from third-party specifications and by the withdrawal threshold ([design/04](./04-objective-means-demand.md) §1.4.2)
- Decimals, growable `Vec`, event logs inside state
- A schema-library dependency in the core package. Of zod / valibot / arktype, only the one specified is used, in a separate package
- WASM as a v0 completion criterion. The IR does not preclude a second backend, but the current path is TS source
- `Rc` / `Cell` / `RefCell`. Collapsing them to values changes observations

## 6. Roles of the documents

| Document | Role |
| --- | --- |
| [design/08](./08-limits-and-roadmap.md) | The whole of the constraints, and the order of additions |
| [design/07](./07-authored-constraints.md) | Capabilities when writing new code, and constraints that remain on the TS side |
| [design/04](./04-objective-means-demand.md) | Definition of equivalence, and semantic decisions |
| [design/05](./05-type-sharing-scope.md) | The boundary with JSON |
| [design/01](./01-surface-flatten-roadmap.md) | Public surface, flattening, the version with generics and Map |
| [design/06](./06-acceptance-survey.md) | Record of measuring existing crates. Not a metric going forward |
