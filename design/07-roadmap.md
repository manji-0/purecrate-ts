# Roadmap

Status: current (2026-09-30, after 0.3.0)

<!-- constrained-by ./02-authoring.md -->
<!-- constrained-by ./06-strategy.md#4-success-and-withdrawal-criteria -->

## 1. How additions are chosen

**Add a capability only when an example written within the constraints cannot be written without it.** Rejection counts in a corpus do not set the order ([90](./90-acceptance-survey.md) is not a metric). Two exceptions skip the wait:

- **Holes** — anything that makes accepted input compute a wrong value, or lets `check` accept what rustc rejects, is fixed immediately.
- **Domain fixes** — changes that tighten the domain of equivalence (closed types) rather than add capability.

Every addition comes with a differential test.

## 2. Evidence from examples

| Example | Source | Got stuck on | Added |
| --- | --- | --- | --- |
| counter | author | — | acceptance criterion |
| vending, control | author | — | test fixtures for `?` and early `return` (all 1296 sequences) |
| order (5 states, `Yen`, `Sku`) | author | quantities as `u32` × `i64` price; `String` literals in a test driver | lossless widening (`widen_equivalence.rs`, 20 widenings at both ends); `String::from` (and closed the literal-in-`String` hole) |
| signup (WHATWG email, NIST SP 800-63B-4 length) | third party | nothing readable from a string but `==`; `Ok(())` rejected as `expected (), found ()` and printed as `[]`; `email` vs `Email` file collision | `str::as_bytes`; `Lit::Unit` fix; renamed the driver (known rule) |
| iban (ISO 13616-1, ISO 7064 MOD 97-10) | third party | nothing; but line count over threshold | integer-range `for` |
| invoice (NTA インボイスQ&A 問57, 問59: consumption tax per rate, rounded once per invoice) | third party | nothing; rejected on the way: a tuple `let`, match guards (three times), a tuple scrutinee, `Vec::is_empty`, `Group`/`group` file collision; the first draft's transitions were 2.2×, a restructured one (a filtered sum per group, as the idiomatic code does) 1.4× | a `Js` literal for every fixture type, so tests pass whole values (`Invoice`) |
| oidc (OIDC Core 1.0 OP login, PKCE RFC 7636, TOTP RFC 6238/4226 as the second factor, amr RFC 8176) | third party, from the authoring skill alone | RFC 4226 truncation's `&` and `<<` (written with `%` and `*`); no way to build a `Vec` for `amr` (a recursive enum instead; the documents' "build with `[a, b]`" was wrong); `const`; `==` on `Option` and enums, `is_some`; `u8::is_ascii_digit`; 15 rejections, about 6 not predictable from the skill | documents corrected (below); later `is_some`/`is_none`, bit operators, and `vec![a, b]` (§3.1) |
| payment (Stripe PaymentIntent lifecycle) | third party | nothing on the first pass; but transition lines at 2.1× (28 of 160 were `=> Err(InvalidTransition)`); the client sends events back, so values must be written as serde JSON; `terms`/`outcome`/`amount` driver functions collided with types (known rule) | `_` and binding-free `A \| B` arms; `toJson` (and rejected unit structs, which serde writes differently from `struct S {}`) |

Semantic cross-checks beyond Rust-vs-TS: signup's acceptance matches WHATWG's own regular expression on node; iban matches an idiomatic-Rust implementation on published valid IBANs, one-character mutations, and malformed input; payment matches an idiomatic-Rust implementation (tuple `match` with guards and a wildcard) on every four-event run under each capture and confirmation method. For payment the client-side step on the server's JSON also writes the server's bytes for every reachable state and event (`wire_write.rs`).

Line counts (non-blank, non-comment) against idiomatic Rust, threshold 2×:

| Example | Idiomatic | Recursion only | With range `for` | With byte literals and `matches!` |
| --- | --- | --- | --- | --- |
| Email (WHATWG) | 28 | 79 (2.8×) | 52 (1.9×) | 42 (1.5×) |
| Password (NIST) | 17 | 28 (1.6×) | 24 (1.4×) | 24 (1.4×) |
| IBAN | 24 | 57 (2.4×) | 45 (1.9×) | 44 (1.8×) |

With range `for` all were inside the threshold with little margin, most of the rest being character classes spelled as numeric comparisons. Byte literals, integer literal and range patterns, and `matches!` were added next (2026-09-29, `int_patterns_equivalence.rs`). They took the local part of the e-mail address from 14 lines of comparisons to one `matches!`; IBAN barely moved, as its gap is the loops. A `match … { … => true, _ => false }` without `matches!` was longer than the comparisons it replaced (IBAN 50 lines), which is why `matches!` came with it.

2026-09-29, after the above: payment's ID check read `b[0] != 112u8 || b[1] != 109u8 || b[2] != 95u8`; `str::len`, `is_empty`, `starts_with`, `ends_with`, and `contains` were added to the allow-list, and it reads `!raw.starts_with("pm_")`.

Payment, transition logic only (the types are the same length, 94 lines, on both sides):

| Example | Idiomatic | One arm per variant | With `_` and `A \| B` |
| --- | --- | --- | --- |
| PaymentIntent (Stripe) | 76 | 160 (2.1×) | 135 (1.8×) |

The rest of the gap is one function per state instead of a tuple `match`, and `match` on `Option` where idiomatic code calls `ok_or`, `unwrap_or`, `map`, and `min`.

2026-09-29, preparing payment for a real server: a crate that derived serde failed `check` (rustc had no serde), and closed types could not keep their invariants on the wire. `check` now compiles against a stand-in serde; `#[serde(try_from = "T")]` with `impl TryFrom<T>` is accepted, and `impl Display` / `Error` are skipped ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). payment reads `Amount` and `PaymentMethodId` through their constructors on both sides.

Invoice, logic only (types differ mostly by derive lines and the checked `Yen`):

| Example | Idiomatic | First draft | Restructured |
| --- | --- | --- | --- |
| Invoice (NTA) | 48 | 104 (2.2×) | 68 (1.4×) |

The first draft kept four running totals in a struct updated line by line; the restructured one sums each group with a range `for`, as the idiomatic code sums with `filter`. Every rejection on the way had a subset spelling. Match guards were the most frequent (three in invoice, and the idiomatic payment uses two): the candidate if a guard is ever the only way to keep an example under the threshold.

The NTA's own worked examples (60,000 × 10/110 ≒ 5,454; 23,894 × 10% ≒ 2,389 where rounding per line would give 2,388; 問59's receipt at 948 both ways) are asserted in `invoice_equivalence.rs`, next to an idiomatic-Rust cross-check and the Rust/TS differential test (about 11,400 invoices, overflow included).

OIDC, 2026-09-30, written by an agent that read only the authoring skill (not this document or 02), to find what the skill leaves out:

| Part | Idiomatic | Constrained |
| --- | --- | --- |
| Types | 198 | 203 (1.0×) |
| Lexical helpers (token and byte scanning) | 31 | 99 (3.2×) |
| Request validation | 83 | 153 (1.8×) |
| TOTP | 47 | 81 (1.7×) |
| State machine and token endpoint | 183 | 251 (1.4×) |
| Logic total | 375 | 617 (1.65×) |

The gap is the scanners: iterator adaptors over tokens and bytes, written as range `for` with index bookkeeping. RFC 4226 truncation without bit operators is correct only because the top bit is masked (`% 128`) before multiplying; a spec that needs `^` or rotations (a hash) would not be writable. The skill had one wrong line (building a `Vec` from `[a, b]`, also in 02 §3.1) and lacked the method allow-list, bit operators, `const`, the enum/`Option` equality rewrites, the phase order of diagnostics, and the by-shape reading of closed types under serde; all were added. Diagnostics to improve: `[expr/operator]` does not name the operator, `==` on enums is reported as `numeric-op`, `[expr/method-call]` says "only methods of the crate's own inherent impls" for std methods outside the allow-list, and some locations point at the statement rather than the call. All four fixed 2026-09-30: the operator is named; `==` and ordering on a type JS compares differently are `[check/comparison]`, with the rewrite for `Option`, the crate's types, and strings; a std method outside the allow-list lists what its receiver allows (and, on `u8`, the `matches!` spelling), a missing method on the crate's own type says so; calls and method calls carry their own position.

2026-09-30, after bit operators, `const`, and discriminants were added, oidc was rewritten with them: the acr values and length limits are consts, `OtpDigits` carries its digit count as the discriminant (`d as u8`), and RFC 4226 truncation reads as §5.4 writes it (`& 0x0f`, `<< 24 |`) instead of `% 16` and a base-256 sum. The non-blank line count barely moves (812 to 810: each named limit costs a declaration), but no number in the file is left unexplained, and the one comment that described a workaround is gone.

The type/function file-name collision (§3.3 rule 5 of [02](./02-authoring.md#33-names-are-unique-across-the-crate)) has now been hit four times: three driver helpers named after the type they build, and `fn group` returning a `Group` in invoice itself. The rule stays; the diagnostic names both items.

## 3. Next

<!-- derived-from ./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts -->

Decided 2026-09-30, from measuring real dual implementations ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)); the first real use (Oxide `Name`) passes without them, but its rewrite is longer and less idiomatic for their absence:

1. **`Result<Self, Self::Error>` in `impl TryFrom<T>`** — done 2026-09-30. `check` and `build` already accepted it; `survey` skipped trait impls, so a `#[serde(try_from)]` type looked as if its `impl TryFrom` were missing (`item/serde-attr`). `survey` now reads the impl as `X::try_from`.
2. **`s.chars()` through iterator `for`** — done 2026-09-30: `for c in s.chars()` only (not iterator `for` in general), code points as `char`; replaces hand-decoded UTF-8 (`for_chars_equivalence.rs`).

Defects found by the same measurement, all fixed 2026-09-30: a non-`pub` method reachable on the exported companion, a hole in closed types ([01 §4](./01-equivalence.md#4-closed-types)); `mod r#impl;` looked up as `r#impl.rs` (skipped by `survey`, an error in `check`); `build --out` emptying a directory it had not written (now refused).

From the Oxide `Name` vendoring ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)): the runtime copied into packages (first as `--bundle-runtime`, then for every package, with string-keyed brands so packages still exchange values); `assertNever`'s unused parameter fixed. Bindings the Rust leaves unused, fixed 2026-09-30: the generated `tsconfig.json` sets `noUnusedLocals` and `noUnusedParameters`, so every differential test type-checks under them, and what that turned up is fixed (unused `let`s, arm and `if let` bindings, write-only `let mut`s, parameters, `for` variables, the `?` temporary of `x?;`, and imports gathered from the IR but not printed; `unused_equivalence.rs`). A consumer's CI had no published `purecrate-ts` to run `check --out` with; since 0.1.0 (2026-09-30) each release tag carries Linux and macOS binaries, and `cargo install --git … --tag` builds the same version.

`uuid::Uuid` added 2026-09-30, on one example's evidence (Oxide `Name` hand-wrote `Uuid::parse_str` for the two forms its earlier checks let through, a copy that would drift if the check order or the crate changed) and on how common UUID identifiers are in Rust domain code; a deliberate exception to waiting for a second example ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)).

### 3.1 When an example needs it

1. **Strings and `char`** — as specified in [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods), one method at a time. `char` and `for c in s.chars()` are in.
2. **Iteration** — `while`, `break`/`continue`, iterator `for`, when range `for` plus recursion is not enough. Planned for 0.2.0 (§8).
3. **std methods** the example calls, via the allow-list. `Vec::is_empty` and `Option::is_some`/`is_none` (three of four real sites and oidc) are in, 2026-09-30. Iterator `map`/`filter`/`collect` are not added: they are how state sequences grow as arrays.
4. **Tuple scrutinees** — `match (state, event)`, if one function per state keeps an example over the threshold after `_`. Done 2026-09-30: tuple arms of `_`, bindings, and arm patterns, `|` of whole tuples binding nothing; not nested tuples (`tuple_match_equivalence.rs`). Printed as nested `switch`es, one per element, so TS checks exhaustiveness too rather than trusting rustc's; the cost is copies of arms that several cases reach.
5. **Bitwise operators and shifts** on the fixed-width integers — asked for by oidc and Stoat's permission flags (both worked around), needed by any spec written in them. Done 2026-09-30 (not on `usize` or `bool`).
6. **Building a `Vec` in the crate** (`vec![a, b]` of a fixed length) — oidc's `amr` is a list claim that TS callers expect as an array. Done 2026-09-30: `vec![a, b]` and `vec![]` print as array literals; `vec![x; n]` and every way of growing one stay rejected (`vec_build_equivalence.rs`). oidc's `AmrList` enum is now `Vec<String>`, 8 lines shorter, and the cross-check against the idiomatic reference no longer rewrites one side's `Debug` text.
7. **`const` and enum discriminants** — oidc (`const`) and Stoat (18 permission bits as functions, `as` on the enum). Done 2026-09-30: crate-level `const` of integers, floats, `bool`, `char`, `&str`, folded at check time into `consts.ts`; discriminants on fieldless enums with `#[repr(<int>)]`, and `e as T` when `T` holds every discriminant (`flags_equivalence.rs`, a permission set in Stoat's shape). Found on the way: with consts accepted, `Some(MAX) =>` would bind `MAX` rather than compare with it, so a const used as a pattern is refused; and one file per const collided with the usual `MAX_LEN` / `fn max_len` pair, so all consts share `consts.ts`.

## 4. Specified but not yet implemented

`chars()` other than as `for c in s.chars()`; the Unicode-table `char` methods; byte slicing, `String` ordering; `isize`; `while`, `loop`, `break`/`continue`, `a..=b` in `for`, iterator `for`; byte string literals; the std allow-list beyond `Vec::len`, `Vec::is_empty`, `Option::is_some`/`is_none`, indexing, `str::as_bytes`, `len`, `is_empty`, `starts_with`, `ends_with`, `contains`, `String::as_str`; `static` and associated consts.

## 5. v1: when type expressiveness runs out

Waits for an example that cannot be written without it.

- Unbounded type parameters, emitted as TS generics (no monomorphization). No bounds, `where`, or associated types.
- `HashMap`/`BTreeMap` with `String` keys only, as `ReadonlyMap<string, V>`. Insertion order is not matched; functions depending on it are rejection candidates.

## 6. Not doing

- Allow-lists aimed at passing existing crates.
- Decimals; growable `Vec`; event logs inside state.
- A schema-library dependency in the core runtime.
- WASM. The IR does not preclude a second backend, but the path is TS source.
- `Rc` / `Cell` / `RefCell`.

## 7. Open questions

- Should output typing come from rustc's type information instead of the in-house inference ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate))?
- Hermes support for `JSON.parse` source text ([04 §7](./04-wire.md#7-open-questions)).
- A shared error type with field paths for validation ([01 §4](./01-equivalence.md#4-closed-types)).
- Demand: see [06 §5](./06-strategy.md#5-validating-demand-next).

## 8. Releases

<!-- derived-from #2-evidence-from-examples -->
<!-- constrained-by #1-how-additions-are-chosen -->

0.1.0 (2026-09-30) shipped what §3.1 lists as done. Decided the same day: later releases take language capabilities first, in the order the examples' measured gaps give, and adoption is left to happen on its own, with no outreach and no requests to adopt ([06 §5](./06-strategy.md#5-validating-demand-next)). Distribution work waits for someone to need it.

### 0.2.0: iteration (released 2026-09-30)

The largest gap measured so far: oidc's lexical helpers at 3.2× idiomatic Rust (§2), and Windmill's scope and pipeline parsers ([91 §2](./91-real-use-candidates.md#2-fit-measured)), all index bookkeeping over bytes and tokens that idiomatic code writes as iteration.

- `for x in &xs` and `for x in xs.iter()` over a `Vec` or slice, `for b in s.bytes()`: printed as `for..of`, one binding, the body rules of range `for`. Done 2026-09-30 (`for_each_equivalence.rs`); also `for x in xs` on an owned `Vec` and `s.as_bytes()`, and the named adaptors are refused with a message.
- `while`, `break`, `continue`: no labels, no `loop` with a value. Done 2026-09-30 (`while_break_equivalence.rs`): every loop a jump leaves gets a label, since a bare JS `break` in the `switch` a `match` prints as leaves the `switch` (a run without labels fails the test); a `?` in the condition turns the loop into `while (true)` that computes the condition first. The generated tsconfig also sets `allowUnreachableCode: false`, and no `break;` follows a jump in a `case`.
- oidc's scanners rewritten with them and measured again against the idiomatic reference. Done 2026-09-30: the byte checks (`state`, PKCE strings, `max_age`, the OTP), the redirect-URI lookup, and the TOTP candidate loop now use `for b in s.bytes()` and `for x in &xs`. The lexical helpers went from 99 to 95 lines (3.1× the idiomatic 31), TOTP from 104 to 102, the file from 810 to 804. Smaller than expected: most of the gap is splitting a space-delimited list into tokens (`span_equals`, `has_token`, `parse_prompt`), which idiomatic code writes as `split(' ')` and which needs the token boundaries that a plain `for` does not give. `for t in s.split(' ')` over an ASCII separator is the evidence-backed candidate after this (splitting at an ASCII byte cuts at the same places in UTF-8 and UTF-16, and JS `split` keeps the empty tokens Rust keeps).
- `for t in s.split(c)` with a `char` separator, added to 0.2.0 on that evidence (any code point splits alike in UTF-8 and UTF-16; a `&str` separator is refused, since an empty one does not). Done 2026-09-30: oidc's `has_token` and `parse_prompt` read their lists with it and `span_equals` is gone; the lexical helpers went from 95 to 68 lines (2.2× the idiomatic 31), the file to 777.

### 0.3.0: guards and `Option` (released 2026-09-30)

- Match guards: invoice reached for them three times and the idiomatic payment uses two. A guarded arm is tried in order and does not count toward exhaustiveness, in the TS check as in rustc's. Done 2026-09-30 (`guards_equivalence.rs`), also on binding arms and in `matches!`. A first version put the arms after a guard inside the `_` of a `switch` on the same value, which TS narrowed so that the later `switch` could not name the other variants; each test and take is now a standalone `match` in an `if` chain.
- `Option::unwrap_or`, `ok_or`, and `map` with a closure: most of payment's remaining gap (§2). `unwrap_or(e)` evaluates `e` first, as Rust does. Done 2026-09-30 (`option_methods_equivalence.rs`); `map` also takes a function name.
- `survey` reporting every cause per function, to estimate a rewrite ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). Done 2026-09-30 as `survey --all-causes`: an expression or statement the parser cannot take is recorded and stood in for, so the rest of the item is still lowered and type-checked; the default output is unchanged.
- Wire schemas (added on request, not from an example's gap): zod and valibot print dependencies first with `lazy` only for recursive types, one arm or field per line when long, through the adapters' `unitVariant` and `optionalField`; a refused `try_from` names the error's variant in all three libraries; `toJson` writes a fieldless enum in one line. Behavior unchanged (`wire.rs`, `wire_write.rs`, with mutually recursive and later-declared types added). The zod adapter and the generated zod code moved to zod 4.6 the same day (peer `^4.6.0`; `z.ZodType<T, In>` without `ZodTypeDef`, issues with `code: "custom"`): a breaking change for users on zod 3, who stay on purecrate-ts 0.2. arktype's schemas keep nested errors at their path instead of replacing them with one at the type (`errors_keep_their_location` in `wire_write.rs`).

### Later, on evidence

- Generics and string-keyed maps (§5). `format!`, asked for by Windmill only so far; `Display` of floats is a large surface to match.
- crates.io and Windows binaries, when a user asks. Since 0.3.0 the dependencies are crates.io requirements, vendored by source replacement.
- 1.0: a compatibility policy for the output bytes and the reason codes, and every withdrawal criterion of [06 §4](./06-strategy.md#4-success-and-withdrawal-criteria) answered. The real-use criterion stays open until the tool is adopted unprompted; Oxide `Name` remains local evidence ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)).
