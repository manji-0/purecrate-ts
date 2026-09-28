# Sharpening the objective, validity of the means, and expected demand

Date: 2026-09-27 (§1.3.1, §1.4, §3 revised 2026-09-28; §1.3.1 and §1.6 revised 2026-09-29)
Status: Analysis (review after the v0 implementation)
Prerequisites: `00-foundations.md` through `03-ir.md`, the P0–P2 implementation

## 0. Conclusions

1. **Redefine the objective as "sharing behavior", not "sharing types".** The core value is being able to use pure domain functions, with Rust as the source of truth, on the TS side in a form that **returns the same results**. The moment equivalence breaks, this mechanism is worse than a hand-written dual implementation (you cannot notice that it is broken).
2. **The means (subset translation without WASM) is conditionally valid.** For small transition functions called frequently, avoiding WASM's boundary cost and initialization is a real advantage. However, the current analysis uses only `syn` and does not know expression types, so it **cannot preserve integer arithmetic semantics**. Measured: `i32` `7 / 2` becomes `3.5` in TS (§2.4). Before widening the syntax, local type inference and numeric semantics must be settled.
3. **Demand: "the adjacent market is large, direct demand is unverified."** Rust→TS type generation is clearly large (ts-rs: 15.93M cumulative downloads). Behavior sharing is done via WASM (Crux) or hand-written dual implementations plus parity tests; there is no mature direct competitor. This can be read either as open ground or as small demand. Since the target is now "code newly written within the constraints" (§5), the question is not the acceptance rate of existing crates but whether there are users willing to pay the cost of writing within the constraints (§3).

## 1. Sharpening the objective

### 1.1 Problems with the current objective statement

`00` §1 says "translate a Rust crate satisfying the constraints into a single TS package". That describes the means, and leaves the following undefined.

- **Whose problem it solves**: the dual implementation, and its drift, for developers who want the same decisions in both Rust and TS.
- **What counts as "the same"**: the same types, the same values, or even the same way of failing.
- **The role of the Rust side**: Rust as the source of truth with TS as a derivative is an implicit assumption. Unless stated, a practice of editing the TS side emerges and drift detection (P2) loses its meaning.

### 1.2 The sharpened objective

> Taking pure domain functions (ADTs and transitions) written in Rust as the source of truth, generate a package callable in a JS runtime **without WASM or a serialization boundary**, **with idiomatic TS values as-is**. The output must be verifiable as **observationally equivalent to Rust** over the entire domain of accepted inputs. Inputs that cannot be accepted are rejected without generating anything.

### 1.3 Definition of "observationally equivalent" (including open points)

For a function `f` and its output `f'`, the following holds for every input `x` within the accepted range.

| Item | Requirement | Status |
| --- | --- | --- |
| Return value | `f(x)` and `f'(x')` are equal as values (`x'` is the TS representation of `x`) | Differentially tested on counter |
| Types | TS types represent the Rust types exactly (`i64` → `bigint`, etc.) | counter, all differential-test fixtures, and the zod, valibot, and arktype wire schemas are checked with tsc strict on TypeScript 6 and 7 (the differential harness runs `tsc -p` before execution) |
| Expected failure | `Result::Err` has the same variant and the same value | Matches in differential tests including `?` and early `return` (control, vending: all 1296 sequences) |
| Integer arithmetic | Truncating division, sign of remainder, and overflow match | Match (§2.4, per-operator differential tests) |
| Unexpected failure (panic) | Handling of inputs that panic in Rust | Arithmetic panics also throw in TS (decided in §5). `unreachable!` is `assertNever` |
| Strings | Unit of length and indexing (Rust: UTF-8 bytes, JS: UTF-16) | Decided to reproduce UTF-8 (§1.5). `as_bytes` implemented and differentially tested with empty strings, 2–4-byte characters, and out-of-range indices (2026-09-29). The rest is unimplemented |

Handling of panics and overflow needs a design decision. There are three options.

1. **Reject**: do not accept operations that may panic (integer `/`, `%`, arithmetic that may overflow). Safe, but greatly narrows the accepted range.
2. **Reproduce**: throw under the same conditions on the TS side (treated as an "unexpected fault", like `assertNever`). The definition of equivalence can then include "both fail on the same input".
3. **Restrict to checked APIs**: accept only the `Option` returned by `checked_add` etc., and reject plain arithmetic on `i32`.

Option 2 (reproduce) was adopted (§5). It matches Rust debug-build behavior and is consistent with kamae-ts's "the unexpected is an exception". It does not match the wrapping behavior of release builds, so the reference for equivalence is stated to be **the Rust debug build**.

#### 1.3.1 Domain of equivalence

"Every input `x` within the accepted range" above is not every value constructible in TS. The domain is **the image of Rust values mapped to their TS representation**. For a TS value `x'`, `f'(x')` is equivalent to `f(x)` only when there is a Rust value `x` whose representation is `x'`, and the call stays within the resource limits below. No promise is made about the result for values outside the image.

Values outside the image, and inputs for which equivalence does not hold, are as follows.

| Item | What happens | Handling |
| --- | --- | --- |
| Invariants of non-public fields | Rust can guard invariants with non-public fields and smart constructors. Under the policy up to 2026-09-28, the output emitted `of` for every struct and erased field visibility. From TS, values that Rust's `new` rejects could be created with `of` | Revised 2026-09-29 (§1.6). Types with non-public fields get no `of` and get a brand. The image narrows to values returned by the crate's public functions. Values typed with `as` are outside the image, like `5 as I32` |
| Brand casts | Values branded without a check, like `5 as I32`; out-of-range integers; non-integer `number`s | Outside the image. Values entering from outside are checked with `Int.i32.of` |
| `string` containing lone surrogates | Not a representation of a Rust `String` (§1.5) | Outside the image |
| Recursion depth | The two sides have different limits on how deep a recursive function can walk a recursive enum. Measured 2026-09-28 (a function counting the length of a `List::Cons`, Node 24.21 default stack, macOS): TS passes at 10,000 levels and throws `RangeError` at 12,000. The Rust debug build passes 50,000 levels on the main thread (8 MB) and at 100,000 levels **the process aborts** on stack overflow. This is not a panic, so it cannot be caught. The limit varies with stack size, frame size, and thread (test threads have 2 MB) | Outside equivalence. Even "both fail on the same input" does not hold. States holding deep sequences carry depth as a constraint |
| JSON nesting depth | serde_json's `Deserializer` by default errors on JSON nested deeper than 128 levels. The generated wire schemas (zod, etc.) have no limit | Equivalence at the wire boundary is limited to 128 levels or fewer. When passing deep recursive data as JSON, the Rust side's read fails first |
| Original value after the call | In Rust, a non-`Copy` by-value argument is moved, and the original `state` cannot be read after the call. In TS the caller still has the original object | Equivalence speaks about the return value. Reading the remaining original has no Rust counterpart and is not promised |
| Mutation through aliases | The output is `Readonly` but not `Object.freeze`d. If the caller casts away the type and mutates, the change is visible through aliases to the same object (e.g. subtrees shared in the return value) | Outside the image. Values that are not shared in Rust are shared in TS |
| `usize` at or above 2^53 | Rust does not panic up to 2^64. TS throws above 2^53−1 | Explicit non-equivalence. Lengths and indices do not reach this range |

`usize` was previously described as "the only explicit non-equivalence". Given the recursion depth and wire depth above, it is not the only one.

### 1.4 Success criteria (measurable form)

<!-- constrained-by ./07-authored-constraints.md -->

| Metric | Criterion |
| --- | --- |
| Equivalence | Differential tests (including boundary values) and property-based tests agree on every accepted example. The whole value is compared (§1.4.1) |
| Types | Output has 0 errors under `tsc --strict` |
| Determinism | Same input yields the same bytes (detected by `check --out`) |
| Rejection quality | Every rejection returns `path:line:col` and a reason, with no partial generation. Problems inside a function body point to the statement, the trailing expression of the block, or the `match` arm (the parser marks them with `Expr::At`, removed after checking) |
| Authoring capability | Transitions newly written within the constraints preserve ADTs, exhaustiveness, `Result`, and debug integer semantics (design/07) |
| Idiomaticity | TS users need not write serialization, initialization, or async loading |

The acceptance rate of existing crates was measured and recorded in design/06. With the target now being newly written code, the metrics are "Authoring capability" above and design/07.

Every metric in the table above is measured on examples written by this repository's author (counter, vending, order). The author knows the constraints and writes around forms that cannot be written. That does not tell whether the constraints hold up in practice. Hence the external criteria in §1.4.2.

#### 1.4.1 What the differential tests compare

The differential tests render the Rust and TS results as canonical-form strings and compare them (since 2026-09-29). Both sides build the canonical form from the same IR types.

- On the Rust side, the test-only proc-macro `purecrate_canon::fixture!` generates `Show` for every struct and enum in the fixture. `Show` for containers such as `Option`, `Result`, `Vec`, tuples, and `Box`, and for numbers and strings, lives in the test support (`crates/cli/tests/support`).
- On the TS side, the harness generates a per-type printing function from the same IR and selects it by each case's return type. It renders the output's value shapes (`kind`, `content`, newtypes with erased brands, `null`, `undefined`) as the same string as Rust.
- The form is close to Rust's `Debug`: `Order::Placed { lines: Lines::Cons(…), total: Yen(450) }`. Floats are written as their `f64` bit pattern; strings write anything other than printable ASCII as `\u{…}`.

Previously, tests compared projections chosen by the test, using a hand-written `Show` per test and a TS `show` that emitted only `kind` and `content`. `OrderError::AmountMismatch { .. }` compared only the variant name and missed swapping `expected` and `got`. That swap was deliberately injected into the emitter, and it was confirmed that it is now detected (2026-09-29).

Two projections remain. One is cases routed through a driver function returning a scalar: fields the driver does not read are not compared. examples/order adds `trace4`, which returns the state itself, and also compares the whole final state. The other is the counter acceptance test, which uses its own driver and compares only `State.n`.

#### 1.4.2 External criteria and withdrawal thresholds (2026-09-28)

Three criteria that do not depend on the author's own examples. The numbers are initial values, to be revisited after the first measurement.

1. **Write from third-party specifications.** Pick a state-machine specification published by someone other than the author (payment state transitions, approval flows, game rules, etc.) and write it within the constraints, reading only design/07 and design/08. Record the number of places rewritten due to rejection and why, and the number of rejections the documents alone could not resolve.
2. **Compare with idiomatic code.** Write the same specification also in unconstrained idiomatic Rust and in kamae-ts idiomatic TS. Compare line counts, the number of style changes made for the constraints (expanding `_ =>`, replacing with recursive enums, `Int.i32.add`, etc.), and the readability of generated TS versus hand-written TS.
3. **Withdrawal thresholds.** If any of the following is hit, reconsider continuing this approach (F in §2.1), comparing it against switching to WASM (B) or types-only generation (C).
   - More than half of the third-party specifications cannot be written within the constraints.
   - Constrained Rust exceeds twice the line count of idiomatic Rust for the same specification.
   - Holes that silently return wrong values on accepted input keep being found, via differential tests or user reports. As a guide: one or more per new example written.
   - Not a single real use replacing a dual implementation (item 2 of §3.5) is obtained.

### 1.5 Semantics of strings, `char`, `usize`, and std methods (decided 2026-09-27)

The corpus often handles strings at byte granularity (`chars` 910, `len` 804, `is_ascii_digit` 803, `as char` 813, `as u32` 485, byte-position slicing `&s[a..b]` 310, `bytes` 233, `to_uppercase` 132; design/06). Rejecting encoding-dependent operations would block the core of validation code. So Rust's semantics are reproduced as-is.

#### Strings reproduce UTF-8 byte units

The TS value stays a `string`. Only encoding-dependent operations are mapped to functions in the output's runtime `str.ts`.

| Rust | Meaning in generated TS |
| --- | --- |
| `s.len()` | UTF-8 byte count. Counts 1–4 per code point (O(n)) |
| `&s[a..b]` | Slices at byte positions. Out of range or not on a char boundary throws with the same message as Rust's panic |
| `s.bytes()`, `s.as_bytes()` | UTF-8 byte sequence (`u8`) |
| `s.chars()` | Sequence of code points. Same as the JS string iterator |
| `a < b` (`String`) | Code point order (same as UTF-8 byte order). JS `<` uses UTF-16 unit order, which inverts U+E000–U+FFFF relative to supplementary planes (measured: `"\u{ffff}" < "\u{10000}"` is `true` in Rust, `false` in JS) |

As a precondition, `string`s passed from TS are assumed well-formed (no lone surrogates). Strings with lone surrogates are not representations of a Rust `String`, so they are outside equivalence. Checking at the boundary (`isWellFormed`) is the codec's job (design/05).

#### `char` is a branded `string` of one code point

```ts
declare const CharBrand: unique symbol;
export type Char = string & { readonly [CharBrand]: true };
```

JSON matches serde (`char` is a one-character string). `==` is correct as string equality. Ordering and `c as u32` use `codePointAt(0)` (for the same reason as the table above). `u8 as char` is `String.fromCodePoint`. The mapping for U+0000–U+00FF is the same as Rust.

#### `usize` is a range-checked `number`

`usize` and `isize` become `number`. Operations are checked by the `Int` runtime like other integers, but the upper bound is `Number.MAX_SAFE_INTEGER` (2^53−1), throwing above it. 64-bit Rust does not panic up to 2^64, so **`usize` at or above 2^53 is an explicit non-equivalence** (other non-equivalences are in §1.3.1). Lengths and indices do not reach this range. `bigint` was not chosen because lengths and indices as `bigint` in TS APIs are awkward to mix with arrays and loops.

#### std methods are limited to an exact-match allow-list

- The allow-list holds, per (receiver type, method name), the signature and the TS mapping in one line. Resolution works like the receiver syntax in TODO 29: if the crate has no method of its own, the allow-list is consulted.
- Each entry requires a differential test, including empty input, non-ASCII, supplementary-plane characters, boundary indices, and panicking inputs.
- Anything that cannot be made to match is rejected for that receiver type. Accepting with a documented difference is not done. Measured examples:
  - `f64::to_string`: 1e21 is `1000000000000000000000` in Rust and `1e+21` in JS. Rejected (`to_string` for integers and `bool` matches, so those are accepted).
  - `str::trim`: JS `trim()` also strips U+FEFF; Rust does not. Not mapped to JS `trim()`; instead a function stripping by the Unicode `White_Space` property is used.

#### Methods that depend on Unicode tables are accepted with a note

`to_uppercase`, `to_lowercase`, `is_alphabetic`, `is_numeric`, etc. rely on Unicode tables held separately by Rust and JS. They are accepted, with equivalence noted as "holds for code points assigned in both the Rust toolchain's and the JS engine's Unicode versions". As of 2026-09-27, both are Unicode 17.0 (`char::UNICODE_VERSION` and `process.versions.unicode`, Node 24.21). Special uppercasing rules (`ß` → `SS`) and final sigma in lowercasing (`Σ` → `ς`) were also measured to match. Differential tests first check that both Unicode versions match.

#### Reserved names in the output

The runtime `str.ts`'s name `Str`, the file name `str`, and the name `Char` are reserved (treated like `Int` in design/00 §8.1).

### 1.6 Sharing validation and public constructors

<!-- derived-from #131-domain-of-equivalence -->
<!-- derived-from #32-use-cases-and-whether-they-can-be-written-within-the-constraints -->

Decided 2026-09-29. Closed types are implemented (`crates/cli/tests/closed_equivalence.rs`).

§3.2 names sharing input validation as the use case with the most visible demand. At the same time, it is the one least writable within the constraints. The cause lies before the lack of string operations: **the domain of equivalence includes values that the Rust types do not allow to be constructed**.

#### The three layers of validation

The request "we want to share validation" mixes three things.

| Layer | Question | Handling |
| --- | --- | --- |
| Shape | Is this JSON the serde representation of `Order`? | Wire schemas (design/05 §7). Already exist |
| Meaning | Is this string an `Email`? Is this combination valid? | Handled in this section |
| Screen | What to show in which field. Wording, i18n, focus | Out of scope. The Shell's job |

What is shared is the "meaning" layer. The core of this layer is not transitions. It is **checked constructors** such as `fn parse(raw) -> Result<Valid, E>`. Transitions only receive values that have already passed the check.

#### Closed types

In Rust, a struct with even one non-`pub` field cannot be built with a struct literal from outside the crate. The only way to obtain a value from outside is the crate's public functions. This rule is carried over to the output as-is.

- A struct with non-`pub` fields (including newtypes) is called a **closed type**. `pub(crate)` and `pub(super)` are treated the same as non-`pub` (design/00).
- Closed types get a `unique symbol` brand regardless of whether they have fields. Structs with named fields are currently a plain `Readonly<{…}>`. In that shape, even if `of` is hidden, an object literal passes as the same type.
- The companion of a closed type does not emit `of`. To assemble struct literals inside the output, an internal construction function is emitted. It is exported from the type's file but not from `index.ts`. The package's `exports` is only `./src/index.ts`, so users cannot import it via deep paths either. Its name is `Email$of`. Rust identifiers do not contain `$`, and `rename` only appends `$` and digits, so there is no collision.
- A type whose fields are all `pub` is an **open type**. It emits `of` as now. In Rust too it can be built with a literal from outside the crate, so there is no invariant the type guards.

Which functions are "checked constructors" is not determined, because there is no mechanical way to identify invariants. Values of a closed type can only be obtained from return values of public functions. Whether named `new`, `parse`, or `try_from` does not matter. This rule is the same as Rust's.

#### Revised domain

Of §1.3.1's domain, "the image of Rust values mapped to their TS representation", the part for closed types is narrowed as follows.

> Values of a closed type are limited to values returned by the output's public functions, or by wire reads (design/05 §7.7).

In Rust too, values of a closed type come from the same set. So this revision does not break the correspondence with Rust. The domain becomes narrower than now, and also precise as a definition.

#### What is not fully closed

- `"x" as Email` passes type checking. The brand exists only in the types. Like `5 as I32`, this is outside the image. We do not say "cannot be created in TS". We can only say "the type checker does not let you create it unless you write `as`".
- Using classes and `#private` fields would close it even at runtime. That is not adopted: it breaks §1.2's "idiomatic TS values", JSON, and structured cloning. It would also split the representation from open types.
- The output is not `Object.freeze`d (mutation through aliases in §1.3.1). Values of a closed type mutated after casting away the type are also outside the image.

#### Shape of failures

The shape of failures stays the domain enum of `Err` written in Rust. A common type for multiple errors with field paths is not introduced for now. Assignment to form fields is done by the caller by inspecting the variant. A common type is introduced when the same assignment table is duplicated across several examples.

Error messages are not shared. What is shared is the variant and its payload. Wording and language belong to the screen layer; including them in equivalence would tie differential tests to the text.

#### What this decision does not add

- String operations and `for` are not added by this decision. As in design/08 §5.3, they are added one at a time when a validation example gets stuck. Semantics are as in §1.5.
- Regular expressions are not included. Rust's `regex` and JS `RegExp` differ in the range of `\w`, Unicode handling, and lookahead support. The same pattern on both sides does not give the same result. Interpreting the same small pattern language on both sides is not considered until an example calls for it.
- If building-block types (non-empty strings, ASCII-only strings, length bounds, etc.) are provided, they are concrete types. User type parameters are not accepted, so forms like `NonEmpty<T>` are not possible (design/08 §2.3). Building-block types also take Rust as the source of truth and are generated by the same translator. They are not hand-written in the TS runtime.

#### Requirements on differential tests

What validation needs to observe is the rejection side. Differential tests for rejection cases compare the payload in addition to the `Err` variant. The comparison was widened to the whole value on 2026-09-29 (§1.4.1).

## 2. Validity of the means

### 2.1 Alternatives

| Means | Equivalence | Ergonomics in TS | Runtime constraints | Adoption cost | Expressiveness |
| --- | --- | --- | --- | --- | --- |
| A. Hand-written dual implementation + parity tests | Depends on tests | Best | None | Continuously high | Unlimited |
| B. Compile Rust to WASM (wasm-bindgen, Crux) | Nearly complete (same binary) | Serialization at the boundary, async initialization | Needs a WASM-capable runtime | Medium | Nearly all of Rust |
| C. Generate types only (ts-rs, specta, tsify) + hand-written logic | Types only | Good | None | Low | Types only |
| D. Write in a language targeting both (Gleam, Kotlin Multiplatform) | Guaranteed by the language implementation | Depends on the language | None | Requires a language migration | Whole language |
| E. LLM translation | No guarantee | Good | None | Low but poor maintainability | Arbitrary |
| **F. This approach: translating a pure subset** | Depends on translator correctness | Best (plain TS values) | None | Low to medium | Subset only |

### 2.2 Conditions where this approach is stronger

- **Fine-grained, frequent calls**: a transition function does a small computation per call. WASM needs value encoding and copying at every boundary crossing, and calling small functions many times can be slower than JS (rs4ts.dev, wasm-bindgen issue #2355). This approach has no boundary.
- **Wanting to handle values as TS types**: Crux's web shell serializes with bincode to pass to the core and decodes the result. Plain objects that can be held directly as React state are easier to work with.
- **Runtimes where WASM is awkward**: React Native's Hermes lacked `WebAssembly` for a long time. Hermes V1 in RN 0.84 (2026-02) added WASM support, but whether wasm-bindgen output runs is reported as unconfirmed (paritytech/verifiablejs#25). **This advantage is shrinking**, and it should not be a main justification.
- **Wanting to review the output**: generated TS is readable and can be stepped through in a debugger. WASM cannot be read.

### 2.3 Conditions where this approach is weaker

- **Expressiveness**: the current subset lacks loops, iterators (`iter().map()`), closures, string manipulation, `HashMap`, generics, and traits. Real domain code uses these heavily, so the acceptance rate is expected to be low (unmeasured).
- **Burden of correctness**: with B, the Rust compiler guarantees semantics. With F, a home-made translator must guarantee them, and every semantic hole is our own responsibility (§2.4 is an example).
- **No compiler**: `syn` is only a syntax tree, with no name resolution or types. P1 implemented name resolution and exhaustiveness in-house, but there is no type inference.

### 2.4 Measured semantic gaps

Results of running the output on Node in `/tmp/sem` (2026-09-27):

| Function | Rust (debug) | Generated TS |
| --- | --- | --- |
| `half(7)` (`n / 2`, `i32`) | `3` | `3.5` |
| `rem(-7)` (`n % 3`) | `-1` | `-1` |
| `inc(2147483647)` (`n + 1`) | panic (overflow) | `2147483648` |
| `div0(1)` (`n / 0`) | panic | `Infinity` |

**Addressed (TODO 18)**: with local type inference and the `Int` runtime, 57 cases including the 4 above match Rust (`crates/cli/tests/arith_equivalence.rs`; spec in design/00 §8.1). The following is the analysis at the time.

There is a single cause: **IR expressions have no types**. The printer cannot decide whether to emit `BinOp::Div` as integer division (`Math.trunc`) or floating-point division (`/`). counter uses only `+` and `-`, so the differential tests could not detect it. Adding more inputs per function to the differential tests is not enough; **per-operator semantic tests** are needed.

### 2.5 Evaluation of design decisions

Judged sound:

- Making the IR independent of the print target (does not block a second target such as WASM)
- Preferring rejection over partial generation (mandatory given equivalence as the objective)
- Discriminated unions by `kind` and companions (exhaustive `switch` works on the TS side)
- Mapping `i64` to `bigint` (does not silently lose precision)
- Having golden tests, differential tests, and drift detection from the start

To reconsider:

- **Doing analysis with `syn` alone**: local type inference is unavoidable. Since function signatures require annotations, forward inference of local variable and literal types from signatures can be built small. Using rustc's type information (rust-analyzer or `rustc_public`) is more precise but heavy in dependencies and maintenance, and judged excessive for v0's scope.
- **The plan to extend syntax first (old P3)**: adding `?` or iterators without type inference increases semantic holes. Swap the order (§4).
- **The reference for equivalence being implicit**: state explicitly whether behavior matches debug or release (§1.3).

## 3. Expected demand

<!-- constrained-by ./07-authored-constraints.md -->

On 2026-09-28, the target was narrowed to "code newly written within PureCrate's constraints" (§5). The question of this section is accordingly not whether existing Rust passes, but **whether there are users who will pay the cost of writing domain transitions once, in constrained Rust**. The alternatives compared are: calling idiomatic Rust via WASM (B), dual implementation in Rust and TS (A), and writing only in TS and running JS on the server too.

### 3.1 Observable signals (as of 2026-09, crates.io)

| Crate | Total DL | Recent DL | Nature |
| --- | --- | --- | --- |
| ts-rs | 15.93M | 6.2M | Rust → TS type generation |
| tsify | 9.75M | 1.76M | Type generation for wasm-bindgen |
| specta | 2.61M | 1.23M | Type introspection and TS output (tauri-specta, rspc) |

Caveats in reading these:

- This is demand for **sharing types**, not for sharing behavior.
- Download counts include transitive dependencies (e.g. via tauri-specta) and are not user counts.
- Still, they show that the population motivated by "keep TS aligned with Rust as the source of truth" is large.

Signals for sharing behavior:

- **Crux** (Red Badger): shares a side-effect-free Rust core across iOS, Android, and Web. On the Web it connects via WASM and bincode. Centering on transition functions is the same structure as this approach; only the means differ.
- **The zod_gen case**: a report of eliminating triple maintenance of Rust structs, TS interfaces, and Zod schemas (1,300 lines deleted). Dual maintenance of types and validation schemas is a real pain point.
- **r/rust "I want to share validation between backend and frontend"**: answers split between compiling to WASM and dual implementation, with some saying "we translate with an LLM". There is no definitive solution.
- **jsonstat/validator**: runs parity tests in CI checking three implementations (TS, Rust, WASM) against a shared corpus. A real example of the maintenance cost of choosing dual implementation.
- **Direct competitors**: no mature tool translating Rust→TS down to function bodies was found. tamusjroyce/rust-to-ts is a PoC whose policy is to leave unsupported syntax as comments (partial generation), the opposite design decision from this approach.

### 3.2 Use cases and whether they can be written within the constraints

"Can be written" is judged by the capabilities in design/07, not by whether existing code passes as-is.

| Use case | Strength of demand (estimated) | Writable within the constraints | Cost paid by the author |
| --- | --- | --- | --- |
| Workflows and state machines (order state, approval flows) | Medium | **High** (examples/order) | No `_ =>`; write arms for the product of states and events |
| Optimistic UI and offline-first (apply the same transition on the client before the server) | Medium to high | High | Write growing/shrinking sequences as recursive enums. `Vec` cannot grow |
| Turn-based game rules (server-authoritative + client prediction) | Medium | Medium | Write loops as recursion. When the board becomes deep recursion, it hits the recursion depth limit (§1.3.1) |
| Sharing input validation | **High** (the most signals) | **Medium** (examples/signup; design/08 §5.2) | Read strings as `as_bytes` byte sequences with indexing and recursion. WHATWG email took 2.8× the lines of idiomatic Rust. No regular expressions. Closed types (§1.6) prevent bypassing checked constructors |
| Pricing, fees, tax calculation | Medium | High | Write amounts as integer newtypes in the smallest unit (`struct Yen(i64)`). No decimal type |
| Identical decisions in edge functions | Low to medium | High | None (possible with WASM too) |

Input validation, where demand is most visible, is the least writable within the constraints. Conversely, the writable state machines have only indirect demand signals (the existence of Crux, kamae-style design). **Demand and capability are misaligned**, and this is the project's biggest strategic risk. Targeting newly written code does not remove this misalignment; it just adds one more cost for the author to pay.

### 3.3 Expected users

- Small to mid-sized teams with a Rust backend and a TS frontend (Web or React Native)
- Already sharing types with ts-rs or similar, and next struggling with duplicated logic
- Wanting to avoid the complexity of WASM builds, initialization, and bundling

Conversely, Tauri apps can call Rust directly over IPC, so the need there is low. It is limited to cases wanting synchronous decisions on the frontend alone (e.g. instant form validation).

### 3.4 Weaknesses of the demand hypothesis

- The size of the "want to share behavior but avoid WASM" segment has not been measured directly.
- Within that segment, those willing to rewrite the domain in constrained Rust are even fewer. Code written in idiomatic Rust does not pass, so it cannot be used to share existing Rust backend logic as-is.
- As RN's WASM support progresses, one reason to avoid WASM goes away.
- Implementing type inference, iterators, and strings raises the burden of maintaining a "small compiler" by a notch.

### 3.5 How to validate demand

1. **Write from third-party specifications** (items 1 and 2 of §1.4.2): write a published state-machine specification in constrained Rust, idiomatic Rust, and idiomatic TS, and measure the difference in effort. The acceptance rate of existing crates (design/06) is not used as an indicator of demand for this target.
2. **One real use**: replace one place in a state-machine system that is actually dual-implemented on the TS side, and record the lines removed and the scope guaranteed by tests.
3. **Material for a comparison article**: for the same transition function, measure bundle size, time to first call, and per-call cost for WASM (wasm-bindgen) versus this approach. Put numbers on the advantages in §2.2.

## 4. Recommended priorities

This section is the work order as of 2026-09-27. For the current overall picture and subsequent order, see [design/08](./08-limits-and-roadmap.md).

Put in the foundation supporting equivalence before the old plan's P3 (syntax extension).

1. **Numeric semantics and local type inference**: type expressions, emit integer `/` as truncating division and `%` as remainder. `i32` arithmetic throws when out of range (option 2 in §1.3). Add per-operator semantic tests to the differential tests.
2. **Infrastructure for measuring acceptance rate**: emit `check`'s rejection reasons in machine-readable form (JSON), for the aggregation in §3.5.
3. **Syntax for state machines**: `?`, `if let`, `Option` branching, local `mut`. (Done: TODO 22–24. Literal patterns in `match` and `while`/`for` are unsupported)
4. **After seeing the acceptance-rate results**: `Vec` iterators (`map`, `filter`, `fold` to array methods), string manipulation (handling the UTF-8 vs UTF-16 difference explicitly), generics (v1), `HashMap` (v1).
5. **Distribution**: `cargo install`, and instructions for integrating into npm scripts and CI (`check --out`).

## 5. Decisions and open questions

Decisions (2026-09-27):

- Operations that may panic are **reproduced**. The TS side throws under the same conditions, and the reference for equivalence is the Rust debug build (option 2 of §1.3).
- The center of demand is placed on **state machines and workflows**. Build `?`, `if let`, `Option`, and `mut` before strings and iterators. Input validation is reconsidered after seeing the acceptance-rate measurements.
- Strings reproduce UTF-8 byte units. `char` is a branded `string` of one code point; `usize` is a `number` checked up to 2^53−1. std methods are limited to an exact-match allow-list, and those depending on Unicode tables are accepted with a Unicode-version note (§1.5).
- The target is **code newly written within PureCrate's constraints**. The acceptance rate of existing crates is not a metric of success for this target. Evaluation is in design/07.
- **No decimal type.** Neither a TS `Decimal` runtime nor `rust_decimal` is accepted. Amounts are written as integer newtypes in the smallest unit (`struct Yen(i64)`).
- **State types do not hold mutable arrays** (2026-09-28). Transitions return the next state. Events are not accumulated in the state. Growing/shrinking sequences are returned as new values of recursive enums. `Vec<T>` is limited to reading sequences whose length is fixed externally (design/02 §1.1).
- **Numeric widths are distinguished by TS types** (2026-09-28). `i32` is `I32`, `f64` is `F64`. Both are `number` at runtime, but different brands keep them from mixing. The result of `+` falls to `number`, so `Int.i32.add` is used to return to the domain. `of` at the boundary checks integrality and range.
- **Closed types can only be created by public functions** (2026-09-29, §1.6). Structs with non-`pub` fields get a brand and do not expose `of`. The domain of equivalence for closed types is limited to values returned by public functions and wire reads. Validation failures are returned as the domain's `Err`, and wording is not shared.
- **rustc decides whether it compiles** (2026-09-28). After the subset check, `check` and `build` run the input through `rustc --crate-type lib --emit=metadata` and reject on errors. The subset check erases borrows and does not track moves or lifetimes, so on its own it does not reject the same range as rustc. rustc's type information is not read. The typing that determines output remains the in-house inference.

Open:

- Whether to switch the typing that determines output from the in-house inference to rustc's type information. Currently rustc is used only for pass/fail.

## References

- React Native 0.84 release notes (Hermes V1 made default): <https://reactnative.dev/blog/2026/02/11/react-native-0.84>
- Discussion of Hermes WASM support and wasm-bindgen compatibility: <https://github.com/paritytech/verifiablejs/issues/25>
- WASM boundary cost: <https://rs4ts.dev/19-wasm/09-performance/>, <https://github.com/rustwasm/wasm-bindgen/issues/2355>
- Crux: <https://github.com/redbadger/crux>, React shell: <https://redbadger.github.io/crux/part-2/shell/react.html>
- crates.io: <https://crates.io/crates/ts-rs>, <https://crates.io/crates/tsify>, <https://crates.io/crates/specta>
- The zod_gen case: <https://kamil.chm.ski/bridging-rust-and-typescript-with-zod_gen>
- Discussion on sharing validation: <https://www.reddit.com/r/rust/comments/1m0x8k2/share_validation_schemas_between_backend_and/>
- Parity tests across three implementations: <https://github.com/jsonstat/validator>
- Partial-translation PoC: <https://github.com/tamusjroyce/rust-to-ts>
- Gleam multi-target: <https://gleam.run/news/v0.34-multi-target-projects/>
