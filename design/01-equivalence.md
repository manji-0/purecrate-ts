# Equivalence

Status: current (2026-10-05, 0.13.0)

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

The main use of the output is a server that shares the same types and functions. That server is built with `--release`, where `overflow-checks` is off by default, so on overflow Rust wraps while TS throws: the two sides disagree on exactly the inputs where it matters. Equivalence with the server holds only when the crate (or workspace) sets `[profile.release] overflow-checks = true` (and `debug-assertions` if anything depends on them). `check` and `build` warn when the release profile does not.

## 2. Domain

The domain is **the image of Rust values under the TS representation**, not every value TS can construct. For `x'` outside the image nothing is promised.

| Outside the image | Why |
| --- | --- |
| `5 as I32`, `"x" as Email`, out-of-range or non-integer `number`s | Brands exist only in types. Values entering from outside go through `Int.i32.of` or a wire schema |
| Values of closed types not returned by public functions | See §4 |
| Strings with lone surrogates | Not a Rust `String`. A `pub` function panics on one in a string parameter (§6), and a wire schema refuses one |
| Objects mutated in place | The output is not `Object.freeze`d, and `Readonly` stops only assignment through the type: `Object.assign(Version.preRelease(v), [..])` or `Object.assign(rule, { interval: .. })` compiles with no cast. Any mutation is visible through aliases that Rust would not share (an accessor returns the value it holds, not a copy, and a caller's array passed in is the one kept), and can break a closed type's invariant |
| Reading the original argument after the call | Rust moved it; TS still has it. Not promised either way |

## 3. Known non-equivalences

| Gap | Detail |
| --- | --- |
| `usize` ≥ 2^53 | Rust is fine up to 2^64; TS throws above 2^53−1. Lengths and indices do not reach this range. `bigint` was rejected because it does not mix with arrays and loops. The integer methods work in Rust's 64 bits on `usize` (`checked_add` is `None` only past 2^64, `wrapping_sub(0, 1)` is 2^64−1) and throw where that result is above 2^53−1 |
| Recursion depth | Measured 2026-09-28 (list length, Node 24.21, macOS): TS passes 10,000 levels and throws `RangeError` at 12,000. Rust debug passes 50,000 on the main thread and **aborts** at 100,000 (not a catchable panic; test threads have 2 MB). Even "both fail" does not hold |
| JSON nesting depth | serde_json rejects nesting deeper than 128; the wire schemas have no limit |
| Release wrapping | Not matched, unless `[profile.release] overflow-checks = true` (the server's usual `--release` build wraps; generated TS always panics). `check` warns when the crate's release profile leaves the default |
| Non-finite `f64` over JSON | serde_json and `toJson` both write `NaN` and infinities as `null`, and neither reads `null` back as a float. Same bytes, same asymmetry ([04 §6](./04-wire.md#6-reading-and-writing-text)) |
| `u128` literals ≥ 2^127 | Literals and folded constants are held as `i128`, so a `u128` literal at 2^127 or above is refused (build it as `!0`, or from shifts); values of the type reach 2^128−1 at run time |
| Unicode-table methods | If added, equivalence holds only for code points assigned in both toolchains' Unicode versions (both 17.0 as of 2026-09-27) |

## 4. Closed types

<!-- derived-from #2-domain -->

In Rust, a struct with any non-`pub` field cannot be built by a literal outside its crate; values come only from public functions. That rule is carried over. Verified by `crates/cli/tests/it/closed_equivalence.rs`.

| Rust struct | Kind | TS companion | Values come from |
| --- | --- | --- | --- |
| Any non-`pub` field (`pub(crate)` and `pub(super)` count as non-`pub`), including newtypes | **closed**, branded by a declaration its own file makes and does not export (a newtype by a `unique symbol` key, `{ readonly [Meters$brand]: true }`; a struct by a class with a `private` member, `Line$brand`, which an object spread does not copy), so no literal or spread outside the package can be one | no `of` | public functions, or the wire (§4.2) |
| All fields `pub` | **open** | `of` | anyone, as in Rust |

Which function is the "checked constructor" is not inferred. Whatever public function returns the type is the way in, whether named `new`, `parse`, or `try_from`.

### 4.1 Internal names

- **`unsafeMakeEmail`.** The generator builds values of a closed type through an internal `unsafeMakeEmail`, marked `@internal` (and left out of the `.d.ts` by `stripInternal`). It is exported from the type's file but not from `index.ts`; `exports` exposes only the index, so an installed package cannot deep-import it. When the sources are vendored, `import { unsafeMakeEmail } from "./gen/src/email.ts"` reaches it and builds a value without a cast. Closedness is that convention plus the consumer's `as` lint ([03 §5.5](./03-output.md#55-closed-types)), not a type-level seal. The name is checked against the crate's top-level names and numbered where one has it (`unsafeMakeEmail2` beside a `fn unsafe_make_email`). Until 0.8.0 it was `Email$of`.
- **Non-`pub` methods** are not on the companion either. They are emitted as `emailUnchecked`, `@internal`, exported from the file but not from `index.ts`, like `unsafeMakeEmail` and named the same way (until 0.8.0, `Email$unchecked`). Otherwise a private `fn unchecked(raw) -> Email` would let TS callers build what Rust callers cannot (a hole found by real use, [07 §8.1](./07-roadmap.md#81-010-2026-09-30)).

### 4.2 The domain of a closed type

Values returned by public functions, or read from the wire. The wire reads by shape as serde's derive does, or, with `#[serde(try_from = "T")]`, through the checked constructor ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). Rust has the same set, so correspondence is kept.

### 4.3 Not closed at runtime

`as` still works, fields are readable, and objects are not frozen, so `Object.assign` changes a closed value's fields with no cast (the brand keeps out a literal or a spread, not mutation in place). Freezing at construction would be shallow (an array or a nested value stays open) and cost every hot path; classes with `#private` fields would close it, but break idiomatic values, JSON, and structured cloning.

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
| `i64` / `u64` / `i128` / `u128` literals | `5n` | 128-bit integers are `bigint`s as 64-bit ones are, with `Int.i128` / `Int.u128` checking 128 bits (`wide_int_equivalence.rs`) |
| widening `i64::from(x)` | value unchanged; `BigInt(x)` when crossing to `bigint` | |
| byte string `b".."` | `Str.bytes("..")` where the bytes are UTF-8 (the same bytes `"..".as_bytes()` gives), else an array of byte literals | as a `&[u8]` const or in an expression; `wide_int_equivalence.rs` |

Verified by per-operator differential tests: 59 cases, floats included (`arith_equivalence.rs`); bitwise operators and shifts in §5.3.

### 5.1 Type inference

Inference is bidirectional and closed within the expression tree. It does not use rustc's later-use inference or the `i32`/`f64` defaults, so an untyped literal is rejected with a request for a suffix or annotation. Comparisons whose result differs between JS and Rust (struct/enum `==`, ordering on `bool` or the crate's types) are rejected; `String` ordering goes through the runtime (§6.1).

### 5.2 Integer arithmetic

- **Rule.** `/` truncates. Overflow and division by zero throw Rust's panic message. `-0` is normalized to `0`.
- **Why typed.** Printed as JS operators, `i32` `7 / 2` would be `3.5`, `i32::MAX + 1` `2147483648`, and `1 / 0` `Infinity`, which is what the output computed before the IR had types.

### 5.3 Bitwise operators and shifts

- **Results wrap to the width** and never panic: JS int32 operators, then sign- or zero-extension; `BigInt.asIntN`/`asUintN` for 64 bits.
- **Shift amounts.** An amount of any integer type outside `0..bits`, compared as its whole value (`-1`, `2^32 + 1`), throws `attempt to shift left with overflow`, as a debug build panics. An unsuffixed amount is `i32`, as in rustc.
- **Refused.** `usize`: Rust gives it 64 bits, the TS `number` 53. `& | ^` on `bool`, in favor of `&& || !=`.
- **Verified by** `bits_equivalence.rs`: every width, operator, and amount type, boundary values, compound assignment, RFC 4226 truncation.

## 6. Strings, `char`, `usize`, std methods

<!-- derived-from #3-known-non-equivalences -->

Methods are added one at a time, as examples ask ([07 §1](./07-roadmap.md#1-how-additions-are-chosen)). Two rules hold throughout:

- **Allow-list, exact match.** A method is accepted by (receiver type, method), each with its own differential test over empty, non-ASCII, supplementary-plane, boundary, and panicking inputs. A method that cannot be matched is rejected, not accepted with a documented difference: `f64::to_string` (Rust `1000000000000000000000`, JS `1e+21`) and `str::trim` (JS `trim()` also strips U+FEFF) are out.
- **Well-formed strings only.** Rust strings are UTF-8 and JS strings UTF-16. Every equivalence below relies on a string holding only whole scalar values, which a Rust `String` always does; a TS string with lone surrogates is outside the domain (§2). A `pub` function checks each parameter that may hold a string on entry (`Str.wellFormed`, which walks the value) and panics on one, so no later step meets it: a `String` / `&str`, or one anywhere in an open struct, an enum, an `Option`, a `Vec`, or a tuple. A private helper is not checked again; a closed type's strings came through its own `pub` functions, checked there; a type with no string is not walked. A string a TS callback returns is not checked (tested: lead, trail, and reversed halves, in an `Option`, a `Vec`, a nested struct, a variant and its field, a tuple, and a constructor, against pairs and U+10FFFF, `lone_surrogate.rs`).

| Receiver | Accepted | TS | Verified by |
| --- | --- | --- | --- |
| `String`, `&str` | `len`, `is_empty`, `starts_with` / `ends_with` / `contains` / `strip_prefix` / `strip_suffix` (a `&str` needle), `split_once` (a `char` or a `&str`), `eq_ignore_ascii_case` (a `&str`), `as_bytes`, `as_str`, slicing `&s[a..b]`, `==`, `<` `<=` `>` `>=`, `cmp`, literal patterns | `Str.len`, `length === 0`, `startsWith` / `endsWith` / `includes` / `Str.stripPrefix` / `Str.stripSuffix` / `Str.splitOnce`, `Str.eqIgnoreAsciiCase`, `Str.bytes`, `Str.slice`, the string, `===`, `Str.cmp(a, b) < 0` etc. | `strings_equivalence.rs`, `str_methods_equivalence.rs`, `str_patterns_equivalence.rs`, `slicing_equivalence.rs`, `ordering_equivalence.rs`, `collect_equivalence.rs` |
| a string in a `for` head | `chars()`, `bytes()`, `split(c)` with a `char` | `for..of` over `s`, `Str.bytes(s)`, `s.split(c)` | `for_chars_equivalence.rs`, `for_each_equivalence.rs` |
| `char` | literals, `==`, `<`, `cmp`, ranges; `u32::from`, `u64::from`, `char::from(u8)`, `char::from_u32`; `is_ascii*`, `to_ascii_{upper,lower}case`, `eq_ignore_ascii_case`, `len_utf8`, `is_digit` / `to_digit` | `Char` (branded `string`), compared through `Char.code` | `chars_equivalence.rs`, `ordering_equivalence.rs` |
| `uuid::Uuid` | `Uuid::parse_str`, `try_parse`, `nil`, `==`, `<`, `cmp` | `Uuid` (branded canonical `string`), `===`, `<` | `uuid_equivalence.rs`, `ordering_equivalence.rs` |
| integers, `bool` | `cmp` (§6.6) | `Ord.cmp`, by JS `<` | `ordering_equivalence.rs` |
| `std::cmp::Ordering` | `Less` / `Equal` / `Greater`, `==`, `is_eq` … `is_ge`, `reverse`, `then`, `then_with` (§6.6, §7) | a fieldless enum | `ordering_equivalence.rs` |
| `Vec`, slices, `as_bytes()` | indexing, `len`, `is_empty`, slicing `&xs[a..b]`, `cmp`, `clone`, `iter()` / `into_iter()` through `map` / `filter` (§7.13); built as `vec![a, b]`, by `collect()` (§7.12, §7.13), or grown by `push` or `insert` and edited by `remove` or `v[i] = x` on a `let mut` local (§7.14) | `Slice.at(xs, i)`, a bounds check with Rust's panic message, `length`, `length === 0`, `Slice.range` with Rust's checks, `Ord.cmpList`, `[...xs]`; the array, its `.map` / `.filter`, `Array.from` of `Iter.map` / `Iter.filter`, or `Iter.tryCollect`; `v.push(x)`, `Slice.insert(v, i, x)`, `Slice.remove(v, i)`, and `Slice.set(v, i, x)` (Rust's panic past the end, then `splice` or the write) on an `Array<T>` | `std_methods_equivalence.rs`, `slicing_equivalence.rs`, `vec_build_equivalence.rs`, `collect_equivalence.rs`, `adapters_equivalence.rs`, `grow_equivalence.rs`, `vec_insert_equivalence.rs`, `vec_edit_equivalence.rs`, `ordering_equivalence.rs` |
| `Option` | `is_some`, `is_none`; `unwrap_or`, `ok_or`, `map` (§7); `clone`, `as_ref`, `as_deref` (on `Option<String>`) | `!== null`, `=== null`; the value itself for the last three | `std_methods_equivalence.rs`, `option_methods_equivalence.rs`, `grow_equivalence.rs` |
| `Result` | `ok`, `map`, `map_err` (§7.1) | the `match` std writes | `parse_equivalence.rs` |
| `String`, `&str` into an integer | `s.parse::<T>()` for every integer `T`, into `Result<T, ParseIntError>` (§6.7) | `Int.<t>.parse(s)` | `parse_equivalence.rs` |
| integers | `min`, `max`, `abs`, `pow`, `checked_*`, `saturating_*`, `wrapping_*` (§7) | `Int.<ty>.min` etc. | `int_methods_equivalence.rs` |

`wire.rs` and `wire_write.rs` cover the JSON forms of `char` and `Uuid` against serde_json through each schema library.

### 6.1 Strings

The TS value is a plain `string`; operations that depend on the encoding go through the runtime `Str`, and reproduce UTF-8 byte units: `len` counts UTF-8 bytes, and `bytes` / `as_bytes` yield `u8`s.

What prints as the plain JS operation, and why that is the same:

- **Prefix, suffix, substring.** On well-formed strings a match on char boundaries is the same in UTF-8 bytes and UTF-16 units, so `is_empty`, `starts_with`, `ends_with`, `contains` need no encoding step. `char` and closure needles are rejected (tested: every pairing of 14 strings from empty to U+10FFFF).
- **Equality and literal patterns.** Well-formed strings are equal as UTF-8 exactly when they are as UTF-16, so `==` and string literal patterns print as `===`; `String::as_str` prints as the string itself (tested: 16 strings including the literals, a shared prefix, NFC vs NFD, U+FFFF, and a supplementary-plane neighbor).
- **`for c in s.chars()`.** A JS string iterates by code point, which for well-formed strings is Rust's sequence of scalar values, so it prints as `for (const c of s)` (tested: 14 strings across every UTF-8 length, the surrogate gap and U+E000, with `?`, early `return`, nesting, closures, and overflow in the body). `chars()` as a value and its adaptors are rejected.
- **`for t in s.split(c)`** with a `char` separator prints as `s.split(c)`. One code point occurs at the same places of a well-formed string in UTF-8 and UTF-16 (a supplementary one is a surrogate pair that appears nowhere else), and JS `split` keeps the empty pieces Rust keeps: leading, trailing, and between adjacent separators (tested: ten strings against separators of every UTF-8 length). A `&str` separator is refused, since an empty one splits differently (`"ab".split("")` is `["", "a", "b", ""]` in Rust, `["a", "b"]` in JS), and `split` as a value stays off the list, except before a consumer or `collect` (§7.12).
- **`split_once`** with a `char` or a `&str` prints as `Str.splitOnce`, `Option<(&str, &str)>`. `indexOf` finds the first match, and the two slices are the same strings on a well-formed input, including an empty needle (`"ab".split_once("")` is `Some(("", "ab"))` on both sides). A needle that is not text is refused.
- **`eq_ignore_ascii_case`** with a `&str` prints as `Str.eqIgnoreAsciiCase`: equal lengths, and each UTF-16 unit equal once `A`..=`Z` are folded to lower case. Rust folds the same letters on UTF-8 bytes and leaves every other byte; folding maps ASCII to ASCII and nothing else, so two strings agree folded as bytes exactly when they agree folded as units. No Unicode case folding on either side (`"é".eq_ignore_ascii_case("É")` is `false`). Tested on 20 strings pairwise, `@` and `` ` `` beside the letters among them.

What goes through the runtime, and what it keeps:

- **Slicing** `&s[a..b]`, `&s[a..]`, `&s[..b]` goes through `Str.slice`, which takes UTF-8 byte positions. It panics as Rust does and in Rust's order: a start past the end, an end past the end, a reversed range, then a start or end inside a character. That last message shows the character as `Debug` does, escaping a grapheme extender or a code point of category Zs (but space), Zl, Zp, Cc, Cf, Cs, Co, or Cn as `\u{..}`; the runtime tests these with the engine's Unicode properties, which agree with Rust's tables where both use one Unicode version (§3). `&xs[a..b]` on a `Vec` or slice checks the same order with Rust's slice messages (tested: seven strings covering every UTF-8 width and each escaped category, every start and end up to one past the length, nested slices, `as_bytes()`, and `Vec`s).
- **`strip_prefix` / `strip_suffix`** with a `&str` return `Option<&str>`. A prefix or suffix of a well-formed string on char boundaries has the same extent in UTF-8 and UTF-16, so the rest is the same string.

- **Ordering** `<`, `<=`, `>`, `>=`, and `cmp` compare code points, which is Rust's order of UTF-8 bytes. JS `<` compares UTF-16 units and so orders U+E000–U+FFFF above the supplementary planes, which Rust orders last; the runtime's `Str.cmp` moves those units back below the surrogates at the first unit that differs, and a shorter prefix orders first (tested: all pairs of 33 strings across every UTF-8 width, U+FFFF and U+E000 against U+10000, shared prefixes, NFC vs NFD, `String` against `&str`; replacing the key with plain UTF-16 order fails the test).

### 6.2 `char`

- **Representation.** A branded one-code-point `string` (`Char`). The brand also excludes lone surrogates, as a Rust `char` is a Unicode scalar value. JSON matches serde: a string of exactly one scalar value, anything else a schema failure.
- **Ordering.** Comparisons and range patterns use `Char.code` (`codePointAt(0)`), never the strings, for the UTF-16 order given above.
- **Conversions** are std's `From` only; `char::from_u32(n)` is `None` for a surrogate or past U+10FFFF, and `as` stays rejected.
- **Methods** are the ASCII and code-point ones, which need no Unicode table; `to_digit` and `is_digit` panic outside radix 2..=36 with Rust's message. The table-driven ones (`is_alphabetic`, `is_whitespace`, …) and `to_uppercase` (an iterator) are rejected; see §6.5.

Tested on 40 characters across every UTF-8 length boundary and the surrogate gap, all pairs for ordering, radixes 0–37, every `u8`, and `from_u32` around the gap.

### 6.3 `uuid::Uuid`

Added for the Oxide `Name` rule, which had to spell `Uuid::parse_str` by hand ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). Verified against `uuid` 1.26.1.

- **Representation.** A branded `string` that only ever holds the lowercase hyphenated form (8-4-4-4-12), the one serde writes. In that form `===` is Rust's `==`, and JS string order is the order of the 16 bytes, since the hyphens sit at the same places in every value, so `==` and `<` print as themselves.
- **Parsing.** `parse_str` and `try_parse` accept what the crate's parser does, chosen by UTF-8 length as it chooses: 32 hex digits, hyphenated (36), braced (38), and `urn:uuid:` (45, the prefix in lowercase only), hex in either case; the result is the canonical form. The TS parser measures UTF-16 units, but a non-ASCII character fails the hex check at either length, so the accepted sets are equal. `uuid::Error` is opaque: nothing translated can read or compare it.
- **Rejected.** Generating (`new_v4`), `to_string`, and byte access. `nil()` is the only constructor besides parsing.
- **JSON** is serde's: read from any string `parse_str` accepts, written canonical.

Tested on about 3,400 strings (every form the crate prints in three cases, each with one-character edits, cuts, extensions, and wrappings, and non-ASCII characters at the length boundaries) and all pairs of 45 UUIDs for ordering.

### 6.4 `usize`

A `number` checked to 0..2^53−1; the gap above that is in §3.

### 6.5 Other methods

- **`Vec` and slices**: `len`, and `is_empty` as `.length === 0`, on values, borrows, fields, slices, and `as_bytes()` results.
- **`Option::is_some` / `is_none`** print as `!== null` / `=== null`. This holds because `Option<T>` is `T | null` with nested `Option` rejected, so a falsy payload (`0`, `0n`, `false`, `""`) is still `Some`.
- **Unicode-table methods** (`to_uppercase`, `is_alphabetic`, …) are not accepted. If added, they carry the version gap of §3, and their differential tests first check that both toolchains' Unicode versions agree; `ß` → `SS` and final sigma were measured to match.

### 6.6 `std::cmp::Ordering`

- **Naming.** `Ordering` after `use std::cmp::Ordering;` (or `core::`, alone or in a group), or the full path `std::cmp::Ordering` in types, expressions, and patterns. `cmp::Ordering` through `use std::cmp;`, a renaming `use`, a glob, and importing the variants bare are refused, as is a crate item named `Ordering` beside std's. A crate that defines its own `Ordering` and does not name std's is unaffected.
- **Representation.** A fieldless enum `Ordering { Less, Equal, Greater }` with discriminants −1, 0, 1, added to the crate when it names std's (exported) or only calls `cmp` (internal). It prints like any crate enum, so `match`, tuple `match`, guards, `matches!`, and exhaustiveness need nothing new; rustc still checks the source against std's type.
- **`cmp`** on integers, `char`, `bool` (`false` first), `String` / `&str`, and `Uuid`, and the methods of `Ordering`, are rewritten (§7.10). `==` and `!=` on two `Ordering`s compare the variant: std derives `PartialEq`, so equality is structural, unlike a crate enum's (§5.1).
- **Refused.** `cmp` on floats (`partial_cmp` too), tuples, `Vec`, `Option`, and the crate's types; `impl Ord` / `PartialOrd` (trait impls); `<` on `Ordering` or `bool`; `Ordering` in a struct or enum that derives `Serialize` or `Deserialize`, since serde gives it neither ([04 §3.2](./04-wire.md#32-serde-in-the-input)).

### 6.7 `str::parse` into an integer

`s.parse::<T>()`, `T` any integer type, is Rust's `T::from_str_radix(s, 10)`: an optional `+`, or a `-` when `T` is signed, then one or more ASCII digits, with the value in `T`'s range. Everything else is `Err`: an empty string, a sign alone, whitespace, `_`, a non-ASCII digit, a value out of range. Leading zeros are accepted. The target is named by the turbofish or by a `let` of `Result<T, ParseIntError>`. `Int.<t>.parse` checks the same grammar with an ASCII regular expression and the range on a `bigint`.

`ParseIntError` carries nothing: nothing translated reads its `kind()`, and the two sides agree on whether the result is `Ok` and on its value, not on which kind an `Err` is. A `usize` above 2^53−1 parses in Rust and panics in TS, as any `usize` that large does (§3). Floats, `bool`, `char`, and a crate's own `FromStr` are refused.

Tested with every integer type at its bounds and one past them, signs alone and doubled, leading zeros, whitespace, `_`, hex and exponent forms, and Arabic-Indic digits, through `ok()`, `match`, `map_err`, and `map_err(..)?`.

## 7. Rewritten constructs

Some accepted Rust has no one-to-one TS form. It is rewritten into constructs that are already equivalent, and the rewrite keeps Rust's evaluation order.

| Construct | Becomes | Verified by |
| --- | --- | --- |
| [`Option::unwrap_or`, `ok_or`, `map`; `Result::ok`, `map`, `map_err`](#71-option-and-result-methods) | the `match` std writes | `option_methods_equivalence.rs`, `parse_equivalence.rs` |
| [match guards](#72-match-guards) | an `if` at the leaf of the decision tree | `guards_equivalence.rs` |
| [scalar consumers](#73-scalar-consumers): `all`, `any`, `position`, `count`, `sum`; `for` over `.enumerate()` | `Iter.<method>`, the loop std runs | `consumers_equivalence.rs` |
| [`bool` patterns](#74-bool-patterns) | an `if` chain, the last named arm the `else` | `bool_patterns_equivalence.rs` |
| [tuple patterns](#75-tuple-patterns) in `let`, closure parameters, `for` | a one-arm tuple `match` | `destructure_equivalence.rs` |
| [`vec![a, b]`](#76-veca-b) | the array literal | `vec_build_equivalence.rs` |
| [`const`, enum discriminants](#77-const-and-discriminants) | the folded value | `flags_equivalence.rs`, `consts.rs` in `check` |
| [`const` in a block](#78-const-in-a-block) | a `let` at the top of the block | `local_consts_equivalence.rs` |
| [integer methods](#79-integer-methods) | the exact result, then checked, clamped, or wrapped | `int_methods_equivalence.rs` |
| [`cmp` and `Ordering`'s methods](#710-cmp-and-orderings-methods) | `Ord.cmp` / `Ord.cmpStr` / `Ord.then`; a `match` on the `Ordering` | `ordering_equivalence.rs` |
| [`impl Display` with a fixed text](#711-impl-display-with-a-fixed-text) | the method `toString`, the text per value | `display_equivalence.rs` |
| [`s.split(c).collect()`](#712-a-list-collected-from-text) and `.map(f).collect()` | the array, its `map`, or `Iter.tryCollect` | `collect_equivalence.rs` |

A closure that is inlined (`map`, the consumers) may not use `?` or `return`, which would leave the enclosing function.

### 7.1 `Option` and `Result` methods

The receiver is bound once. `unwrap_or(d)` and `ok_or(e)` evaluate their argument before the `match`, whether or not the option is `Some`, as Rust does; JS `??` would skip it, and a `d` that overflows would then not panic. `map(f)` runs `f` only on `Some`. `o.map(f).unwrap_or(d)` is one `match o { Some(x) => f(x), None => d }` when `d` is a name or a literal: it has no effect and cannot panic, so whether it runs before `f` or only on `None` is not observable.

On a `Result`, `ok()` is `Some` of the `Ok` value or `None`; `map(f)` runs `f` only on `Ok`, `map_err(f)` only on `Err`. `f` is a closure of one parameter without `?` or `return`, a function name, or a one-field tuple variant (`PreId::Numeric`), as for `Option::map`. `r.map_err(f)?` is `match r { Ok(v) => v, Err(e) => return Err(f(e)) }`: the mapped `Result` is not built.

### 7.2 Match guards

A `match` with a guard goes through the decision tree of a tuple `match` (a single value as a tuple of one): each element is tested once on a path, and where an arm's pattern has matched, its guard is tested with the arm's bindings read from their places; if it is false, the `else` is the tree of the arms after it that can still match there.

- A guard runs only when its pattern matched, and the arms are tried in Rust's order.
- A guarded arm covers nothing, so `A if g => .., _ => ..` sends a failing `A` to `_`.
- Where two literal arms may overlap (a range and a literal inside it), the later one is tested again after the earlier guard fails.
- A `?` or `return` in a guard is refused.

Tested with a guard that overflows only where its pattern matched, `_` after a guarded arm, overlapping literal arms, `bool` elements, and a name read only by the guard. Two earlier printings were replaced by this one ([07 §8.3](./07-roadmap.md#83-030-guards-and-option-2026-09-30)).

### 7.3 Scalar consumers

`all`, `any`, `position`, `count`, and `sum` on `s.chars()`, `s.bytes()`, `s.split(c)`, and `xs.iter()` call the runtime's `Iter.all`, `Iter.any`, `Iter.position`, `Iter.count`, and `Iter.sum`, which run the loop std's default methods run: the source is evaluated once, the closure is passed as an arrow function (a function name as itself; neither may use `?` or `return`, which would have to leave the enclosing function), and the loop stops where std's does (`all` at the first `false`, `any` and `position` at the first `true`), so a predicate that would overflow on a later item does not run on it.

- `position` counts items (chars, not bytes, on `chars()`).
- `sum` adds from zero left to right with the checked operator, so it panics on overflow where a debug build does; it is refused on floats, whose `Sum` starts from `-0.0`.
- `for (i, x) in ...enumerate()` reads a `usize` counter into `i` and advances it at the top of each pass, so `continue` keeps it right.

Tested on strings of every width, early stops before an overflow, `sum` overflowing either way, and `continue` in an enumerated loop.

### 7.4 `bool` patterns

`true` and `false` are tried in order as an `if` chain, like integer literals; a `match` naming both needs no `_`, and its last arm becomes the `else`. In a tuple `match` they split like any literal column. Tested with both named, one and `_`, `false | true`, a three-element tuple `match` with a guard, and `matches!`.

### 7.5 Tuple patterns

In `let`, a closure parameter, or a `for` variable, a tuple pattern becomes `match value { (a, b) => rest }`, which Rust's irrefutable pattern is: the value is evaluated once (a tuple expression left to right, so the first overflow panics), then each element is bound. A `mut` element binds a fresh name that the body rebinds with `let mut`. Tested with `mut` and `_` elements, an annotation, overflow in either element, `&(a, b)` over `iter()`, and `break` / `continue` in the loop.

### 7.6 `vec![a, b]`

Prints as `[a, b]`, elements evaluated left to right as in Rust. The `Vec` it builds is never mutated (no `push`, `insert`, `remove`, or index assignment unless it is bound to a `let mut` local, which owns an array of its own: §7.14), so sharing the element values with the array is unobservable. The element type comes from context like any literal's; `vec![x; n]` is rejected. Tested with element types from the return type and a struct field, `vec![]`, nesting, `Option` elements, and the first of several overflows reported.

### 7.7 `const` and discriminants

- **`const`** is folded by `check` with rustc's const-evaluation rules (checked arithmetic, wrapped shifts with the amount checked), and the TS holds the value, not the computation. rustc rejects a const that overflows, so there is nothing to throw at run time, and module load order cannot matter.
- **A const in a pattern is refused**: Rust compares with its value, where the IR would bind a new name that matches anything.
- **Discriminants** of a fieldless enum (explicit, or one past the previous; the first `0`) are folded the same way, in the `#[repr]` type or `isize` (64 bits). `e as T` is accepted only when `T` holds every discriminant, so the cast never truncates or wraps; it prints as a table indexed by `kind` (a constant variant folds to the literal). `x as usize` from a `u8`, `u16`, or `u32` holds every value on every target and is how Rust writes the last (std has no `usize::from(u32)`); it is the widening `usize::from` would be, the same `number` under another brand. Between any other two integer types that std's `From` does not join, `x as T` keeps the value modulo 2^bits of `T`, read signed for a signed `T`, as Rust's `as` does: `Int.<t>.cast(x)`, which for a `number` target takes the low bits with `<<` and `>>` / `>>>` (exact on any integer below 2^53, as JS shifts first reduce modulo 2^32) and for a `bigint` source or target `BigInt.asIntN` / `asUintN`. Into `usize` the result is Rust's 64-bit one, so above 2^53−1 it panics, as every `usize` that large does (§3). A pair `From` joins is refused as `as` and written `T::from(x)`. Tested over all 54 pairs at each type's bounds and the powers of two around 8, 16, 32, 53, and 64 bits (`int_cast_equivalence.rs`).

### 7.8 `const` in a block

Not folded: it becomes an immutable, typed `let` at the top of its block, in declaration order, so it is visible from the whole block as an item is. The value is computed at run time rather than folded, which is the same value: rustc, which runs on the input, rejects a const whose evaluation overflows, so the computation cannot panic where Rust would have folded one. A local const in a pattern is refused like a crate one, and so is a `let` of its name (rustc reads that `let` as a pattern too). Tested with a use before the declaration, a crate const and a discriminant in the value, an unused and a float const, one named like a crate item, one inside a `match` arm, and overflow computed from one.

### 7.9 Integer methods

`x.min(y)`, `max`, `abs`, `pow`, and the `checked_*`, `saturating_*`, and `wrapping_*` forms are computed from the exact result in `bigint`:

| Form | Result |
| --- | --- |
| `checked_*` | the exact result, or `None` outside the type's range (and on a zero divisor, and on `MIN / -1`) |
| `saturating_*` | clamped to the range; `saturating_pow` of a negative base to an odd exponent saturates to `MIN`, as std does |
| `wrapping_*` | the low bits; `wrapping_pow` works modulo 2^bits |
| `abs`, `pow` | panic outside the range as a debug build does ("attempt to negate with overflow", "attempt to exponentiate with overflow", measured on 1.98.1) |

An exponent is a `u32`; a power is not formed when its magnitude is certainly past the range (a base of magnitude two or more to an exponent of the type's width or more). Tested at every type's edges, `(-2).pow(31)` and `i8` powers near 127 included.

### 7.10 `cmp` and `Ordering`'s methods

- **`a.cmp(&b)`** is a call to the runtime, the receiver then the argument as its arguments, so each is evaluated once and in Rust's order. `Ord.cmp(a, b)` on an integer or `bool` is `Less` if `a < b`, `Equal` if `a === b`, else `Greater`: JS `<` orders numbers and `bigint`s as Rust does, and `false` before `true`. `Ord.cmpStr(a, b)` on a `char`, a string, or a `Uuid` goes through `Str.cmp`, by code point; a `char` is a string of one code point, and a `Uuid` its canonical form.
- **`is_eq` … `is_ge` and `reverse`** are a three-arm `match` on the receiver.
- **`then(o)`** is `Ord.then(receiver, o)`: both are evaluated, the receiver first, then `o` is picked on `Equal`. `o` runs, and can overflow, whatever the receiver is, as Rust evaluates a call's arguments.
- **`then_with(f)`** puts `f`'s body (a closure without parameters, `?`, or `return`) or the call `f()` (a function name) in the `Equal` arm, so it runs only there.

Tested with an overflowing `then` argument after a non-`Equal` receiver, an overflowing `then_with` body that must not run, a function name, a SemVer-style chain, `Ordering` in a tuple `match` with guards, `matches!` with the qualified path, and every predicate.

### 7.11 `impl Display` with a fixed text

`impl Display for X` whose `fmt` writes a text fixed per value becomes the method `X.toString(self): string` (`to_string` before `check::rename` spells it for TS), and `x.to_string()` in the crate calls it. The shapes taken: `f.write_str(t)` or `write!(f, "..")` (no `{}`; `{{` and `}}` read as braces), a `match self` whose arms are those or string literals, and `let t = <such a match>; f.write_str(t)`. The text is what `to_string()` gives in Rust: `write!` writes its literal with the braces unescaped, `write_str` its argument. Any other `fmt` (formatting arguments, several writes) is skipped as before, so a crate that compiled keeps compiling, and has no `toString`. Tested with each shape, a non-ASCII text, escaped braces, a struct, and a skipped `fmt` with arguments.

### 7.12 A list collected from text

<!-- derived-from ./07-roadmap.md#88-070-lists-from-text-2026-10-02 -->

`s.split(c).collect()` and `s.split(c).map(f).collect()` build a `Vec` whose length is the input's. `c` is a `char`, so the pieces are the ones `for t in s.split(c)` already walks, empty ones included. `f` is a closure of one `&str` parameter, or a function name, with no `?` or `return`. The target is named by `collect::<Vec<T>>()`, `collect::<Result<Vec<T>, E>>()`, a typed `let`, or the function's return type.

Into a `Vec<T>`, the pieces (or `f`'s results) are the array. Into a `Result<Vec<T>, E>`, `f` returns `Result<T, E>` and `Iter.tryCollect` runs `f` in order and returns the first `Err`, which is what `FromIterator` for `Result` does, so a later piece is not evaluated. A `&str` separator and a missing target type are refused; collecting other sequences is §7.13.

Tested with empty pieces, a non-ASCII separator, a function name and a closure, a turbofish and a typed `let`, an empty needle to `split_once`, and a `"boom"` after `"bad"` that panics only when the `Err` does not come first.

### 7.13 `map` and `filter` over a sequence

<!-- constrained-by ./02-authoring.md#31-state-is-a-value-sequences-are-rebuilt -->

`xs.iter()` (or `into_iter()`, `chars()`, `bytes()`, `s.split(c)`) through any `.map(f)` and `.filter(p)`, then a consumer (`collect`, `sum`, `count`, `all`, `any`, `position`). Rust's adaptors are lazy: for each item, every stage runs, then the consumer, before the next item. `Iter.map` and `Iter.filter` are generators, so the TS runs in that order too, and a panic in a stage or in `sum`'s addition comes at the same item as in Rust. One stage before `collect` prints as the array method (`xs.map(f)`, `xs.filter(p)`): with nothing else running between the calls, the order of `f`'s calls is the same. `filter`'s predicate takes `&T`; it reads the item, as everything here does.

### 7.14 Growing and editing a `Vec`

`let mut v: Vec<T>` is the one array the output writes, by `v.push(x)` or `v.insert(i, x)` (`Slice.insert`: Rust's panic for `i` past the length, then `splice`, the same shift of the tail Rust makes), `v.remove(i)` (`Slice.remove`: Rust's panic for `i` at or past the length, then `splice`), or `v[i] = x` (`Slice.set`: Rust's panic at or past the length, where JS would grow the array, then the write). Rust evaluates an assignment's value before its place, and in `v[i] op= x` on a primitive the `x` before `v[i]`; `Slice.set(v, i, x)` reads `i` first, so where both `x` and something JS reads before it may panic, `x` is bound in a `const` first (`vec_edit_equivalence.rs` has a case of each). It is bound to an array of its own: `Vec::new()`, `vec![..]`, `.clone()`, and `.collect()` make one; any other value is copied when bound or assigned (`[...xs]`), because a TS caller may still hold the array a parameter, a field, or a returned value is. No closure captures it, and nothing pushes to a field, an element, or a parameter. So every other array is never written after it is made, and sharing one is unobservable, as before: `clone` of a `Vec` copies its elements (`[...xs]`, shallow, since the elements are not written either), and `clone` of anything else is the value.

### 7.15 Building a `String`

`String::new()` is `""`. `s.push(c)` and `s.push_str(t)` on a local `let mut s: String` are written by `check` as the assignment `s = s + c`, so every later pass sees an ordinary write of `s` (narrowing forgets what it knew, a loop's head joins it); nothing pushes to a field, an element, or a parameter. JS strings are values, so `+` makes a new one and no other holder of the old string sees a change, as no other holder of a Rust `String` does. Both languages concatenate by code point: UTF-8 bytes and UTF-16 units of the parts in order are those of the whole. A push prints as `s += c` where the piece needs no statements. `collect::<String>()` over a sequence of `char`s collects them as a `Vec<char>` and joins it (`.join("")`); nothing else collects into a `String`. Tested with empty, ASCII, two- to four-byte, and surrogate-edge text, separators of every UTF-8 length, and the length of what was built (`string_build_equivalence.rs`).

## 8. Verification

| Check | Mechanism |
| --- | --- |
| Values | Differential tests: same inputs through Rust and the generated TS on Node, results compared as canonical strings. Inputs are chosen by hand at the edges; where the logic has many paths (integer methods, slicing, guards), a fixed-seed generator adds random ones, half at the edges of each width (`support::Rng`) |
| Types | `tsc --strict` on TypeScript 6 and 7 for every fixture, the runtime packages, and the wire schemas |
| Wire | Readers against serde's default JSON; `toJson` against the vendored serde_json byte for byte (`wire_write.rs`) |
| Determinism | `check --out` compares bytes with the existing output |
| Compilability | rustc must accept the input ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate)) |
| Rejection quality | Every rejection has `path:line:col` and a reason code, and nothing is written |

### 8.1 Canonical form

Both sides render values from the same IR types.

- **Rust side.** The test-only proc-macro `purecrate_canon::fixture!` derives `Show` for every fixture type; containers and scalars live in `crates/cli/tests/it/support`.
- **TS side.** The harness generates a printer per type.
- **Form.** It resembles `Debug` (`Order::Placed { lines: Lines::Cons(…), total: Yen(450) }`). Floats are written as their `f64` bit pattern, non-printable-ASCII as `\u{…}`.
- **Why.** Hand-picked projections missed a deliberately injected swap of `expected` and `got` in `OrderError::AmountMismatch`; whole values catch it.

### 8.2 Whole-value arguments

Arguments are whole values too: `fixture!` also derives `Js`, the TS literal of each fixture type, so a case can pass an `Invoice` built in Rust.

Remaining projections:

- Cases routed through a driver returning a scalar compare only what the driver reads (examples/order adds `trace4` to compare the whole final state).
- The counter acceptance test compares only `State.n`.

### 8.3 Coverage

All of this is measured on examples, so equivalence is as strong as the examples' coverage. Where exhaustive enumeration is small, it is used:

- all 1296 sequences for control and vending;
- all 4-step sequences for order and, under each capture and confirmation method, for payment.
