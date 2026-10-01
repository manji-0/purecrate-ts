# Roadmap

Status: current (2026-10-02, after 0.7.0)

<!-- constrained-by ./02-authoring.md -->
<!-- constrained-by ./06-strategy.md#4-success-and-withdrawal-criteria -->

What comes next and why. §1 is the rule, §2 the evidence it runs on, §3 the candidates it currently yields, §4–§7 what is specified, deferred, refused, or open, and §8 what each release added.

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
| oidc (OIDC Core 1.0 OP login, PKCE RFC 7636, TOTP RFC 6238/4226 as the second factor, amr RFC 8176) | third party, from the authoring skill alone | RFC 4226 truncation's `&` and `<<` (written with `%` and `*`); no way to build a `Vec` for `amr` (a recursive enum instead; the documents' "build with `[a, b]`" was wrong); `const`; `==` on `Option` and enums, `is_some`; `u8::is_ascii_digit`; 15 rejections, about 6 not predictable from the skill | documents corrected (§2.4); later `is_some`/`is_none`, bit operators, and `vec![a, b]` (§8.1) |
| semver (SemVer 2.0.0: parsing, §11 precedence) | third party, from the authoring skill alone | about 12 rejections in 5 rounds, 6 not predictable from the skill: `std::cmp::Ordering` and `.cmp()`, `..` in a tuple variant, a bare binding arm (`other => other`), `?` in a tuple inside an arm, `let mut x = None` without a type, no `u64::from(usize)`; ASCII string order written byte by byte; identifier lists as recursive enums | skill corrected (§2.4); `Ordering`, `cmp`, and string ordering (§8.6) |
| payment (Stripe PaymentIntent lifecycle) | third party | nothing on the first pass; but transition lines at 2.1× (28 of 160 were `=> Err(InvalidTransition)`); the client sends events back, so values must be written as serde JSON; `terms`/`outcome`/`amount` driver functions collided with types (known rule) | `_` and binding-free `A \| B` arms; `toJson` (and rejected unit structs, which serde writes differently from `struct S {}`) |

### 2.2 Line counts against idiomatic Rust

Non-blank, non-comment lines of logic (functions and inherent impls), both sides formatted by rustfmt at width 120 (`scripts/line-counts.py`), so layout does not decide the ratio. Threshold 2× ([06 §4.2](./06-strategy.md#42-external-criteria)).

**Now** (semver remeasured in 0.7.0; the other rows are unchanged since 0.4.1):

| Example | Idiomatic | Constrained | Ratio | Earlier (as written) |
| --- | --- | --- | --- | --- |
| signup: email (WHATWG) | 28 | 42 | 1.5× | 2.8× with recursion only; 1.9× with range `for` |
| signup: password (NIST) | 17 | 24 | 1.4× | 1.6×; 1.4× |
| iban | 24 | 44 | 1.8× | 2.4× with recursion only; 1.9× with range `for` |
| invoice | 48 | 75 | 1.6× | 2.2× first draft; 1.4× restructured |
| oidc | 327 | 423 | 1.3× | 1.65× as written from the skill alone |
| payment | 61 | 96 | 1.6× | 2.1× one arm per variant; 1.8× with `_` and `A \| B` |
| semver | 75 | 138 | 1.8× | 166 (2.2×) with `Ordering`; 209 (2.8×) from the skill alone |

signup is counted by hand, as its test has no idiomatic module. The "earlier" figures were taken as written, before rustfmt normalization; they are comparable with each other, not with the "now" column.

**What moved the numbers.** Each capability, the example it was measured on, and what it replaced:

- **Range `for`** (signup, iban): recursion. Afterwards most of signup's gap was character classes spelled as numeric comparisons.
- **Byte literals, integer literal and range patterns, `matches!`** (`int_patterns_equivalence.rs`): the local part of the e-mail address went from 14 lines of comparisons to one `matches!`. IBAN barely moved; its gap is the loops. A `match … { … => true, _ => false }` was longer than the comparisons it replaced (IBAN 50 lines), which is why `matches!` came with it.
- **`_` and `A | B` arms** (payment): 160 → 135 lines of transitions. The rest was one function per state instead of a tuple `match`, and `match` on `Option` where idiomatic code calls `ok_or`, `unwrap_or`, `map`, and `min`.
- **A filtered sum per group** (invoice, a restructuring, no capability): the first draft kept four running totals in a struct updated line by line (104 lines); summing each group with a range `for`, as idiomatic code sums with `filter`, gave 68.
- **Bit operators, `const`, discriminants** (oidc): barely moved the count, since each named limit costs a declaration, but RFC 4226 truncation now reads as §5.4 writes it (`& 0x0f`, `<< 24 |`) instead of `% 16` and a base-256 sum, and no number is left unexplained.
- **`for b in s.bytes()`, `for x in &xs`** (oidc, 0.2.0): lexical helpers 99 → 95. Smaller than expected: most of the gap was splitting a space-delimited list, which needs token boundaries a plain `for` does not give.
- **`for t in s.split(c)`** (oidc, 0.2.0): lexical helpers 95 → 68; `span_equals` is gone.
- **`vec![a, b]`** (oidc): the `AmrList` enum became `Vec<String>`, 8 lines shorter.
- **Guards, tuple `match`, `ok_or` / `unwrap_or` / `map`, `all` / `any`, `min`, `pow`** (oidc, payment, invoice; the 0.4.0 rewrites): guards wherever a state or a field chose the path, tuple `match` in place of per-state handlers (payment's five, oidc's three), the `Option` methods in place of `match`es. oidc's file went 777 → 629 lines, payment's logic 163 → 111, invoice's 74 → 66 (as written).
- **Unused by the rewrites:** slicing and `strip_*`, `const` in a function, `position`, `count`, `sum`, `enumerate`, the `checked_*` / `saturating_*` / `wrapping_*` forms, tuple `let` (tried in invoice; with long names rustfmt made it longer). Kept anyway (§8.4).

**semver**, the first example over 2× after rustfmt normalization. The draft from the skill alone was 209 lines; restructuring its ordering within the subset (a `then` helper, a byte loop through `compare_u64`) gave 195; with `Ordering` (0.5.0, §8.6), 166. By part:

| Part | Idiomatic | Restructured | With `Ordering` | What idiomatic code uses |
| --- | --- | --- | --- | --- |
| Ordering (`compare` and its helpers) | 10 (+15 in `impl Ord for PreId`) | 57 | 28 | `std::cmp::Ordering`, tuple `cmp`, `then_with`, `String` and `Vec` ordering |
| Identifier parsing | 35 | 52 | 52 | `str::parse`, `u8::is_ascii_*`, `match` on the `&str` with guards |
| `Version::parse` | 30 | 47 | 47 | `split_once`, a fixed array indexed by the piece count |
| Identifier lists | 0 | 20 | 20 | `split('.').map(..).collect()` (the constrained side recurses into cons lists) |
| Accessors | 0 | 19 | 19 | `pub` fields (the constrained side is a closed type) |

Two things the script counts differently from how they read: the idiomatic `impl PartialOrd` / `impl Ord` (15 lines) are logic filed under types, and the accessors exist only on the closed side. Adjusted for both, the restructured draft was 176 / 90 (2.0×) and the rewrite with `Ordering` is 147 / 90 (1.6×), as estimated before it was built.

**With `collect` and `split_once` (0.7.0).** The script counts 138 / 75 (1.84×, printed 1.8×). Identifier lists are `split('.').map(f).collect()`, and `Version::parse` uses `split_once` as `Some((x, y))`; the cons-list parsers are gone. Adjusted the same way as the `Ordering` row — drop the 19 accessor lines, and count the 15 lines of `impl Ord` as logic — that is 119 / 90 (1.3×). `.map(PreId::Numeric)` is still not a function name (§3). `str::parse` stays out (§6): Rust accepts a leading `+`, and the threshold does not need it.

**oidc by section.** Written by an agent that read only the authoring skill, to find what the skill leaves out. As written:

| Section | Idiomatic | From the skill alone | After 0.4.0 |
| --- | --- | --- | --- |
| Types | 198 | 203 (1.0×) | — |
| Lexical helpers (token and byte scanning) | 31 | 99 (3.2×) | 47 (1.5×) |
| Request validation | 83 | 153 (1.8×) | 108 (1.3×) |
| TOTP | 47 | 81 (1.7×) | 88 (1.9×) |
| State machine and token endpoint | 183 | 251 (1.4×) | 260 (1.4×) |

The first gap was the scanners: iterator adaptors over tokens and bytes, written as range `for` with index bookkeeping. RFC 4226 truncation without bit operators was correct only because the top bit is masked (`% 128`) before multiplying; a spec that needs `^` or rotations (a hash) would not have been writable.

**oidc's TOTP**, the highest ratio above, measured with rustfmt: 88 / 70 lines in all, logic 61 / 44 (1.4×). The idiomatic reference fits each type and several functions on one line, which rustfmt spreads out. The logic gap comes from four constructs, none of them nested patterns or closure inference (the two measured candidates in §3):

| Function | Idiomatic | Constrained | What idiomatic code uses |
| --- | --- | --- | --- |
| `truncate_mac` | 5 | 12 | `mac.get(19..)?.last()?`, `try_into` to `[u8; 4]`, `u32::from_be_bytes` (the constrained side writes RFC 4226 §5.4's shifts) |
| `parse_otp` | 5 | 10 | `str::parse` after the digit check (a loop instead) |
| `check_totp` | 22 | 27 | `let … else` (`[expr/let-pattern]`), `filter` / `find` over the candidates (a `for` with a flag) |
| `totp_step` | 3 | 6 | `bool::then` |
| `OtpDigits` helpers | 9 | 6 | a `match` for the count (the constrained side reads the discriminant) |

`filter` / `find` stay out (§6). The others each save a few lines in one function, so none is a candidate on this evidence alone.

### 2.3 Semantic cross-checks

Beyond the Rust-vs-TS differential tests:

| Example | Checked against |
| --- | --- |
| signup | WHATWG's own regular expression, on node |
| iban | an idiomatic-Rust implementation, on published valid IBANs, one-character mutations, and malformed input |
| payment | an idiomatic-Rust implementation (tuple `match` with guards and a wildcard), on every four-event run under each capture and confirmation method. The client-side step on the server's JSON also writes the server's bytes for every reachable state and event (`wire_write.rs`). |
| invoice | the NTA's own worked examples (60,000 × 10/110 ≒ 5,454; 23,894 × 10% ≒ 2,389 where rounding per line would give 2,388; 問59's receipt at 948 both ways), asserted in `invoice_equivalence.rs` next to an idiomatic-Rust cross-check and the Rust/TS differential test (about 11,400 invoices, overflow included) |
| semver | an idiomatic-Rust implementation (`split_once`, `collect`, `impl Ord`), on the spec's examples, one-character edits of them, and `u64` edges, and every pair's precedence; §11's ordered chain asserted on both Rust sides. Swapping numeric and alphanumeric order in the constrained side fails both |
| oidc | the idiomatic reference; since `vec![a, b]` the cross-check no longer rewrites one side's `Debug` text |

### 2.4 What the measurements changed

Besides the capabilities in §2.1:

- **String methods, from payment.** payment's ID check read `b[0] != 112u8 || b[1] != 109u8 || b[2] != 95u8`. `str::len`, `is_empty`, `starts_with`, `ends_with`, and `contains` were added to the allow-list, and it reads `!raw.starts_with("pm_")`.
- **serde, from payment** (preparing it for a real server). A crate that derived serde failed `check` (rustc had no serde), and closed types could not keep their invariants on the wire. `check` now compiles against a stand-in serde; `#[serde(try_from = "T")]` with `impl TryFrom<T>` is accepted, and `impl Display` / `Error` are skipped ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). payment reads `Amount` and `PaymentMethodId` through their constructors on both sides.
- **Match guards, from invoice.** Every rejection on the way had a subset spelling. Guards were the most frequent (three in invoice, and the idiomatic payment uses two), and were added in 0.3.0 (§8.3).
- **The authoring skill, from oidc.** The skill had one wrong line (building a `Vec` from `[a, b]`, also in 02 §3.1) and lacked the method allow-list, bit operators, `const`, the enum/`Option` equality rewrites, the phase order of diagnostics, and the by-shape reading of closed types under serde. All were added.
- **The authoring skill, from semver.** Six of its rejections were not predictable from the skill; each now has a line: `..` in a tuple variant, a bare binding arm, `?` in a tuple inside an arm, `let mut x = None` needing its type, `Ordering` / `cmp` and what to write instead, and no `u64::from(usize)`.
- **Diagnostics, from oidc:**

  | Problem | Fix |
  | --- | --- |
  | `[expr/operator]` does not name the operator | the operator is named |
  | `==` on enums is reported as `numeric-op` | `==` and ordering on a type JS compares differently are `[check/comparison]`, with the rewrite for `Option`, the crate's types, and strings |
  | `[expr/method-call]` says "only methods of the crate's own inherent impls" for std methods outside the allow-list | a std method outside the allow-list lists what its receiver allows (and, on `u8`, the `matches!` spelling); a missing method on the crate's own type says so |
  | some locations point at the statement rather than the call | calls and method calls carry their own position |

- **The type/function file-name collision** (§3.3 rule 5 of [02](./02-authoring.md#33-names-are-unique-across-the-crate)) has been hit four times: three driver helpers named after the type they build, and `fn group` returning a `Group` in invoice itself. The rule stays; the diagnostic names both items.

## 3. Candidates

<!-- derived-from #2-evidence-from-examples -->

What the evidence currently points at, strongest first. None is scheduled until §1 is met: an example that cannot be written, or stays over the threshold, without it. `split_once`, a `Vec` collected once from `s.split(c)`, and `Some((a, b))` were the rest of semver's parsing gap; they are in ([§8.8](#88-070-lists-from-text-2026-10-02)). A variant or a literal nested in a field stays `[pattern/nested]`.

| Candidate | Evidence | Note |
| --- | --- | --- |
| Nested patterns (a literal or a variant inside a variant's fields, `PasswordChecked { verified: false, .. }`, `Some(Some(x))`) | oidc's idiomatic `step` relies on them (0.4.0 rewrites). `Some((a, b))`, a tuple of names, is in (§8.8) | — |
| A local closure's parameter type inferred from its later calls | oidc needed `\|error: ErrorCode\|` (0.4.0 rewrites) | — |
| Growing a `Vec` in a function body (`let mut v = Vec::new(); v.push(x)`), and `iter().map(f).collect()` over a `Vec` | any domain that accumulates a list (line items, audit trail, retries) writes a cons list today: O(n) access, recursion depth, awkward interop for TS callers who expect arrays, and a large part of the line-count gap | the value semantics of `let mut` already exist; a body that only builds a fresh array is the same as `vec![a, b]` with a runtime length. Not scheduled until §1 is met |
| `format!` | Windmill only | `Display` of floats is a large surface; a first step would take only `{}` on integers, `&str`, and `char`, whose text Rust and TS agree on |
| `&mut self` as a function returning the new value (`fn apply(&mut self, e)`) | the aggregate shape in 5 corpus entries | sound because `&mut` excludes aliases, but the TS signature then differs from the Rust one, so the caller contract ([03 §5](./03-output.md#5-caller-contract)) has to say so first |
| Paths through modules (`crate::m::f`, `super::T`) | — | names are already unique after flattening, so this is resolution only |
| Narrowing `as` between integers | — | wraps in Rust and can wrap the same in TS; would give up the single spelling `T::from(x)` for widening |
| Generics and string-keyed maps | — | §5 |
| Associated consts (`impl T { const N: u32 = 3; }`), as members of the type's companion | specified with local `const` in 0.4.0 | waits for a use |
| crates.io and Windows binaries | — | when a user asks; since 0.3.0 the dependencies are crates.io requirements, vendored by source replacement |

**1.0** needs a compatibility policy for the output bytes and the reason codes, and every withdrawal criterion of [06 §4](./06-strategy.md#4-success-and-withdrawal-criteria) answered. The real-use criterion stays open until the tool is adopted unprompted; Oxide `Name` remains local evidence ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)).

## 4. Specified but not yet implemented

`chars()` other than as `for c in s.chars()` or before a consumer; the Unicode-table `char` methods; `isize`; `loop`, labelled `break`/`continue`, `a..=b` in `for`, iterator adaptors; byte string literals; the std allow-list beyond what [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods) lists; `static`.

## 5. v1: when type expressiveness runs out

Waits for an example that cannot be written without it.

- Unbounded type parameters, emitted as TS generics (no monomorphization). No bounds, `where`, or associated types.
- `HashMap`/`BTreeMap` with `String` keys only, as `ReadonlyMap<string, V>`. Insertion order is not matched; functions depending on it are rejection candidates.

## 6. Not doing

- Allow-lists aimed at passing existing crates.
- Iterator `map` / `filter` / `collect` over a `Vec` or a state, and every other way of growing a `Vec`: they are how state sequences grow as arrays. Consumers that yield a scalar are one exception (§8.4). A `Vec` read once from text is the other: `s.split(c).collect()` and `s.split(c).map(f).collect()` into `Vec<T>` or `Result<Vec<T>, E>` (§8.8). `filter`, collecting a `Vec` or anything but `split(c)`, and `split` on a `&str` stay out. Building a list with `let mut v = Vec::new(); v.push(x)` (or `iter().map(f).collect()` over a `Vec`) is a candidate (§3), not scheduled.
- Decimals; event logs inside state.
- A schema-library dependency in the core runtime.
- WASM. The IR does not preclude a second backend, but the path is TS source.
- `Rc` / `Cell` / `RefCell` / `Mutex`.
- `async`, randomness, and other effects (idsmith's `&mut` RNG parameters, 305 of the corpus's functions).

## 7. Open questions

- Should output typing come from rustc's type information instead of the in-house inference ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate))?
- Hermes support for `JSON.parse` source text ([04 §7](./04-wire.md#7-open-questions)).
- A shared error type with field paths for validation ([01 §4](./01-equivalence.md#4-closed-types)).
- Growing a `Vec` only through recursive enums is costly on both sides (O(n) access, recursion depth, TS callers who expect arrays); `let mut v; v.push` is a candidate (§3).
- Demand: see [06 §5](./06-strategy.md#5-validating-demand-next).

## 8. Releases

<!-- derived-from #2-evidence-from-examples -->
<!-- constrained-by #1-how-additions-are-chosen -->

Per release: why, what, and the test that verifies each item. [CHANGELOG.md](../CHANGELOG.md) has the user-facing list. Decided with 0.1.0 ([06 §5.2](./06-strategy.md#52-one-real-use)):

- Later releases take language capabilities first, in the order the examples' measured gaps give.
- Adoption is left to happen on its own, with no outreach and no requests to adopt.
- Distribution work waits for someone to need it.

### 8.1 0.1.0 (2026-09-30)

<!-- derived-from ./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts -->

The subset of [02](./02-authoring.md) as the examples of §2.1 shaped it, plus what the examples and the first real use asked for before release.

**From the examples:**

| Capability | Asked for by | Verified by |
| --- | --- | --- |
| std methods one at a time: `Vec::is_empty`, `Option::is_some` / `is_none` | three of four real sites, and oidc | `std_methods_equivalence.rs` |
| Tuple scrutinees, `match (state, event)` | one function per state kept payment over the threshold after `_` | `tuple_match_equivalence.rs` |
| Bitwise operators and shifts on the fixed-width integers (not `usize` or `bool`) | oidc and Stoat's permission flags (both worked around); any spec written in them | `bits_equivalence.rs` |
| `vec![a, b]` of a fixed length | oidc's `amr`, a list claim that TS callers expect as an array | `vec_build_equivalence.rs` |
| Crate-level `const` and enum discriminants | oidc (`const`) and Stoat (18 permission bits as functions, `as` on the enum) | `flags_equivalence.rs` (a permission set in Stoat's shape) |

- **Tuple scrutinees.** Tuple arms of `_`, bindings, and arm patterns; `|` of whole tuples binding nothing; not nested tuples. Printed as nested `switch`es, one per element, so TS checks exhaustiveness too rather than trusting rustc's. The cost is copies of arms that several cases reach.
- **`vec![a, b]`** and `vec![]` print as array literals; `vec![x; n]` stays rejected (§6).
- **`const` and discriminants.** Crate-level `const` of integers, floats, `bool`, `char`, `&str`, folded at check time into `consts.ts`. Discriminants on fieldless enums with `#[repr(<int>)]`, and `e as T` when `T` holds every discriminant. Found on the way: with consts accepted, `Some(MAX) =>` would bind `MAX` rather than compare with it, so a const used as a pattern is refused; and one file per const collided with the usual `MAX_LEN` / `fn max_len` pair, so all consts share `consts.ts`.

**From measuring real dual implementations** ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). The first real use (Oxide `Name`) passes without these, but its rewrite was longer and less idiomatic for their absence.

| Capability | Verified by |
| --- | --- |
| `Result<Self, Self::Error>` in `impl TryFrom<T>` | — |
| `for c in s.chars()` (not iterator `for` in general), code points as `char`; replaces hand-decoded UTF-8 | `for_chars_equivalence.rs` |
| `uuid::Uuid` | `uuid_equivalence.rs` |

- **`TryFrom`.** `check` and `build` already accepted it. `survey` skipped trait impls, so a `#[serde(try_from)]` type looked as if its `impl TryFrom` were missing (`item/serde-attr`); it now reads the impl as `X::try_from`.
- **`uuid::Uuid`.** Added on one example's evidence and on how common UUID identifiers are in Rust domain code. Oxide `Name` hand-wrote `Uuid::parse_str` for the two forms its earlier checks let through, a copy that would drift if the check order or the crate changed. A deliberate exception to waiting for a second example ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)).

**Defects found by real use.** From the same measurement:

| Defect | Fix |
| --- | --- |
| a non-`pub` method reachable on the exported companion, a hole in closed types ([01 §4](./01-equivalence.md#4-closed-types)) | emitted as `T$name`, off the companion |
| `mod r#impl;` looked up as `r#impl.rs` (skipped by `survey`, an error in `check`) | fixed |
| `build --out` emptying a directory it had not written | now refused |

From the Oxide `Name` vendoring ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)):

| Problem | Change | Verified by |
| --- | --- | --- |
| the runtime | copied into packages: first as `--bundle-runtime`, then for every package, with string-keyed brands so packages still exchange values | — |
| `assertNever`'s unused parameter | fixed | — |
| bindings the Rust leaves unused | the generated `tsconfig.json` sets `noUnusedLocals` and `noUnusedParameters`, so every differential test type-checks under them; fixed what that turned up: unused `let`s, arm and `if let` bindings, write-only `let mut`s, parameters, `for` variables, the `?` temporary of `x?;`, and imports gathered from the IR but not printed | `unused_equivalence.rs` |
| a consumer's CI had no published `purecrate-ts` to run `check --out` with | each release tag carries Linux and macOS binaries, and `cargo install --git … --tag` builds the same version | — |

### 8.2 0.2.0: iteration (2026-09-30)

Why: the largest gap measured so far. oidc's lexical helpers were at 3.2× idiomatic Rust (§2.2), and Windmill's scope and pipeline parsers ([91 §2](./91-real-use-candidates.md#2-fit-measured)) were all index bookkeeping over bytes and tokens that idiomatic code writes as iteration.

| Item | Verified by |
| --- | --- |
| `for` over a `Vec`, slice, or bytes | `for_each_equivalence.rs` |
| `while`, `break`, `continue` | `while_break_equivalence.rs` |
| `for t in s.split(c)` with a `char` separator | `for_each_equivalence.rs` |

- **`for` over a `Vec`, slice, or bytes.** `for x in &xs` and `for x in xs.iter()` over a `Vec` or slice, `for b in s.bytes()`; also `for x in xs` on an owned `Vec` and `s.as_bytes()`. Printed as `for..of`, one binding, the body rules of range `for`. The named adaptors are refused with a message.
- **`while`, `break`, `continue`.** No labels, no `loop` with a value. Every loop a jump leaves gets a label, since a bare JS `break` in the `switch` a `match` prints as leaves the `switch` (a run without labels fails the test). A `?` in the condition turns the loop into `while (true)` that computes the condition first. The generated tsconfig also sets `allowUnreachableCode: false`, and no `break;` follows a jump in a `case`.
- **`for t in s.split(c)`.** Added on oidc's evidence (§2.2). Any code point splits alike in UTF-8 and UTF-16. A `&str` separator is refused, since an empty one does not.
- **oidc's scanners rewritten with them**: the lexical helpers went from 99 to 68 lines (§2.2).

### 8.3 0.3.0: guards and `Option` (2026-09-30)

| Item | Why | Verified by |
| --- | --- | --- |
| Match guards | invoice reached for them three times, the idiomatic payment uses two (§2.4) | `guards_equivalence.rs` |
| `Option::unwrap_or`, `ok_or`, `map` | most of payment's remaining gap (§2.2) | `option_methods_equivalence.rs` |
| `survey --all-causes` | every cause per function, to estimate a rewrite ([91 §4](./91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)) | `survey.rs` |
| Wire schemas in dependency order | on request, not from an example's gap | `wire.rs`, `wire_write.rs` |
| zod 4.6 | the adapter's peer | `wire.rs` |
| arktype errors keep their location | — | `errors_keep_their_location` in `wire_write.rs` |

- **Match guards.** A guarded arm is tried in order and does not count toward exhaustiveness, in the TS check as in rustc's. Also on binding arms and in `matches!`. The printing changed twice: arms after a guard first sat inside the `_` of a `switch` on the same value, which TS narrowed so that the later `switch` could not name the other variants; then each arm became a standalone `match` in an `if` chain, which after the 0.4.0 rewrites made payment's `step.ts` 31.6 KB. Since 0.4.1 guards sit at the leaves of the tuple decision tree ([01 §7.2](./01-equivalence.md#72-match-guards)).
- **`Option` methods.** `unwrap_or(e)` evaluates `e` first, as Rust does. `map` also takes a function name.
- **`survey --all-causes`.** An expression or statement the parser cannot take is recorded and stood in for, so the rest of the item is still lowered and type-checked. The default output is unchanged.
- **Wire schemas.** zod and valibot print dependencies first with `lazy` only for recursive types, one arm or field per line when long, through the adapters' `unitVariant` and `optionalField`. A refused `try_from` names the error's variant in all three libraries. `toJson` writes a fieldless enum in one line. Behavior unchanged; mutually recursive and later-declared types added to the tests.
- **zod 4.6.** Peer `^4.6.0`; `z.ZodType<T, In>` without `ZodTypeDef`, issues with `code: "custom"`. A breaking change for users on zod 3, who stay on purecrate-ts 0.2.
- **arktype errors.** Nested errors stay at their path instead of being replaced by one at the type.

### 8.4 0.4.0: iteration, part two (2026-10-01)

<!-- derived-from ./90-acceptance-survey.md#7-re-measured-with-030-2026-09-30 -->

Why: oidc's lexical helpers were still 2.2× idiomatic Rust (§2.2), and in the corpus re-measured with 0.3.0 ([90 §7](./90-acceptance-survey.md#7-re-measured-with-030-2026-09-30)) the leading causes were `chars()` used as a value and slicing ranges. The corpus shows where idiomatic code goes, not what to accept (§6); each item was to be kept only if rewriting oidc, payment, or invoice used it.

| Item | Verified by |
| --- | --- |
| Iterator consumers that yield a scalar: `all`, `any`, `count`, `position`, `sum`; `for (i, x) in xs.iter().enumerate()` | `consumers_equivalence.rs` |
| Byte slicing `&s[a..b]`, `&s[a..]`, `&s[..b]` and on slices; `strip_prefix` / `strip_suffix` | `slicing_equivalence.rs` |
| Tuple destructuring in `let`, closure parameters, `for` | `destructure_equivalence.rs` |
| `const` inside functions | `local_consts_equivalence.rs` |
| Integer helpers `min`, `max`, `abs`, `pow`, `checked_*`, `saturating_*`, `wrapping_*` | `int_methods_equivalence.rs` |
| A consumer after `s.split(c)`; `bool` literal patterns | `consumers_equivalence.rs`, `bool_patterns_equivalence.rs` |

- **Consumers** on `s.chars()`, `s.bytes()`, and `xs.iter()`, with a function name as the predicate as well. None builds an array, so the reason iterator `map`/`filter`/`collect` stay out (§6) does not apply.
- **Slicing** is specified in [01 §6.1](./01-equivalence.md#61-strings): positions are UTF-8 bytes, and a position off a char boundary or past the end throws with Rust's message, the character escaped as `Debug` escapes it.
- **`const` in a block** is a typed `let` at the top of the block instead of folded ([01 §7](./01-equivalence.md#7-rewritten-constructs); the value is the same, since rustc rejects one that overflows). Associated consts wait for a use (§3).
- **Integer helpers** against Rust's debug-build result, on every integer type, `usize` in Rust's 64 bits (payment's idiomatic code calls `min`).
- **A consumer after `split`, `bool` patterns.** Both were refused in the rewrites: oidc's `has_token` is now `list.split(' ').any(|t| t == word)`, and its `step` matches `(true, None, _)`.
- **Measurement.** oidc, payment, and invoice rewritten and measured again (§2.2). Guards, tuple `match`, the `Option` methods, `all` / `any`, `min`, and `pow` carried the reduction; the other items went unused by these three. Kept anyway: each is implemented and tested, preserves meaning exactly, and answers a leading cause in the corpus (slicing ranges and `chars()` as a value); §1 still decides what comes next.

### 8.5 0.4.1: smaller output (2026-10-01)

From evaluating 0.4.0 ([bench/payment](../bench/payment/README.md)). No change to what is accepted.

- Guards compile into the tuple decision tree: payment's generated code 47.9 → 23.5 KB, oidc's 64.4 → 48.8 KB, back to their sizes before the 0.4.0 rewrites.
- Each package's runtime keeps only what its code uses: payment's bundle 3.1 → 2.3 KB gzipped; first call 1.18 → 0.52 ms with the regular expression built lazily.
- Fixed-seed random cases join the differential tests ([01 §8](./01-equivalence.md#8-verification)).

### 8.6 0.5.0: ordering (2026-10-01)

Why: semver, written from the authoring skill alone, was the first example over 2× after rustfmt normalization, and its ordering was most of the gap (§2.2): 57 lines against 25, with `std::cmp::Ordering`, `cmp`, and `String` order all refused.

| Item | Verified by |
| --- | --- |
| `std::cmp::Ordering` as a fieldless enum, named through `use std::cmp::Ordering;` or the full path | `ordering_equivalence.rs`, `ordering.rs` in `check` |
| `cmp` on integers, `char`, `bool`, `String` / `&str`, `Uuid` | `ordering_equivalence.rs` |
| `<` `<=` `>` `>=` on strings, by code point | `ordering_equivalence.rs` |
| `is_eq` … `is_ge`, `reverse`, `then`, `then_with`; `==` on `Ordering` | `ordering_equivalence.rs` |

- **Representation and rewrites** are in [01 §6.6](./01-equivalence.md#66-stdcmpordering) and [§7.10](./01-equivalence.md#710-cmp-and-orderings-methods). The one new runtime part is `Str.cmp`, which a package carries only when it compares strings.
- **Refused:** `impl Ord` / `PartialOrd`, `cmp` on floats, tuples, `Vec`, `Option`, and the crate's types, and `Ordering` as a field (serde has no form for it).
- **Measurement.** semver rewritten with it: 195 → 166 lines, 2.6× → 2.2× by the script, 1.6× adjusted (§2.2). The skill-only draft's 2.8× was the first draft over the threshold since normalization; like the first drafts before it, it came under once its gap's capability was in, on the adjusted count. By the script it stayed over (2.2×) until parsing (§8.8).

### 8.7 0.6.0: generated TypeScript (2026-10-01)

Why: an exception to taking language capabilities first. Reading the examples' generated packages, now committed beside each example, turned up one hole and much that a reviewer of the TS would trip on: a schema that built a closed type with no serde derive from JSON by shape, inline functions wherever a `match`, `?`, index, or consumer met an expression (28 of them), `$q1`-style temporaries, snake_case names, every doc comment dropped, and lines past 160 characters.

| Item | Verified by |
| --- | --- |
| A wire form only from serde's derives; `item/serde-derive` for what a derive holds | `wire.rs`, `ordering.rs` in `check` |
| `impl Display` with a fixed text as `toString`; `try_from` refusals in its words | `display_equivalence.rs`, `wire_write.rs` |
| `fromJson.T(text)` | `wire_write.rs` |
| Runtime `Slice`, `Ord`, `Iter`; guards for `ok_or(..)?`; `match` as expressions | the differential tests, unchanged; shape tests beside them |
| camelCase, JSDoc, string-keyed brands, fewer files and exports | the differential tests, `names.rs` in `check` |
| Every `as` of a sound kind | `casts.rs` |

- **Measurement.** Inline functions 28 → 2 (the two left evaluate a call inside `&&` once); semver's generated code 25.2 → 19.3 KB and payment's 23.5 → 20.3 KB, doc comments now included; counter's runtime copy 11.9 → 6.5 KB and its files 8 → 5; lines of code past 100 characters 129 → 36.
- **Breaking** for callers: the names, the index's exports, and `--schema` on a crate with no serde derive ([CHANGELOG](../CHANGELOG.md)).

### 8.8 0.7.0: lists from text (2026-10-02)

<!-- derived-from #22-line-counts-against-idiomatic-rust -->

Why: after `Ordering`, semver was still the example over 2×, and what remained was parsing (§2.2): `split_once`, and lists built with `collect`. A `Vec` whose length is the input's, read once from text, is not a sequence that grows with the state, which is why `collect` stays out everywhere else (§6).

| Item | Verified by |
| --- | --- |
| `s.split(c).collect()` and `s.split(c).map(f).collect()` into `Vec<T>` or `Result<Vec<T>, E>`, the target named by a turbofish, a `let` type, or the return type | `collect_equivalence.rs` |
| A `Result` stops at the first `Err`, so a later piece that would panic does not run | `collect_equivalence.rs` |
| `str::split_once` with a `char` or a `&str`, including an empty needle | `collect_equivalence.rs` |
| `Some((a, b))`, and the same tuple of names in `Ok` / `Err` or a variant's fields, including under a guard | `tuple_match.rs`, `collect_equivalence.rs` |

- **What is refused.** Collecting a `Vec`, `chars()`, or anything but `split(c)` with a `char`; `collect` with no target type; `split` on a `&str` (an empty separator differs in JS); a variant constructor passed as `.map(PreId::Numeric)` (built with no fields, or, for a unit variant, not a closure or a function name). A variant, a literal, or a tuple inside that tuple of names is still `[pattern/nested]`, as is a tuple nested in a tuple pattern.
- **`str::parse` stays out.** Rust's `u64` parse accepts a leading `+`. Matching that, and the cases it rejects, is not what brings semver under the threshold.
- **Measurement.** semver rewritten with them: 166 → 138 lines, 2.2× → 1.8× (§2.2). Pre-release and build identifiers are `Vec`s. The generated `parse` prints `split` as the array, `Iter.tryCollect` for a `Result`, and `Str.splitOnce`; the turbofish is not a second binding. `Some((x, y))` reads the two strings as `[0]` and `[1]`.
