# Roadmap

Status: current (2026-10-15, 0.21.0)

<!-- constrained-by ./02-authoring.md -->
<!-- constrained-by ./06-strategy.md#4-success-and-withdrawal-criteria -->

What comes next and why. §1 is the rule, §2 the evidence it runs on, §3 the candidates it currently yields, §4–§7 what is specified, deferred, refused, or open, and §8 what each release added.

## 1. How additions are chosen

**Add a capability only when an example written within the constraints cannot be written without it.** Rejection counts in a corpus do not set the order ([90](./90-acceptance-survey.md) is not a metric). Two exceptions skip the wait:

- **Holes** — anything that makes accepted input compute a wrong value, or lets `check` accept what rustc rejects, is fixed immediately.
- **Domain fixes** — changes that tighten the domain of equivalence (closed types) rather than add capability.

An example whose constrained Rust is over 2× its idiomatic reference ([06 §4.2](./06-strategy.md#42-external-criteria)) meets this rule for the capabilities its excess names, and they go first in §3, if they help code in general rather than one domain. Over 2× is a priority, not a condition to withdraw. Excess that is no missing capability (a reference that does less work, a closed type's checks), or that only a narrow addition would remove (one for JSON alone, say), is named in §2.2 and raises nothing.

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
| oidc (OIDC Core 1.0 OP login, PKCE RFC 7636, TOTP RFC 6238/4226 as the second factor, amr RFC 8176) | third party, from the authoring skill alone | RFC 4226 truncation's `&` and `<<` (written with `%` and `*`); no way to build a `Vec` for `amr` (a recursive enum instead; the documents' "build with `[a, b]`" was wrong); `const`; `==` on `Option` and enums, `is_some`; `u8::is_ascii_digit`; 15 rejections, about 6 not predictable from the skill | documents corrected (§2.4); later `is_some`/`is_none`, bit operators, and `vec![a, b]` ([CHANGELOG 0.1.0](../CHANGELOG.md#rationale-the-first-release)) |
| punycode (RFC 3492 Punycode with §6.4 overflow, and a simplified IDNA layer: `xn--` labels, lengths, the round trip) | third party, from the authoring skill alone | `check`: 5 rejections in 4 rounds, all on building a `String`: `String::new` (twice; the reason code `[expr/external-path]` is in no document), `collect::<String>()`, `String::from(char)`, `+` on `String`. A wall: no `String` can be built from computed characters, so the four entry points return `Vec<char>`. Avoided up front: `Vec::insert` (a rebuilt `Vec`), `loop`, integer `as`, `format!` / `join`, `min` over a filter. Then a hole in accepted code that changed a value: `while !done` on a flag its body sets was decided by the flag's value before the loop (an endless loop in TS; found by the author's own differential run); and oxfmt rewrote a signature whose `=> {` overflowed | the hole closed (a `while` test is decided at the loop's head) and the statements generator taught flags; the layout; the wall closed after it (`String::new`, `push`, `push_str`, `collect::<String>()`; design/01 §7.15) |
| calendar (RFC 3339 timestamps and leap seconds, Hinnant's civil-date algorithms over -9999..=9999, ISO 8601 week dates, RFC 5545 RRULE parsing and expansion: DAILY to YEARLY, INTERVAL, COUNT, UNTIL, BYDAY with ordinals, BYMONTHDAY, BYMONTH, WKST) | third party, from the authoring skill alone | `check`: 3 rejections in 3 rounds, both kinds not predictable: `for` over `a..=b`, and `for m in 1..13` needing a suffix although `m` is not an index. Avoided up front: `div_euclid` (written as `floor_div`), `eq_ignore_ascii_case` on `&str` (a byte loop), `Vec::contains`, narrowing back to `i32` (every field `i64`), a builder or map for the rule's parts (eight `Option` locals). Then holes in accepted code: oxlint an `else` after `return` in a loop's `match` and a cast inside `BigInt(..)`, oxfmt a `while` test calling a function in place, a long `?:` branch, and a parenthesized group | the holes closed (an exit first in a loop body, the layouts); skill corrected (§2.4) |
| ssh (RFC 4253 and RFC 4252, client side: version exchange, KEXINIT negotiation, guessed packets, OpenSSH's strict key exchange, known_hosts, publickey and password, re-exchange) | third party | `check`: five file-name collisions (a known rule); `?` and `return` in the arms of a `match` bound by a tuple `let` (the skill said a whole arm value is fine; not under a tuple pattern); `if let` on an enum variant (the skill listed `if let` without saying it is `Option`'s only); `if c { 0 } else { x }` with `x: u32` needing `: u32`. Then three holes in accepted code: `tsc` refused a `case` narrowing had ruled out (TS2678), oxlint an `as` on a tuple element read right after `?`, and oxfmt three long-line layouts | the holes closed (narrowing knows what an enum's `_` holds; `const [a, b] = result.value`; logical initializers and arguments laid out as oxfmt does); skill corrected (§2.4) |
| semver (SemVer 2.0.0: parsing, §11 precedence) | third party, from the authoring skill alone | about 12 rejections in 5 rounds, 6 not predictable from the skill: `std::cmp::Ordering` and `.cmp()`, `..` in a tuple variant, a bare binding arm (`other => other`), `?` in a tuple inside an arm, `let mut x = None` without a type, no `u64::from(usize)`; ASCII string order written byte by byte; identifier lists as recursive enums | skill corrected (§2.4); `Ordering`, `cmp`, and string ordering ([CHANGELOG 0.5.0](../CHANGELOG.md#rationale-ordering)) |
| payment (Stripe PaymentIntent lifecycle) | third party | nothing on the first pass; but transition lines at 2.1× (28 of 160 were `=> Err(InvalidTransition)`); the client sends events back, so values must be written as serde JSON; `terms`/`outcome`/`amount` driver functions collided with types (known rule) | `_` and binding-free `A \| B` arms; `toJson` (and rejected unit structs, which serde writes differently from `struct S {}`) |
| jsonpatch (RFC 8259 text, RFC 6901 JSON Pointer, RFC 6902 JSON Patch: all six operations, the root, `move` into its own child; integers as `i64` only, a repeated key refused) | third party, from the authoring skill alone (2026-10-11) | `check`: nothing, in one round. Avoided up front, 14 shapes, each named by the skill: in-place `Vec` edits (`remove`, `insert`, `v[i] = x`, `&mut`), `to_string` / `format!` for numbers and `\u00XX`, `.rev()`, `==` on the value tree, `HashMap`, `\|` arms that bind, `?` inside an `if` value. One of those was wrong: `Vec::insert` had been accepted since 0.13.0 and the skill still listed it as refused, so the draft rebuilt the list. Then two holes in accepted code: oxlint refused a `let` of an enum-typed place cast back to its type where nothing had narrowed it (`doc as Json`, a parameter or a `?`'s value), and oxfmt laid out a long `while` test otherwise | the holes closed (only a place a `match` tests, a name its arm binds, or a copy of one is cast; a `while` test opens as an `if`'s); skill and the method allow-list message corrected (§2.4) |
| geo (Google's Encoded Polyline at precision 5 and 6, Geohash at lengths 1 to 12, a closed `LatLng`; all on `f64`) | third party, from the authoring skill alone (2026-10-11); the author wrote both algorithms from memory without fetching the specifications, and the test checked them against the published examples | `check`: nothing, in one round. A wall avoided up front: no float method and no conversion between a float and an integer (`round`, `floor`, `is_nan`, `f64::NAN`, `as i64`, `as f64`, `f64::from`), and no narrowing for a 6-bit chunk to a `u8`. Written around it: rounding by subtracting powers of two from 2^40 down (each subtraction exact, so half away from zero is exact), an integer to a float as a sum of powers of two, the chunk copied bit by bit, NaN as `x != x`, infinity as `x - x != 0.0`. Probed after: `[expr/cast]` on `f64 as i64` advised `T::from(x)`, which `f64::from` refuses, and `f64::NAN` was reported as an undefined function. Then two holes in accepted code: oxlint refused `-x` on a branded `F64`, and a `u32 as usize` passed to `Slice.at`, which takes a plain `number`, as an unnecessary cast | the holes closed (a float is negated as `x * -1`, exact, a literal bare; an index widened from a 32-bit integer is read as it is); both messages say what `as` and std paths allow; skill (§2.4) |
| ulid (the ULID specification: 48-bit time and 80-bit randomness in a closed pair of `u64`s, Crockford base32 both ways, the 16-byte form, monotonic generation with its overflow, ordering) | third party, from the authoring skill alone (2026-10-11) | `check`: nothing, in one round, and no hole: the package passed tsc, oxlint, and oxfmt as generated and agreed with Rust on every case. Avoided up front, each named by the skill: narrowing (`(v & 31) as usize` to index the alphabet, `(v >> 8) as u8` for a byte), `from_be_bytes` / `to_be_bytes`, a byte-string alphabet, `u64::MAX`, `derive(Ord)`, `Display` / `FromStr`, `loop`. Written around: two 32-arm `match`es for the alphabet each way, a byte built bit by bit, the 128 bits as two `u64`s with the carry between them by hand. The test met two gaps in the harness instead: a string with U+0000 before a digit was written as a legacy octal escape node refuses, and methods returning `String`, `Vec<u8>`, or `u64` needed a driver | harness fixed (`\u{0}`; `fixtures/ulid_driver.rs`) |
| negotiate (RFC 9110 §12: `Accept`, `Accept-Encoding`, and `Accept-Language` parsed with quoted strings and q in thousandths, representations ranked by q, specificity, and the server's order, the best chosen or 406; RFC 4647 basic filtering) | third party, from the authoring skill alone (2026-10-11) | `check`: nothing, in one round. Avoided up front, each named by the skill: `sort_by` (a stable insertion sort with `Vec::insert`), `str::to_ascii_lowercase` (`chars().map(..).collect::<String>()`), `HashMap`, `Option` combinators, `==` on `Option`, `loop`, `.rev()`, and a file-name collision. Then one hole in accepted code: a companion `of` whose head fits up to `=> ({` opened its parameters where oxfmt opens the short object body. The test found RFC 9110's own table wrong in one row (`text/html;level=3` at 0.7), as Erratum 7138 (verified) says; the model gives the corrected 0.3 | the hole closed (an arrow's object body opens however short it is, once its head fits) |
| raft (the Raft paper's Figure 2 as a pure `step(node, event)`: elections, the up-to-date check, log replication with conflict repair, the current-term commit rule of §5.4.2; `Uuid` node ids; no membership change or snapshot) | third party, from the authoring skill alone (2026-10-11); the author wrote Figure 2 from memory without fetching the paper, and the test checked it rule by rule and against Figures 7 and 8 | `check`: nothing, in one round, and nothing beyond the subset was needed: per-peer progress and truncate-and-append are lists rebuilt with `push`, and `match node.role.clone()` keeps `..node` usable. Then two holes in accepted code, both layout: oxfmt breaks an opened `if (` test at every top-level `||` though it would fit on one line, and a range `for` whose two declarators do not fit puts the second under the first; a too-long comparison inside an opened test breaks after its operator | the holes closed (`opened_test`, the `for` declarators) |

### 2.2 Line counts against idiomatic Rust

Non-blank, non-comment lines of logic (functions and inherent impls), both sides formatted by rustfmt at width 120 (`scripts/line-counts.py`), so layout does not decide the ratio. Over 2× puts what the excess names first in §3 ([§1](#1-how-additions-are-chosen), [06 §4.2](./06-strategy.md#42-external-criteria)).

**Now** (`scripts/line-counts.py`, 2026-10-14; signup by hand). The independent reviews' fixes (2026-10-05, below) are in these figures:

| Example | Idiomatic | Constrained | Ratio | Earlier (as written) |
| --- | --- | --- | --- | --- |
| signup: email (WHATWG) | 28 | 42 | 1.5× | 2.8× with recursion only; 1.9× with range `for` |
| signup: password (NIST) | 17 | 24 | 1.4× | 1.6×; 1.4× |
| iban | 24 | 46 | 1.9× | 2.4× with recursion only; 1.9× with range `for`; 48 (2.0×) before 0.18.0's `fold` |
| invoice | 54 | 96 | 1.8× | 2.2× first draft; 1.4× restructured |
| punycode | 152 | 250 | 1.6× | 302 (2.0×) as written from the skill alone (2026-10-05), with results as `Vec<char>`; 285 with `String` building; 267 before 0.17.0's `if let`, `let`-`else`, and `loop`; 263 before 0.18.0's `rev`, `map_or`, and `expect` |
| calendar | 439 | 729 | 1.7× | 770 (1.8×) as written from the skill alone (2026-10-05); with `str::eq_ignore_ascii_case`, 750, and 722 with name tables; 738 before 0.17.0's `let`-`else`, `if let`, and `..=`; 732 before 0.18.0's `contains`; by section below |
| jsonpatch | 159 | 621 | 3.9× | not like for like: the reference parses with serde_json; the patch layer alone is 107 / 304 (2.8×), below. 789 as written (2026-10-11), 781 with `Vec::insert`, 648 with 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)), 633 with `format!` for its numbers (0.16.0), 621 with `zip` and `is_some_and` (0.18.0) |
| geo | 135 | 222 | 1.6× | 297 (2.2×) as written (2026-10-11); 222 with 0.13.1's float methods and casts ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)) |
| ulid | 59 | 79 | 1.3× | 201 (3.4×) as written (2026-10-11); 95 with `u128`, narrowing `as`, and a byte-string alphabet ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)); 94 with `let`-`else` (0.17.0); 79 with `fold`, `filter`, and `map_or` (0.18.0) |
| negotiate | 188 | 282 | 1.5× | 494 (2.6×) as written (2026-10-11); 295 with 0.13.1's string methods, sorts, and consumers ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)); 282 with `or_else`, `filter`, and `first` (0.18.0) |
| oidc | 390 | 568 | 1.5× | 1.65× as written from the skill alone; 576 before 0.17.0's `let`-`else`; 573 before 0.18.0's `is_none_or` and `map_or` |
| payment | 52 | 122 | 2.3× | 2.1× one arm per variant; 1.8× with `_` and `A \| B`; 125 (2.4×) before 0.18.0's `is_none_or` |
| semver | 72 | 124 | 1.7× | 165 (2.3×) before 0.16.0, whose `format!` replaced the digits built by hand; 142 (2.0×) before 0.18.0's `map_or`, `zip`, and `fold`; 138 with `collect`; 166 (2.2×) with `Ordering`; 209 (2.8×) from the skill alone |

**After the independent reviews** (2026-10-05, `scripts/line-counts.py`, logic only): the fixes the reviews asked for moved four examples. invoice 53 / 93 (1.8×): one rounding per rate on converted totals, three methods, and checked arithmetic. oidc 369 / 514 (1.4×): SHA-256 and base64url are written in the subset, so the PKCE challenge is computed from the verifier rather than passed in. payment 46 / 109 (2.4×): the checked constructor an intent read from JSON goes through. semver 72 / 165 (2.3×): equality and formatting, with the decimal digits of a `u64` built by hand, as no number turns into text in the subset. punycode went down, 155 / 271 (1.7×), with `Vec::insert`.

**punycode**, as written, sat at the threshold in both parts (the core 99 / 193, the domain layer 55 / 109), and for one reason more than any other: the results are `Vec<char>` pushed one character at a time, where idiomatic code builds a `String` (`collect`, `push`, `format!("xn--{p}")`, `labels.join(".")`). The rest is `Vec::insert` rebuilt by hand, a `min` over a filter written as a scan, and counters kept beside the `Vec`s because no integer converts to `usize`. With `String` building the results are `String`s pushed to directly (302 to 285 lines); what is left is the `insert` and the scan. An independent review then measured the rebuilt `Vec`: decoding 16,000 CJK characters took 337 ms against Node's 6 ms, the array copied twice per code point. `Vec::insert` on a local and `u32 as usize` (std has no `usize::from(u32)`) closed it (17 ms); what is left is the scan.

**calendar by section** (both sides rustfmt'd): civil dates 103 / 150 (1.5×), timestamps 114 / 162 (1.4×), rule types 82 / 121 (1.5×), rule parsing 139 / 274 (2.0×), expansion 138 / 204 (1.5×). With `str::eq_ignore_ascii_case` (added for it), rule parsing is 254 (1.8×), and 226 (1.6×) once the names are looked up in tables (`position`, then a `vec![..]` indexed) as the idiomatic side does. As written, rule parsing carried the gap: names compared without case by a byte loop and an `if` per name (idiomatic code looks them up in a table with `eq_ignore_ascii_case`), and one `Option` local per part with its own repeat check (idiomatic code keeps a list of the parts seen). Expansion stayed close because the constrained side generates candidates in order, so it needs no `sort` / `dedup`.

**jsonpatch** (2026-10-11) is 159 / 781 (4.9×) whole, but the two sides do not do the same work: the idiomatic reference reads and writes JSON with serde_json and walks pointers with its `pointer` / `pointer_mut`, so the constrained side's parser and serializer (407 lines) have no counterpart beyond the 52 lines that make serde_json refuse a repeated key and a non-integer. The patch layer is 107 / 374 (3.5×), over the threshold, and the excess is three things the subset lacks rather than library: no `&mut`, so an edit rebuilds the path from the root down (`edit_at`); no `Vec::remove` or `v[i] = x`, so removing or replacing an element or a member is a loop that copies the rest (four functions); and no `==` on the value tree, so `test` needs a hand-written `json_eq`. As written it was 789; with `Vec::insert`, which the skill had wrongly listed as refused, 781 (§2.4).

**geo** (2026-10-11) is 135 / 297 (2.2×), and the excess is the float wall: `split_magnitude` and `round_half_away` stand for `x.round() as i64`, `int_to_f64` for `n as f64`, `chunk_char` for `(c + 63) as u8`, and the NaN and infinity tests for `is_nan` / `is_finite`. They are exact (the cross-check found no difference in any last bit over 796 values that scale to exactly .5), but a reader must take that on trust where idiomatic code says `round`.

**ulid** (2026-10-11) is 59 / 201 (3.4×). Two thirds of the excess is the alphabet: without a narrowing cast a 5-bit value cannot index a table, so each direction is a 32-arm `match`; the rest is the 128 bits held as two `u64`s (no `u128`), whose shifts and carry cross between them by hand, and bytes built bit by bit (no `to_be_bytes`, no `as u8`).

**negotiate** (2026-10-11) is 188 / 494 (2.6×). The list scanner is 93 lines against 46 (no `split` with a closure, `find`, or `trim_matches`, so quoted strings are read byte by byte), the qvalue grammar 33 against 12, and each of the three rankings about 35 against 15 (no `sort_by` or `max_by_key`: a hand-written stable insertion sort). **raft** is not measured: like ssh, its test asserts Figure 2's rules one by one and runs a simulation instead of a second implementation.

signup is counted by hand, as its test has no idiomatic module; ssh is not measured, as its test has none either (its rules are asserted one by one instead, §2.3). The "earlier" figures were taken as written, before rustfmt normalization; they are comparable with each other, not with the "now" column.

**What moved the numbers.** Each capability, the example it was measured on, and what it replaced:

- **0.13.1's additions** (jsonpatch, geo, ulid, negotiate; [CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)): each example rewritten against its unchanged test. geo 297 → 222: `round`, `is_nan`, `as i64` / `as f64`, `as u8`, a byte-string alphabet replaced four hand-written exact substitutes. ulid 201 → 95: one `u128` and a table indexed by `as usize` replaced the two `u64` halves and the two 32-arm `match`es. negotiate 494 → 295: `scan_list` 93 → 53, the qvalue grammar 33 → 17, each ranking about 35 → 17 (`sort_by` with `then`, `min_by`). jsonpatch 781 → 648, the patch layer 374 → 304: `remove` and `v[i] = x` on a copy replaced the four copy loops, `==` the token comparisons; its `edit_at` still rebuilds the path from the root (no `&mut`), and `json_eq` stays, as RFC 6902 compares object members without their order.
- **Range `for`** (signup, iban): recursion. Afterwards most of signup's gap was character classes spelled as numeric comparisons.
- **Byte literals, integer literal and range patterns, `matches!`** (`int_patterns_equivalence.rs`): the local part of the e-mail address went from 14 lines of comparisons to one `matches!`. IBAN barely moved; its gap is the loops. A `match … { … => true, _ => false }` was longer than the comparisons it replaced (IBAN 50 lines), which is why `matches!` came with it.
- **`_` and `A | B` arms** (payment): 160 → 135 lines of transitions. The rest was one function per state instead of a tuple `match`, and `match` on `Option` where idiomatic code calls `ok_or`, `unwrap_or`, `map`, and `min`.
- **A filtered sum per group** (invoice, a restructuring, no capability): the first draft kept four running totals in a struct updated line by line (104 lines); summing each group with a range `for`, as idiomatic code sums with `filter`, gave 68.
- **Bit operators, `const`, discriminants** (oidc): barely moved the count, since each named limit costs a declaration, but RFC 4226 truncation now reads as §5.4 writes it (`& 0x0f`, `<< 24 |`) instead of `% 16` and a base-256 sum, and no number is left unexplained.
- **`for b in s.bytes()`, `for x in &xs`** (oidc, 0.2.0): lexical helpers 99 → 95. Smaller than expected: most of the gap was splitting a space-delimited list, which needs token boundaries a plain `for` does not give.
- **`for t in s.split(c)`** (oidc, 0.2.0): lexical helpers 95 → 68; `span_equals` is gone.
- **`vec![a, b]`** (oidc): the `AmrList` enum became `Vec<String>`, 8 lines shorter.
- **Guards, tuple `match`, `ok_or` / `unwrap_or` / `map`, `all` / `any`, `min`, `pow`** (oidc, payment, invoice; the 0.4.0 rewrites): guards wherever a state or a field chose the path, tuple `match` in place of per-state handlers (payment's five, oidc's three), the `Option` methods in place of `match`es. oidc's file went 777 → 629 lines, payment's logic 163 → 111, invoice's 74 → 66 (as written).
- **Unused by the rewrites:** slicing and `strip_*`, `const` in a function, `position`, `count`, `sum`, `enumerate`, the `checked_*` / `saturating_*` / `wrapping_*` forms, tuple `let` (tried in invoice; with long names rustfmt made it longer). Kept anyway ([CHANGELOG 0.4.0](../CHANGELOG.md#rationale-iteration-part-two)).

**semver**, the first example over 2× after rustfmt normalization. The draft from the skill alone was 209 lines; restructuring its ordering within the subset (a `then` helper, a byte loop through `compare_u64`) gave 195; with `Ordering` (0.5.0, [CHANGELOG 0.5.0](../CHANGELOG.md#rationale-ordering)), 166. By part:

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
| invoice | the NTA's own worked examples (60,000 × 10/110 ≒ 5,454; 23,894 × 10% ≒ 2,389 where rounding per line would give 2,388; 問59's receipt at 948 both ways), asserted in `invoice_equivalence.rs` next to an idiomatic-Rust cross-check and the Rust/TS differential test (about 28,000 invoices; overflow is `Err(Overflow)` exactly where an `i128` reference passes `i64::MAX`) |
| semver | an idiomatic-Rust implementation (`split_once`, `collect`, `impl Ord`), on the spec's examples, one-character edits of them, and `u64` edges, and every pair's precedence; §11's ordered chain asserted on both Rust sides. Swapping numeric and alphanumeric order in the constrained side fails both |
| oidc | the idiomatic reference; since `vec![a, b]` the cross-check no longer rewrites one side's `Debug` text |
| punycode | RFC 3492 §7.1's samples (A) to (S) both ways, digits upper-cased too, and the IDNA examples; an idiomatic-Rust reference (`String`, `Vec::insert`, `format!`, `join`) on every string of one to three of twelve edge code points, every three-digit label, and names around each length limit |
| calendar | RFC 3339 §5.8's examples and the RRULE examples of RFC 5545 §3.8.5.3 in the subset (daily, every other week with WKST=SU, the first Friday, Friday the 13th, the third-to-last day, WKST=MO against SU, the 20th Monday), asserted on the constrained side; an idiomatic-Rust reference (`div_euclid`, iterators, `sort` / `dedup`, `eq_ignore_ascii_case`) on every day count from -9999 to 9999 and two past each end, on timestamps around each offset and month end, and on 40 rules expanded from six starts under four limits |
| ssh | an independent review (2026-10-05) that read only the specifications and the code: about 31,000 scenarios of its own through the package and the Rust model agreed byte for byte; it found five departures of the model from the specifications (the server's guessed packet, EXT_INFO replacement, known_hosts names, `kex-strict-s`, @revoked's scope) and two gaps (PK_OK's key, RSA trial and error), fixed or declared since. And each rule asserted as the specifications state it (`ssh_equivalence.rs`): the version line and its 255-character limit, §7.1 negotiation per direction with no MAC under an AEAD cipher, both sides' guesses, strict key exchange (KEXINIT first, no IGNORE during it, sequence numbers from 0 after NEWKEYS), known_hosts under each policy and `[host]:port`, RFC 4252's methods and PK_OK, UNIMPLEMENTED's sequence number, re-exchange keeping the session identifier and the host key, framing, and the key lengths of §7.2. The differential test adds every two of 18 events after each point of the handshake |
| jsonpatch | RFC 6902 Appendix A.1 to A.16 (A.13's repeated `op` refused as a repeated key, which the RFC leaves open) and RFC 6901 §5's twelve pointers, asserted on the constrained side; an idiomatic reference over `serde_json::Value` (`pointer_mut`, `Vec::insert` / `remove`, `==`, a strict visitor for repeated keys and non-integers) on 82 texts read and written back (54 malformed), 368 pointer evaluations, and 54,204 patches (every single operation over 46 pointers and the values, every `move` / `copy` pair, malformed operations, and every two-operation patch over a smaller set on four documents); the serializer's output with members sorted is byte for byte serde_json's. Errors are compared by where the patch went wrong (the operation's index and one of four kinds), not by their detail |
| geo | Google's worked examples both ways (the three points, -179.9832104), precision 6 round trips, Geohash `ezs42` and `u4pruydqqvj`, and every error case, asserted on the constrained side; an idiomatic reference (`round`, `as`, `is_nan`, iterators) on 2,710 coordinates (796 scaling to exactly .5, ±0, subnormals, the range ends ±2 ulps, NaN, ±inf), about 82,000 single points, 300 seeded lists decoded whole and by prefix, texts around each accumulator limit, 261 malformed texts, and 82,704 geohashes over cell boundaries ±1 ulp; floats compared bit for bit |
| ulid | the README's example (`01ARZ3NDEKTSV4RRFFQ69G5FAV`, its timestamp, lower and mixed case), its layout example, the maximum and the overflow, both monotonic sequences to exhaustion, and the binary layout, asserted on the constrained side; an idiomatic reference (`u128`, `to_be_bytes`, a byte-string alphabet, `derive(Ord)`) on 109,216 single-character mutations, every length to 30, 5,430 values through every form, 32,467 ordered pairs (`compare`, text, and bytes agree), 18,000 `next` steps, and byte inputs of every length to 20 |
| negotiate | RFC 9110 §12.5.1's precedence example and its table (with Erratum 7138), `audio/*; q=0.2, audio/basic`, §12.5.3's five Accept-Encoding fields and the identity rules, §12.5.4's example, and §12.4.2's qvalue grammar with exact error offsets, asserted on the constrained side; an idiomatic reference (`sort_by`, `max_by_key`, `to_ascii_lowercase`) on 12,000 fields drawn from the grammar and 57,390 one-character mutations, comparing the parse, the whole ranking, and the choice |
| raft | 14 tests asserting Figure 2's rules one by one (RequestVote and the up-to-date check, AppendEntries' five steps, All Servers in each role, candidates, leaders' back-off and the §5.4.2 commit rule), Figure 7 repaired by a term-8 leader, both branches of Figure 8; a simulation of 3 and 5 nodes over 80 seeds × 4,000 events (drops, duplicates, reordering, crashes, restarts) checking Election Safety, Log Matching, Leader Completeness, and State Machine Safety after every step; the differential test steps every (node, event) pair four recorded runs reached, every event kind after 14 states, and 16 whole replays in TS |

### 2.4 What the measurements changed

Besides the capabilities in §2.1:

- **String methods, from payment.** payment's ID check read `b[0] != 112u8 || b[1] != 109u8 || b[2] != 95u8`. `str::len`, `is_empty`, `starts_with`, `ends_with`, and `contains` were added to the allow-list, and it reads `!raw.starts_with("pm_")`.
- **serde, from payment** (preparing it for a real server). A crate that derived serde failed `check` (rustc had no serde), and closed types could not keep their invariants on the wire. `check` now compiles against a stand-in serde; `#[serde(try_from = "T")]` with `impl TryFrom<T>` is accepted, and `impl Display` / `Error` are skipped ([04 §5](./04-wire.md#5-closed-types-on-the-wire)). payment reads `Amount` and `PaymentMethodId` through their constructors on both sides.
- **Match guards, from invoice.** Every rejection on the way had a subset spelling. Guards were the most frequent (three in invoice, and the idiomatic payment uses two), and were added in 0.3.0 ([CHANGELOG 0.3.0](../CHANGELOG.md#rationale-guards-and-option)).
- **The authoring skill, from oidc.** The skill had one wrong line (building a `Vec` from `[a, b]`, also in 02 §3.1) and lacked the method allow-list, bit operators, `const`, the enum/`Option` equality rewrites, the phase order of diagnostics, and the by-shape reading of closed types under serde. All were added.
- **The authoring skill, from calendar.** Both kinds of rejection now have a line (`for` ranges are half-open only; a range of two bare literals needs a suffix, wherever the variable goes), as do two things the author found only by trying: integers never narrow, so a chain that meets an `i64` stays `i64`; and a method may share a type's file name.
- **The authoring skill, from ssh.** Three rejections were not predictable from the skill, and now have a line: `if let` takes `Some(x)` only; a tuple `let` bound to a `match` takes no `?` or `return` in its arms; and an `if` whose one side is a bare literal needs the binding typed.
- **The authoring skill, from semver.** Six of its rejections were not predictable from the skill; each now has a line: `..` in a tuple variant, a bare binding arm, `?` in a tuple inside an arm, `let mut x = None` needing its type, `Ordering` / `cmp` and what to write instead, and no `u64::from(usize)`.
- **The authoring skill, from jsonpatch.** The draft met no rejection, but the skill had fallen behind 0.13.0: it listed `insert` among what a `Vec` refuses, and named neither `x as usize` from `u8` / `u16` / `u32` nor `for _ in`. The author believed it and rebuilt a list where `insert` would do. All three now have a line, `remove` is named as refused (rebuild with `push`), and `[expr/method-call]` on a `Vec` lists `insert` beside `push`.
- **The authoring skill, from geo.** The skill named `f32`/`f64` and nothing else about them. It now says what floats take (operators, unary `-`, `1e5`), that no method or integer conversion is accepted, and how to test NaN.
- **Diagnostics, from oidc:**

  | Problem | Fix |
  | --- | --- |
  | `[expr/operator]` does not name the operator | the operator is named |
  | `==` on enums is reported as `numeric-op` | `==` and ordering on a type JS compares differently are `[check/comparison]`, with the rewrite for `Option`, the crate's types, and strings |
  | `[expr/method-call]` says "only methods of the crate's own inherent impls" for std methods outside the allow-list | a std method outside the allow-list lists what its receiver allows (and, on `u8`, the `matches!` spelling); a missing method on the crate's own type says so |
  | some locations point at the statement rather than the call | calls and method calls carry their own position |

- **The type/function file-name collision** (§3.3 rule 5 of [02](./02-authoring.md#33-names-are-unique-across-the-crate)) has been hit four times: three driver helpers named after the type they build, and `fn group` returning a `Group` in invoice itself. The rule stays; the diagnostic names both items.

### 2.5 Distance to the ideal (2026-10-13, 0.21.0 2026-10-15)

<!-- constrained-by ./02-authoring.md#1-what-constraint-costs -->

An estimate, not a criterion. The ideal is 02 §1's goal read widely: the shapes domain code commonly takes in the use cases in focus, beyond those the sixteen examples happened to need, each written as idiomatic Rust writes it. `corpus/patterns.rs` holds one public function per shape, and `purecrate-ts survey corpus/patterns.rs` judges it; the shapes are grouped below, weighted by how often domain code takes them (3, 2, 1: judgment, backed by [90 §8](./90-acceptance-survey.md#8-re-measured-with-0150-2026-10-13)'s counts where it has them), and scored 1 (written as idiomatic Rust writes it), 0.5 (another spelling, about as short), 0.2 (written by hand, much longer), or 0 (cannot be written). Nothing scored 0.

| Shapes | Weight | Score | Evidence |
| --- | --- | --- | --- |
| ADTs, closed types, newtypes | 3 | 1 | accepted |
| `match`, guards, tuple `match`, nested patterns | 3 | 1 | accepted |
| `?`, early `return`, `Result` / `Option` | 3 | 1 | accepted |
| Integers with debug semantics, `checked_*`, `i64` to `u128` | 3 | 1 | accepted |
| `Uuid` | 1 | 1 | accepted |
| Floats | 1 | 0.8 | accepted (0.13.1); no `Display` |
| `map` / `filter` / `collect` / `sum`, `sort_by`, `find`, `min_by_key` | 3 | 0.9 | accepted |
| String comparison, search, slicing, `split(char)`, `trim`, `parse` | 3 | 0.85 | accepted |
| `if let` on an enum's variant, `let`-`else` | 3 | 1 | accepted from 0.17.0 (0.6 before: a `match` with `_`) |
| `Option` / `Result` `and_then`, `unwrap_or_else`, `filter` | 3 | 1 | accepted from 0.18.0 (0.5 before: a `match`) |
| `fold`, `zip`, `flat_map`, `filter_map`, `windows`, `dedup`, `get`, `last`, `contains` on a `Vec` | 3 | 1 | `fold`, `zip`, `get`, `last`, `contains` accepted from 0.18.0 (0.4 before: a `for`); `flat_map`, `filter_map`, `flatten`, `windows`, `chunks`, `dedup` from 0.21.0 (0.75 before) |
| A tuple's `.0` | 2 | 1 | accepted from 0.17.0 (0.7 before: a tuple `let`) |
| `loop`, `while let` | 2 | 1 | accepted from 0.17.0 (0.5 before) |
| `a..=b`, `step_by`, `rev` | 2 | 1 | `a..=b` and `rev` on a range accepted from 0.17.0 (0.5 before), `rev` right after a sequence's source from 0.18.0; `step_by` on a range, and a range as any sequence's source (`(1..=n).sum()`), from 0.21.0 (0.85 before) |
| Slice patterns (`[x, ..]`) | 1 | 0.9 | accepted from 0.21.0 (0.5 before), `[]`, `[x, ..]`, `[first, rest @ .., last]`, exhaustive by length; a `match` whose elements leave a value out asks for a last `_`, also where ranges cover a whole type (`[0..=255]` on `&[u8]`) |
| `to_lowercase`, `split(&str)`, `lines`, `join`, `repeat` | 2 | 1 | accepted from 0.21.0 (0.4 before: `[expr/method-call]`), with `split_whitespace` and `to_string`; the literal `split("")` is refused |
| Numbers to text (`format!`, `to_string`) | 3 | 0.9 | accepted from 0.16.0 (0.2 before: digits built by hand; idsmith 82 `format!`, 37 `to_string`); `to_string` is `format!("{n}")`, floats stay out |
| Keyed collections (`HashMap`, `BTreeMap`, sets) | 3 | 0.6 | `BTreeMap<String, V>` / `BTreeSet<String>` accepted from 0.19.0 (0.2 before: a `Vec` of pairs searched in order); `HashMap` is written `BTreeMap`, another spelling; other key types stay `[type/disallowed]` |
| Generic types and functions (`Page<T>`) | 2 | 0.9 | accepted from 0.20.0, on the wire too (0.2 before: one copy per type; 16 functions and 21 types in the corpus outside idsmith); a `T` is not ordered |
| Fixed-size arrays and tables | 2 | 0.5 | `[type/array]`: a `Vec` or a `match` |
| Associated consts, calls through module paths | 2 | 1 | accepted from 0.21.0 (0.5 before: `[check/undefined-fn]`, `[expr/external-path]`); an associated const in a pattern takes a guard |
| Dates (on epoch integers) | 2 | 0.5 | arithmetic accepted; the calendar by hand (examples/calendar) |
| Recursion depth (game rules) | 1 | 0.6 | JS's stack |

**About 60% when measured (2026-10-13); 67% with 0.16.0's `format!`; 73% with 0.17.0's control flow; 78% with 0.18.0's std methods; 81% with 0.19.0's keyed collections; 83% with 0.20.0's type parameters; 90% with 0.21.0's notation** (the plan said 75%, 79%, 83%, and 86%: keys other than `String` wait for a use, and a `T` is not ordered). `survey corpus/patterns.rs` accepts 91 of its 99 functions with 0.21.0, 47 of 72 before it; the eight left are `HashMap`, a set of integers, an array, a trait method, `&mut self`, `&` on a `usize`, `{:.2}` on a float, and `partition`. Weighted, these rows gave 63% (62% unweighted; merging the first five rows into one gives 55%, so the figure is good to about a tenth). Counting the two shapes no document had decided on when this was measured (`panic!` / `assert!` / `unwrap`, refused as `[expr/macro]` and `[expr/method-call]` then, accepted from 0.18.0 but for `Result::unwrap` and `assert_eq!`, 0.9 now; traits with `Display`, after type parameters, 0.3; each weight 2) gives 61% as measured and 88% now; counting also what §6 leaves out on purpose (`&mut self` written as `self` → `Self`, 0.7; decimals as integers in the smallest unit, 0.5; serde's `default`, `skip*`, `flatten`, `untagged`, 0) gives 59%. Those are a lasting cost of writing within the constraints, not distance still to go.

**What moves it.** The whole plan in §3 is done: `format!` on integers, the control flow, the std on `Option` / `Result` / `Vec`, keyed collections, and type parameters, to 83% (86% planned); 0.21.0 took the largest notation left (text cut and built, the adaptors and ranges, associated consts and module paths, slice patterns), to 90%. What is left is mostly traits (§5), which `Display`, `PartialOrd` on a `T`, and the corpus's trait method wait for.

**Outside the language.** Equivalence, the wire, and real use are measured by [06 §4](./06-strategy.md#4-success-and-withdrawal-criteria): the internal criteria hold, two withdrawal conditions are not met, and real use is open with a deadline (2027-04-13).

## 3. Candidates

<!-- derived-from #2-evidence-from-examples -->

What the evidence currently points at, strongest first. The five examples of 2026-10-11 met §1 for every row struck through, taken together in 0.13.1 by decision. None of the rest is scheduled until §1 is met: an example that cannot be written, or stays over 2×, without it; the rows such an example raises come first. `split_once`, a `Vec` collected once from `s.split(c)`, and `Some((a, b))` were the rest of semver's parsing gap; they are in ([CHANGELOG 0.7.0](../CHANGELOG.md#rationale-lists-from-text)), with patterns nested in a case.

**Plan (2026-10-14, by decision).** §2.5 found the distance to the ideal in shapes common to domain code rather than in any one example, so the following are taken without waiting for §1, as 0.9.0 and 0.13.1 were, in this order. Each comes with a fixture, a shape in the statements generator, and `purecrate-ts test` over every example.

1. ~~**0.16.0, `format!`.**~~ Done ([01 §6.8](./01-equivalence.md#68-format)). `{}` on integers, `&str`, `String`, `char`, and `Uuid`, with a width (`{:5}`, `{:<5}`, `{:>5}`) and zero padding on integers (`{:05}`), whose text Rust and TS agree on; named and positional arguments, `{{` / `}}`. Out: floats (`Display` of a float is a large surface), `{:x}` and the other radixes (a negative signed value prints its two's complement in Rust), `{:?}`, and the crate's own types (no `Display`). `.to_string()` stays refused, `format!("{}", n)` its one spelling.
2. ~~**0.17.0, control flow.**~~ Done ([01 §7.16](./01-equivalence.md#716-control-flow-on-patterns-and-ranges)). `if let` and `let`-`else` on any pattern that can fail (a `let`-`else` on a variant prints as a guard), `loop` with `break` and a value, `while let`, `for` over `a..=b` and over `(a..b).rev()` / `(a..=b).rev()`, and a tuple's `.N`. Out: labels, a struct pattern in `if let` or `let`-`else`, `rev` on an iterator (with `zip` in 0.18.0). The statements generator's new forms found three miscompiles released through 0.16.0, fixed with it (CHANGELOG 0.17.0).
3. ~~**0.18.0, std on `Option`, `Result`, and `Vec`.**~~ Done ([01 §7.1](./01-equivalence.md#71-option-and-result-methods), [§7.17](./01-equivalence.md#717-panics), [§7.18](./01-equivalence.md#718-reading-a-vec-get-first-last-contains)). `and_then`, `unwrap_or_else`, `ok_or_else`, `or`, `or_else`, `filter`, `map_or`, `map_or_else`, `is_some_and`, `is_none_or` on `Option`, their counterparts on `Result`; `get`, `first`, `last`, `contains` on a `Vec`; `fold`, and `rev` and `zip` right after a sequence's source; `panic!`, `unreachable!`, `assert!`, and `Option::unwrap` / `expect`, each with Rust's message. Out: `Result::unwrap` / `expect` and `assert_eq!` / `assert_ne!`, whose messages print a value with `{:?}`; `rev` or `zip` after a `map` or `filter`; `step_by`, `flat_map`, `filter_map`. The statements generator's new forms found four places where flow narrowed otherwise than TS, each also in 0.17.0, fixed with it (CHANGELOG 0.18.0).
4. ~~**0.19.0, `BTreeMap<String, V>` and `BTreeSet<String>`**~~ Done ([01 §7.19](./01-equivalence.md#719-btreemapstring-v-and-btreesetstring), [04 §3.3](./04-wire.md#33-reading-rules)). A `Map` / `Set` read in key order by code point; `get`, `contains_key`, `len`, `is_empty`, `clone`, `iter` / `keys` / `values`, `for`, `collect`, `==`, and `insert` / `remove` on a local; a JSON object (a set an array) on the wire, every own key a key. Out: keys other than `String`, `HashMap` / `HashSet`, `entry`, `range`, `cmp`.
5. ~~**0.20.0, type parameters**~~ Done ([01 §7.20](./01-equivalence.md#720-type-parameters)). On structs, enums, inherent `impl` blocks, and functions, emitted as TS generics, the type arguments inferred at each call and construction; the bounds `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Debug`. On the wire, a generic type's schema is a function of its parameters' schemas. Out: ordering a `T` (no comparator for an unknown type), a turbofish, traits.
6. ~~**0.21.0, notation**~~ Done (2026-10-15, by decision after the plan above: the rows of §2.5 with the most weight left): [01 §7.21](./01-equivalence.md#721-cutting-and-building-text) `to_lowercase` / `to_uppercase`, `split` on a `&str`, `split_whitespace`, `lines`, `join`, `repeat`, `to_string`; [§7.22](./01-equivalence.md#722-iterator-adaptors-and-ranges) `filter_map`, `flat_map`, `flatten`, `windows`, `chunks`, `dedup`, `step_by`, a range as a sequence, an accumulator's literal typed by its first use; [§7.23](./01-equivalence.md#723-associated-consts-and-paths-through-modules) associated consts and paths through modules; [§7.24](./01-equivalence.md#724-slice-patterns) slice patterns.

| Candidate | Evidence | Note |
| --- | --- | --- |
| ~~`format!` on integers, strings, `char`, and `Uuid`~~ | semver (2.3×): the decimal digits of a `u64` built by hand; idsmith (82 `format!`); Windmill | done in 0.16.0: semver 2.3× → 2.0×, jsonpatch 4.1× → 4.0× |
| A local closure's parameter type inferred from its later calls | oidc needed `\|error: ErrorCode\|` (0.4.0 rewrites) | — |
| ~~`v.remove(i)` and `v[i] = x` on a local `let mut` `Vec`, as `push` and `insert` are~~ | jsonpatch: four copy loops in the patch layer (3.5×) | done in 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)) |
| ~~Float methods and conversions~~ | geo: four hand-written replacements (2.2×) | done in 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)) |
| ~~`u128` and byte-string literals~~ | ulid: 128 bits as two `u64`s; an alphabet as `match` arms | done in 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)), with `i128` |
| ~~`==` on the crate's enums and structs that derive `PartialEq`~~ | jsonpatch's `json_eq`; oidc and order wrote `eq` methods | done in 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)) |
| ~~String trimming and search, sorting, and picking an item~~ | negotiate: a byte-by-byte list scanner, insertion sorts (2.6×) | done in 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)); tuple sort keys and `Reverse` stay out |
| ~~Growing a `Vec` in a function body, and `map` / `filter` / `collect` over a `Vec`~~ | taken on 2026-10-04 without §1 being met: no example stayed over the threshold, but order's cons list, invoice's sums, and the cost of growing lists only as recursive enums (O(n) access, recursion depth, TS callers who expect arrays) were judged enough. A local `let mut v: Vec<T>` is pushed to, every other array stays unwritten (02 §3.1) | done in 0.9.0 |
| ~~Paths through modules (`crate::m::f`, `super::T`)~~ | — | done in 0.21.0: resolution only, as names are unique after flattening |
| ~~Narrowing `as` between integers~~ | ulid: two 32-arm alphabet `match`es and bytes built bit by bit (3.4×); geo's 6-bit chunk to a `u8` | done in 0.13.1 ([CHANGELOG 0.13.1](../CHANGELOG.md#rationale-what-the-five-examples-asked-for)); a widening `From` takes keeps `T::from(x)` as its one spelling |
| Type parameters and string-keyed maps | §2.5 | 0.19.0 and 0.20.0 in the plan above; §5 |
| ~~Associated consts (`impl T { const N: u32 = 3; }`), as members of the type's companion~~ | specified with local `const` in 0.4.0 | done in 0.21.0 |
| crates.io and Windows binaries | — | when a user asks; since 0.3.0 the dependencies are crates.io requirements, vendored by source replacement |

**1.0** needs every withdrawal criterion of [06 §4](./06-strategy.md#4-success-and-withdrawal-criteria) answered. The generated API's compatibility within a minor series is [§9](#9-generated-api-stability). The real-use criterion stays open until the tool is adopted unprompted; Oxide `Name` remains local evidence ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)).

## 4. Specified but not yet implemented

`chars()` other than as `for c in s.chars()` or before a consumer; the Unicode-table `char` methods; `isize`; labelled `break`/`continue`; the adaptors `take`, `skip`, `take_while`, `skip_while`, `peekable`, and `partition`; the std allow-list beyond what [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods) lists; `static`.

## 5. v1: when type expressiveness runs out

Planned by decision (2026-10-14, §3), no longer waiting for an example.

- ~~**`BTreeMap<String, V>` and `BTreeSet<String>`**~~ (done in 0.19.0, §3) as `ReadonlyMap<string, V>` / `ReadonlySet<string>`, iterated in Rust's key order (bytes of UTF-8, which is code-point order: `Ord.cmpStr`, not JS's UTF-16 sort). `new`, `get`, `contains_key`, `len`, `is_empty`, iteration, and `insert` / `remove` on a local `let mut`, as a `Vec`'s; on the wire a JSON object. `HashMap` and `HashSet` stay refused: their iteration order is unspecified in Rust itself, so an order-dependent function has no one result to match. Keys of other types wait for a use.
- ~~**Type parameters**~~ (done in 0.20.0, §3) on structs, enums, and functions, emitted as TS generics (no monomorphization), with the bounds `Clone`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Debug`, which the structural output already honors. No other bounds, `where` beyond these, or associated types. Wire schemas for a generic type are functions of the parameter's schema.
- **Traits** wait until type parameters are in: a user trait is decided then, with dispatch through those parameters.

## 6. Not doing

- Allow-lists aimed at passing existing crates.
- Mutating a `Vec` other than a local's `push`, `insert`, `remove`, `v[i] = x`, and stable `sort*`: through a field, an element's array, a parameter, or `pop` / `truncate` / `extend` / `sort_unstable`.
- Decimals; event logs inside state.
- A schema-library dependency in the core runtime.
- serde's `untagged`, `flatten`, `default`, `skip*`, `alias`, `deny_unknown_fields`, and `transparent`: refused with their location; the reasons are in [04 §4](./04-wire.md#4-out-of-scope).
- WASM. The IR does not preclude a second backend, but the path is TS source.
- `Rc` / `Cell` / `RefCell` / `Mutex`.
- `&mut`, as `&mut self` returning the new value or into a nested value (jsonpatch's `edit_at`; the aggregate shape in 5 corpus entries): each form makes the TS signature or the write path differ from the Rust one, a new place for wrong values; a local `Vec`'s edits above are as far as it goes (decided 2026-10-13).
- `async`, randomness, and other effects (idsmith's `&mut` RNG parameters, 305 of the corpus's functions).

## 7. Open questions

- Should output typing come from rustc's type information instead of the in-house inference ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate))?
- Hermes support for `JSON.parse` source text ([04 §7](./04-wire.md#7-open-questions)).
- A shared error type with field paths for validation ([01 §4](./01-equivalence.md#4-closed-types)).
- Demand: see [06 §5](./06-strategy.md#5-validating-demand-next).

## 8. Releases

<!-- derived-from #2-evidence-from-examples -->
<!-- constrained-by #1-how-additions-are-chosen -->

Per release, why, what, and the test that verifies each item are in [CHANGELOG.md](../CHANGELOG.md), under each version's "Rationale" heading, beside the user-facing list. Decided with 0.1.0 ([06 §5.2](./06-strategy.md#52-one-real-use)):

- Later releases take language capabilities first, in the order the examples' measured gaps give.
- Adoption is left to happen on its own, with no outreach and no requests to adopt. One exception was added on 2026-10-13: proposing the Oxide `Name` change upstream, with a deadline of 2027-04-13 for the real-use criterion ([06 §5.2](./06-strategy.md#52-one-real-use)).
- Distribution work waits for someone to need it.

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
