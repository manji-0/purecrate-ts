# Equivalence

Status: current (2026-09-29)

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
| `usize` ≥ 2^53 | Rust is fine up to 2^64; TS throws above 2^53−1. Lengths and indices do not reach this range. `bigint` was rejected because it does not mix with arrays and loops |
| Recursion depth | Measured 2026-09-28 (list length, Node 24.21, macOS): TS passes 10,000 levels and throws `RangeError` at 12,000. Rust debug passes 50,000 on the main thread and **aborts** at 100,000 (not a catchable panic; test threads have 2 MB). Even "both fail" does not hold |
| JSON nesting depth | serde_json rejects nesting deeper than 128; the wire schemas have no limit |
| Release wrapping | Not matched |
| Non-finite `f64` over JSON | serde_json and `toJson` both write `NaN` and infinities as `null`, and neither reads `null` back as a float. Same bytes, same asymmetry ([04 §6](./04-wire.md#6-writing-domain-values)) |
| Unicode-table methods | If added, equivalence holds only for code points assigned in both toolchains' Unicode versions (both 17.0 as of 2026-09-27) |

## 4. Closed types

<!-- derived-from #2-domain -->

In Rust, a struct with any non-`pub` field cannot be built by a literal outside its crate; values come only from public functions. That rule is carried over (decided and implemented 2026-09-29, `crates/cli/tests/closed_equivalence.rs`).

- A struct with a non-`pub` field (`pub(crate)` and `pub(super)` count as non-`pub`), including newtypes, is **closed**. It gets a `unique symbol` brand, and its companion has no `of`. The generator builds values through an internal `Email$of`, exported from the type's file but not from `index.ts`; `exports` exposes only the index, so deep imports cannot reach it. `$` cannot appear in Rust identifiers, so the name cannot collide.
- Methods that are not `pub` are not on the companion either: they are emitted as `Email$unchecked`, exported from the file but not from `index.ts`, like `$of`. Until 2026-09-30 every method sat on the exported companion, so a private `fn unchecked(raw) -> Email` let TS callers build what Rust callers cannot (found measuring Windmill's MCP scope, [91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)).
- A struct whose fields are all `pub` is **open** and gets `of`, as in Rust anyone can build it.
- Which function is the "checked constructor" is not inferred. Whatever public function returns the type is the way in, whether named `new`, `parse`, or `try_from`.

So the domain for closed types is: values returned by public functions, or read from the wire, by shape as serde's derive does or, with `#[serde(try_from = "T")]`, through the checked constructor ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). Rust has the same set, so correspondence is kept.

Not closed at runtime: `as` still works, fields are readable, objects are not frozen. Classes with `#private` fields would close it, but break idiomatic values, JSON, and structured cloning.

Validation has three layers. **Shape** (is this JSON an `Order`?) is the wire schema's job. **Meaning** (is this an `Email`?) is shared via closed types and checked constructors. **Screen** (wording, i18n, which field) is the caller's. Error messages are therefore not shared; the `Err` variant and payload are. A common error type with field paths will be introduced only when the same field-assignment table is duplicated across examples.

## 5. Numbers

`check::accept` infers a type for every expression and rewrites arithmetic before printing.

| Rust | Generated TS |
| --- | --- |
| integer `+ - * / %`, unary `-` | `Int.<ty>.add(a, b)` etc. `/` truncates; overflow and division by zero throw Rust's panic message; `-0` is normalized to `0` |
| integer `& \| ^`, `!`, `<< >>` (2026-09-30) | `Int.<ty>.and(a, b)`, `or`, `xor`, `not`, `shl`, `shr`. Results wrap to the width (JS int32 operators, then sign- or zero-extension; `BigInt.asIntN`/`asUintN` for 64 bits) and never panic; a shift amount of any integer type outside `0..bits`, compared as its whole value (`-1`, `2^32 + 1`), throws `attempt to shift left with overflow` as a debug build panics. An unsuffixed amount is `i32`, as in rustc. `usize` is refused: Rust gives it 64 bits, the TS `number` 53. `& \| ^` on `bool` are refused in favor of `&& \|\| !=` (`bits_equivalence.rs`: every width, operator, and amount type, boundary values, compound assignment, RFC 4226 truncation) |
| `f32` arithmetic | `Math.fround(a op b)`. `f32` literals are rounded once from decimal by the converter (not via `f64`) |
| `f64` arithmetic | JS operators |
| `i64` / `u64` literals | `5n` |
| widening `i64::from(x)` | value unchanged; `BigInt(x)` when crossing to `bigint` |

Inference is bidirectional and closed within the expression tree. It does not use rustc's later-use inference or the `i32`/`f64` defaults, so an untyped literal is rejected with a request for a suffix or annotation. Comparisons whose result differs between JS and Rust (struct/enum `==`, `String` ordering) are rejected.

Before this was built, the output computed `i32` `7 / 2` as `3.5`, `i32::MAX + 1` as `2147483648`, and `1 / 0` as `Infinity` (measured 2026-09-27). The cause was that the IR had no types. Per-operator differential tests now cover 57 cases (`arith_equivalence.rs`).

## 6. Strings, `char`, `usize`, std methods

Decided 2026-09-27; only parts are implemented ([07 §4](./07-roadmap.md#4-specified-but-not-yet-implemented)).

- **Strings reproduce UTF-8 byte units.** The TS value is a plain `string`; encoding-dependent operations go through the runtime `Str`. `len` counts UTF-8 bytes, `&s[a..b]` slices at byte positions and throws like Rust off a char boundary, `bytes`/`as_bytes` yield `u8`s, `chars` yields code points, ordering is code-point order (JS `<` orders U+E000–U+FFFF above supplementary planes; Rust does not). Implemented: `as_bytes` (`strings_equivalence.rs`, including empty, 2–4-byte characters, out-of-range indices); `len` as `Str.len`, and `is_empty`, `starts_with`, `ends_with`, `contains` with a `&str` needle as the JS `length === 0`, `startsWith`, `endsWith`, `includes` (`str_methods_equivalence.rs`, every pairing of 14 strings from empty to U+10FFFF). For well-formed strings a prefix, suffix, or substring on char boundaries is the same in UTF-8 bytes and UTF-16 units, so these four need no encoding step; `char` and closure needles are rejected. String literal patterns print as `===` for the same reason: well-formed strings are equal as UTF-8 exactly when they are as UTF-16; `String::as_str` prints as the string itself (`str_patterns_equivalence.rs`, 16 strings including the literals, a shared prefix, NFC vs NFD, U+FFFF, and a supplementary-plane neighbor). `chars` only as `for c in s.chars()` (2026-09-30): a JS string iterates by code point, which is Rust's sequence of scalar values for well-formed strings, so it prints as `for (const c of s)` (`for_chars_equivalence.rs`, 14 strings across every UTF-8 length, the surrogate gap and U+E000, with `?`, early `return`, nesting, closures, and overflow in the body). `chars()` as a value and its adaptors are rejected.
- **`char`** is a branded one-code-point `string` (`Char`); the brand also excludes lone surrogates, as a Rust `char` is a Unicode scalar value. JSON matches serde: a string of exactly one scalar value, anything else a schema failure. Ordering and range patterns compare `Char.code` (`codePointAt(0)`), never the strings: JS orders by UTF-16 unit, which puts U+E000–U+FFFF above the supplementary planes. Conversions are std's `From` only: `u32::from(c)`, `u64::from(c)`, `char::from(u8)`, `char::from_u32(n)` (`None` for a surrogate or past U+10FFFF); `as` stays rejected. Methods are the ASCII and code-point ones, which need no Unicode table: `is_ascii*`, `to_ascii_{upper,lower}case`, `eq_ignore_ascii_case`, `len_utf8`, `is_digit`/`to_digit` (panicking outside radix 2..=36 with Rust's message). The table-driven ones (`is_alphabetic`, `is_whitespace`, …) and `to_uppercase` (an iterator) are rejected. Implemented 2026-09-29 (`chars_equivalence.rs`: 40 characters across every UTF-8 length boundary and the surrogate gap, all pairs for ordering, radixes 0–37, every `u8`, `from_u32` around the gap; `wire.rs`, `wire_write.rs` for JSON).
- **`uuid::Uuid`** is a branded `string` (`Uuid`) that only ever holds the lowercase hyphenated form (8-4-4-4-12), the one serde writes. In that form `===` is Rust's `==`, and JS string order is the order of the 16 bytes, since the hyphens sit at the same places in every value, so `==` and `<` print as themselves. `Uuid::parse_str` and `try_parse` accept what the `uuid` crate's parser does, chosen by UTF-8 length as it chooses: 32 hex digits, hyphenated (36), braced (38), and `urn:uuid:` (45, the prefix in lowercase only), hex in either case; the result is the canonical form. The TS parser measures UTF-16 units, but a non-ASCII character fails the hex check at either length, so the accepted sets are equal. `uuid::Error` is an opaque value: nothing translated can read or compare it. `Uuid::nil()` is the only constructor besides parsing; generating (`new_v4`), `to_string`, and byte access are rejected. The JSON form is serde's: read from any string `parse_str` accepts, written canonical. Implemented 2026-09-30, from the Oxide `Name` rule, which had to spell `Uuid::parse_str` by hand ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). Verified against `uuid` 1.26.1, vendored for the tests: `uuid_equivalence.rs` runs about 3,400 strings (every form the crate prints in three cases, each with one-character edits, cuts, extensions, and wrappings, and non-ASCII characters at the length boundaries) and all pairs of 45 UUIDs for ordering; `wire.rs` and `wire_write.rs` read and write it through each schema library against serde_json.
- **`usize`** is a `number` checked to 0..2^53−1 (§3).
- **std methods** come from an exact-match allow-list keyed by (receiver type, method), each with its own differential test over empty, non-ASCII, supplementary-plane, boundary, and panicking inputs. A method that cannot be matched is rejected, not accepted with a documented difference. Examples: `f64::to_string` (Rust `1000000000000000000000`, JS `1e+21`) is rejected; `str::trim` is not JS `trim()` (which also strips U+FEFF). Beyond `str` and `char` (2026-09-30): `Vec::len`, `Vec::is_empty` as `.length === 0`, and `Option::is_some` / `is_none` as `!== null` / `=== null`, which holds because `Option<T>` is `T | null` with nested `Option` rejected, so a falsy payload (`0`, `0n`, `false`, `""`) is still `Some` (`std_methods_equivalence.rs`, on values, borrows, fields, slices, and `as_bytes()` results).
- **`Option::unwrap_or`, `ok_or`, `map`** become the `match` std writes, with the receiver bound once. `unwrap_or(d)` and `ok_or(e)` evaluate their argument before the `match`, whether or not the option is `Some`, as Rust does (JS `??` would skip it, and a `d` that overflows would then not panic); `map(f)` runs `f` only on `Some`. A closure passed to `map` may not use `?` or `return`, which would leave the enclosing function once inlined. `option_methods_equivalence.rs`, 2026-09-30.
- **Match guards** are rewritten before checking into an `if` chain over standalone matches: for each arm in order, `match $g { p => guard, _ => false }` tests it and `match $g { p => body, _ => unreachable }` takes it, with the scrutinee (each element of a tuple one) bound once to `$g`. A guard therefore runs only when its pattern matched, as in Rust, which a guard that overflows for other variants checks (`guards_equivalence.rs`, 2026-09-30).
- **`for t in s.split(c)`** with a `char` separator prints as `s.split(c)`: one code point occurs at the same places of a well-formed string in UTF-8 and UTF-16 (a supplementary one is a surrogate pair that appears nowhere else), and JS `split` keeps the empty pieces Rust keeps, leading, trailing, and between adjacent separators. A `&str` separator is refused: an empty one splits differently (`"ab".split("")` is `["", "a", "b", ""]` in Rust, `["a", "b"]` in JS). Only as the source of a `for`; `split` as a value stays off the allow-list. Implemented 2026-09-30 (`for_each_equivalence.rs`: ten strings against separators of every UTF-8 length).
- **`vec![a, b]`** prints as the array literal `[a, b]`, elements evaluated left to right as in Rust; the `Vec` it builds is never mutated (no `push`, no index assignment), so sharing the element values with the array is unobservable. The element type comes from context like any literal's; `vec![x; n]` is rejected. Implemented 2026-09-30 (`vec_build_equivalence.rs`: element types from the return type and a struct field, `vec![]`, nesting, `Option` elements, and the first of several overflows reported).
- **`const`** items are folded by `check` with rustc's const-evaluation rules (checked arithmetic, wrapped shifts with the amount checked), and the TS holds the value, not the computation: rustc rejects a const that overflows, so there is nothing to throw at run time, and module load order cannot matter. A const in a pattern is refused: Rust compares with its value, where the IR would bind a new name that matches anything. Implemented 2026-09-30 (`flags_equivalence.rs`, `consts.rs` in `check`).
- **Discriminants** of a fieldless enum (explicit, or one past the previous; the first `0`) are folded the same way, in the `#[repr]` type or `isize` (64 bits). `e as T` is accepted only when `T` holds every discriminant, so the cast never truncates or wraps; it prints as a table indexed by `kind` (a constant variant folds to the literal). Every other `as` stays rejected.
- **Unicode-table methods** (`to_uppercase`, `is_alphabetic`, …) are accepted with a version note (§3). Differential tests first check both Unicode versions agree. `ß` → `SS` and final sigma were measured to match.

## 7. Verification

| Check | Mechanism |
| --- | --- |
| Values | Differential tests: same inputs through Rust and the generated TS on Node, results compared as canonical strings |
| Types | `tsc --strict` on TypeScript 6 and 7 for every fixture, the runtime packages, and the wire schemas |
| Wire | Readers against serde's default JSON; `toJson` against the vendored serde_json byte for byte (`wire_write.rs`) |
| Determinism | `check --out` compares bytes with the existing output |
| Compilability | rustc must accept the input ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate)) |
| Rejection quality | Every rejection has `path:line:col` and a reason code, and nothing is written |

**Canonical form** (since 2026-09-29): both sides render values from the same IR types. On the Rust side the test-only proc-macro `purecrate_canon::fixture!` derives `Show` for every fixture type; containers and scalars live in `crates/cli/tests/support`. On the TS side the harness generates a printer per type. The form resembles `Debug` (`Order::Placed { lines: Lines::Cons(…), total: Yen(450) }`); floats are written as their `f64` bit pattern, non-printable-ASCII as `\u{…}`. Previously tests compared hand-picked projections, and a deliberately injected swap of `expected` and `got` in `OrderError::AmountMismatch` went unnoticed; it is now caught.

Arguments are whole values too (since 2026-09-29): `fixture!` also derives `Js`, the TS literal of each fixture type, so a case can pass an `Invoice` built in Rust. Remaining projections: cases routed through a driver returning a scalar compare only what the driver reads (examples/order adds `trace4` to compare the whole final state), and the counter acceptance test compares only `State.n`.

All of this is measured on examples, so equivalence is as strong as the examples' coverage. Where exhaustive enumeration is small, it is used (all 1296 sequences for control and vending; all 4-step sequences for order and, under each capture and confirmation method, for payment).
