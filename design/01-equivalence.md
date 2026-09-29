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
| `f32` arithmetic | `Math.fround(a op b)`. `f32` literals are rounded once from decimal by the converter (not via `f64`) |
| `f64` arithmetic | JS operators |
| `i64` / `u64` literals | `5n` |
| widening `i64::from(x)` | value unchanged; `BigInt(x)` when crossing to `bigint` |

Inference is bidirectional and closed within the expression tree. It does not use rustc's later-use inference or the `i32`/`f64` defaults, so an untyped literal is rejected with a request for a suffix or annotation. Comparisons whose result differs between JS and Rust (struct/enum `==`, `String` ordering) are rejected.

Before this was built, the output computed `i32` `7 / 2` as `3.5`, `i32::MAX + 1` as `2147483648`, and `1 / 0` as `Infinity` (measured 2026-09-27). The cause was that the IR had no types. Per-operator differential tests now cover 57 cases (`arith_equivalence.rs`).

## 6. Strings, `char`, `usize`, std methods

Decided 2026-09-27; only parts are implemented ([07 §4](./07-roadmap.md#4-specified-but-not-yet-implemented)).

- **Strings reproduce UTF-8 byte units.** The TS value is a plain `string`; encoding-dependent operations go through the runtime `Str`. `len` counts UTF-8 bytes, `&s[a..b]` slices at byte positions and throws like Rust off a char boundary, `bytes`/`as_bytes` yield `u8`s, `chars` yields code points, ordering is code-point order (JS `<` orders U+E000–U+FFFF above supplementary planes; Rust does not). Implemented: `as_bytes` (`strings_equivalence.rs`, including empty, 2–4-byte characters, out-of-range indices); `len` as `Str.len`, and `is_empty`, `starts_with`, `ends_with`, `contains` with a `&str` needle as the JS `length === 0`, `startsWith`, `endsWith`, `includes` (`str_methods_equivalence.rs`, every pairing of 14 strings from empty to U+10FFFF). For well-formed strings a prefix, suffix, or substring on char boundaries is the same in UTF-8 bytes and UTF-16 units, so these four need no encoding step; `char` and closure needles are rejected.
- **`char`** is a branded one-code-point `string`. JSON matches serde. Ordering and `as u32` use `codePointAt(0)`.
- **`usize`** is a `number` checked to 0..2^53−1 (§3).
- **std methods** come from an exact-match allow-list keyed by (receiver type, method), each with its own differential test over empty, non-ASCII, supplementary-plane, boundary, and panicking inputs. A method that cannot be matched is rejected, not accepted with a documented difference. Examples: `f64::to_string` (Rust `1000000000000000000000`, JS `1e+21`) is rejected; `str::trim` is not JS `trim()` (which also strips U+FEFF).
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
