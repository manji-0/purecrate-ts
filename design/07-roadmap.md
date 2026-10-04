# Roadmap

Status: current (2026-10-04, after 0.10.0; §8.15 unreleased)

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

**Now** (invoice, oidc, payment, and semver remeasured on 2026-10-04, after `clone`, `map` / `sum` over a `Vec`, struct patterns, and `_` in `collect`; oidc's idiomatic side grew with its lockout and consent fixes; payment's struct pattern is longer as rustfmt lays it out; iban is unchanged since 0.4.1):

| Example | Idiomatic | Constrained | Ratio | Earlier (as written) |
| --- | --- | --- | --- | --- |
| signup: email (WHATWG) | 28 | 42 | 1.5× | 2.8× with recursion only; 1.9× with range `for` |
| signup: password (NIST) | 17 | 24 | 1.4× | 1.6×; 1.4× |
| iban | 24 | 44 | 1.8× | 2.4× with recursion only; 1.9× with range `for` |
| invoice | 48 | 76 | 1.6× | 2.2× first draft; 1.4× restructured |
| oidc | 369 | 429 | 1.2× | 1.65× as written from the skill alone |
| payment | 61 | 103 | 1.7× | 2.1× one arm per variant; 1.8× with `_` and `A \| B` |
| semver | 75 | 123 | 1.6× | 138 with `collect`; 166 (2.2×) with `Ordering`; 209 (2.8×) from the skill alone |

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

`find` stays out (§4); `filter` is in since 0.9.0. The others each save a few lines in one function, so none is a candidate on this evidence alone.

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

What the evidence currently points at, strongest first. None is scheduled until §1 is met: an example that cannot be written, or stays over the threshold, without it. `split_once`, a `Vec` collected once from `s.split(c)`, and `Some((a, b))` were the rest of semver's parsing gap; they are in ([§8.8](#88-070-lists-from-text-2026-10-02)), with patterns nested in a case.

| Candidate | Evidence | Note |
| --- | --- | --- |
| Narrowing as TS's control-flow analysis does it: the variants each place may hold, merged where branches join and at a loop's head, a write resetting them | generated bodies: 60 of seeds 1 to 120 refused by tsc (`no overlap`, a property of `never`), values all agreeing | no value disagrees, so it is not a hole; it decides whether generated code type-checks. The fold in `join.rs` is a stack of what enclosing arms, jumps, and tests decided, extended four times in §8.15, each buying a few seeds |
| A local closure's parameter type inferred from its later calls | oidc needed `\|error: ErrorCode\|` (0.4.0 rewrites) | — |
| ~~Growing a `Vec` in a function body, and `map` / `filter` / `collect` over a `Vec`~~ | taken on 2026-10-04 without §1 being met: no example stayed over the threshold, but order's cons list, invoice's sums, and the cost of growing lists only as recursive enums (O(n) access, recursion depth, TS callers who expect arrays) were judged enough. A local `let mut v: Vec<T>` is pushed to, every other array stays unwritten (02 §3.1) | done in 0.9.0 |
| `format!` | Windmill only | `Display` of floats is a large surface; a first step would take only `{}` on integers, `&str`, and `char`, whose text Rust and TS agree on |
| `&mut self` as a function returning the new value (`fn apply(&mut self, e)`) | the aggregate shape in 5 corpus entries | sound because `&mut` excludes aliases, but the TS signature then differs from the Rust one, so the caller contract ([03 §5](./03-output.md#5-caller-contract)) has to say so first |
| Paths through modules (`crate::m::f`, `super::T`) | — | names are already unique after flattening, so this is resolution only |
| Narrowing `as` between integers | — | wraps in Rust and can wrap the same in TS; would give up the single spelling `T::from(x)` for widening |
| Generics and string-keyed maps | — | §5 |
| Associated consts (`impl T { const N: u32 = 3; }`), as members of the type's companion | specified with local `const` in 0.4.0 | waits for a use |
| crates.io and Windows binaries | — | when a user asks; since 0.3.0 the dependencies are crates.io requirements, vendored by source replacement |

**1.0** needs every withdrawal criterion of [06 §4](./06-strategy.md#4-success-and-withdrawal-criteria) answered. The generated API's compatibility within a minor series is [§9](#9-generated-api-stability). The real-use criterion stays open until the tool is adopted unprompted; Oxide `Name` remains local evidence ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)).

## 4. Specified but not yet implemented

`chars()` other than as `for c in s.chars()` or before a consumer; the Unicode-table `char` methods; `isize`; `loop`, labelled `break`/`continue`, `a..=b` in `for`, iterator adaptors other than `map`, `filter`, `copied`, and `cloned` (`rev`, `zip`, `enumerate` before a consumer, `take`, `skip`); byte string literals; the std allow-list beyond what [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods) lists; `static`.

## 5. v1: when type expressiveness runs out

Waits for an example that cannot be written without it.

- Unbounded type parameters, emitted as TS generics (no monomorphization). No bounds, `where`, or associated types.
- `HashMap`/`BTreeMap` with `String` keys only, as `ReadonlyMap<string, V>`. Insertion order is not matched; functions depending on it are rejection candidates.

## 6. Not doing

- Allow-lists aimed at passing existing crates.
- Mutating a `Vec` other than a local's `push`: through a field, an element, a parameter, or `pop` / `insert` / `remove` / `extend`. `split` on a `&str`.
- Decimals; event logs inside state.
- A schema-library dependency in the core runtime.
- WASM. The IR does not preclude a second backend, but the path is TS source.
- `Rc` / `Cell` / `RefCell` / `Mutex`.
- `async`, randomness, and other effects (idsmith's `&mut` RNG parameters, 305 of the corpus's functions).

## 7. Open questions

- Should output typing come from rustc's type information instead of the in-house inference ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate))?
- Hermes support for `JSON.parse` source text ([04 §7](./04-wire.md#7-open-questions)).
- A shared error type with field paths for validation ([01 §4](./01-equivalence.md#4-closed-types)).
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
| A pattern nested in a case: a literal, a range, a variant, or `Some` / `Ok` / `Err` in a variant's field or a payload, to any depth (`PasswordChecked { verified: false, .. }`, `Some(Event::Pay { amount })`) | `nested_patterns_equivalence.rs`, `tuple_match.rs` |

- **What is refused.** Collecting a `Vec`, `chars()`, or anything but `split(c)` with a `char`; `collect` with no target type; `split` on a `&str` (an empty separator differs in JS); a variant constructor passed as `.map(PreId::Numeric)` (built with no fields, or, for a unit variant, not a closure or a function name). A tuple holding anything but names and `_` is still `[pattern/nested]`, as is a tuple nested in a tuple pattern and a side of `|` that tests inside its variant (`A | B(1)`).
- **Nested patterns** go through the decision tree that tuple `match`es and guards already use ([03 §3.3.1](./03-output.md#331-tuple-match)): where a case is chosen, a field some arm tests becomes one more element, matched further in. rustc checks exhaustiveness, so `check` no longer counts cases for such a `match`; every `switch` the tree prints still names every case. oidc's lockout arm reads `PasswordChecked { verified: false, .. }` as its idiomatic code does, which changes no line count: a guard `if !verified` was one line too, and moving `second_factor: SecondFactor::Totp(..)` into the tuple arm would repeat the arm's head. The evidence for them was readability, not the threshold. A `bool` arm now prints as `x` / `!x`, not `x === true`.
- **`str::parse` stays out.** Rust's `u64` parse accepts a leading `+`. Matching that, and the cases it rejects, is not what brings semver under the threshold.
- **Measurement.** semver rewritten with them: 166 → 138 lines, 2.2× → 1.8× (§2.2). Pre-release and build identifiers are `Vec`s. The generated `parse` prints `split` as the array, `Iter.tryCollect` for a `Result`, and `Str.splitOnce`; the turbofish is not a second binding. `Some((x, y))` reads the two strings as `[0]` and `[1]`.

### 8.9 0.8.0: generated TypeScript that reads like the source (2026-10-03)

<!-- derived-from #88-070-lists-from-text-2026-10-02 -->

Why: the four patterns 0.7.0 still refused, and an exception to taking language capabilities first, as in 0.6.0 (§8.7): reading the generated packages beside their Rust, what a reviewer tripped on was the machine in the output (`$`-names, `else` chains, temporaries a combinator left), what the source said and the output dropped (`//` comments, `b'@'` printed as `64`), and a runtime copy of 36 KB whatever the package used. The naming changes reach the stable surface (§9), so this is a minor.

| Item | Verified by |
| --- | --- |
| `A \| B(1)`, `Some((1, b))`, a tuple nested in a case or a tuple | `tuple_match_equivalence.rs`, `nested_patterns_equivalence.rs` |
| `.map(PreId::Numeric)`; `s.parse::<T>()` into an integer; `Result::ok`, `map`, `map_err` | `semver_equivalence.rs`, `parse_equivalence.rs`, `option_methods_equivalence.rs` |
| A one-field tuple variant as `{ kind, value }`, with serde's JSON unchanged | `wire.rs`, `wire_write.rs`, the differential tests |
| No `$` in local or internal names | `scoped_names.rs` |
| Guards instead of `else`; fewer temporaries (`map(f).unwrap_or(d)`, a copied `Some` as `??`) | the differential tests, unchanged; `option_methods_equivalence.rs` |
| `//` comments kept; bodies in paragraphs; byte literals named | `comments.rs`; `paragraph.rs` and `expr.rs` in `emit_ts` |
| A runtime copy of what the package uses | `trim.rs` in `pack` |
| Every generated file laid out and linted as oxfmt and oxlint want, fixtures included | `scripts/verify.sh` |

- **Comments are a barrier.** A `// ..` above a statement is a node between it and the one before, so a rewrite that joins two statements stops at it (`const c = ..;` `// ..` `return c;`). Comments above a `match` arm, after code on a line, or before a block's `}` are still dropped.
- **Paragraphs are counted on the printed TS**, not the Rust statements, so a paragraph can split what one Rust statement became where the TS takes several lines. Blank lines in the source are not carried over: they would be a barrier like comments, at every one of them.
- **Measurement.** All eight examples' generated domain code 3372 → 3225 lines, the comments and 92 blank lines it gains included; semver 414 → 343, oidc 1441 → 1368. Runtime copies 1882 → 1161 lines: counter 175 → 84, payment 261 → 119, semver 351 → 239.
- **Breaking** for callers: `.content[0]` on a one-field variant is `.value`, and the index exports `Int` only when the public surface holds a number ([CHANGELOG](../CHANGELOG.md)).

### 8.10 0.8.1: fixes from an audit (2026-10-03)

<!-- derived-from #89-080-generated-typescript-that-reads-like-the-source-2026-10-03 -->

Why: reading the generator for rules written to the examples' shapes, with each suspected input built and run. No change to what is accepted.

| Item | Verified by |
| --- | --- |
| `??` beside `?:`, `\|\|`, `&&` parenthesized; a commented operand keeps its grouping; a tuple read in part not taken for the whole | `precedence_equivalence.rs` |
| An empty arm under a returned value returns; no `return` after what never falls through | `empty_arm_equivalence.rs` |

- **Open at release**, each reproduced and fixed after it ([CHANGELOG](../CHANGELOG.md), Unreleased): a `?` in a `for` range's end ran before a start that can panic; a `?` inside `map_err(..)?` or an `unwrap_or` argument in an operand position stayed inside an inline function; two arms binding one name to different fields declared it twice; a `match` arm that reassigns its scrutinee read the new value. tsc rejected all but the first.
- **Layout.** `tidy::wrap` is a chain of line rules fitted to the examples; outside them its breaks often differ from oxfmt's, and it measures width in bytes.

### 8.11 0.8.2: the rest of the audit, and layout (2026-10-04)

<!-- derived-from #810-081-fixes-from-an-audit-2026-10-03 -->

Why: the four defects 0.8.1 left open (§8.10), readability items found reading the output beside its Rust, and the audit's finding that the layout rules were fitted to the examples' lines. No change to what is accepted.

| Item | Verified by |
| --- | --- |
| A `for` range's bounds in order; an operand `?` that typing made a `match` leaves the function; a reassigned scrutinee read before the assignment | `order_of_eval_equivalence.rs` |
| Two arms binding one name to different fields | `guards_equivalence.rs` |
| Destructured fields, a flipped negated test, the matched place returned, a variant rebuilt from its fields as the place | `readability_equivalence.rs` |
| Layout: width in columns, a lone import, a break after `=`, an arrow's one-field object, a `?:` chain's test, a hugged last arrow, the wire module's generic type | `fixtures/layout.rs` through `oxfmt --check` in `scripts/verify.sh`; `line_width.rs` |

- **The layout check has an oracle outside the examples now.** `fixtures/layout.rs` gives every construct a name long enough to break it; the shapes it found differing from oxfmt were the ones fixed here. `tidy::wrap` is still a chain of line rules, not oxfmt's printer (oxc's formatter is not published as a crate), so a shape the fixture lacks may still differ; the fixture is where to add it.
- **A block in an argument is lifted** before its statement (`let value;` then `if`), which also removes an inline function from the output; the lifted `let` has no annotation, which TS infers from its assignments.

### 8.12 0.9.0: lists built in a function, and the examples against their specifications (2026-10-04)

<!-- derived-from #811-082-the-rest-of-the-audit-and-layout-2026-10-04 -->

Why: a review of the examples against their specifications (RFC 4226/6238, ISO 13616, SemVer 2.0.0, Stripe's PaymentIntent) found defects, and rewriting them found generator bugs; the authoring gaps it listed (a `Vec` grown in a function, adaptors, struct patterns, `clone`) were taken without §1's threshold being met (§3).

| Item | Verified by |
| --- | --- |
| A helper with one caller's file printed in it; locals renamed apart from the whole file's imports | `readability_equivalence.rs`, `build.rs`; the emitter's homes asserted equal to rename's |
| `?` inside what `ok_or(e)?` takes; two `ok_or(e)?` or `map_err(f)?` in a body | `order_of_eval_equivalence.rs` |
| Temporaries named after their local; literal `matches!` as `includes`; no block around a `?` in `unwrap_or`'s value | `readability_equivalence.rs` |
| `_` in `collect`'s target | `collect_equivalence.rs` |
| Struct patterns | `struct_patterns_equivalence.rs` |
| `cmp` on `Vec`s | `ordering_equivalence.rs` |
| `map` / `filter` over sequences, lazy | `adapters_equivalence.rs`, with an eager `Iter.map` failing it |
| `push` on a local, `clone`, `as_ref`, `as_deref` | `grow_equivalence.rs`: the caller's arrays come back unchanged |
| More `//` comments | `comments.rs` |
| iban's check digits, oidc's lockout and consent, order's overflow and prices, semver's unbounded pre-release numbers | each example's equivalence test and its idiomatic reference |

- **The output writes one kind of array.** A local `let mut v: Vec<T>` is `Array<T>` and is pushed to; it is bound to an array of its own, copied unless new. Every other array stays `ReadonlyArray`, so the earlier argument that sharing is unobservable still holds (01 §7.14).
- **Exhaustiveness and struct patterns.** A struct pattern is lowered to a guard, so a `match` whose struct patterns cover a field still needs `_`; that refuses code rustc takes, never the reverse.

### 8.13 0.9.1: fixes from an audit of 0.9.0 (2026-10-04)

<!-- derived-from #812-090-lists-built-in-a-function-and-the-examples-against-their-specifications-2026-10-04 -->

Why: four audits after 0.9.0 (its new features, the older generator, the output's readability, the documents) found output that disagreed with Rust; each case reached only through shapes the fixtures lacked.

| Item | Verified by |
| --- | --- |
| A choice under `!== null`, `=== null`, or a member parenthesized | `precedence_equivalence.rs`, failing before the fix |
| `ok()` of `Result<Option<T>, E>` refused | `check/tests/it/option_result.rs` |
| A struct pattern's guard and shadowing | `struct_patterns_equivalence.rs`, failing before the fix |
| Line terminators in carried comments | `comments.rs` |
| `else if`, no `c ? true : false`, nested equality | `readability_equivalence.rs`, and oxlint / oxfmt over the fixtures |

- **Where precedence was decided.** The printer grouped an operand by its IR node, and a call (`is_some`) or a `match` printed as an operator slipped past; `grouped` now takes the looser of the node's and the printed text's precedence, and the object of a member is parenthesized unless atomic.
- **Open:** the readability findings (needless copies after `?` and `ok_or`, numbered shadows, scattered destructuring, hex literals, comments outside bodies) and some over-rejections (`i32::MIN` in an expression, a hosted helper named like a type) wait for the next minor.

### 8.14 0.10.0: tests generated from seeds, and what they asked for (2026-10-04)

<!-- derived-from #813-091-fixes-from-an-audit-of-090-2026-10-04 -->

Why: an evaluation of 0.9.1 found that every audit since 0.8.1 turned up output that disagreed with Rust, each only in a shape no fixture spelled, and that the printer decided parentheses by reading its own text back. Hand-written cases could not keep up; generated ones can. A minor, because the runtime's signatures change (§9).

| Item | Verified by |
| --- | --- |
| Functions generated from seeds, run through rustc and the generated TS, compared call by call | `generated_equivalence.rs`: seeds 1 to 120 agree (7,200 functions, 86,400 calls); 1 to 8 on every run |
| A `?` (or `map_err(f)?`) in what `ok_or` takes leaves the function; `unwrap_or` and `ok_or(..)?` keep their options apart; `match o.ok_or(e)` and an annotated block as a scrutinee are typed; a parameter read only in a folded branch is `_` | `order_of_eval_equivalence.rs`, each failing before the fix |
| `Result.ok` / `Result.err` default the type they cannot infer to `never` | the generated seeds, which stopped at `tsc` on `Result<T, unknown>` before |
| What TS has narrowed is folded, only where nothing writes the place | `narrowing_equivalence.rs`; `reassigned_scrutinee` caught a fold that missed a `let mut` written in an arm |
| Parentheses and negation on a tree of the printed operators (`emit_ts::tx`) | `tx.rs` tests; the examples' and fixtures' output byte for byte unchanged, 40 seeds' output only losing parentheses around inline functions |
| rustfmt (120 columns) and clippy in CI | `verify.yml` lint job; `scripts/verify.sh` |

- **06 §4.3 reads the generator, not only the examples.** The row on silent wrong values now names the audits' findings and the generated test's count.
- **Open:** line breaking (`tidy::wrap`) still reads printed lines, as a layout pass; oxc's formatter is not published as a crate.

### 8.15 Unreleased: generated function bodies

<!-- derived-from #814-0100-tests-generated-from-seeds-and-what-they-asked-for-2026-10-04 -->

Why: the generator of 0.10.0 drew expressions only, over four types. Two of the four disagreements the audits found were in statements and crate types, outside it. This adds statements around those expressions; crate types, patterns on them, strings, and `Vec` are still outside.

| Item | Verified by |
| --- | --- |
| Function bodies generated from seeds: `let` / `let mut`, assignment and `op=`, `if` and `match` as statements, range `for` and `while`, `break`, `continue`, early `return`, `?` in a range's ends and a loop's test | `generated_statements_equivalence.rs`: seeds 1 to 8 on every run; 1 to 120 agree on every value (4,800 functions, 57,600 calls), 60 of them type-check |
| `match o.ok_or(x.ok_or(e)?)` leaves the function (it returned from an inline function and took the `Err` arm: Rust `Err(e)`, TS `Ok(..)`) | `order_of_eval_equivalence.rs`, failing on values before the fix |
| A temporary named after a local is numbered (two `xResult` in one block) | `order_of_eval_equivalence.rs` |
| What folding leaves type-checks: a value nothing reads runs only what it does; a decided test keeps the side taken (`if (true) { .. }` where it declares); what follows a jump goes; `r?` on a known `Err` is the `return` | `narrowing_equivalence.rs`, each refused by tsc before |
| Narrowing past a statement `match` with jumping arms, past an `if` that always leaves, and of `bool`s an `if` decided; `if c { p } else { p }` and `{ let t = p; t }` are `p` | `narrowing_equivalence.rs` |
| A sweep compares values even where tsc refuses (`PURECRATE_GEN_TYPES=report`, only with `PURECRATE_GEN_SEED`); every run's seeds still type-check | `support::assert_values_equivalent` |

- **Open:** the 60 seeds tsc refuses (§3, narrowing as TS does it). `?` in `matches!`'s first argument inside a test is refused (`[check/position]`) although that operand always runs; safe, and written around with a `let`.
- **The examples' output is unchanged.** Fixtures' output changed only where a decided `bool` or test folded (`guards.rs`, `bool_patterns.rs`).

## 9. Generated API stability

Users commit the generated output and check it with `check --out` in CI, so a change to the bytes is a repo-wide diff plus caller updates. Within a **minor** series (`0.N.x` today; `N.x` after 1.0) the following are stable, and a change to them is a **minor** bump (a **major** after 1.0):

| Stable | Examples |
| --- | --- |
| Public export names | functions, types, companions, `toJson` / `fromJson` keys |
| Type shapes | `kind` unions, field names, brand keys, `Option` as `T \| null`, `Result` as `{ kind, value \| error }` |
| Wire format | serde's default JSON, and the adapter that reads it |
| Runtime API | `Int`, `Result`, `Panic`, `Str`, `Slice`, `Ord`, `Iter`, `parseJson` — names, signatures, panic messages |

The following may change in a **patch** (formatting of generated files included). Callers must not depend on them:

| Unstable | Examples |
| --- | --- |
| Formatting | line wrapping, parentheses that precedence does not need, import grouping, JSDoc layout |
| Internal helpers | `unsafeMakeX`, a non-`pub` method's `xName`, the wire module's `DomainX` aliases, temps (`majorResult`, `majorOr`), names of locals, the file a function the package does not export is printed in |
| Local names | a binding that is not an export; numbering when a name is shadowed |
| Trimmed runtime shape | empty namespaces dropped, which `Int` widths, operators, and methods a copy keeps |

`check --out` still fails on any byte change, including unstable ones: pin the `purecrate-ts` version that generated the committed output. Reason codes (`[type/mutex]`, `[check/nested-option]`) are stable within a minor series; messages may change.

Cleanup of generated code (identity casts, scoped names, wrapping) is therefore a patch when behavior and the stable surface stay the same. A change such as single-field tuple variants becoming `{ kind, value }` instead of `{ kind, content: [T] }` (0.8.0), or honouring `#[serde(rename_all = "camelCase")]` on field names, is a minor (a major after 1.0).
