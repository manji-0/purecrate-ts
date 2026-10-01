# Changelog

## Unreleased

### Changed

- `--schema` gives a wire form only to what serde does: a schema for each public struct and enum that derives `Deserialize`, and a `toJson` entry for each that derives `Serialize`. Before, every public type had both, so a closed type with no derive (signup's `Email`) could be built from JSON by shape, bypassing its constructor, though Rust cannot read it at all. A crate where no public type derives either is refused with `--schema`. Output generated with `--schema` must be regenerated.
- A type that derives `Serialize` or `Deserialize` must hold only types that derive it too, as the real derive requires (`check` sees a stand-in serde); the new reason `item/serde-derive` reports the rest. `std::cmp::Ordering` is refused only in such a type: a type without a serde derive may hold one.
- Indexing `xs[i]` and slicing `&xs[a..b]` on a `Vec` or slice call the runtime's `Slice.at` and `Slice.range` instead of an inline function at every use, with the same checks and panic messages; iban's generated code goes from 6.0 to 4.9 KB. `Slice` is now a reserved name.
- `let x = o.ok_or(e)?` prints as a guard, `if ($opt === null) return Result.err($arg);`, instead of an inline function that built a `Result` for `?` to take apart (15 lines to 4); `e` still runs first. A statement `if c { return v; }` prints on one line when it fits.
- A `?` inside an expression binds its value once and tests it in place, `const $q1 = f(x); if ($q1.kind === "Err") return $q1;`, read as `$q1.value` (an `Option`'s as `$q1`), instead of a second binding for the payload.
- A lowered `match` (a tuple `match`, guards, `Option` and `Result` cases) binds a variant's fields and a payload to the arm's own names, `const conversion = method.conversion;`, instead of a fresh `$f1` / `$v1` copied into each arm's name with an `as` cast; guards read the same name. semver's `compare_pre_ids` loses 9 lines.
- `a.cmp(&b)` and `o.then(p)` call the runtime's `Ord.cmp` (integers, `bool`), `Ord.cmpStr` (`char`, strings, `Uuid`, by code point), and `Ord.then`, with each argument evaluated once in Rust's order, instead of an `if` chain over fresh bindings and a `match` per `then`. semver's `compare` goes from 80 lines to 44. `Ord` is now a reserved name.
- `all`, `any`, `position`, `count`, and `sum` call the runtime's `Iter`, the closure passed as an arrow, instead of a block with fresh bindings, a labelled loop, and an inline function wherever the result was an operand. They stop where std's methods stop, as before. `Iter` is now a reserved name.
- A `match` or `matches!` on a place inside an expression, its arms expressions, prints as `?:` or as `||` / `&&` on the arms' tests (`(k.kind === "A")`, `(o !== null ? o : 0)`) instead of an inline function with a `switch`; a `match` whose arms are all `true` or `false` prints as its test also as a statement. The inline function remains for a value that is not a place, which it evaluates once. payment's generated code goes from 22.3 to 20.6 KB.
- Tidier output: no parentheses around a whole condition, `return` value, or initializer; `{ reason }` for a field set from a variable of its name; `!(a === b)` as `a !== b`; and a function or method whose body is one `return` is an expression-bodied arrow.
- A line past 100 characters is broken inside its outermost bracket with commas, one item per line with a trailing comma, as prettier breaks it: long signatures, calls, and object literals. A condition joined only by `&&` / `||` stays on one line.

### Added

- `///` (and `/** */`) comments carry over as JSDoc: on structs and enums, struct fields, each variant's constructor, functions, methods, consts, and aliases. Before, every doc comment was dropped.

### Fixed

- A package's copy of the runtime no longer has runs of blank lines where unused parts were left out.

### Examples

- Each example keeps its generated package beside its source: `examples/<name>/ts/plain`, and, for invoice and payment, which derive serde, `examples/<name>/ts/<lib>` with each schema library. `scripts/examples.sh` regenerates them; `scripts/verify.sh` checks them for drift and runs `tsc` over them.

## 0.5.0 — 2026-10-01

Ordering: what kept semver, the first example written from the authoring skill alone since line counts are normalized, over twice the idiomatic Rust ([roadmap §2.2](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#22-line-counts-against-idiomatic-rust)).

### Added

- `std::cmp::Ordering`, named after `use std::cmp::Ordering;` or by its full path, as a fieldless enum: `Ordering::Less` in patterns, `==` and `!=`, `is_eq` / `is_ne` / `is_lt` / `is_gt` / `is_le` / `is_ge`, `reverse`, `then(o)`, and `then_with(|| ..)` or `then_with(f)`.
- `a.cmp(&b)` on every integer type, `char`, `bool`, `String` / `&str`, and `Uuid`, receiver then argument, each evaluated once.
- `<`, `<=`, `>`, `>=` on strings, by code point as Rust orders them (not by UTF-16 unit as JS `<` does).

### Changed

- The runtime gains `Str.cmp`, carried only by packages that compare strings.
- `Ordering` cannot be a struct or enum field (serde has no form for it). A crate item named `Ordering` beside std's, `use std::cmp::Ordering::*`, and `cmp::Ordering` through `use std::cmp;` are refused with a message.
- The authoring skill describes six rejections that semver's author could not predict from it.

### Examples

- semver (SemVer 2.0.0 parsing and precedence), written from the authoring skill alone and checked against an idiomatic-Rust reference on every pair's precedence: 209 lines of logic as first written, 166 with ordering.

## 0.4.1 — 2026-10-01

Smaller output, found by evaluating 0.4.0: guarded matches no longer repeat themselves, and each package carries only the runtime it uses. The language accepted is unchanged; output generated with 0.4.0 must be regenerated.

### Changed

- A package's copy of the runtime keeps only the parts its code uses (each integer type's bitwise operators and methods, the string operations, the JSON writer); the types, `of` and arithmetic, and what the index exports (`Char`, `Uuid`, `parseJson`) are always kept. payment's bundle goes from 3.1 to 2.3 KB gzipped and its first call from 0.72 to 0.52 ms. Goldens committed with 0.4.0 must be regenerated.
- A `match` with guards compiles into the decision tree a tuple `match` uses, testing each value once and each guard where its pattern matched, instead of an `if` chain that repeated the whole match per arm. payment's generated code shrinks from 47.9 to 23.5 KB and oidc's from 64.4 to 48.8 KB. Output committed with 0.4.0 that uses guards must be regenerated.
- A `?` or `return` in a match guard is refused with its own message.
- The runtime builds its Unicode-property regular expression (for a slicing panic's message) on first use instead of at module load, which took about half a millisecond.

### Fixed

- A guard whose pattern overlaps a later arm's (a range and a literal inside it) falls through to that arm when it is false, and a `match` whose arms all jump gets no unreachable `break` after it; both arose with guards in the decision tree and are covered by tests.
- `bench/payment/measure.sh` builds its WASM crate again: the workspace's vendored sources had applied to it since 0.3.0.

### Tests

- Randomized differential cases (fixed seed) for the integer methods, slicing, and guarded matches, next to the hand-chosen edges. `scripts/line-counts.py` counts each example against its idiomatic reference with both sides rustfmt-ed.

## 0.4.0 — 2026-10-01

Iteration, part two: what idiomatic code reaches for once loops exist, measured by rewriting oidc, payment, and invoice ([roadmap §2.2](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#22-line-counts-against-idiomatic-rust)).

### Added

- Iterator consumers on `s.chars()`, `s.bytes()`, `s.split(c)`, `xs.iter()`, and `xs.into_iter()`: `all`, `any`, `position`, `count`, and integer `sum` / `sum::<T>()`, with a closure (not using `?` or `return`) or a function name. Each is the loop std runs and stops where std stops. `for (i, x) in ...enumerate()`. `|&x|` closure parameters.
- Tuple patterns in `let (a, mut b, _) = t;`, closure parameters `|(a, b)|`, and `for (k, v) in &pairs`.
- Slicing `&s[a..b]`, `&s[a..]`, `&s[..b]` at UTF-8 byte positions, and on `Vec`s and slices, panicking as Rust does and with its messages; `strip_prefix` / `strip_suffix` with a `&str`.
- Integer methods: `min`, `max`, `abs`, `pow`, and the `checked_*`, `saturating_*`, and `wrapping_*` forms of the operators, on every integer type.
- `const` items inside function bodies.
- `true` and `false` as patterns, also in tuple `match`es.

### Changed

- The runtime copied into each package gains `Str.slice`, `Str.stripPrefix`, `Str.stripSuffix`, and the integer methods. Output committed with 0.3.0 and checked with `check --out` must be regenerated.
- Rejection messages for a std method list the new ones among what the receiver allows.

### Examples

- oidc 777 → 629 lines (lexical helpers 68 → 47 against the idiomatic 31, request validation 153 → 108), payment's transitions one tuple `match` instead of five per-state functions (logic 163 → 111), invoice logic 74 → 66.

## 0.3.0 — 2026-09-30

Match guards and `Option` combinators, the gaps invoice and payment measured against idiomatic Rust ([roadmap §8](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#8-releases)), and more readable wire schemas.

### Added

- Match guards (`p if c =>`), also on arms that bind and in `matches!`. A guarded arm is tried in order and does not count toward exhaustiveness, as in rustc; the output is an `if` chain of standalone matches.
- `Option::unwrap_or`, `ok_or`, and `map` with a closure or a function name. `unwrap_or(e)` and `ok_or(e)` evaluate `e` first, as Rust does; `map` refuses a closure containing `?` or `return`.
- `survey --all-causes` lists every cause that keeps an item out, not only the first, to estimate a rewrite. The default output is unchanged.

### Changed

- **Breaking:** the zod adapter and the generated zod schemas target zod 4.6 (peer `^4.6.0`). Users on zod 3 stay on purecrate-ts 0.2.
- zod and valibot schemas are printed dependencies first, with `lazy` only for types in a cycle, and long unions and objects one arm or field per line. A refused `try_from` names the error's variant in all three libraries. arktype reports a bad nested field at its path instead of one error at the type. What each schema reads or refuses is unchanged. Output committed with 0.2.0 and checked with `check --out` must be regenerated.
- Dependencies are crates.io requirements, vendored by source replacement (`scripts/vendor.sh`); syn and proc-macro2 move to their latest 2.x and 1.x.

## 0.2.0 — 2026-09-30

Iteration, the largest gap the examples measured against idiomatic Rust ([roadmap §8](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#8-releases)).

### Added

- `for x in &xs`, `xs.iter()`, `xs.into_iter()`, and `for x in xs` over a `Vec` or slice, and `for b in s.bytes()`; printed as `for..of`. Iterator adaptors (`enumerate`, `rev`, `zip`, …) are refused with a message.
- `for t in s.split(c)` with a `char` separator: each `&str` piece, empty ones included, as JS `split` gives them. A `&str` separator is refused (an empty one splits differently in Rust and JS).
- `while`, `break`, and `continue`, without labels or values. A loop that a jump leaves is labelled in the output, since a bare JS `break` inside the `switch` a `match` prints as would leave the `switch`. A `?` in a `while` condition runs before every pass.
- `purecrate-ts --version` and `--help`.

### Changed

- The generated `tsconfig.json` sets `allowUnreachableCode: false`, and a `case` that ends in a jump has no trailing `break;`. Output committed with 0.1.0 and checked with `check --out` must be regenerated.
- `loop`, `while let`, and labelled jumps are still refused, now with messages that say what to write.

### Examples

- oidc reads bytes, lists, and candidates with the new loops: its lexical helpers went from 99 to 68 lines (from 3.2× to 2.2× the idiomatic code), the file from 810 to 777.

## 0.1.0 — 2026-09-30

First release. purecrate-ts translates pure domain functions written in a subset of Rust into an ordinary TypeScript package, without WASM, and rejects what it cannot translate with the same meaning. The subset is described in [design/02-authoring](https://github.com/manji-0/purecrate-ts/blob/main/design/02-authoring.md); the equivalence it keeps, in [design/01-equivalence](https://github.com/manji-0/purecrate-ts/blob/main/design/01-equivalence.md).

### What it translates

- Structs, enums (as `kind` unions), newtypes (as brands), `Option`, `Result`, `?`, early `return`, `if let`, and exhaustive `match`: arms naming a variant, `A | B` binding nothing, a last `_`, literal and range patterns on integers, `char`, and `&str`, and `match (state, event)` on tuples.
- Integers with Rust's debug-build semantics: overflow, division by zero, and out-of-range shifts throw with Rust's panic message; `i64`/`u64` are `bigint`; bitwise operators and shifts; widening with `T::from(x)`; `f32`/`f64`.
- `const` items (folded at check time into `consts.ts`) and enum discriminants, read with `e as T` when `T` holds every discriminant.
- Strings as UTF-8 byte units (`len`, `starts_with`, `as_bytes`, …), `char` as a branded code point, `for c in s.chars()`, `uuid::Uuid` as the `uuid` crate parses it.
- `Vec` read by index and `len`, and built as a fixed list with `vec![a, b]`; growing sequences as recursive enums.
- Local `let mut`, closures over immutable bindings, struct update, `for i in a..b`, modules (flattened).
- Closed types: structs with private fields are built only through their constructors, in TS as in Rust.
- serde: the same types derive `Serialize`/`Deserialize` for the server; `--schema zod|valibot|arktype` reads serde's JSON into domain values, and `toJson` writes the bytes serde_json writes.

### Commands

- `build` writes an npm package (the runtime copied in, `"private": true` unless `--publishable`).
- `check` rejects out-of-subset input with `path:line:col` and a reason code (most messages also say what to write instead), then runs rustc; with `--out`, it fails when a committed output differs from what `build` would write.
- `survey` estimates how much of an existing crate falls inside the subset.

### Output

- Plain `Readonly` values and arrow functions; passes `tsc --strict` with `noUnusedLocals`, `noUnusedParameters`, `erasableSyntaxOnly`, and `verbatimModuleSyntax` on TypeScript 6 and 7, and runs under Node's type stripping.
- Checked by differential tests: every example and fixture runs the same inputs through Rust and the generated TS and compares the results, panics included.

### Known limitations

- `check` and `build` run the input through `rustc`, which must be installed. The differential tests were measured on rustc 1.98.1.
- Release binaries are built for Linux (x86_64, aarch64) and macOS (x86_64, arm64). Windows is not built or tested.
- `usize` is a `number` checked to 2^53−1; past that the TS throws where Rust would not.
- No `while`/`loop`, iterator adaptors, growable `Vec`, `HashMap`, generics, or traits (other than `TryFrom` for serde and the skipped `Display`/`Error`). See [design/07-roadmap](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md) for what is added next and why.
