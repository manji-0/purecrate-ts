# Equivalence

Status: current (2026-09-30)

<!-- derived-from ./00-overview.md#1-claim -->

## 1. Definition

For an accepted public function `f` and its generated counterpart `f'`, and for every Rust value `x` in its domain with TS representation `x'`:

| Aspect | Requirement |
| --- | --- |
| Return value | `f(x)` and `f'(x')` are equal as values |
| Types | TS types represent the Rust types exactly (`i64` → `bigint`, width brands) and pass `tsc --strict` |
| Expected failure | `Err` has the same variant and the same payload |
| Unexpected failure | Where Rust panics, TS throws (same message for arithmetic) |

**The reference is the Rust debug build.** Overflow and division by zero panic there, so they throw in TS. Release-mode wrapping is not matched. This was chosen over rejecting every operation that may panic (too narrow) and over accepting only `checked_*` APIs (too awkward), and it agrees with kamae's "the unexpected is an exception".

## 2. Domain

The domain is **the image of Rust values under the TS representation**, not every value TS can construct. For `x'` outside the image nothing is promised.

| Outside the image | Why |
| --- | --- |
| `5 as I32`, `"x" as Email`, out-of-range or non-integer `number`s | Brands exist only in types. Values entering from outside go through `Int.i32.of` or a wire schema |
| Values of closed types not returned by public functions | See §4 |
| Strings with lone surrogates | Not a Rust `String`. Checking at the boundary is the codec's job |
| Objects mutated after casting away `Readonly` | The output is not `Object.freeze`d; mutation is visible through aliases that Rust would not share |
| Reading the original argument after the call | Rust moved it; TS still has it. Not promised either way |

## 3. Known non-equivalences

| Gap | Detail |
| --- | --- |
| `usize` ≥ 2^53 | Rust is fine up to 2^64; TS throws above 2^53−1. Lengths and indices do not reach this range. `bigint` was rejected because it does not mix with arrays and loops. The integer methods work in Rust's 64 bits on `usize` (`checked_add` is `None` only past 2^64, `wrapping_sub(0, 1)` is 2^64−1) and throw where that result is above 2^53−1 |
| Recursion depth | Measured 2026-09-28 (list length, Node 24.21, macOS): TS passes 10,000 levels and throws `RangeError` at 12,000. Rust debug passes 50,000 on the main thread and **aborts** at 100,000 (not a catchable panic; test threads have 2 MB). Even "both fail" does not hold |
| JSON nesting depth | serde_json rejects nesting deeper than 128; the wire schemas have no limit |
| Release wrapping | Not matched |
| Non-finite `f64` over JSON | serde_json and `toJson` both write `NaN` and infinities as `null`, and neither reads `null` back as a float. Same bytes, same asymmetry ([04 §6](./04-wire.md#6-writing-domain-values)) |
| Unicode-table methods | If added, equivalence holds only for code points assigned in both toolchains' Unicode versions (both 17.0 as of 2026-09-27) |

## 4. Closed types

<!-- derived-from #2-domain -->

In Rust, a struct with any non-`pub` field cannot be built by a literal outside its crate; values come only from public functions. That rule is carried over. Decided and implemented 2026-09-29; verified by `crates/cli/tests/it/closed_equivalence.rs`.

| Rust struct | Kind | TS companion | Values come from |
| --- | --- | --- | --- |
| Any non-`pub` field (`pub(crate)` and `pub(super)` count as non-`pub`), including newtypes | **closed**, with a `unique symbol` brand | no `of` | public functions, or the wire (§4.2) |
| All fields `pub` | **open** | `of` | anyone, as in Rust |

Which function is the "checked constructor" is not inferred. Whatever public function returns the type is the way in, whether named `new`, `parse`, or `try_from`.

### 4.1 Internal names

- **`Email$of`.** The generator builds values of a closed type through an internal `Email$of`. It is exported from the type's file but not from `index.ts`; `exports` exposes only the index, so deep imports cannot reach it. `$` cannot appear in Rust identifiers, so the name cannot collide.
- **Non-`pub` methods** are not on the companion either. They are emitted as `Email$unchecked`, exported from the file but not from `index.ts`, like `$of`. History: until 2026-09-30 every method sat on the exported companion, so a private `fn unchecked(raw) -> Email` let TS callers build what Rust callers cannot (found measuring Windmill's MCP scope, [91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)).

### 4.2 The domain of a closed type

Values returned by public functions, or read from the wire. The wire reads by shape as serde's derive does, or, with `#[serde(try_from = "T")]`, through the checked constructor ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). Rust has the same set, so correspondence is kept.

### 4.3 Not closed at runtime

`as` still works, fields are readable, and objects are not frozen. Classes with `#private` fields would close it, but break idiomatic values, JSON, and structured cloning.

### 4.4 Validation layers

| Layer | Question | Owner |
| --- | --- | --- |
| **Shape** | Is this JSON an `Order`? | the wire schema |
| **Meaning** | Is this an `Email`? | shared, via closed types and checked constructors |
| **Screen** | Wording, i18n, which field | the caller |

Error messages are therefore not shared; the `Err` variant and payload are. A common error type with field paths will be introduced only when the same field-assignment table is duplicated across examples.

## 5. Numbers

`check::accept` infers a type for every expression and rewrites arithmetic before printing.

| Rust | Generated TS | Details |
| --- | --- | --- |
| integer `+ - * / %`, unary `-` | `Int.<ty>.add(a, b)` etc. | §5.2 |
| integer `& \| ^`, `!`, `<< >>` | `Int.<ty>.and(a, b)`, `or`, `xor`, `not`, `shl`, `shr` | §5.3 |
| `f32` arithmetic | `Math.fround(a op b)`. `f32` literals are rounded once from decimal by the converter (not via `f64`) | |
| `f64` arithmetic | JS operators | |
| `i64` / `u64` literals | `5n` | |
| widening `i64::from(x)` | value unchanged; `BigInt(x)` when crossing to `bigint` | |

Verified by per-operator differential tests: 59 cases, floats included (`arith_equivalence.rs`); bitwise operators and shifts in §5.3.

### 5.1 Type inference

Inference is bidirectional and closed within the expression tree. It does not use rustc's later-use inference or the `i32`/`f64` defaults, so an untyped literal is rejected with a request for a suffix or annotation. Comparisons whose result differs between JS and Rust (struct/enum `==`, `String` ordering) are rejected.

### 5.2 Integer arithmetic

- **Rule.** `/` truncates. Overflow and division by zero throw Rust's panic message. `-0` is normalized to `0`.
- **History.** Before this was built, the output computed `i32` `7 / 2` as `3.5`, `i32::MAX + 1` as `2147483648`, and `1 / 0` as `Infinity` (measured 2026-09-27). The cause was that the IR had no types.

### 5.3 Bitwise operators and shifts

Added 2026-09-30.

- **Results wrap to the width** and never panic: JS int32 operators, then sign- or zero-extension; `BigInt.asIntN`/`asUintN` for 64 bits.
- **Shift amounts.** An amount of any integer type outside `0..bits`, compared as its whole value (`-1`, `2^32 + 1`), throws `attempt to shift left with overflow`, as a debug build panics. An unsuffixed amount is `i32`, as in rustc.
- **Refused.** `usize`: Rust gives it 64 bits, the TS `number` 53. `& | ^` on `bool`, in favor of `&& || !=`.
- **Verified by** `bits_equivalence.rs`: every width, operator, and amount type, boundary values, compound assignment, RFC 4226 truncation.

## 6. Strings, `char`, `usize`, std methods

<!-- derived-from #3-known-non-equivalences -->

Decided 2026-09-27 and implemented one method at a time as examples asked ([07 §3.1](./07-roadmap.md#31-when-an-example-needs-it)). Two rules hold throughout:

- **Allow-list, exact match.** A method is accepted by (receiver type, method), each with its own differential test over empty, non-ASCII, supplementary-plane, boundary, and panicking inputs. A method that cannot be matched is rejected, not accepted with a documented difference: `f64::to_string` (Rust `1000000000000000000000`, JS `1e+21`) and `str::trim` (JS `trim()` also strips U+FEFF) are out.
- **Well-formed strings only.** Rust strings are UTF-8 and JS strings UTF-16. Every equivalence below relies on a string holding only whole scalar values, which a Rust `String` always does; a TS string with lone surrogates is outside the domain (§2).

| Receiver | Accepted | TS | Verified by |
| --- | --- | --- | --- |
| `String`, `&str` | `len`, `is_empty`, `starts_with` / `ends_with` / `contains` / `strip_prefix` / `strip_suffix` (a `&str` needle), `as_bytes`, `as_str`, slicing `&s[a..b]`, `==`, literal patterns | `Str.len`, `length === 0`, `startsWith` / `endsWith` / `includes` / `Str.stripPrefix` / `Str.stripSuffix`, `Str.bytes`, `Str.slice`, the string, `===` | `strings_equivalence.rs`, `str_methods_equivalence.rs`, `str_patterns_equivalence.rs`, `slicing_equivalence.rs` |
| a string in a `for` head | `chars()`, `bytes()`, `split(c)` with a `char` | `for..of` over `s`, `Str.bytes(s)`, `s.split(c)` | `for_chars_equivalence.rs`, `for_each_equivalence.rs` |
| `char` | literals, `==`, `<`, ranges; `u32::from`, `u64::from`, `char::from(u8)`, `char::from_u32`; `is_ascii*`, `to_ascii_{upper,lower}case`, `eq_ignore_ascii_case`, `len_utf8`, `is_digit` / `to_digit` | `Char` (branded `string`), compared through `Char.code` | `chars_equivalence.rs` |
| `uuid::Uuid` | `Uuid::parse_str`, `try_parse`, `nil`, `==`, `<` | `Uuid` (branded canonical `string`), `===`, `<` | `uuid_equivalence.rs` |
| `Vec`, slices, `as_bytes()` | indexing, `len`, `is_empty`, slicing `&xs[a..b]` | `xs[i]` behind a bounds check with Rust's panic message, `length`, `length === 0`, `slice` behind Rust's checks | `std_methods_equivalence.rs`, `slicing_equivalence.rs` |
| `Option` | `is_some`, `is_none`; `unwrap_or`, `ok_or`, `map` (§7) | `!== null`, `=== null` | `std_methods_equivalence.rs`, `option_methods_equivalence.rs` |
| integers | `min`, `max`, `abs`, `pow`, `checked_*`, `saturating_*`, `wrapping_*` (§7) | `Int.<ty>.min` etc. | `int_methods_equivalence.rs` |

`wire.rs` and `wire_write.rs` cover the JSON forms of `char` and `Uuid` against serde_json through each schema library.

### 6.1 Strings

The TS value is a plain `string`; operations that depend on the encoding go through the runtime `Str`, and reproduce UTF-8 byte units: `len` counts UTF-8 bytes, and `bytes` / `as_bytes` yield `u8`s.

What prints as the plain JS operation, and why that is the same:

- **Prefix, suffix, substring.** On well-formed strings a match on char boundaries is the same in UTF-8 bytes and UTF-16 units, so `is_empty`, `starts_with`, `ends_with`, `contains` need no encoding step. `char` and closure needles are rejected (tested: every pairing of 14 strings from empty to U+10FFFF).
- **Equality and literal patterns.** Well-formed strings are equal as UTF-8 exactly when they are as UTF-16, so `==` and string literal patterns print as `===`; `String::as_str` prints as the string itself (tested: 16 strings including the literals, a shared prefix, NFC vs NFD, U+FFFF, and a supplementary-plane neighbor).
- **`for c in s.chars()`** (2026-09-30). A JS string iterates by code point, which for well-formed strings is Rust's sequence of scalar values, so it prints as `for (const c of s)` (tested: 14 strings across every UTF-8 length, the surrogate gap and U+E000, with `?`, early `return`, nesting, closures, and overflow in the body). `chars()` as a value and its adaptors are rejected.
- **`for t in s.split(c)`** with a `char` separator (2026-09-30) prints as `s.split(c)`. One code point occurs at the same places of a well-formed string in UTF-8 and UTF-16 (a supplementary one is a surrogate pair that appears nowhere else), and JS `split` keeps the empty pieces Rust keeps: leading, trailing, and between adjacent separators (tested: ten strings against separators of every UTF-8 length). A `&str` separator is refused, since an empty one splits differently (`"ab".split("")` is `["", "a", "b", ""]` in Rust, `["a", "b"]` in JS), and `split` as a value stays off the list.

What goes through the runtime, and what it keeps:

- **Slicing** `&s[a..b]`, `&s[a..]`, `&s[..b]` (2026-09-30) goes through `Str.slice`, which takes UTF-8 byte positions. It panics as Rust does and in Rust's order: a start past the end, an end past the end, a reversed range, then a start or end inside a character. That last message shows the character as `Debug` does, escaping a grapheme extender or a code point of category Zs (but space), Zl, Zp, Cc, Cf, Cs, Co, or Cn as `\u{..}`; the runtime tests these with the engine's Unicode properties, which agree with Rust's tables where both use one Unicode version (§3). `&xs[a..b]` on a `Vec` or slice checks the same order with Rust's slice messages (tested: seven strings covering every UTF-8 width and each escaped category, every start and end up to one past the length, nested slices, `as_bytes()`, and `Vec`s).
- **`strip_prefix` / `strip_suffix`** with a `&str` return `Option<&str>`. A prefix or suffix of a well-formed string on char boundaries has the same extent in UTF-8 and UTF-16, so the rest is the same string.

Specified, not yet implemented ([07 §4](./07-roadmap.md#4-specified-but-not-yet-implemented)): `String` ordering, which would compare code points, since JS `<` orders U+E000–U+FFFF above the supplementary planes and Rust does not.

### 6.2 `char`

Implemented 2026-09-29.

- **Representation.** A branded one-code-point `string` (`Char`). The brand also excludes lone surrogates, as a Rust `char` is a Unicode scalar value. JSON matches serde: a string of exactly one scalar value, anything else a schema failure.
- **Ordering.** Comparisons and range patterns use `Char.code` (`codePointAt(0)`), never the strings, for the UTF-16 order given above.
- **Conversions** are std's `From` only; `char::from_u32(n)` is `None` for a surrogate or past U+10FFFF, and `as` stays rejected.
- **Methods** are the ASCII and code-point ones, which need no Unicode table; `to_digit` and `is_digit` panic outside radix 2..=36 with Rust's message. The table-driven ones (`is_alphabetic`, `is_whitespace`, …) and `to_uppercase` (an iterator) are rejected; see §6.5.

Tested on 40 characters across every UTF-8 length boundary and the surrogate gap, all pairs for ordering, radixes 0–37, every `u8`, and `from_u32` around the gap.

### 6.3 `uuid::Uuid`

Implemented 2026-09-30, from the Oxide `Name` rule, which had to spell `Uuid::parse_str` by hand ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). Verified against `uuid` 1.26.1.

- **Representation.** A branded `string` that only ever holds the lowercase hyphenated form (8-4-4-4-12), the one serde writes. In that form `===` is Rust's `==`, and JS string order is the order of the 16 bytes, since the hyphens sit at the same places in every value, so `==` and `<` print as themselves.
- **Parsing.** `parse_str` and `try_parse` accept what the crate's parser does, chosen by UTF-8 length as it chooses: 32 hex digits, hyphenated (36), braced (38), and `urn:uuid:` (45, the prefix in lowercase only), hex in either case; the result is the canonical form. The TS parser measures UTF-16 units, but a non-ASCII character fails the hex check at either length, so the accepted sets are equal. `uuid::Error` is opaque: nothing translated can read or compare it.
- **Rejected.** Generating (`new_v4`), `to_string`, and byte access. `nil()` is the only constructor besides parsing.
- **JSON** is serde's: read from any string `parse_str` accepts, written canonical.

Tested on about 3,400 strings (every form the crate prints in three cases, each with one-character edits, cuts, extensions, and wrappings, and non-ASCII characters at the length boundaries) and all pairs of 45 UUIDs for ordering.

### 6.4 `usize`

A `number` checked to 0..2^53−1; the gap above that is in §3.

### 6.5 Other methods

- **`Vec` and slices** (2026-09-30): `len`, and `is_empty` as `.length === 0`, on values, borrows, fields, slices, and `as_bytes()` results.
- **`Option::is_some` / `is_none`** (2026-09-30) print as `!== null` / `=== null`. This holds because `Option<T>` is `T | null` with nested `Option` rejected, so a falsy payload (`0`, `0n`, `false`, `""`) is still `Some`.
- **Unicode-table methods** (`to_uppercase`, `is_alphabetic`, …) are not accepted. If added, they carry the version gap of §3, and their differential tests first check that both toolchains' Unicode versions agree; `ß` → `SS` and final sigma were measured to match.

## 7. Rewritten constructs

Some accepted Rust has no one-to-one TS form. It is rewritten into constructs that are already equivalent, and the rewrite keeps Rust's evaluation order.

| Construct | Becomes | Verified by |
| --- | --- | --- |
| `Option::unwrap_or`, `ok_or`, `map` | the `match` std writes | `option_methods_equivalence.rs` |
| match guards | an `if` chain over standalone matches | `guards_equivalence.rs` |
| `vec![a, b]` | the array literal | `vec_build_equivalence.rs` |
| tuple patterns in `let`, closure parameters, `for` | a one-arm tuple `match` | `destructure_equivalence.rs` |
| `all`, `any`, `position`, `count`, `sum`; `for` over `.enumerate()` | the loop std runs | `consumers_equivalence.rs` |
| `const`, enum discriminants | the folded value | `flags_equivalence.rs`, `consts.rs` in `check` |
| `const` in a block | a `let` at the top of the block | `local_consts_equivalence.rs` |
| integer methods | the exact result, then checked, clamped, or wrapped | `int_methods_equivalence.rs` |

- **`Option::unwrap_or`, `ok_or`, `map`** (2026-09-30). The receiver is bound once. `unwrap_or(d)` and `ok_or(e)` evaluate their argument before the `match`, whether or not the option is `Some`, as Rust does; JS `??` would skip it, and a `d` that overflows would then not panic. `map(f)` runs `f` only on `Some`. A closure passed to `map` may not use `?` or `return`, which would leave the enclosing function once inlined.
- **Match guards** (2026-09-30) are rewritten before checking. For each arm in order, `match $g { p => guard, _ => false }` tests it and `match $g { p => body, _ => unreachable }` takes it, with the scrutinee (each element of a tuple one) bound once to `$g`. A guard therefore runs only when its pattern matched, as in Rust; the tests include a guard that overflows for other variants.
- **Scalar consumers** (2026-09-30). `all`, `any`, `position`, `count`, and `sum` on `s.chars()`, `s.bytes()`, and `xs.iter()` become the loop std's default methods run: the source is bound once, the closure's body is inlined with its parameter bound to each item, and the loop stops where std's does (`all` at the first `false`, `any` and `position` at the first `true`), so a predicate that would overflow on a later item does not run on it. Inlining is why the closure may not use `?` or `return`. `position` counts items (chars, not bytes, on `chars()`). `sum` adds from zero left to right with the checked operator, so it panics on overflow where a debug build does; it is refused on floats, whose `Sum` starts from `-0.0`. `for (i, x) in ...enumerate()` reads a `usize` counter into `i` and advances it at the top of each pass, so `continue` keeps it right. Tested on strings of every width, early stops before an overflow, `sum` overflowing either way, and `continue` in an enumerated loop.
- **Tuple patterns** (2026-09-30) in `let`, a closure parameter, or a `for` variable become `match value { (a, b) => rest }`, which Rust's irrefutable pattern is: the value is evaluated once (a tuple expression left to right, so the first overflow panics), then each element is bound. A `mut` element binds a fresh name that the body rebinds with `let mut`. Tested with `mut` and `_` elements, an annotation, overflow in either element, `&(a, b)` over `iter()`, and `break` / `continue` in the loop.
- **`vec![a, b]`** (2026-09-30) prints as `[a, b]`, elements evaluated left to right as in Rust. The `Vec` it builds is never mutated (no `push`, no index assignment), so sharing the element values with the array is unobservable. The element type comes from context like any literal's; `vec![x; n]` is rejected. Tested with element types from the return type and a struct field, `vec![]`, nesting, `Option` elements, and the first of several overflows reported.
- **`const`** (2026-09-30) is folded by `check` with rustc's const-evaluation rules (checked arithmetic, wrapped shifts with the amount checked), and the TS holds the value, not the computation. rustc rejects a const that overflows, so there is nothing to throw at run time, and module load order cannot matter. A const in a pattern is refused: Rust compares with its value, where the IR would bind a new name that matches anything.
- **`const` in a block** (2026-09-30) is not folded: it becomes an immutable, typed `let` at the top of its block, in declaration order, so it is visible from the whole block as an item is. The value is computed at run time rather than folded, which is the same value: rustc, which runs on the input, rejects a const whose evaluation overflows, so the computation cannot panic where Rust would have folded one. A local const in a pattern is refused like a crate one, and so is a `let` of its name (rustc reads that `let` as a pattern too). Tested with a use before the declaration, a crate const and a discriminant in the value, an unused and a float const, one named like a crate item, one inside a `match` arm, and overflow computed from one.
- **Integer methods** (2026-09-30). `x.min(y)`, `max`, `abs`, `pow`, and the `checked_*`, `saturating_*`, and `wrapping_*` forms are computed from the exact result in `bigint`: `checked_*` gives it or `None` outside the type's range (and on a zero divisor, and on `MIN / -1`), `saturating_*` clamps it, `wrapping_*` keeps its low bits, and `abs` and `pow` panic outside the range as a debug build does ("attempt to negate with overflow", "attempt to exponentiate with overflow", measured on 1.98.1). An exponent is a `u32`; a power is not formed when its magnitude is certainly past the range (a base of magnitude two or more to an exponent of the type's width or more), and `wrapping_pow` works modulo 2^bits. `saturating_pow` of a negative base to an odd exponent saturates to `MIN`, as std does. Tested at every type's edges, `(-2).pow(31)` and `i8` powers near 127 included.
- **Discriminants** of a fieldless enum (explicit, or one past the previous; the first `0`) are folded the same way, in the `#[repr]` type or `isize` (64 bits). `e as T` is accepted only when `T` holds every discriminant, so the cast never truncates or wraps; it prints as a table indexed by `kind` (a constant variant folds to the literal). Every other `as` stays rejected.

## 8. Verification

| Check | Mechanism |
| --- | --- |
| Values | Differential tests: same inputs through Rust and the generated TS on Node, results compared as canonical strings |
| Types | `tsc --strict` on TypeScript 6 and 7 for every fixture, the runtime packages, and the wire schemas |
| Wire | Readers against serde's default JSON; `toJson` against the vendored serde_json byte for byte (`wire_write.rs`) |
| Determinism | `check --out` compares bytes with the existing output |
| Compilability | rustc must accept the input ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate)) |
| Rejection quality | Every rejection has `path:line:col` and a reason code, and nothing is written |

### 8.1 Canonical form

Since 2026-09-29, both sides render values from the same IR types.

- **Rust side.** The test-only proc-macro `purecrate_canon::fixture!` derives `Show` for every fixture type; containers and scalars live in `crates/cli/tests/it/support`.
- **TS side.** The harness generates a printer per type.
- **Form.** It resembles `Debug` (`Order::Placed { lines: Lines::Cons(…), total: Yen(450) }`). Floats are written as their `f64` bit pattern, non-printable-ASCII as `\u{…}`.
- **Why.** Previously tests compared hand-picked projections, and a deliberately injected swap of `expected` and `got` in `OrderError::AmountMismatch` went unnoticed. It is now caught.

### 8.2 Whole-value arguments

Since 2026-09-29, arguments are whole values too: `fixture!` also derives `Js`, the TS literal of each fixture type, so a case can pass an `Invoice` built in Rust.

Remaining projections:

- Cases routed through a driver returning a scalar compare only what the driver reads (examples/order adds `trace4` to compare the whole final state).
- The counter acceptance test compares only `State.n`.

### 8.3 Coverage

All of this is measured on examples, so equivalence is as strong as the examples' coverage. Where exhaustive enumeration is small, it is used:

- all 1296 sequences for control and vending;
- all 4-step sequences for order and, under each capture and confirmation method, for payment.
