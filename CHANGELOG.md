# Changelog

## Unreleased

### Breaking

- **A one-field tuple variant is `{ kind, value }`.** `PreId::Numeric(5)` was `{ kind: "Numeric", content: [5n] }`, read as `id.content[0]`; it is now `{ kind: "Numeric", value: 5n }`, read as `id.value`, and the constructor's parameter is `value`. Two or more fields stay `content: [..]`. The JSON is serde's as before (`{"Numeric": 5}`). Regenerate, and replace `.content[0]` on one-field variants with `.value` in calling code.
- **`Int` is exported only with a number.** The index re-exports `Int` only when the public surface holds an integer or float type; a package whose surface has none has no `Int` in its copy of the runtime to export.

### Changed

- **A smaller runtime copy.** The copy keeps each `Int` width, operator, and method the package uses, the brand and helper types its code names, and what those read in turn; nothing else. counter's copy is 175 → 84 lines, payment's 261 → 119, semver's 370 → 239.
- **No `$` in local names.** Temporaries say what they hold, without the `$` (`$v_major` → `majorResult`, `$m_3_$t` → `option`, `$e_2` → `end`, `$l2` → `loop`), and a shadowed binding counts from two (`pre$1` → `pre2`). A made name is numbered only where an identifier in the block that holds its uses already has it. A loop with a literal end reads it in place (`i < 4`). File-level helpers other files import keep theirs (`Version$of`).
- **Shorter generated code.** Where a value is returned, a branch that returns is followed by the next one instead of an `else`, and a branch of one `return` sits on its `if`'s line, so a chain of guards reads top to bottom. A function name passed as a function is the name, not an arrow around it. A cast stands bare as an argument, element, or field value (`Int.i32.add(n, 1 as I32)`), and a comparison reads a literal or a length bare (`parts.length < 2`, `b >= 48`). A value of one side of a `match` or `if` whose other side returns is the exit and then a `const` (`if (o === null) return e;` `const x = o;`); a `match` on a call whose arms are expressions binds the call and is one `?:`; a tuple only taken apart is `const [a, b] = ..`. semver's generated domain code is 409 → 336 lines, oidc's 1441 → 1389.

### Added

- A side of `|` may test inside its case (`Started | Paid(Method::Card, Some(0)) =>`), and a tuple inside a case or a tuple may hold any pattern (`Some((1, b))`, `(a, (true, n))`). Both were `[pattern/nested]`. The sides of `|` still bind nothing.
- `Option::map` takes a one-field tuple variant as its function (`.map(PreId::Numeric)`), as Rust does. semver uses it.
- `s.parse::<T>()` into every integer type, as `Int.<t>.parse(s)`: Rust's `from_str_radix(s, 10)` (a `+`, a `-` when signed, ASCII digits, in range). The error is `ParseIntError`, which carries nothing. semver drops its digit loop for it.
- `Result::ok`, `map`, and `map_err`, as the `match` std writes; `r.map_err(f)?` returns `Err(f(e))` without building the mapped `Result`.

## 0.7.0 — 2026-10-02

`split_once`, and a `Vec` collected once from `s.split(c)`, which is what kept semver over twice the idiomatic Rust after ordering; patterns nested in a case; and two rounds of review fixes (panics as `Panic`, `Mutex` refused, generated-code layout, CI and release hardening). Packages generated with 0.6.0 must be regenerated (Breaking, below) ([roadmap §8.8](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#88-070-lists-from-text-2026-10-02)).

### Breaking

- **`Mutex` is refused** (`[type/mutex]`): it is shared mutable state, which a pure-function subset does not have. `Box` and `Arc` stay erased.
- **Fewer index exports.** The index re-exports `Result` and each of `I8`…`F64` only when the public surface holds that type, and a trimmed runtime drops a namespace whose members are all gone (`export const Iter = {}`).

### Added

- `s.split(c).collect()` and `s.split(c).map(f).collect()` into `Vec<T>` or `Result<Vec<T>, E>`. `c` is a `char`. `f` is `|x| ..` without `?` or `return`, or a function name. The target is `collect::<..>()`, a typed `let`, or the return type. Into a `Result`, iteration stops at the first `Err`.
- `str::split_once` with a `char` or a `&str`, including an empty needle, as `Str.splitOnce`.
- `Some((a, b))`, and the same tuple of names and `_` in `Ok`, `Err`, or a variant's fields.
- Patterns nested in a case: a literal, a range, a variant, or `Some` / `Ok` / `Err` in a variant's field or a payload, to any depth (`PasswordChecked { verified: false, .. }`, `Paid(Method::Card, Some(0))`, `Ok(Some(Dir::Up))`), beside bindings, before guards, in tuple `match`es and `matches!`. Lowered into the decision tree tuple `match`es use; rustc checks exhaustiveness. A tuple holding anything but names and `_`, and a side of `|` testing inside its variant (`A | B(1)`), stay `[pattern/nested]`.
- `check` and `build` warn when the crate's (or workspace's) `[profile.release]` does not set `overflow-checks = true`: generated TypeScript panics on overflow as a debug build does, so a `--release` server that wraps will disagree.
- Runtime panics are `class Panic extends Error` (`instanceof` works across copies via `Symbol.for("purecrate.Panic")`); `message` is still Rust's panic text.
- Generated `package.json` has `license` (from `Cargo.toml`), `sideEffects: false`, and with `--schema` `engines.node >= 21`.

### Changed

- The runtime gains `Iter.tryCollect` and `Str.splitOnce`, each carried only by a package that uses it.
- Collecting a `Vec`, `chars()`, or anything but `s.split(c)`, a `&str` separator for `split`, and `collect` with no target type stay refused. A variant constructor is not a function name (`.map(PreId::Numeric)`); write `|n| PreId::Numeric(n)`.
- A `bool` arm of the decision tree prints as `x` / `!x`, not `x === true`; a field an arm tests inside is read into `$<field>`.
- Constructor parameters of struct variants and of `S.of` are camelCase (`lastError`); field names stay the JSON keys (`last_error`).
- `release.yml` runs `scripts/verify.sh` before drafting or attaching binaries, and smoke-tests the x86_64 macOS binary under Rosetta on the arm64 runner.
- `check` reuses compiled `serde` and `uuid` stand-ins from a per-user cache keyed by `rustc -vV` and the purecrate-ts version, instead of rebuilding the proc-macro on every run.
- Workflows default to `contents: read`; `contents: write` is only on the jobs that draft, upload, or publish. Actions are pinned to commit SHAs. `verify.yml` (and the release verify job) cache the Nix store.
- `packages/boundary*` versions (and the adapters' `purecrate` peer range) follow the workspace version.

### Fixed

- Lets of a union annotate the binding (`const x: T = …`) instead of `x as T` when the value is already that type. A let of a named type from a place keeps `as T` so a `switch` arm can widen a narrowed union. `casts.rs` fails on identity casts of a place.
- Shadowed locals are numbered (`method$1`) only when the name is live in the same JS scope. Match arms reuse the Rust name; sequential `let`s in one function still number.
- `unwrap_or` / `ok_or` / `?` print `x ?? d` and `if (x === null) return Result.err(e)` when the receiver and default are a name, literal, or field (they cannot panic). A default that may overflow still binds first.
- A tuple `match`'s `_` prints as `default:` instead of listing every remaining case. Hoisting `(_, Event::Cancel)` out of every state is a candidate.
- Generated lines wrap at 100 characters, including `if (…) return …;` and long conditions. Parentheses follow operator precedence. `line_width.rs` fails on a generated domain line over the limit.
- The wire module imports each domain file once: value and `type` aliases (and `{E as E$text}` for a `try_from` refusal) sit in a single `import { … } from "./….ts"`.
- README's `cargo install --tag` pin is `v0.7.0`. `scripts/verify.sh` fails when any `--tag vX.Y.Z` in `README.md` or `skills/purecrate-authoring/SKILL.md` disagrees with the workspace version.
- `build --out` keeps `node_modules/` across a rebuild, so an `npm install` in the generated package is not deleted. `dist/` is still dropped (it is stale).
- rustc's scratch directory is a unique 0700 path, created exclusively and removed on drop, instead of `purecrate-rustc-<pid>` in the shared temp dir.
- Copied runtime and adapter files include the MIT copyright and permission notice, not only a link.
- `scripts/verify.sh` and `scripts/examples.sh` ask the binary (`--schema`) whether a crate has a wire form, instead of grepping `src/*.rs`.
- The rustc stand-in cache takes a lock around the first build, so parallel `check` processes do not clobber the serde proc-macro objects.

### Docs

- design/03: constructor parameters are camelCase; fields stay JSON keys. `#[serde(rename_all = "camelCase")]` remains a candidate.
- design/03: a one-field tuple variant stays `{ kind, content: [T] }`; `{ kind, value }` is a candidate (design/07 §9).
- README and design/01: equivalence with a `--release` server holds only with `[profile.release] overflow-checks = true`.
- README, design/01, and design/03: closed-type brands are string-keyed, as generated; closedness is a convention (`$of` not re-exported from `index.ts`) plus a consumer `as` lint, not a `unique symbol` seal. Vendoring can import `$of` directly.
- design/03 next to the `Option` mapping, and design/02: nested `Option` stays refused because `T | null` (and serde's default JSON) cannot tell the two `None`s apart.
- design/07: growing a `Vec` with `push` / `iter().map(f).collect()` is a candidate, still refused until an example cannot be written without it.
- README and design/07 §9: generated API stability within a minor series (export names, type shapes, wire format, runtime API vs. formatting, internal helpers, local names).
- design/03 §3.3.5: locals are numbered only in the same JS scope; match arms reuse the Rust name.
- design/03 §5.2: panics are `Panic`, not a plain `Error`.
- Vendoring generated sources needs `allowImportingTsExtensions` (with `noEmit` or a bundler) or `rewriteRelativeImportExtensions` when emitting, or the package / `purecrate-source` route.
- design/02 merges the duplicate "Text lists" / "A list from text" rows.
- design/03 documents `Str.splitOnce` and `Iter.tryCollect`; design/03, 04, and 05 are marked current at 0.7.0 (04 reviewed, unchanged).
- design/02, 03, 07 and the authoring skill: patterns nested in a case.

### Examples

- semver (SemVer 2.0.0 parsing and precedence) builds its identifier lists with `collect` and splits on `+` and `-` with `split_once` as `Some((x, y))`: 166 lines of logic with ordering, 138 with these (1.8× the idiomatic Rust). Pre-release and build metadata are `Vec`s.
- oidc's lockout arm matches `PasswordChecked { verified: false, .. }`, as its idiomatic code does.

## 0.6.0 — 2026-10-01

Generated TypeScript that reads as written by hand: camelCase names, JSDoc from `///`, runtime helpers in place of inline functions, and lines broken at 100 characters. The wire module gives a form only to what serde does, reads text with `fromJson`, and reports a refused value in serde's words. Packages generated with 0.5.0 must be regenerated, and their callers updated (Breaking, below). Measured by reading the examples' output ([roadmap §8.7](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#87-060-generated-typescript-2026-10-01)).

### Breaking

- **camelCase.** Functions, methods, consts, parameters, and locals print in camelCase: `compare_pre_ids` → `comparePreIds`, `Yen::try_from` → `Yen.tryFrom`. Fields, types, and variants keep the Rust name (a field is the JSON key), and UPPER_SNAKE consts are unchanged. Callers use the new names. A name that would print as a TS reserved word or as a name the package defines (`parse_json` → `parseJson`), and two methods of one type that print alike, are rejected.
- **Only serde's derives make a wire form.** `--schema` writes a schema for each public type that derives `Deserialize` and a `toJson` entry for each that derives `Serialize`; before, every public type had both, so a closed type with no derive (signup's `Email`) could be built from JSON by shape, which Rust cannot do at all. A crate where no public type derives either is refused with `--schema`.
- **What a derive holds.** A type that derives `Serialize` or `Deserialize` must hold only types that derive it too, as the real derive requires; `check` reports the rest as `item/serde-derive`. `std::cmp::Ordering` is now refused only in such a type.
- **Fewer index exports.** The index exports `Char` only when the public surface holds a `char`, `Uuid` and `UuidError` only when it holds a `Uuid`, and `parseJson` only with `--schema`. Stop importing one from a package that no longer exports it.
- **Reserved names.** `Slice`, `Ord`, and `Iter` name runtime objects the generated code calls. `result`, `assert-never`, `int`, and `str` are free as file stems.

### Added

- `fromJson.T(text)`, the inverse of `toJson.T`: serde_json's text read through `parseJson` and the schema, so a 64-bit integer past 2^53 stays exact; malformed text or a refused value throws, as `JSON.parse` and the library's `parse` do.
- `impl Display` whose `fmt` writes a text fixed per value (`f.write_str("..")`, `write!(f, "..")` without arguments, a `match self` of those, or `let t = <such a match>; f.write_str(t)`) becomes `X.toString`, and `x.to_string()` in the crate calls it; any other `Display` is skipped as before. invoice's and payment's errors have it.
- A refused `#[serde(try_from)]` read says what serde says, the error's `Display` text (`Amount: amount must be 50 to 99999999`), where there is one; the variant's name otherwise.
- `///` and `/** */` comments carry over as JSDoc on types, fields, variant constructors, functions, methods, consts, aliases, and wire schemas.

### Changed

Generated code, with the same behavior, checked by the differential tests:

- Runtime helpers instead of inline code: `Slice.at` / `Slice.range` for indexing and slicing (iban 6.0 → 4.9 KB), `Ord.cmp` / `Ord.cmpStr` / `Ord.then` for `cmp` and `then` (semver's `compare` 80 → 44 lines), and `Iter.all` / `any` / `position` / `count` / `sum` with the closure as an arrow. Each evaluates its arguments once in Rust's order and stops where std stops.
- `?` and `ok_or`: `let x = o.ok_or(e)?` is a guard, `if ($o === null) return Result.err($oOr);` (15 lines → 4, `e` still first); a `?` inside an expression binds once, `const $f = f(x); if ($f.kind === "Err") return $f;`, read as `$f.value`.
- `match`: a lowered `match` binds the arm's own names, not a `$f1` copied with a cast; a `match` or `matches!` on a place inside an expression is `?:` or `||` / `&&` on the arms' tests; a `match` of `true` / `false` arms prints as its test; `return match ..` returns from each arm. An inline function remains only for a `match` on a value that is not a place, inside an expression, where hoisting would change when it runs (payment 22.3 → 20.6 KB).
- Names the generator makes say what they hold: `$parseCoreNumber`, `$major` / `$majorOr`, not `$q1`, `$opt9`, `$arg11`.
- Brands are keyed by string, `{ readonly "invoice.Yen": true }`, as the runtime's are: the key reads in a hover, and two copies of one crate's package exchange values.
- `Result` and `assertNever` live in the runtime copy, each file imports what it uses in one line, and `result.ts`, `assert-never.ts`, `int.ts`, and `str.ts` are gone (counter 8 files → 5). The runtime copy keeps only what the package uses and exports (counter 11.9 → 6.5 KB, payment 15.8 → 10.3 KB).
- Layout: lines past 100 characters break inside their outermost bracket, one item per line, as prettier does; no parentheses around a whole condition, `return` value, or initializer; field shorthand; `!(a === b)` as `a !== b`; `if (c) return v;` on one line; a body of one `return` is an expression-bodied arrow; `Box<T>` and friends are `/* Box */ T` in a type and nothing in an expression.

Wire module:

- An enum whose variants all are unit is the adapter's `unitEnum([..])` (payment's arktype module 454 → 356 lines).
- A struct is the object schema alone, without a `.transform` copying every field, except for a closed struct, a `()` field, or valibot's `Option` field.
- `toJson` writes through `Json.object` and `Json.tuple`, broken one field per line, instead of a one-line template literal; the bytes are the same.
- The module is wrapped and documented as the rest of the package is, and a transform reads its value as `x`, not valibot's `v`.

### Fixed

- A `match` on an enum of one variant failed `tsc`: TS does not narrow a type that is not a union, so `assertNever` stayed reachable. The one arm prints without a `switch`.
- The runtime copy no longer has runs of blank lines where unused parts were left out.

### Tests

- `casts.rs` fails on an `as` in any example's or fixture's output that is not of a kind design/03 §1.1 lists, and on a cast to one of the crate's brands outside its constructor.
- `display_equivalence.rs` checks `toString` against Rust's `to_string()`; wire tests cover `fromJson`, `unitEnum`, and the derive rules.

### Examples

- Each example keeps its generated package beside its source: `examples/<name>/ts/plain`, and for invoice and payment, which derive serde, `examples/<name>/ts/<lib>` per schema library. `scripts/examples.sh` regenerates them; `scripts/verify.sh` checks them for drift and runs `tsc` over them.

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
