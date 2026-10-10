# Changelog

## Unreleased

### Fixed

- A `let` of an enum-typed place was cast back to its type even where nothing had narrowed it (a parameter, a `?`'s value), which oxlint refuses as unnecessary; only a place a `match` tests, a name its arm binds, or a copy of one is cast now.
- A `while (..) {` whose test does not fit opened as oxfmt does not; it opens as an `if`'s.
- `[expr/method-call]` on a `Vec` lists `insert` beside `push`.
- `-x` on an `f64` printed `-` on a brand, which oxlint refuses; a value is multiplied by `-1`, which flips the sign exactly, and a literal is bare (`-90.0 as F64`).
- A companion `of` whose head fits up to `=> ({` opened its parameters where oxfmt opens a short object body.
- An opened `if (` / `while (` test broke only where it did not fit; oxfmt breaks it at every top-level `||` (else `&&`), and a comparison that still does not fit after its operator. A range `for` whose declarators do not fit puts the second under the first.
- `xs[i as usize]` with `i: u32` printed a cast `Slice.at` does not need, which oxlint refuses.
- `[expr/cast]` says what `as` allows (a discriminant, `u8`/`u16`/`u32` to `usize`) and that no float converts to or from an integer, instead of advising `T::from(x)`; `f64::NAN` and other std paths are no longer called undefined functions.

### Documentation

- The authoring skill caught up with 0.13.0: `Vec::insert`, `x as usize` from `u8` / `u16` / `u32`, and `for _ in`; `Vec::remove` named as refused.
- The authoring skill says what floats take and that none converts to or from an integer.

### Examples

- **jsonpatch**: RFC 8259 text, RFC 6901 JSON Pointer, and RFC 6902 JSON Patch, written from the authoring skill alone; checked against RFC 6902 Appendix A, RFC 6901 §5, and an idiomatic reference over serde_json (design/07 §2).
- **geo**: Google's Encoded Polyline and Geohash on `f64`, written from the authoring skill alone; with no float rounding or conversion in the subset, it rounds exactly by subtracting powers of two.
- **ulid**: the ULID specification (base32, the 16-byte form, monotonic generation), written from the authoring skill alone; no hole, at 3.4× its idiomatic reference for want of narrowing and `u128`.
- **negotiate**: RFC 9110 §12 proactive negotiation, written from the authoring skill alone; its test cites RFC 9110 Erratum 7138.
- **raft**: the Raft paper's Figure 2 as a pure `step`, written from the authoring skill alone, with a simulation checking the four safety properties.

## 0.13.0 — 2026-10-06

Every example was reviewed independently twice, each reviewer driving the generated package and the Rust model with its own scenarios from the primary specifications (RFCs, the NTA Q&A, Stripe's documentation, SemVer, the SWIFT registry, WHATWG and NIST). The domain logic agreed byte for byte on every scenario both times (millions in all). What did not hold was at the edges of the generated package: the JSON wire, the TS type boundary, and the layout oxfmt and oxlint ask for. Those are fixed, as is every departure of the examples' models from their specifications, or it is declared. `Vec::insert` and `as usize` join the subset, for punycode. Several changes are breaking for code that relied on the old behaviour: see Changed.

### Added

- **`v.insert(i, x)`** on a local `let mut v: Vec<T>`, as `push` is: `Slice.insert` panics as Rust does past the end, then `splice`s (design/01 §7.14). Taken off "Not doing" (design/07 §6) by decision, for punycode, whose decoding of 16,000 CJK characters went from 337 ms to 17 ms.
- **`x as usize`** from a `u8`, `u16`, or `u32`, which holds every value on every target; std has no `usize::from(u32)`. Every other `as` stays refused.
- **`for _ in ..`** over a range or a `Vec`.

### Changed

- **The wire reads JSON as serde_json does** (checked against serde_json 1.0.145 on each case): an integer refuses a digit string, a float written as `50.0` / `5e1` / `-0`, and a number past 2^53 read without `parseJson`; a struct is an object or its sequence form, an array of exactly its fields (`[3,null]`), never an array of another length (valibot and arktype had read every field as missing); a field twice, or a key with a lone surrogate, is refused where serde reads the object, and read past where it ignores it; a string with a lone surrogate is refused. `parseJson` no longer throws on a duplicate key: it notes it for the schema that reads the object.
- **A closed type cannot be built outside the package without a cast.** A newtype's brand is a `unique symbol` its file does not export; a struct's is a class with a `private` member, which an object spread does not copy (`{ ...line, qty: 0 }` had been a `Line`). Two copies of one package no longer exchange closed values.
- **A `pub` function panics on a lone surrogate** anywhere in an argument that may hold a string (a field of an open type, a variant, an `Option`, a `Vec`, a tuple), which no Rust `str` holds; an object checked once is not walked again.
- **design/01 §2, §4.3, 03 §5.6**: mutation in place is outside the guarantee with a cast or without one (`Object.assign` needs none), and values are shared, not copied.

### Fixed

What tsc, oxlint, or oxfmt refused in generated code, or `check` refused where rustc accepts, found by the fixes; each is in `fixtures/review_holes.rs`:

- `enumerate` over enum elements cast each one; the loop now iterates the element itself.
- `a && x.cmp(y).is_eq()` and other operands that need statements printed inline functions oxfmt lays out otherwise: an `Ordering` test reads `call.kind` once, and such an operand is an `if` where a statement stands (not in a test, which keeps TS's narrowing).
- `s.push(match ..)` printed `s = s + ..`; a long logical `return`, `} else if (..)`, and assignment broke inside a call; an array of number literals broke one per line; a byte literal's comment sat inside its cast's parentheses; a returned `?:` from a `let` was not parenthesized.
- `for x in v.clone()` printed a spread oxlint refuses (the copy is named when the body grows `v`); `for _high` printed a name oxlint refuses (a local drops its leading `_`; an unread `for..of` variable is `_`).
- A guarded arm that returns printed an `else` oxlint refuses; a field read by a literal pattern hid an outer local (`no-shadow`).
- `let x = if c { 59 } else { second };` could not type `59`; `let (a, b) = match x { .., None => return 0 };` was refused.
- A statement `match` of one variant and `_ => {}` printed an empty `else`.

### Examples

- **ssh**, after two reviews: the next packet is dropped after a wrong guess; a later EXT_INFO replaces an earlier one, and it is accepted only where RFC 8308 §2.4 puts it; strict KEX follows draft-ietf-sshm-strict-kex-02 (both client names offered, a matching pair needed, no non-KEX message until both NEWKEYS); PK_OK must echo the key (`Identity { key_type, public_key }`); RSA tries each signature algorithm; empty name-lists and packets under 16 bytes are refused; known_hosts matching handles negation, `*`/`?` wildcards, and hashed names (HMAC-SHA1 written in the example), and fails closed on a line it cannot read.
- **oidc**: an OTP accepted in one flow is recorded and refused in others, and the lockout holds across flows; `max_age=0` re-authenticates; the PKCE challenge is computed from the verifier (SHA-256 and base64url written in the example; `check_redemption` takes only the verifier); error redirects echo `state`; password failures count per subject; unknown `prompt` values are ignored; an S256 challenge must be 43 characters; `Authentication` is closed.
- **invoice**: tax is rounded once per rate on converted totals (問57, 問59), mixed bases are refused, overflow is an error, and `Summary` reports the tax.
- **payment**: an intent read from JSON goes through a checked constructor (`UncheckedIntent`); a bank debit under manual capture is refused.
- **order**: the types are closed, `step` takes `&Order` so a refusal keeps it, and a cancel's reason is tied to the status.
- **calendar**: UNTIL may end at second 60; a YEARLY BYDAY ordinal counts within the year without BYMONTH (erratum 3779); a leap second is taken at the end of any month.
- **punycode**: uses `Vec::insert`; `xn--é` is refused both ways; the dead round-trip check is removed.
- **semver** formats a version and compares two; **iban** counts characters and reads its value back; **signup** refuses the e-mail as the password and reads its values back; **counter** states its rules. Every header now states what the model leaves out, and why.

## 0.12.0 — 2026-10-05

Building a `String`, and `examples/punycode`, written from the authoring skill alone, which found a `while` that ran forever in TS where Rust returned: a loop on a flag its body sets. That is fixed, and the statements generator now writes such loops; the narrowing they reached is closed too. Programs that passed 0.11.0 translate as before, but for that loop; the other examples' output is byte for byte the same.

### Fixed

- **A `while` on a flag its body sets ran forever in TS.** `let mut done = false; while !done { ..; done = c; }` had its test decided by the flag's value before the loop, as an `if` is, so the body's write was dropped and the loop never ended (Rust returned). A `while` test is now decided at the loop's head, where the body's writes join. The statements generator now writes such loops (a flag the body sets last), which it never did; seeds 1 to 120 of all five generators pass.
- **oxfmt rewrote a signature** whose `=>` fits the width but whose `=> {` does not: the parameters open, not the return type.
- **What tsc refused in generated code the new loops reached** (values agreed; seeds 7, 12, 70, 92, and 112 of the statements generator once it writes flags). Narrowing now follows TS in four more places: an `if` used as a test that prints as `c && a` or `!c || a` (one that needs statements) narrows as those; a binding set from a `bool`, `Option`, or `Result` place known to hold one case holds it too; `b === c` and `b !== c` narrow a `bool` against a literal or a place of one known value; and a value tested against a literal (`x !== 7`) is no longer taken to contradict its type's case (`Some`), which had made the other side look never taken.

### Added

- **Building a `String`**: `String::new()`, `push(c)` and `push_str(t)` on a local `let mut`, and `collect::<String>()` over `char`s (design/01 §7.15). A push is the assignment `s = s + c` to every pass, and prints as `s += c`.
- **`examples/punycode`**, written from the authoring skill alone: RFC 3492 Punycode with §6.4 overflow, and a simplified IDNA layer. It found the `while` hole above. As written its results were `Vec<char>`, since the subset could not build a `String` (design/07 §2.1), and its logic was 2.0× the idiomatic reference's; with `String` building (above) they are `String`s, at 1.9×.

## 0.11.0 — 2026-10-05

`examples/calendar`, written from the authoring skill alone, and `str::eq_ignore_ascii_case`, added for its rule parsing; the holes the example found in lint and layout are closed. Programs that passed 0.10.5 translate as before; the other examples' output is byte for byte the same.

### Added

- **`examples/calendar`**, written from the authoring skill alone: RFC 3339 timestamps (leap seconds by §5.7, "-00:00"), Hinnant's civil-date algorithms over years -9999 to 9999, ISO 8601 week dates, and RFC 5545 RRULE parsing and expansion (DAILY to YEARLY, INTERVAL, COUNT, UNTIL, BYDAY with ordinals, BYMONTHDAY, BYMONTH, WKST). Its test reproduces RFC 3339 §5.8 and RFC 5545 §3.8.5.3's examples, agrees with an idiomatic-Rust reference on every day count in range, and compares the package with Rust. Its logic was 1.8× the idiomatic reference's as written, rule parsing 2.0×; 1.7× and 1.6× with `eq_ignore_ascii_case` (below; design/07 §2.2).

- **`str::eq_ignore_ascii_case`** with a `&str`, as `Str.eqIgnoreAsciiCase`: ASCII letters fold, nothing else does, as in Rust. calendar's rule parsing uses it with name tables: 274 lines to 226 (2.0× to 1.6× the idiomatic reference).

### Fixed

- **oxlint refused an `else` after `return`** where a `match` ending a loop's body leaves in one arm (`None => return e`); the exit is printed first, as elsewhere.
- **oxlint refused the cast in `BigInt(o ?? (1 as U32))`** (`i64::from(o.unwrap_or(1))`).
- **oxfmt rewrote** a `while` test that calls a function in place, a `?:` branch too long for its line, and a parenthesized `||` or `?:` group.

### Changed

- The authoring skill: `for` ranges are half-open only; a range of two bare literals needs a suffix; integers never narrow; a method may share a type's file name.

## 0.10.5 — 2026-10-05

`examples/ssh`, and three holes it found in accepted code (a `case` tsc refused, an `as` oxlint refused, long logical lines oxfmt rewrote). Nothing on the stable surface changes; the other examples' output is byte for byte the same.

### Added

- **`examples/ssh`**: an SSH client from the server's identification line to the end of user authentication (RFC 4253 version exchange, §7.1 negotiation and guessed packets, OpenSSH's strict key exchange, known_hosts, RFC 4252 publickey and password, RFC 8308 server-sig-algs, re-exchange), with packet framing (§6) and the keys to derive (§7.2). Its test asserts each rule as the specifications state it, and compares the package with Rust on those runs and on every two of 18 events after each point of the handshake.

### Fixed

Three holes in accepted code that the example found:

- **`tsc` refused a `case` narrowing had ruled out** (TS2678): a later `match` on a place a guard or a failed `matches!` had narrowed kept the excluded variant among the cases its `_` names. Narrowing now knows what an enum's `_` holds, a test may leave several cases, and an arm loses the variants the place cannot hold.
- **oxlint refused `as T` on a tuple element** read right after `let (a, b) = f()?`. It is `const [a, b] = result.value;`, as without `?`; a tuple from a call is taken apart without an annotation.
- **oxfmt rewrote long logical initializers and arguments**: `const x = a && b && c;` now breaks after `=`, one operand a line, and a chain as an argument puts its later operands one indent in.

### Changed

- A two-arm boolean `match` whose first arm is the other variants is tested as the one: `o.kind === "Equal"`, not `!(o.kind === "Less" || o.kind === "Greater")`. No example's output changes.
- The authoring skill and 02-authoring: `if let` takes `Some(x)` only; a tuple `let` bound to a `match` takes no `?` or `return` in its arms; an `if` whose one side is a bare literal needs its binding typed.

## 0.10.4 — 2026-10-05

A refactor, and the generated sweeps run locally. Nothing on the stable surface changes; the examples' output is byte for byte the same.

### Added

- **`scripts/gen-sweep.sh`** runs the five generated equivalence tests over seeds 1 to 120 (or `-s`), one process per seed, in parallel, and reports per generator the seeds that agree and type-check, agree but `tsc` refuses, or fail, with the command that reruns each. Run locally, not in CI (README, Generated sweeps).
- **`scripts/gen-reduce.py`** reduces a failing seed to a small crate: the first function `tsc` refuses or `build` crashes on, cut out and shrunk while the same refusal remains; a value mismatch's function cut out as it stands, for a fixture.

### Changed

- **Refactor, output unchanged.** The walks each pass wrote by hand go through `Expr::search` / `any` / `walk`, `Ty::children`, and `Pattern::children` in `ir`; the large files are split by concern (`emit_ts`'s `stmt`, `join`, `tidy`, and `lib`; `check`'s `types/methods`). `scripts/output-snapshot.sh` gives the same digest as before, and `scripts/gen-sweep.sh` passes seeds 1 to 120 of all five generators.
- **Tests.** `support::equivalence` and `grid!` replace the frame and the nested `for` loops of the fixture tests; the generated tests share `support::generated::compare_rows`; the package writing six tests copied is `support::write_package`. `golden.rs` and a second `Vec` panic test, which other tests covered, are gone.

## 0.10.3 — 2026-10-04

The rest of what tsc refused in generated code ([roadmap §8.17](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#817-0103-the-rest-of-what-tsc-refused-2026-10-04)): every generated function of seeds 1 to 120, in all five generators, now type-checks. Nothing on the stable surface changes; the examples' output is byte for byte the same.

### Fixed

- **`null ?? a`** where a `match` on `{ let t = None; t }` or on `Ok(r.value)` was left to run; it takes its arm.
- **Unreachable code** past a test decided in a `matches!` whose scrutinee is a call (`matches!(x.checked_mul(b), Some(_) if false)`), and past `v = x?` that always leaves.
- **A temporary TS typed from itself in a loop** (`const value = opt !== null ? 3 : null` read by its own loop's test): `o.map(f).is_some()` is folded into the `match` it tests.
- Narrowing follows what those print: `const v = r.kind === "Ok"` narrows `r` through `if (v)`, as TS does; a `match` used as a test narrows on both sides; code the fold thinks unreachable is still folded, as TS still checks it.
- A file whose folds print a `switch` imports `assertNever`.

## 0.10.2 — 2026-10-04

Narrowing as TS's control flow does it ([roadmap §8.16](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#816-0102-narrowing-as-ts-does-it-2026-10-04)). Nothing on the stable surface changes; the examples' output is byte for byte the same.

### Fixed

- **Output tsc refused where TS had narrowed a value and the printer had not**, none of it a wrong value: past an `if` / `else` whose sides each leave on `Err`, inside and past a loop, past `if c { return }`, inside `if matches!(o, Some(_) if g)`, `if c || c`, or `if t == ","`, after `let v = true`. The printer now follows each place's possible cases as TS does, joining them where control flow meets and finding a loop's head by running its body until they settle. Generated bodies that type-check, seeds 1 to 120: 60 -> 103; transitions and text: all.
- A test decided though part of it must run (`100 / a > 0 && c`, `c` known `false`) runs that part, then the side taken.
- An `if` run for one side's effect prints as a statement, not `c ? undefined : f()`.

## 0.10.1 — 2026-10-04

Tests generated across the subset, and what they found ([roadmap §8.15](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#815-0101-the-generator-across-the-subset-2026-10-04)). Nothing on the stable surface changes; the examples' output is byte for byte the same.

### Fixed

- **`match o.ok_or(x.ok_or(e)?)` returned the wrong value.** The inner `?` sat in an inline function, so its `return` left only that function and the `Err` arm ran: Rust gave `Err(e)`, TS `Ok(..)`. It now runs before the `match`, as Rust evaluates the argument first. Also in an `if` or `while` test, a range's end, and an operand.
- **`if let Some(0) = o { .. } else { .. }` threw for `Some(5)`.** The `else` stood for `None` only, so a `Some` the payload did not match reached no arm ("unexpected variant"); where the payload may fail to match, the `else` takes every other value, as in Rust. Likewise `Ok(..)` and `Err(..)` with a literal or range inside.
- **`check` and `build` overflowed their stack** on `E::V(Some(_)) if g => .., E::V(v) => ..`: the printer followed the `let v = v` the decision tree binds as an alias, forever.
- **A long `=> (a ? b : c)` broke to `(..,)`**, a syntax error; a group takes no trailing comma.
- **Two temporaries named after one local** (`let x = r.ok()`, then `x.ok_or(e)` matched) were both `xResult` in one block.
- **Output tsc refused under the generated tsconfig**, none of it a wrong value: an `unwrap_or` default left `const optOr` and an empty `if {} else {}` where the value was unread; a test folding decided left unreachable code (`while (false)`, code after a `return`); `r?` on a known `Err`, a place past a `match` whose other arms `break`, and a `bool` an enclosing `if` decided were compared as TS knew they could not be; a parameter read only by an unread `t.as_str()` kept its name.
- A decided side that declares a name keeps a block of its own (`{ .. }`), so a later `let` of the same name is not a redeclaration.
- **Output lint refused**, in the shapes these folds leave: an unread comparison printed as a statement, a block around a value that declares nothing, `Result` imported as a value where only its type is read.

### Added

- Four generated tests beside the expressions of 0.10.0: function bodies (`generated_statements_equivalence.rs`), transitions over a crate's types with nested patterns and guards (`generated_patterns_equivalence.rs`), integer widths (`generated_widths_equivalence.rs`), and strings and `Vec` (`generated_text_equivalence.rs`). Seeds 1 to 120 of each agree on every value; how many type-check is in the roadmap.

## 0.10.0 — 2026-10-04

Tests generated from seeds, and what they found; parentheses decided on a tree of the printed operators; rustfmt and clippy in CI. A minor release: the runtime's `Result.ok` and `Result.err` signatures change, which the stable surface lists ([roadmap §8.14](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#814-0100-tests-generated-from-seeds-and-what-they-asked-for-2026-10-04)). Regenerate committed output: each copied runtime changes by two lines.

### Changed

- **`Result.ok` and `Result.err` default the type they cannot infer to `never`** (`ok: <T, E = never>`, `err: <T = never, E = never>`), instead of `unknown`. `c ? Result.ok(a) : r` and `r.kind === "Ok" ? Result.ok(0) : Result.err(r.error)` are the `Result` they read as; before, TS took `Result<I32, unknown>` and refused the code. Every copied runtime changes by these two lines.
- **Parentheses come from a tree of the operators printed, not from reading the text back.** The printer builds each operator, comparison, `!`, `?:`, and `??` it writes as a node (`emit_ts::tx`) and parenthesizes by precedence there; negation turns a comparison or an `&&` chain round on the tree. The text reader it replaces missed operators a call or a `match` printed (0.9.1's `r.ok().is_some()`) and parenthesized an inline function called beside an operator (`((() => { .. })()) >= a`), which is now `(() => { .. })() >= a`. The examples' output is unchanged.
- **What TS has narrowed is folded instead of printed.** `r.ok()` inside `r`'s own `Err` arm is `null`, a second `r.map_err(f)?` after a first never returns, `match` on `Some(a)` or on a `let` of one takes its arm, `1 == 65535` is `false`; TS refused each as a comparison with no overlap, `null ?? d`, or unreachable code, and the bindings and `let`s it left unread as unused. A place written in an arm or after (`o = None`) is not folded (`fixtures/narrowing.rs`).

- **`let x = f(..).ok_or(e)?` holds the option in `x`.** `const qty = Int.u32.checkedAdd(..); if (qty === null) return ..;` instead of a `qtyOpt` copied into `qty`. A `let mut`, a made local, and a receiver that is already a place keep the old form.
- **A hexadecimal literal stays hexadecimal**, in whole bytes: `0x0f as U8`, `b >= 0x20 && b <= 0x7e`.

### Fixed

Found by a new test that generates functions from seeds (`generated_equivalence.rs`) instead of spelling them; each was refused by `tsc`, none returned a wrong value. Seeds 1 to 120 now pass, eight on every run.

- **A `?` in what `ok_or` takes leaves the function.** `let x = o.ok_or(a / r?)?` printed `r?` (or `r.map_err(f)?`) in an inline function whose `return` left only itself; it now runs before the test, as Rust evaluates the argument whether or not `o` is `Some`.
- **An `unwrap_or` inside what `ok_or(e)?` takes keeps its option apart.** Both were `const opt` in one block.
- **`match o.ok_or(e) { .. }` names the error type.** `Result.ok(o)` alone left TS with `Result<I32, unknown>`; so did a block `{ let t: Result<A, B> = Err(b); t }` as a scrutinee, whose annotation was dropped.
- **A parameter read only in a branch that a constant test folds away** (`if false { a } else { 1 }`) prints as `_a`, as an unread one does.
- **A long `return (a !== b) !== (..)` breaks without a trailing comma.** The layout took any `(` after a word for a call's arguments, `return (` and `typeof (` included; `(x,)` is a syntax error, and with two items the comma operator.
- `[check/position]` names a `{ .. }` block too, which it refuses beside `&&`, `||`, `if`, and `match`.
- **A `let` Rust leaves unused keeps only what may panic.** `let a = r.ok().unwrap_or(0);` unread printed `const opt = ..` and `if (..) {} else {}`; reads and values built of reads are left out.

## 0.9.1 — 2026-10-04

Fixes from an audit of 0.9.0: four outputs that disagreed with Rust, two shapes oxlint refused, and documents that lagged the subset. No change to what is accepted, but `ok()` of a `Result` holding an `Option`, which is now refused ([roadmap §8.13](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#813-091-fixes-from-an-audit-of-090-2026-10-04)).

### Fixed

- **A choice under `is_some`, `===`, or a member is parenthesized.** `r.ok().is_some()` printed `r.kind === "Ok" ? r.value : null !== null ? ..`, which JS reads with the comparison inside the else branch: `Ok(0)` gave 0 where Rust gives 1. Likewise `o.map(f).is_none()`, `!o.map(f).is_some()`, `o.is_some() == p.map(f).is_some()`, and `o.unwrap_or(vec![]).len()`.
- **`ok()` of a `Result<Option<T>, E>` is refused** (`[check/nested-option]`): both `Ok(None)` and `Err(_)` were `null`.
- **A struct pattern's guard leaves names its own bindings hide.** `Q { a, xs } if xs.iter().any(|a| *a > 3)` tested `q.a > 3` inside the closure.
- **A `//` comment stays one line in JS.** A lone CR, U+2028, or U+2029 in a carried comment ended the line for JS, and the rest ran as code; each is a space now.
- **`else if` for an `else` that is one `if`** (oxlint's `no-lonely-if`), **and no `c ? true : false`** (`no-unneeded-ternary`). An equality inside an equality is parenthesized, as oxfmt prints it.
- **Documents:** the skill and 02's tables no longer call `cmp` on `Vec`s, `map` / `filter`, or chained `chars()` unavailable; 01, the overview, and 03's counter sample match the output; a refused method's message lists the 0.9.0 methods.

## 0.9.0 — 2026-10-04

Lists built in a function, iterator stages, struct patterns, and `clone`; the examples checked against their specifications, with the generator bugs that rewriting them found. The accepted subset grows ([roadmap §8.12](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#812-090-lists-built-in-a-function-and-the-examples-against-their-specifications-2026-10-04)).

### Changed

- **A helper with one caller's file is printed in that file.** A function the package does not export, called only from one other file, is a `const` of that file, unexported, instead of a file of its own that the caller imports: `isDigit` and `isUpper` are in `iban.ts`, `parseSeconds` and `parsePrompt` in `validate-request.ts`. A helper of a helper follows it. This alone took the examples' plain outputs from 143 files to 113 (oidc 51 → 43, semver 16 → 8). Declarations that share a file are a blank line apart.
- **Examples follow their specifications more closely.** iban refuses check digits 00, 01, and 99, which MOD 97 cannot tell from 97, 98, and 02. oidc keeps the OTP failure count in the stored enrollment, so a new flow does not reset it (RFC 4226 §7.3), names the subject in `Locked`, does not count a clock before T0 against the End-User, and issues the code without the consent screen when consent is on file. order returns `Overflow` instead of panicking on a large quantity or total, and refuses a SKU added again at another price. semver accepts a numeric pre-release identifier of any length (`1.0.0-18446744073709551616`), as the grammar does, and orders it exactly; `PreReleaseTooLarge` is gone. Each example's header names what it leaves out (payment: `automatic_async`, fees at creation, currencies; invoice: the registration number). signup and order are shorter (`split_once`, `chars().count()`, `map_err(..)?`; one `match (order, cmd)`). Since then they use the new forms: order's lines are a `Vec<Line>` grown by `push` instead of a cons list, oidc copies with `clone()` instead of a `match`, invoice sums a group with `iter().map(..).sum()`, semver collects into `Result<Vec<_>, _>`, and payment's bank-debit cancel is a struct pattern.
- **Fewer machine-made names and inline functions.** `let sum = a.checked_add(b).ok_or(e)?` holds the option in `sumOpt`, not `opt`; `let email = Email::parse(s).map_err(f)?` holds the result in `emailResult`, not `result`. `matches!(&s[0..2], "00" | "01")` beside `||` is `["00", "01"].includes(Str.slice(s, 0, 2))` instead of an inline function.
- **No block around a value whose `?` sits in what `unwrap_or` takes.** `let s = n.checked_add(f(n)?).unwrap_or(0)` printed `let s; { if (..) return ..; s = .. }`; the `?` now leaves before the statement, and `s` is a `const`.
- **`_` in the target of `collect`.** `s.split(c).map(f).collect::<Result<Vec<_>, _>>()` takes `T` and `E` from `f`'s `Result<T, E>`; `collect::<Vec<_>>()` keeps what `f` returns, a `Result` included, as Rust does.
- **Struct patterns in `match` arms and `matches!`.** `Status::Processing { method: PaymentMethod { kind: MethodKind::BankDebit, .. } }` tests `kind` before the arm's guard and binds the named fields at the head of its body. A field pattern binds a whole field or nothing. payment uses it; its TS is unchanged.
- **`cmp` on two `Vec`s** of integers, `char`, `bool`, strings, or `Uuid`: element by element, then the shorter first, as std orders slices (`Ord.cmpList(a, b, Ord.cmp)`).
- **`map` and `filter` over a `Vec`, a string's chars or bytes, or a split**, with `copied()` and `cloned()`, before `collect`, `sum`, `count`, `all`, `any`, or `position`. The stages are lazy (`Iter.map`, `Iter.filter`), so each item runs every stage before the next and the first panic is Rust's; one stage before `collect` is the array's own method. A call whose argument is a call taking an arrow opens every argument, as oxfmt does.
- **A local `Vec` grows by `push`.** `let mut v: Vec<T> = Vec::new(); v.push(x);` prints as `const v: Array<T> = []; v.push(x);`. Only a `let mut` local is pushed to; a value that is not a new array is copied when bound (`[...xs]`), so the arrays a caller holds are never written (every other array stays `ReadonlyArray`, so `tsc` refuses a push to one as well). A push through a field or an element is refused. This reverses a line of the roadmap's "Not doing" (07 §3, §6).
- **`clone`, `as_ref`, and `as_deref`.** `xs.clone()` on a `Vec` is `[...xs]`; on anything else, and `as_ref()` / `as_deref()` on an `Option`, the value itself.
- **More `//` comments carried.** Above a `match` arm (printed in its `case`), across a blank line above a statement, after the code on a statement's line (moved above it), before a block's `}`, and directly above a crate `const` (in `consts.ts`).
- **A local is renamed apart from what its whole file imports.** With helpers sharing their caller's file, a parameter `total` beside a function that calls `total` would hide the import, so it is `total2`; a local never takes the name of a function the package does not export.

### Fixed

- **A `?` inside what `ok_or(e)?` takes leaves the function.** `x.checked_add(g()?).ok_or(e)?` printed `g()?` inside an inline function, whose `return` left only itself.
- **Two `ok_or(e)?` or `map_err(f)?` in one body keep their values apart.** Each declared `const opt` (or `result`) in the same block.
- **A last-argument arrow whose body is not a call, `?:`, or `as` opens every argument**, as oxfmt prints it, instead of keeping its head on the call's line.

## 0.8.2 — 2026-10-04

The rest of the audit's fixes, output that reads more as written by hand, and a layout that matches oxfmt for long names outside the examples. No change to what is accepted ([roadmap §8.11](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#811-082-the-rest-of-the-audit-and-layout-2026-10-04)).

### Changed

- **Same-named fields are destructured.** `const request = flow.request;` `const failures = flow.failures;` is `const { request, failures } = flow;`. A local whose name differs from its field (`secondFactor` for `second_factor`) stays a line of its own.
- **A negated test is flipped.** `if !apart { a } else { b }` as a value is `apart ? b : a`, unless `b` is itself a choice.
- **An arm that returns the variant it matched returns the place.** `Ordering::Less => Ordering::Less, Equal => .., Greater => Ordering::Greater` is `if (ord.kind !== "Equal") return ord;` and the `Equal` arm after it: arms of unit variants that do the same share one arm.
- **Layout closer to oxfmt outside the examples.** A test fixture of long names (`fixtures/layout.rs`) makes each construct break, and `scripts/verify.sh` checks its output with oxfmt like every fixture's. What it found is fixed: width counts a wide East Asian character as two columns (a line of Japanese text no longer breaks early); an `import` of one name stays on its line; `const x = f(a);` that does not fit breaks after `=`; an arrow's object body opens at `({` with one field too; a test inside a chain of `?:` goes on four columns in; a last argument that is an arrow keeps its head on the call's line; the wire module's `v.GenericSchema<..>` breaks its arguments and keeps the value after `> =`.
- **No inline function for a block in an argument.** A `match` or block with statements as an argument, element, or operand runs before the statement, bound to a name (`let value; if (..) { .. } else { .. }` then `f({ .., b: value })`), instead of `(() => { .. })()` printed inside the call.
- **A variant built again from all its own fields is the place.** `Lines::Cons(head, rest) => { let lines = Lines::Cons(head, rest); .. }` reads `lines`, without the copy of each field.

### Fixed

- **A `for` range's start runs before a `?` in its end.** `for i in (n + 1)..s.parse()?` returned the parse error where Rust panics on `n + 1` first.
- **A `?` that typing turns into a `match` leaves the function from an operand.** `Ok(s.parse().map_err(f)? + 1)` and `o.unwrap_or(s.parse()?)` in an argument printed an inline function whose `return` left only itself; the `match` now runs before the statement, bound to a name, after the operands before it.
- **Two arms that bind one name to different fields keep both.** `P(x, _) if c => x, P(_, x) => x` declared `const x` twice; a row's name is bound in the pattern only where nothing else in the arm binds it. Such an arm reads the field under a made name (`field`, `field2`) and binds each row's own name from it.
- **A payload is read before its arm assigns the place.** `let v = match o { Some(v) => { o = None; v } .. }` read `o` after the assignment.

## 0.8.1 — 2026-10-03

An audit of the generator for rules fitted to the examples' shapes found output that Rust does not mean; this release fixes those that 0.8.0 printed for inputs outside the examples. No change to what is accepted ([roadmap §8.10](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#810-081-fixes-from-an-audit-2026-10-03)).

### Fixed

- **`??` beside `?:`, `||`, or `&&` is parenthesized.** `match o { Some(v) => v, None => if c { 1 } else { 2 } }` printed `o ?? c ? 1 : 2`, which JS reads as `(o ?? c) ? 1 : 2`: `Some(5)` gave `1`. `o ?? a || b` and `c || o ?? d` were syntax errors. `??` now has its own precedence, below `||` and parenthesized beside it.
- **A comment above an operand no longer drops its parentheses.** Since 0.8.0, `c && { // .. \n a || b }` printed `c && a || b`. A comment that is not above a statement is left out before printing.
- **An empty arm where the value is returned returns.** `M::A => match s { "" => {}, _ => .. }` in a unit function merged the empty arm into the next test, and `case "A"` ran into `case "B"`: a panic Rust does not have. The merge is kept only where nothing is handed on.
- **No `return` after a statement that never falls through.** A `match` whose arms all return, then `;`, and a `while true` no `break` leaves, were followed by `return undefined;`, which the generated tsconfig rejects as unreachable.
- **A tuple read in part is not taken for the whole.** 0.8.0's `Some((a, b)) => (a, Some(b))` → `x ?? d` matched the printed text, so `Some((a, b, _)) => (a, b)` became `x ?? [0, 0]`, the three-element tuple. The rewrite is withdrawn; semver's `parse` binds `option` again.

## 0.8.0 — 2026-10-03

Generated TypeScript that reads closer to the Rust it came from: no `$` in any name, flat guards instead of `else` chains, the source's `//` comments kept, bodies in paragraphs, byte literals that name their character, and fewer temporaries; a runtime copy that keeps only what the package uses; and the last four patterns 0.7.0 refused (`A | B(1)`, `Some((1, b))`, `.map(PreId::Numeric)`, `str::parse` into an integer). Packages generated with 0.7.0 must be regenerated, and callers that read a one-field tuple variant updated (Breaking, below) ([roadmap §8.9](https://github.com/manji-0/purecrate-ts/blob/main/design/07-roadmap.md#89-080-generated-typescript-that-reads-like-the-source-2026-10-03)).

### Breaking

- **A one-field tuple variant is `{ kind, value }`.** `PreId::Numeric(5)` was `{ kind: "Numeric", content: [5n] }`, read as `id.content[0]`; it is now `{ kind: "Numeric", value: 5n }`, read as `id.value`, and the constructor's parameter is `value`. Two or more fields stay `content: [..]`. The JSON is serde's as before (`{"Numeric": 5}`). Regenerate, and replace `.content[0]` on one-field variants with `.value` in calling code.
- **`Int` is exported only with a number.** The index re-exports `Int` only when the public surface holds an integer or float type; a package whose surface has none has no `Int` in its copy of the runtime to export.

### Changed

- **A copied `Some` is `??`.** `match s.split_once('+') { Some((x, y)) => (x, Some(y)), None => (s, None) }` is `Str.splitOnce(s, "+") ?? [s, null]`, with no temporary; a `None` arm of `None` drops the `?? null`.
- **`map(f).unwrap_or(d)` is one test.** With a name or literal `d`, `o.map(|x| ..).unwrap_or(d)` no longer holds the mapped `Option` in a temporary: oidc's `const opt = ..; const fresh = opt === null || opt;` is `const fresh = enrollment.last_used_step === null || c.step > enrollment.last_used_step;`.
- **A byte literal names its character.** `b'.'` prints as `/* '.' */ 46` rather than a bare `46`, so `isLocal` and `Email.parse` say which characters they test.
- **Bodies read in paragraphs.** A statement over several lines (an `if` or `for` with a block, a `switch`, `let x: T;` and what assigns it) has a blank line before and after it, and a comment a blank line before it; statements of one line run on. The `const` a block's first line reads stays with it. The examples' output gains 92 blank lines.
- **Comments in a function body are kept.** The `//` lines directly above a statement or a block's tail are printed above what that statement becomes, so a guard keeps the reason written for it (`// RFC 6749 §10.12: this OP requires state from every client.`). A comment after code on its line, before a block's `}`, above a block-level `const`, or above a `match` arm is still left out. The examples' output gains 18 comment lines.
- **A smaller runtime copy.** The copy keeps each `Int` width, operator, and method the package uses, the brand and helper types its code names, and what those read in turn; nothing else. Against 0.7.0, counter's copy is 175 → 84 lines, payment's 261 → 119, semver's 351 → 239; all eight examples 1882 → 1161.
- **No `$` in local names.** Temporaries say what they hold, without the `$` (`$v_major` → `majorResult`, `$m_3_$t` → `option`, `$e_2` → `end`, `$l2` → `loop`), and a shadowed binding counts from two (`pre$1` → `pre2`). A made name is numbered only where an identifier in the block that holds its uses already has it. A loop with a literal end reads it in place (`i < 4`). 
- **No `$` in shared internal names either.** A closed type's constructor is `unsafeMakeVersion` (was `Version$of`) and a non-`pub` method `yenRaw` (was `Yen$raw`), both `@internal` and left out of the `.d.ts` (`stripInternal`); each is numbered where a crate name has it. The wire module imports a domain type and its companion under one alias, `DomainAmount` (was `Amount$`, `Amount$value`, `Amount$text`), and names arktype's shapes `yenWire` and arms `methodCardArm`. None of these is exported from `index.ts`.
- **Shorter generated code.** Where a value is returned, a branch that returns is followed by the next one instead of an `else`, and a branch of one `return` sits on its `if`'s line, so a chain of guards reads top to bottom. A function name passed as a function is the name, not an arrow around it. A cast stands bare as an argument, element, or field value (`Int.i32.add(n, 1 as I32)`), and a comparison reads a literal or a length bare (`parts.length < 2`, `b >= 48`). A value of one side of a `match` or `if` whose other side returns is the exit and then a `const` (`if (o === null) return e;` `const x = o;`); a `match` on a call whose arms are expressions binds the call and is one `?:`; a tuple only taken apart is `const [a, b] = ..`. Against 0.7.0, semver's generated domain code is 414 → 343 lines and oidc's 1441 → 1368; all eight examples 3372 → 3225, with the comments and blank lines below counted.

- **Lines wrap as oxfmt would.** An arrow whose parameters fit keeps them on its line: an object, block, or array body opens (`(a: A, b: B): T => ({` then the fields), any other body moves one indent in after `=>`, so a constructor no longer opens its parameters and a return type no longer splits as `Result<\n  Sku,\n  OrderError\n>`. A condition on its own line breaks at each top-level `||` (else `&&`) before any call in it opens, a `?:` splits before the `&&` / `||` or a bracket in its branches (`return !apart` / `? divide(` / `: (0n as I64)`, a nested `?:` one indent further and without parentheses), and a line broken at `&&` / `||` no longer ends in a space.
- **Fewer parentheses and casts.** A test printed for `is_empty`, `is_some`, or a `match` takes parentheses by precedence (`s.length === 0 || ..`, was `(s.length === 0) || ..`); a widening cast, a discriminant table's entries, a `const`'s value, and an arrow passed as an argument stand bare; a cast in a branch of `?:` is always parenthesized, as oxfmt prints it. A place of a struct type is bound without `as` (`const terms: Terms = intent.terms`), since only a union needs it to undo a narrowing.
- **A `match` on a call that returns on one side exits first.** `let e = match Email::parse(raw) { Ok(e) => e, Err(e) => return Err(..) }` binds the call, returns on `Err` with the payload read in place, then `const e = result.value`, as a `match` on a place already did; signup's `Signup.parse` is 25 → 9 lines. A branch before the last that returns no longer leaves the last in an `else` when the last declares nothing (`if (!stateIsValid(n)) return ..;` `nonce = n;`).
- **Imports as organize-imports leaves them.** One declaration per module (`import { unsafeMakeYen, type Yen } from "./yen.ts"`, was two lines), `import type` where a module brings only types (`import type { I32 }`, was `import { type I32 }`, which leaves a bare `import "./purecrate-runtime.ts"` under `verbatimModuleSyntax`), values then types, each by name, and the crate's files by path.
- **A local keeps the name of a function its body does not read.** `fn paid(lines: Lines, total: Yen, ..)` is `(lines, total, cmd)`, was `total2`, since its file does not import `total`. A body that calls or reads the function still numbers the local.
- **`Some(m)` on a variable reads the variable.** `match method { Some(m) => .. }` is `if (method !== null) { .. method .. }`, without `const m = method;`.
- **No annotation an arrow or a `boolean` already states.** `const fail = (error: ErrorCode): AuthorizationError => ..`, was `const fail: ((_0: ErrorCode) => AuthorizationError) = ..`; `let replayed = false`. A closed type's constructor's JSDoc opens with “Makes `Email` values without a check.”
- **oxfmt's parentheses and breaks for `&&` / `||`.** An `&&` under `||` is parenthesized (`(b >= 48 && b <= 57) || ..`), and an arrow's body that is an `&&` chain breaks at each `&&` before its last call opens.
- **A discriminant reads from a checked table.** `d as u8 as usize` is `({ Six: 6, Seven: 7, Eight: 8 } satisfies Record<OtpDigits["kind"], number>)[d.kind] as Usize`, was `(({ Six: (6 as U8), .. } as Record<string, U8>)[d.kind] as U8) as number as Usize`. A shift amount and a literal `Slice.at` index stand bare (`Int.u32.shl(x, 24)`, `Slice.at(parts, 0)`).
- **No copy of a narrowed variable.** `let client = match client { Some(c) => c, None => return .. }` and `let method = method.ok_or(e)?` leave the variable as is after their exit, without `const client2: Client = client`, where the binding is the same Rust name. A temporary bound only for a `let`'s value is a `const` beside it, not a block (`let result; { .. }`), and `opt ?? true` reads `opt === null || opt`, was `!(opt !== null) || opt`.
- **The examples' output is linted with oxlint.** `scripts/verify.sh` runs oxlint, type-aware, with `examples/.oxlintrc.json` over every example's generated files; `npm ci` in `examples/` installs it and oxfmt, whose `examples/.oxfmtrc.json` is the layout the printer follows.
- **No empty branch, no needless block.** An arm that does nothing before the last (`"" => {}`) turns into the last's test (`else if (token !== "") { return null; }`); the last statement of a block leaves its last branch out of an `else` even where that branch declares names; a `match` on a call no longer opens a block for its temporary; and a local is never named like the function it is in (`step2`, so `step` is not hidden).
- **Laid out as oxfmt lays it out.** Every generated file passes `oxfmt --check` (`examples/.oxfmtrc.json`, width 100), which `scripts/verify.sh` now runs: a union is on one line where it fits (`export type Rate = Readonly<{ kind: "Standard" }> | Readonly<{ kind: "Reduced" }>;`) and a variant that breaks indents past its `| `; a long `?:` whose test is a comparison breaks after `=`, a nested `?:` parenthesized on one line and bare once split; a call whose one argument is a call opens around it (`unsafeMakeYen(` then `Int.i64.add(..),`), as does a parameter list of one that does not fit (`(` then `fields: Readonly<{ .. }>,`).
- **The wire module is laid out as oxfmt lays it out, and linted.** `purecrate-wire.ts` is built as a small TS syntax tree and printed with oxfmt's layout model (`emit_ts::doc`, `emit_ts::js`): member chains, a hugged last argument, function composition broken out, arrays of pairs one per line, and an arktype morph whose head does not fit opened around its callback. Zod's declared type drops its default (`z.ZodType<DomainYen>`, was `z.ZodType<DomainYen, unknown>`), and each `toJson` writer's `switch` ends in `default: return assertNever(x)`, so oxlint passes it with the domain files.
- **A discriminant table is a `const` of its file.** `d as u8` reads `otpDigitsDiscriminants[d.kind] as U8` from `const otpDigitsDiscriminants = { Six: 6, Seven: 7, Eight: 8 } satisfies Record<OtpDigits["kind"], number>;`, was the table inline in the expression.
- **The test fixtures' output is checked for layout too.** `scripts/verify.sh` builds every fixture that builds, with each schema, and runs `oxfmt --check` over the result; the forms it reached (a chain of `?:`, a `?:` arrow body, `??` beside a cast or in a `?:`, a cast's binary operand, a string's quote, a function called at once inside an array or an `if` test, an empty companion) now print as oxfmt prints them.
- **A guard that falls back is part of the test.** `match o { Some(s) if ok(s) => a, _ => b }` is `o !== null && ok(o) ? a : b`, was `o !== null ? (ok(o) ? a : b) : b`, and `Some(id) if *id == x => {} _ => return e` is one exit, `if (o === null || o !== x) return e;`, where `e` was printed twice. A `None` side that returns goes first and the `Some` side follows it, read in place (`if (rt === null) return ..;` `if (rt !== "code") return ..;`, was the `Some` side in an `if` with the exit in its `else`).
- **A returned choice inside a choice is `if` lines.** A `match` or `if` that is returned and holds another choice in a side prints as `if (..) return ..;` lines where its `?:` would not fit on one line (oidc's `begin` ends in `if (reusable !== null) { .. }` then the `None` side, was a `?:` three deep); one that fits stays one `?:`. The join of a guard into its test now happens on the code before it is printed, so it reaches an inner two-way `match` too (`field.kind === "Card" && field2 !== null ? field2 : 0`, was a `switch` with `assertNever`), and `if true { a } else { b }` is `a`.
- **Arms that do the same share their cases.** In a `switch`, arms that bind nothing and have the same body list their cases together (payment's `case "Succeeded":` `case "Canceled":`, was the inner `switch` twice).
- **No statement without an effect.** An `if` whose sides do nothing (what a write never read leaves), a value only built (`Box::new(P { a });`), and `t = String::from(&t)` are left out, and a `let mut` no longer written is a `const`.
- **A loop label only where a `switch` would take the jump.** A loop whose `break` / `continue` is not inside a `match` has no label (`for (..) {` and `break;`, was `loop: for` and `break loop;`), and a jump that is all of its `if` sits on its line (`if (k === 0) break;`).
- **`loop` is `for (;;)`.** A loop with no test (`loop`, and a `while` whose test runs in its body) is `for (;;)`, was `while (true)`, which typescript-eslint's `no-unnecessary-condition` flags.
- **The test fixtures' output is linted too.** `scripts/verify.sh` runs the examples' oxlint over every fixture's output, less what a fixture writes on purpose (`scripts/fixtures.oxlintrc.json`, each exception with its reason).
- **No annotation a cast already states.** `export const MAX_STATE_LEN = 512 as Usize;` and `let n = 0 as U32;`, were `MAX_STATE_LEN: Usize = 512 as Usize` and `n: U32 = 0 as U32`; the declared type is the same. A narrowed union given back its type reads `const s = state as State`.
- **An unwrapped `Option` is held in its binding.** `let current = match totp_step(..) { Some(t) => t, None => return .. }` is `const current = totpStep(..);` `if (current === null) return ..;`, was `const option = ..` and then `const current: I64 = option;`. A `let mut` keeps the temporary, since its type would hold the `null`.
- **No annotation a `const`'s value already has.** A `const` of a call (`const otpCheck = checkTotp(..)`, `const failures2 = Int.u32.add(..)`), of a test (`const fresh = opt === null || opt`), of `x?`, or of a place that is no union (`const terms = intent.terms`) states no type, as TS infers the same one; a literal, a variant, a `?:`, a `let`, and a type with a wrapper's comment (`/* Box */ P`) keep it. A `match` on a call binds it the same way.
- **A case is a block only where it declares a name.** `case "ConsentDenied":` then its `return`, was `case "ConsentDenied": {` .. `}`: a `case` whose statements bind nothing at their top no longer opens a block, which only keeps a `const` from the other cases.
- **A `match` on a value a case has narrowed is the arm it takes.** `match (state, event) { .., (s, _) => if matches!(s, State::Done) { a } else { b } }` copied the whole test under each `case` of `state`, with `const s = state as State` to make it type-check; under `case "Running":` it is now `b`, and a case of `Idle | Done` whose sides differ splits into a case each. The fixture lint no longer needs its `as State` exception.
- **One variant and the rest is an `if`.** A `match` on an enum with an arm for one variant and one for the rest (`_`, or the other variants the decision tree lists) is an `if` on the variant instead of a `switch` of one `case` and `default`: payment's `Succeeded` / `Canceled` is `if (event.kind === "Cancel") return ..;` `return ..;`. Where the rest is one exit and the variant's side takes statements, the exit comes first (`if (event.kind !== "PasswordChecked") return ..;`, then the arm unindented, as in oidc's `step` and order's `paid`), and `true` against `false` is the test itself (`return ordering.kind === "Greater";`).
- **`Result` as a type only is a type import.** A file that annotates `Result<..>` but builds none imports `type Result`.
- **`Ordering` compares with its variant's name.** `o != Ordering::Equal` is `o.kind !== "Equal"`, not `o.kind !== { kind: "Equal" }.kind`.

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
