# Roadmap

Status: current (2026-10-01, after 0.4.0)

<!-- constrained-by ./02-authoring.md -->
<!-- constrained-by ./06-strategy.md#4-success-and-withdrawal-criteria -->

## 1. How additions are chosen

**Add a capability only when an example written within the constraints cannot be written without it.** Rejection counts in a corpus do not set the order ([90](./90-acceptance-survey.md) is not a metric). Two exceptions skip the wait:

- **Holes** — anything that makes accepted input compute a wrong value, or lets `check` accept what rustc rejects, is fixed immediately.
- **Domain fixes** — changes that tighten the domain of equivalence (closed types) rather than add capability.

Every addition comes with a differential test.

## 2. Evidence from examples

Per example: where `check` stopped it (§2.1), its length against idiomatic Rust (§2.2), what it was checked against beyond Rust-vs-TS (§2.3), and what else the measurements changed (§2.4).

### 2.1 What each example hit

| Example | Source | Got stuck on | Added |
| --- | --- | --- | --- |
| counter | author | — | acceptance criterion |
| vending, control | author | — | test fixtures for `?` and early `return` (all 1296 sequences) |
| order (5 states, `Yen`, `Sku`) | author | quantities as `u32` × `i64` price; `String` literals in a test driver | lossless widening (`widen_equivalence.rs`, 20 widenings at both ends); `String::from` (and closed the literal-in-`String` hole) |
| signup (WHATWG email, NIST SP 800-63B-4 length) | third party | nothing readable from a string but `==`; `Ok(())` rejected as `expected (), found ()` and printed as `[]`; `email` vs `Email` file collision | `str::as_bytes`; `Lit::Unit` fix; renamed the driver (known rule) |
| iban (ISO 13616-1, ISO 7064 MOD 97-10) | third party | nothing; but line count over threshold | integer-range `for` |
| invoice (NTA インボイスQ&A 問57, 問59: consumption tax per rate, rounded once per invoice) | third party | nothing; rejected on the way: a tuple `let`, match guards (three times), a tuple scrutinee, `Vec::is_empty`, `Group`/`group` file collision; the first draft's transitions were 2.2×, a restructured one (a filtered sum per group, as the idiomatic code does) 1.4× | a `Js` literal for every fixture type, so tests pass whole values (`Invoice`) |
| oidc (OIDC Core 1.0 OP login, PKCE RFC 7636, TOTP RFC 6238/4226 as the second factor, amr RFC 8176) | third party, from the authoring skill alone | RFC 4226 truncation's `&` and `<<` (written with `%` and `*`); no way to build a `Vec` for `amr` (a recursive enum instead; the documents' "build with `[a, b]`" was wrong); `const`; `==` on `Option` and enums, `is_some`; `u8::is_ascii_digit`; 15 rejections, about 6 not predictable from the skill | documents corrected (§2.4); later `is_some`/`is_none`, bit operators, and `vec![a, b]` (§3.1) |
| payment (Stripe PaymentIntent lifecycle) | third party | nothing on the first pass; but transition lines at 2.1× (28 of 160 were `=> Err(InvalidTransition)`); the client sends events back, so values must be written as serde JSON; `terms`/`outcome`/`amount` driver functions collided with types (known rule) | `_` and binding-free `A \| B` arms; `toJson` (and rejected unit structs, which serde writes differently from `struct S {}`) |

### 2.2 Line counts against idiomatic Rust

Non-blank, non-comment lines, threshold 2×.

**signup and iban.** Email (WHATWG) and password (NIST) are signup's validators.

| Example | Idiomatic | Recursion only | With range `for` | With byte literals and `matches!` |
| --- | --- | --- | --- | --- |
| Email (WHATWG) | 28 | 79 (2.8×) | 52 (1.9×) | 42 (1.5×) |
| Password (NIST) | 17 | 28 (1.6×) | 24 (1.4×) | 24 (1.4×) |
| IBAN | 24 | 57 (2.4×) | 45 (1.9×) | 44 (1.8×) |

- With range `for`, all were inside the threshold with little margin. Most of the rest was character classes spelled as numeric comparisons.
- Byte literals, integer literal and range patterns, and `matches!` were added next (2026-09-29, `int_patterns_equivalence.rs`). They took the local part of the e-mail address from 14 lines of comparisons to one `matches!`. IBAN barely moved, as its gap is the loops.
- A `match … { … => true, _ => false }` without `matches!` was longer than the comparisons it replaced (IBAN 50 lines), which is why `matches!` came with it.

**payment**, transition logic only (the types are the same length, 94 lines, on both sides):

| Example | Idiomatic | One arm per variant | With `_` and `A \| B` |
| --- | --- | --- | --- |
| PaymentIntent (Stripe) | 76 | 160 (2.1×) | 135 (1.8×) |

The rest of the gap is one function per state instead of a tuple `match`, and `match` on `Option` where idiomatic code calls `ok_or`, `unwrap_or`, `map`, and `min`.

**invoice**, logic only (types differ mostly by derive lines and the checked `Yen`):

| Example | Idiomatic | First draft | Restructured |
| --- | --- | --- | --- |
| Invoice (NTA) | 48 | 104 (2.2×) | 68 (1.4×) |

The first draft kept four running totals in a struct updated line by line. The restructured one sums each group with a range `for`, as the idiomatic code sums with `filter`.

**oidc**, 2026-09-30, written by an agent that read only the authoring skill (not this document or 02), to find what the skill leaves out:

| Part | Idiomatic | Constrained |
| --- | --- | --- |
| Types | 198 | 203 (1.0×) |
| Lexical helpers (token and byte scanning) | 31 | 99 (3.2×) |
| Request validation | 83 | 153 (1.8×) |
| TOTP | 47 | 81 (1.7×) |
| State machine and token endpoint | 183 | 251 (1.4×) |
| Logic total | 375 | 617 (1.65×) |

The gap is the scanners: iterator adaptors over tokens and bytes, written as range `for` with index bookkeeping. RFC 4226 truncation without bit operators is correct only because the top bit is masked (`% 128`) before multiplying; a spec that needs `^` or rotations (a hash) would not be writable.

oidc was rewritten as capabilities were added (all 2026-09-30):

| Rewritten with | Lexical helpers | TOTP | File |
| --- | --- | --- | --- |
| bit operators, `const`, discriminants | — | — | 812 → 810 |
| `for b in s.bytes()`, `for x in &xs` (0.2.0) | 99 → 95 (3.1× the idiomatic 31) | 104 → 102 | 810 → 804 |
| `for t in s.split(c)` (0.2.0) | 95 → 68 (2.2× the idiomatic 31) | — | → 777 |

- **Bit operators, `const`, discriminants.** The acr values and length limits are consts, `OtpDigits` carries its digit count as the discriminant (`d as u8`), and RFC 4226 truncation reads as §5.4 writes it (`& 0x0f`, `<< 24 |`) instead of `% 16` and a base-256 sum. The non-blank line count barely moves, since each named limit costs a declaration. But no number in the file is left unexplained, and the one comment that described a workaround is gone.
- **`for b in s.bytes()`, `for x in &xs`.** Used by the byte checks (`state`, PKCE strings, `max_age`, the OTP), the redirect-URI lookup, and the TOTP candidate loop. Smaller than expected: most of the gap is splitting a space-delimited list into tokens (`span_equals`, `has_token`, `parse_prompt`), which idiomatic code writes as `split(' ')` and which needs the token boundaries that a plain `for` does not give. That made `for t in s.split(' ')` over an ASCII separator the evidence-backed candidate after this (splitting at an ASCII byte cuts at the same places in UTF-8 and UTF-16, and JS `split` keeps the empty tokens Rust keeps).
- **`for t in s.split(c)`.** `has_token` and `parse_prompt` read their lists with it and `span_equals` is gone.
- **`vec![a, b]`** (§3.1). oidc's `AmrList` enum is now `Vec<String>`, 8 lines shorter.

**0.4.0 rewrites** (2026-09-30). oidc, payment, and invoice rewritten with guards, tuple `match`, the `Option` methods, and the 0.4.0 items, public API and behavior unchanged (their differential and idiomatic cross-check tests pass). Counted per section for oidc as above, and for payment and invoice as types (structs, enums, trait impls) against logic (functions and inherent impls); the idiomatic references in the tests are counted the same way.

| Part | Idiomatic | Before | After |
| --- | --- | --- | --- |
| oidc lexical helpers | 31 | 68 (2.2×) | 47 (1.5×) |
| oidc request validation | 83 | 153 (1.8×) | 108 (1.3×) |
| oidc TOTP | 47 | 102 (2.2×) | 88 (1.9×) |
| oidc state machine and token endpoint | 183 | 328 (1.8×) | 260 (1.4×) |
| oidc file | — | 777 | 629 |
| payment logic | 36 | 163 | 111 |
| invoice logic | 34 | 74 | 66 |

payment's and invoice's idiomatic references are written one arm or field per line, so their ratios read high against rustfmt output; the before and after columns are comparable with each other. Formatted alike with rustfmt at width 120 (`scripts/line-counts.py`, 2026-10-01), logic against logic: iban 44 / 24 (1.8×), invoice 75 / 48 (1.6×), oidc 423 / 327 (1.3×), payment 96 / 61 (1.6×).

- **Used:** guards everywhere a state or a field chose the path (oidc's request validation and `begin`, payment's manual capture and confirmation, invoice's `share`); tuple `match` (oidc's `step` in place of three handlers, PKCE and code redemption; payment's `step` in place of five per-state functions; invoice's `share`); `ok_or`, `unwrap_or`, `map` (payment's four `Option` matches, oidc's `prompt`, `max_age`, and freshness); `all` and `any` (oidc's byte checks and redirect-URI lookup); `min` (payment's fee cap) and `pow` (oidc's digit modulus).
- **Not used by these three:** slicing and `strip_*`, `const` in a function, `position`, `count`, `sum`, `enumerate`, the `checked_*` / `saturating_*` / `wrapping_*` forms, tuple `let` (tried in invoice; with long names rustfmt made it longer).
- **Refused, from oidc:** `list.split(' ').any(|t| t == word)` (a consumer after `split`) and `bool` literals in a tuple `match` (`(true, None, _)`), both added since (§8; the lexical helpers' 47 includes the first); a literal or a variant inside a variant's fields (`PasswordChecked { verified: false, .. }`, `SecondFactor::Totp(e)` inside an `Event` pattern), which the idiomatic `step` relies on; a local closure's parameter type is not inferred from its later calls (`|error: ErrorCode|` needed).

### 2.3 Semantic cross-checks

Beyond the Rust-vs-TS differential tests:

| Example | Checked against |
| --- | --- |
| signup | WHATWG's own regular expression, on node |
| iban | an idiomatic-Rust implementation, on published valid IBANs, one-character mutations, and malformed input |
| payment | an idiomatic-Rust implementation (tuple `match` with guards and a wildcard), on every four-event run under each capture and confirmation method. The client-side step on the server's JSON also writes the server's bytes for every reachable state and event (`wire_write.rs`). |
| invoice | the NTA's own worked examples (60,000 × 10/110 ≒ 5,454; 23,894 × 10% ≒ 2,389 where rounding per line would give 2,388; 問59's receipt at 948 both ways), asserted in `invoice_equivalence.rs` next to an idiomatic-Rust cross-check and the Rust/TS differential test (about 11,400 invoices, overflow included) |
| oidc | the idiomatic reference; since `vec![a, b]` (§3.1) the cross-check no longer rewrites one side's `Debug` text |

### 2.4 What the measurements changed

Besides the capabilities in §2.1:

- **String methods, from payment** (2026-09-29). payment's ID check read `b[0] != 112u8 || b[1] != 109u8 || b[2] != 95u8`. `str::len`, `is_empty`, `starts_with`, `ends_with`, and `contains` were added to the allow-list, and it reads `!raw.starts_with("pm_")`.
- **serde, from payment** (2026-09-29, preparing payment for a real server). A crate that derived serde failed `check` (rustc had no serde), and closed types could not keep their invariants on the wire. `check` now compiles against a stand-in serde; `#[serde(try_from = "T")]` with `impl TryFrom<T>` is accepted, and `impl Display` / `Error` are skipped ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). payment reads `Amount` and `PaymentMethodId` through their constructors on both sides.
- **Match guards, from invoice.** Every rejection on the way had a subset spelling. Match guards were the most frequent (three in invoice, and the idiomatic payment uses two): the candidate if a guard is ever the only way to keep an example under the threshold. Added in 0.3.0 (§8).
- **The authoring skill, from oidc.** The skill had one wrong line (building a `Vec` from `[a, b]`, also in 02 §3.1) and lacked the method allow-list, bit operators, `const`, the enum/`Option` equality rewrites, the phase order of diagnostics, and the by-shape reading of closed types under serde. All were added.
- **Diagnostics, from oidc.** All four fixed 2026-09-30:

  | Problem | Fix |
  | --- | --- |
  | `[expr/operator]` does not name the operator | the operator is named |
  | `==` on enums is reported as `numeric-op` | `==` and ordering on a type JS compares differently are `[check/comparison]`, with the rewrite for `Option`, the crate's types, and strings |
  | `[expr/method-call]` says "only methods of the crate's own inherent impls" for std methods outside the allow-list | a std method outside the allow-list lists what its receiver allows (and, on `u8`, the `matches!` spelling); a missing method on the crate's own type says so |
  | some locations point at the statement rather than the call | calls and method calls carry their own position |

- **The type/function file-name collision** (§3.3 rule 5 of [02](./02-authoring.md#33-names-are-unique-across-the-crate)) has now been hit four times: three driver helpers named after the type they build, and `fn group` returning a `Group` in invoice itself. The rule stays; the diagnostic names both items.

## 3. Next

<!-- derived-from ./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts -->

The first two rows were decided 2026-09-30, from measuring real dual implementations ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). The first real use (Oxide `Name`) passes without them, but its rewrite is longer and less idiomatic for their absence.

| Capability | Asked for by | Status | Verified by |
| --- | --- | --- | --- |
| `Result<Self, Self::Error>` in `impl TryFrom<T>` | the measurement | done 2026-09-30 | — |
| `s.chars()` through iterator `for` | the measurement | done 2026-09-30 | `for_chars_equivalence.rs` |
| `uuid::Uuid` | Oxide `Name` | done 2026-09-30 | — |

- **`TryFrom`.** `check` and `build` already accepted it. `survey` skipped trait impls, so a `#[serde(try_from)]` type looked as if its `impl TryFrom` were missing (`item/serde-attr`). `survey` now reads the impl as `X::try_from`.
- **`s.chars()`.** `for c in s.chars()` only (not iterator `for` in general), code points as `char`. Replaces hand-decoded UTF-8.
- **`uuid::Uuid`.** Added on one example's evidence and on how common UUID identifiers are in Rust domain code. Oxide `Name` hand-wrote `Uuid::parse_str` for the two forms its earlier checks let through, a copy that would drift if the check order or the crate changed. A deliberate exception to waiting for a second example ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)).

Defects found by real use are in §3.2.

### 3.1 When an example needs it

| Capability | Trigger or asked for by | Status | Verified by |
| --- | --- | --- | --- |
| Strings and `char`, as specified in [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods) | the example | one method at a time; `char` and `for c in s.chars()` are in | `for_chars_equivalence.rs` (§3) |
| Iteration: `while`, `break`/`continue`, iterator `for` | range `for` plus recursion is not enough | done in 0.2.0 (§8) | §8 |
| std methods the example calls, via the allow-list | the example; `Vec::is_empty` and `Option::is_some`/`is_none`: three of four real sites and oidc | `Vec::is_empty`, `is_some`/`is_none` in 2026-09-30; `Option::unwrap_or`, `ok_or`, and `map` in 0.3.0 (§8) | `option_methods_equivalence.rs` (0.3.0) |
| Tuple scrutinees, `match (state, event)` | one function per state keeps an example over the threshold after `_` | done 2026-09-30 | `tuple_match_equivalence.rs` |
| Bitwise operators and shifts on the fixed-width integers | oidc and Stoat's permission flags (both worked around); needed by any spec written in them | done 2026-09-30 (not on `usize` or `bool`) | — |
| Building a `Vec` in the crate (`vec![a, b]` of a fixed length) | oidc's `amr`, a list claim that TS callers expect as an array | done 2026-09-30 | `vec_build_equivalence.rs` |
| `const` and enum discriminants | oidc (`const`) and Stoat (18 permission bits as functions, `as` on the enum) | done 2026-09-30 | `flags_equivalence.rs` (a permission set in Stoat's shape) |

Still conditional: strings and `char`, and std methods, one at a time as examples call them. Iterator `map`/`filter`/`collect` are not added: they are how state sequences grow as arrays.

- **Tuple scrutinees.** Tuple arms of `_`, bindings, and arm patterns; `|` of whole tuples binding nothing; not nested tuples. Printed as nested `switch`es, one per element, so TS checks exhaustiveness too rather than trusting rustc's. The cost is copies of arms that several cases reach.
- **Building a `Vec`.** `vec![a, b]` and `vec![]` print as array literals; `vec![x; n]` and every way of growing one stay rejected. oidc's `AmrList` enum is now `Vec<String>` (§2.2, §2.3).
- **`const` and discriminants.** Crate-level `const` of integers, floats, `bool`, `char`, `&str`, folded at check time into `consts.ts`. Discriminants on fieldless enums with `#[repr(<int>)]`, and `e as T` when `T` holds every discriminant. Found on the way: with consts accepted, `Some(MAX) =>` would bind `MAX` rather than compare with it, so a const used as a pattern is refused; and one file per const collided with the usual `MAX_LEN` / `fn max_len` pair, so all consts share `consts.ts`.

### 3.2 Defects found by real use

From measuring real dual implementations ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)), all fixed 2026-09-30:

| Defect | Fix |
| --- | --- |
| a non-`pub` method reachable on the exported companion, a hole in closed types ([01 §4](./01-equivalence.md#4-closed-types)) | fixed |
| `mod r#impl;` looked up as `r#impl.rs` (skipped by `survey`, an error in `check`) | fixed |
| `build --out` emptying a directory it had not written | now refused |

From the Oxide `Name` vendoring ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)):

| Problem | Change | Verified by |
| --- | --- | --- |
| the runtime | copied into packages: first as `--bundle-runtime`, then for every package, with string-keyed brands so packages still exchange values | — |
| `assertNever`'s unused parameter | fixed | — |
| bindings the Rust leaves unused | fixed 2026-09-30. The generated `tsconfig.json` sets `noUnusedLocals` and `noUnusedParameters`, so every differential test type-checks under them, and what that turned up is fixed: unused `let`s, arm and `if let` bindings, write-only `let mut`s, parameters, `for` variables, the `?` temporary of `x?;`, and imports gathered from the IR but not printed | `unused_equivalence.rs` |
| a consumer's CI had no published `purecrate-ts` to run `check --out` with | since 0.1.0 (2026-09-30) each release tag carries Linux and macOS binaries, and `cargo install --git … --tag` builds the same version | — |

## 4. Specified but not yet implemented

`chars()` other than as `for c in s.chars()`; the Unicode-table `char` methods; `String` ordering; `isize`; `loop`, labelled `break`/`continue`, `a..=b` in `for`, iterator adaptors; byte string literals; the std allow-list beyond what [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods) lists; `static` and associated consts.

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
- `async`, randomness, and other effects (idsmith's `&mut` RNG parameters, 305 of the corpus's functions).

## 7. Open questions

- Should output typing come from rustc's type information instead of the in-house inference ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate))?
- Hermes support for `JSON.parse` source text ([04 §7](./04-wire.md#7-open-questions)).
- A shared error type with field paths for validation ([01 §4](./01-equivalence.md#4-closed-types)).
- Demand: see [06 §5](./06-strategy.md#5-validating-demand-next).

## 8. Releases

<!-- derived-from #2-evidence-from-examples -->
<!-- constrained-by #1-how-additions-are-chosen -->

0.1.0 (2026-09-30) shipped what §3 and §3.1 list as done, except what is marked 0.2.0 or 0.3.0. Decided the same day ([06 §5.2](./06-strategy.md#52-one-real-use)):

- Later releases take language capabilities first, in the order the examples' measured gaps give.
- Adoption is left to happen on its own, with no outreach and no requests to adopt.
- Distribution work waits for someone to need it.

### 0.2.0: iteration (released 2026-09-30)

Why: the largest gap measured so far. oidc's lexical helpers were at 3.2× idiomatic Rust (§2.2), and Windmill's scope and pipeline parsers ([91 §2](./91-real-use-candidates.md#2-fit-measured)) were all index bookkeeping over bytes and tokens that idiomatic code writes as iteration.

All items done 2026-09-30.

- **`for` over a `Vec`, slice, or bytes.** `for x in &xs` and `for x in xs.iter()` over a `Vec` or slice, `for b in s.bytes()`; also `for x in xs` on an owned `Vec` and `s.as_bytes()`. Printed as `for..of`, one binding, the body rules of range `for`. The named adaptors are refused with a message. Verified by `for_each_equivalence.rs`.
- **`while`, `break`, `continue`.** No labels, no `loop` with a value. Every loop a jump leaves gets a label, since a bare JS `break` in the `switch` a `match` prints as leaves the `switch` (a run without labels fails the test). A `?` in the condition turns the loop into `while (true)` that computes the condition first. The generated tsconfig also sets `allowUnreachableCode: false`, and no `break;` follows a jump in a `case`. Verified by `while_break_equivalence.rs`.
- **`for t in s.split(c)` with a `char` separator.** Added to 0.2.0 on oidc's evidence (§2.2). Any code point splits alike in UTF-8 and UTF-16. A `&str` separator is refused, since an empty one does not.
- **oidc's scanners rewritten with them** and measured again against the idiomatic reference: the lexical helpers went from 99 to 68 lines (2.2× the idiomatic 31). Each step is in §2.2.

### 0.3.0: guards and `Option` (released 2026-09-30)

All items done 2026-09-30.

- **Match guards.** Why: invoice reached for them three times and the idiomatic payment uses two (§2.4). A guarded arm is tried in order and does not count toward exhaustiveness, in the TS check as in rustc's. Also on binding arms and in `matches!`. Verified by `guards_equivalence.rs`. History: a first version put the arms after a guard inside the `_` of a `switch` on the same value, which TS narrowed so that the later `switch` could not name the other variants; the second made each test and take a standalone `match` in an `if` chain, which after the 0.4.0 rewrites made payment's `step.ts` 31.6 KB; since 2026-10-01 guards sit at the leaves of the tuple decision tree (payment's generated code 47.9 → 23.5 KB, oidc's 64.4 → 48.8 KB, back to their sizes before the rewrites).
- **`Option::unwrap_or`, `ok_or`, and `map` with a closure.** Why: most of payment's remaining gap (§2.2). `unwrap_or(e)` evaluates `e` first, as Rust does. `map` also takes a function name. Verified by `option_methods_equivalence.rs`.
- **`survey --all-causes`.** Why: reporting every cause per function, to estimate a rewrite ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). An expression or statement the parser cannot take is recorded and stood in for, so the rest of the item is still lowered and type-checked. The default output is unchanged.
- **Wire schemas.** Why: added on request, not from an example's gap. zod and valibot print dependencies first with `lazy` only for recursive types, one arm or field per line when long, through the adapters' `unitVariant` and `optionalField`. A refused `try_from` names the error's variant in all three libraries. `toJson` writes a fieldless enum in one line. Behavior unchanged. Verified by `wire.rs` and `wire_write.rs`, with mutually recursive and later-declared types added.
- **zod 4.6.** The zod adapter and the generated zod code moved to zod 4.6 the same day (peer `^4.6.0`; `z.ZodType<T, In>` without `ZodTypeDef`, issues with `code: "custom"`). A breaking change for users on zod 3, who stay on purecrate-ts 0.2.
- **arktype errors.** arktype's schemas keep nested errors at their path instead of replacing them with one at the type. Verified by `errors_keep_their_location` in `wire_write.rs`.

### 0.4.0: iteration, part two (released 2026-10-01)

<!-- derived-from ./90-acceptance-survey.md#7-re-measured-with-030-2026-09-30 -->

Why: oidc's lexical helpers are still 2.2× idiomatic Rust (§2.2), and in the corpus re-measured with 0.3.0 ([90 §7](./90-acceptance-survey.md#7-re-measured-with-030-2026-09-30)) the leading causes are `chars()` used as a value and slicing ranges. The corpus shows where idiomatic code goes, not what to accept (§6); each item below is kept only if rewriting oidc, payment, or invoice uses it.

- **Iterator consumers that yield a scalar.** `all`, `any`, `count`, `position` on `s.chars()`, `s.bytes()`, and `xs.iter()`; `sum` with Rust's overflow panic; `for (i, x) in xs.iter().enumerate()`. None builds an array, so the reason iterator `map`/`filter`/`collect` stay out (§3.1) does not apply. Done 2026-09-30 (`consumers_equivalence.rs`), with a function name as the predicate as well.
- **Byte slicing.** `&s[a..b]`, `&s[a..]`, `&s[..b]`, and the same on slices; `strip_prefix` / `strip_suffix` returning `Option<&str>`. Specified in [01 §6.1](./01-equivalence.md#61-strings): positions are UTF-8 bytes, and a position off a char boundary or past the end throws with Rust's message. Done 2026-09-30 (`slicing_equivalence.rs`), with the character in the boundary message escaped as `Debug` escapes it.
- **Tuple destructuring.** `let (a, b) = t;`, `|(a, b)|`, and `for (k, v) in ..`, with the element rules of tuple `match`. Done 2026-09-30 (`destructure_equivalence.rs`).
- **`const` inside functions and associated consts** (`impl T { const N: u32 = 3; }`), folded like crate-level consts. Done 2026-09-30 for consts in a block, as a typed `let` at the top of the block instead of folded ([01 §7](./01-equivalence.md#7-rewritten-constructs); the value is the same, since rustc rejects one that overflows). Associated consts, as members of the type's companion (`Order.MAX_ITEMS`), wait for the example rewrites to show a use.
- **Integer helpers.** `min`, `max`, `abs`, `pow`, and the `checked_*`, `saturating_*`, `wrapping_*` forms of the operators, each against Rust's debug-build result (payment's idiomatic code calls `min`). Done 2026-09-30 (`int_methods_equivalence.rs`), on every integer type, `usize` in Rust's 64 bits.
- **From the measurement** (added 2026-09-30): a consumer after `s.split(c)` (oidc's `has_token` is now `list.split(' ').any(|t| t == word)`), and `bool` literal patterns (`(true, None, _)` in a tuple `match`), both refused in the rewrites. Verified by `consumers_equivalence.rs` and `bool_patterns_equivalence.rs`.
- **Measurement.** oidc's lexical helpers, payment, and invoice rewritten with guards, the `Option` methods, and the above, and measured again against their idiomatic references (§2.2). Done 2026-09-30: guards, tuple `match`, the `Option` methods, `all` / `any`, `min`, and `pow` carried the reduction; slicing, `strip_*`, local `const`, `position` / `count` / `sum` / `enumerate`, and the checked, saturating, and wrapping forms went unused by these three (§2.2). Kept anyway (decided 2026-09-30): each is implemented and tested, preserves meaning exactly, and answers a leading cause in the corpus ([90 §7](./90-acceptance-survey.md#7-re-measured-with-030-2026-09-30): slicing ranges and `chars()` as a value); the rule of §1 still decides what comes next.

### Later, on evidence

- Generics and string-keyed maps (§5). `format!`, asked for by Windmill only so far; `Display` of floats is a large surface to match, so a first step would take only `{}` on integers, `&str`, and `char`, whose text Rust and TS agree on.
- `&mut self` as a function returning the new value (`fn apply(&mut self, e)`, the aggregate shape in 5 corpus entries). Sound because `&mut` excludes aliases, but the TS signature then differs from the Rust one, so the caller contract ([03 §5](./03-output.md#5-caller-contract)) has to say so first.
- Paths through modules (`crate::m::f`, `super::T`): names are already unique after flattening, so this is resolution only.
- Nested patterns (a literal or a variant inside a variant's fields, `PasswordChecked { verified: false, .. }`), which oidc's idiomatic `step` relies on, and a local closure's parameter type inferred from its calls; both measured in the 0.4.0 rewrites (§2.2).
- Narrowing `as` between integers, which wraps in Rust and can wrap the same in TS; it would give up the single spelling `T::from(x)` for widening.
- crates.io and Windows binaries, when a user asks. Since 0.3.0 the dependencies are crates.io requirements, vendored by source replacement.
- 1.0: a compatibility policy for the output bytes and the reason codes, and every withdrawal criterion of [06 §4](./06-strategy.md#4-success-and-withdrawal-criteria) answered. The real-use criterion stays open until the tool is adopted unprompted; Oxide `Name` remains local evidence ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)).
