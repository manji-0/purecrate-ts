# Changelog

## Unreleased

### Fixed

- **A `#[serde(..)]` refusal was lost to a later key.** Each key reset what the one before it had refused, so `#[serde(deny_unknown_fields, rename_all = "camelCase")]` was accepted though the same keys the other way round were refused, and `rename = "a", rename = "b", tag = "t"` lost its "given twice". The first refusal now stands.

## 0.20.0 — 2026-10-14

Type parameters on structs, enums, `impl` blocks, and functions, emitted as TS generics with their type arguments inferred. On the wire, a generic type's schema takes its parameters'. The plan of 0.16.0 to 0.20.0 is done; the distance to the ideal is 83%.

### Added

- **Type parameters** (design/01 §7.20): `struct Page<T>`, `enum Tree<T>`, `impl<T: Clone> Page<T>`, `fn first<T: Clone>(xs: &[T]) -> Option<T>`, each one TS declaration with a type parameter (`export type Page<T>`, `<T>(xs: ReadonlyArray<T>): T | null => ..`), no copy per type. A `T` may be moved, returned, put in a `Vec`, `Option`, tuple, struct, or variant, cloned, and compared with `==` (`Eq.deep`). Bounds: `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Debug`, on the parameter or in `where`. Type arguments are inferred at each call from the arguments and the type the value goes to, at a construction from the fields, and for a method from the receiver; one nothing gives asks for an annotation. A constructor that holds no `T` gives `never` there (`Tree.Leaf(): Tree<never>`).
- **Generic types on the wire** (design/04 §3.3): a schema of the parameters' schemas (`Page(User)`), a writer of their writers (`toJson.Page(x, (v) => toJson.User(v))`), `fromJson.Page(text, User)`; a recursive one behind `lazy` inside. With arktype the parameter is any `Type` and its value `WT["infer"]`, as the adapter's `nullable` takes one, read inside as a plain `Type`, which arktype's type-level check of a definition takes.
- `purecrate-ts test` calls a generic function with each type parameter at `i32`, and draws and prints generic types through their parameters.

### Refused

- Ordering a `T` (`<`, `cmp`, `max`, `sort`), whatever its bound: the output has no comparator for a value whose type it does not know; order by a key of a known type. Other bounds, const generics, defaults, a turbofish on a call, and a generic trait impl.
- A type argument that is an `Option`, `()`, or `!` (a generic item's `Option<T>` would print `null` both ways), or that `Eq.deep` does not compare as Rust does.

Tested against Rust with a generic struct as a parameter, field, and result, nested in `Option` and `Vec`, two-parameter types, a recursive generic enum through `Box`, a generic function at two types in one body and recursively, struct update on a generic struct, and `==` on generic values (`generics_equivalence.rs`); the refusals in `check`'s `generics.rs`. `corpus/patterns.rs`'s `Page<T>` and `p07_generic` are accepted. On the wire, a struct holding a generic struct, a two-parameter enum, and a recursive generic enum read from serde_json and are written back byte for byte.

### Fixed

- **The wire's round trip ran in zod alone.** 0.19.0 said its maps were round-tripped in all three libraries; the shapes test read with zod's `parse` only. It now reads through `fromJson` in zod, valibot, and arktype, every shape and map among them, and all three agree with serde_json byte for byte.

### Documentation

- **Distance to the ideal** (design/07 §2.5): 83% with 0.20.0, from 81%; the plan said 86%, but a `T` is not ordered.

## 0.19.0 — 2026-10-14

`BTreeMap<String, V>` and `BTreeSet<String>`, as a TS `Map` / `Set` read in Rust's key order and as a JSON object on the wire. Every generator agrees with Rust on seeds 1 to 120, and all but statements seed 119 (0.18.0's known one) type-check. The distance to the ideal is 81%.

### Added

- **`BTreeMap<String, V>` and `BTreeSet<String>`** (design/01 §7.19). A `ReadonlyMap<string, V>` / `ReadonlySet<string>`, sorted at every read in order: by code point, as Rust orders a `String`'s bytes, where JS's `sort()` puts U+E000..U+FFFF above a key outside the BMP. `BTreeMap::new()`, `get` (an `Option`), `contains_key` / `contains`, `len`, `is_empty`, `clone`, `iter` / `keys` / `values` and `for (k, v) in &m` into consumers, `collect` and loops, `collect()` from `(String, V)` pairs (a later key's value replacing an earlier one's) or from `String`s, and `==` (`Eq.deep` compares maps and sets by key). `insert` and `remove` write a `let mut` local that owns its map, as a grown `Vec` owns its array. Keys are `String` only; a map of `Option` or `()` values is refused, as is `HashMap` / `HashSet`, whose order Rust leaves unspecified (the refusal names `BTreeMap`).
- **On the wire** (design/04 §3.3): a map is a JSON object, its keys written in Rust's order and read whatever order they come in; a set an array. Every own key is a key, `__proto__` and `constructor` too, which zod's and valibot's records drop, so each adapter reads the object itself (`btreeMap`, `btreeSet`); a key with a lone surrogate is refused and a repeated key keeps its last value, as serde does. Round-tripped byte for byte against serde_json in all three libraries.
- `Str.wellFormed` reads a map's keys and values and a set's keys.

Tested against Rust with keys on both sides of U+FFFD and outside the BMP, maps built in reverse order, a key twice, `__proto__` as a key, the caller's map left as it was, overflow at the first value in key order, and a struct holding a map and a set compared with `==` (`btree_equivalence.rs`, `lone_surrogate.rs`, `wire_write.rs`); and as three new statements of the collections generator, seeds 1 to 120.

### Documentation

- **Distance to the ideal** (design/07 §2.5): 81% with 0.19.0, from 78%; the plan said 83%, but keys other than `String` wait for a use.

## 0.18.0 — 2026-10-14

`Option`'s and `Result`'s combinators, `get`, `first`, `last`, and `contains` on a `Vec`, `fold`, `rev` and `zip` on a sequence, and `panic!`, `unreachable!`, `assert!`, and `Option::unwrap` / `expect` with Rust's messages; eleven examples use them. The statements generator's new forms found four places, each also in 0.17.0, where narrowing differed from TS's, fixed here. Every generator agrees with Rust on seeds 1 to 120, and all but one of the 960 runs type-check (below). The distance to the ideal is 78%.

### Added

- **`Option`: `and_then`, `filter`, `map_or`, `map_or_else`, `unwrap_or_else`, `ok_or_else`, `or`, `or_else`, `is_some_and`, `is_none_or`; `Result`: `err`, `is_ok`, `is_err`, `is_ok_and`, `is_err_and`, `unwrap_or`, `unwrap_or_else`, `map_or`, `and_then`, `or_else`** (design/01 §7.1). Each is the `match` std writes. A value argument (`map_or`'s default, `or`'s option) runs before the `match`, on both sides, as Rust evaluates it; a function runs only on its own side. One that would make an `Option<Option<_>>` is refused.
- **`panic!`, `unreachable!`, `assert!`, and `Option::unwrap` / `expect`** (design/01 §7.17), each throwing `Panic` with Rust's message: `explicit panic`, `internal error: entered unreachable code`, a `format!`-style message, `assertion failed: ` and the condition as rustc prints it (names, literals, operators, casts, calls, method calls, fields, indexing, references, parentheses; any other condition asks for a message), ``called `Option::unwrap()` on a `None` value``, and `expect`'s argument, evaluated on `Some` too. `assert!`'s message is built only when it fails. Refused: `unwrap` and `expect` on a `Result` and `assert_eq!` / `assert_ne!`, whose messages print a value with `{:?}`.
- **`get`, `first`, `last`, and `contains` on a `Vec`** (design/01 §7.18): `Slice.get`, `Slice.last`, and `Slice.contains`, which compares as the derived `eq` does (NaN unequal to itself, `-0.0` equal to `0.0`, where JS `includes` would find NaN).
- **`fold(init, |acc, x| ..)`** on a sequence (`Iter.fold`), `init` evaluated before any item. An unsuffixed number as `init` with no type from the context is refused, as Rust infers its type from the closure.
- **`.rev()` and `.zip(ys)` right after a sequence's source**, before any `map` or `filter` (design/01 §7.13), in a consumer, `collect`, or a `for`: the items reversed, or paired up to the shorter side, as a new array. After a stage they are refused: Rust runs a `map` from the back under `rev`.

Tested against Rust with arguments that overflow on the side Rust evaluates them, lists that are empty or end early, NaN and `-0.0`, structs, every message form, and messages whose argument overflows only when the assertion fails (`std_more_equivalence.rs`, `panics_equivalence.rs`); and as forms of the statements generator, `unwrap` and `expect` on `None` and `assert!` and `panic!` among them, seeds 1 to 120.

### Fixed

- **A name after a spread was not seen by the runtime's trim** (`[...Str.bytes(s)]`), so a package's copy could leave out what it called; `s.bytes().rev()` prints one.

Output tsc refused where narrowing differed from TS's, values unchanged, each also in 0.17.0: `matches!` on a value that is no place (`r.ok()`) did not decide its guard, so a comparison TS had as `true !== false` was printed; `v && !v` was not folded where TS narrows `!v` by `v`; a `const` aliasing a test of a place (`!c || r.ok().is_some()`) was not read again once the place was known, so code TS has as `never` was printed; and `if a { c } else { true }` was taken as `!a || c`, which the printer leaves a `?:` TS does not narrow by (`narrowing.rs`: `guard_on_built`, `contradiction`, `alias_decided_late`, `true_else_side`). A panic narrows neither TS nor the generator's flow: `Panic.raise` is a property with no written type, which TS does not read as ending control flow.

### Known

- **Statements generator seed 119 does not type-check**, values unchanged; it is the same in 0.17.0. A `while` test that needs statements prints as a function called in place, and where that function returns a name the loop narrows (`opt ?? v4`), tsc cannot infer its return type (TS7024). Rare in written code; the fix, binding the test's statements at the top of a `for (;;)`, needs the printer to lift a binding out of an operand.

### Examples

- **ulid, semver, jsonpatch, negotiate, oidc, ssh, raft, punycode, calendar, payment, and iban** use the new methods where they wrote the `match` or the loop (design/07 §2.2): ulid 94 → 79 lines (1.6× → 1.3×), semver 142 → 124 (2.0× → 1.7×), punycode 263 → 250, negotiate 295 → 282, jsonpatch 633 → 621, oidc 573 → 568, payment 125 → 122, iban 48 → 46, calendar 732 → 729.

### Documentation

- **Distance to the ideal** (design/07 §2.5): 78% with 0.18.0, from 73%; the plan said 79%, but `step_by`, `flat_map`, `filter_map`, `windows`, and `dedup` stay out.

## 0.17.0 — 2026-10-14

`loop` with a value, `while let`, `if let` and `let`-`else` on any pattern that can fail, `a..=b`, a reversed range, and a tuple's `.0`; seven examples use them. The statements generator's new forms found three miscompiles released through 0.16.0, fixed here, and every generator agrees with Rust and type-checks on seeds 1 to 120. The distance to the ideal is 73%.

### Added

- **`loop`, and `break` with a value** (design/01 §7.16). A `loop` is `for (;;)`, of type `!` when nothing breaks out of it, `()` when its `break`s carry nothing, and otherwise the type they carry: `let x = loop { .. break v; .. };` declares `x` and assigns it before each `break`, a loop that ends a function returns its value, and an unread one runs `v` for its effects.
- **`while let P = e { .. }`**, `e` evaluated before every pass, as Rust's desugaring does.
- **`if let` and `let`-`else` on any pattern a `match` arm may be that can fail**: a variant (`if let Shape::Circle(r) = s`, `let Role::Leader { progress } = node.role else { return .. };`), a literal, a range, or one nested in `Some`. A `let`-`else`'s names are bound for the rest of the block, a `mut` one rebound; on a variant it prints as a guard (`if (s.kind !== "Rect") return 0;`), on `Some(x)` / `Ok(x)` / `Err(x)` as before. Refused: a struct pattern, a type annotation on the `let`-`else`, and a pattern that always matches.
- **`for i in a..=b`, `(a..b).rev()`, and `(a..=b).rev()`**, the bounds evaluated once, start first. `..=` tests `i <= end`; its last step, past `b`, is a JS number or `bigint` the body never sees, so `0..=255` on a `u8` neither wraps nor overflows. A reversed range starts at `b - 1` (or `b`) and steps down while `i >= a`. `rev` on an iterator stays refused.
- **A tuple's `.0`, `.1`, …**, read as `t[0]`; an index past the tuple is refused.

Tested against Rust at both ends of `u8`, `i8`, and `u64` (past 2^53), with empty ranges, bounds that panic in Rust's order, `break` values from a `switch`'s arms and nested loops, `while let` on a method, a `Result`, and a variant inside `Some`, and `let`-`else` with several names, `mut`, `continue`, and `break` (`loop_value_equivalence.rs`, `if_let_equivalence.rs`); and as five new statements of the statements generator, seeds 1 to 120.

### Changed

- **A loop's label is printed only where a jump inside a `switch` needs it** (design/03 §3.3.3): a `match` printed as an `if` chain, or a `break` with a value that returns, leaves a bare `break`.
- **A variant's arm followed by a rest that is one jump prints as a guard** where nothing follows the `match` in its block: `if (node.role.kind !== "Leader") return finish(..);` then the arm, not the arm nested in an `if`.

### Fixed

Three miscompiles, released through 0.16.0, found by the statements generator's new forms; each printed code that returned a different value:

- **A variant inside `Some` beside a last `_`** (`match o { Some(E::A(n)) => .., _ => .. }`) always took `_`. An `Option` prints as its payload or `null`, so one place is tested for `null` and then for the variant; narrowing took the second test's cases as a subset of the first's, found none, and folded the arm away. Cases of different kinds no longer rule each other out (`nested_patterns.rs`: `method_in_some`, `login_in_some`).
- **An `if` side holding `map_err(f)?` inside a value** (`if c { a } else { r.map_err(f)? | 3 }`) printed as a `?:` whose side was an inline function: its `return` left only that function, and the `Err` became an operand. A side where a `return` or `?` stands anywhere outside a closure is statements (`control.rs`: `side_map_err`).
- **`map_err(f)?` as a `match`'s scrutinee** printed as an inline function, so the `Err` was matched against the arms instead of returned. It is bound before the `match` (`control.rs`: `scrutinee_map_err`).

And output tsc refused where narrowing knew less than TS, values unchanged: a `?` on a place known to fail as an arm's value or a statement printed its test, and the code after it, which TS takes as unreachable (`never`), instead of the `return`; a place was not known to have passed a `?` inside a block (`{ let a = x < r?; }`) past it; a closure made and never called kept a pattern's binding it read (TS6133); a closure reading a place left an arm TS had ruled out; and an `if` whose `else` is a literal, printed `c && a`, did not narrow by `c` (`narrowing.rs`: `known_failing_arm`, `try_in_block`, `uncalled_closure`, `closure_reads_place`, `literal_side_test`, `decided_then_side`). Every generator agrees with Rust and passes tsc on seeds 1 to 120.

### Examples

- **calendar, geo, oidc, punycode, raft, ssh, and ulid** use `let`-`else`, `if let`, `..=`, a reversed range, and `loop` where they worked around them: calendar 738 → 732 lines, punycode 267 → 263 (1.8× → 1.7×), oidc 576 → 573, ulid 95 → 94 (design/07 §2.2); raft's commit rule counts down with `(commit_index + 1..=len).rev()`, and punycode's `while k > 0`, which stood for the RFC's endless loop, is `loop`.

### Documentation

- **Distance to the ideal** (design/07 §2.5): 73% with 0.17.0, from 67%; the plan said 75%, but `rev` and `step_by` on an iterator stay out.

## 0.16.0 — 2026-10-14

`format!` builds a `String` from integers, `bool`, `char`, strings, and `Uuid`, printed as a template literal; semver and jsonpatch use it where they built digits by hand. The plan to 0.20.0 is set by decision, and over 2× the idiomatic lines is now a priority indicator rather than a withdrawal condition.

### Added

- **`format!`** builds a `String` from integers of every width, `bool`, `char`, `String`, `&str`, and `Uuid` (design/01 §6.8): `{}`, `{0}`, `{name}` with `name = ..`, names captured inline (`{x}`), `{{` / `}}`, a width counted in `char`s, `<` / `^` / `>` with a fill, and zero padding on an integer after its sign. It prints as a template literal, padded through `Str.pad` / `Str.padZero`. Refused, with what v0 takes: floats, `{:?}`, radixes and exponents, `+`, `#`, a precision, a width from an argument, the crate's own types, zero padding on anything but an integer (Rust pads it with spaces), a width on a `Uuid` (Rust ignores it), and an argument other than a local, a literal, or a field placed out of order or twice, since a template literal runs its `${..}` in the order it places them. `.to_string()` stays refused. Tested against Rust on every integer at its bounds, fills and text outside the BMP, escapes, and arguments that panic (`format_equivalence.rs`), and as a shape of the strings generator.

### Examples

- **semver** writes its version with `format!` where it built a `u64`'s digits by hand: 165 → 142 lines, 2.3× → 2.0× its idiomatic reference. **jsonpatch** writes its numbers with `format!`: 648 → 633, 4.1× → 4.0× (design/07 §2.2).

### Documentation

- **Over 2× the lines of idiomatic Rust is an indicator of what to improve first, no longer a condition to withdraw** (design/06 §4.2, design/07 §1). A capability an example's excess names goes first among the candidates only if it helps code in general, not one domain: `format!` on integers (semver) now heads design/07 §3. jsonpatch's excess (`&mut` into a nested value) and payment's (its checked constructor, which the reference does not do) raise nothing. `&mut`, in either form, moves from the candidates to design/07 §6: it would be a new place for wrong values. The open question in design/07 §7 is closed.
- **Distance to the ideal** (design/07 §2.5): about 60%, from common shapes of domain code in `corpus/patterns.rs`, judged by `survey` and weighted; `format!` on integers, `String`-keyed maps, and type parameters would take it to about 75%.
- **The corpus re-measured with 0.15.0** (design/90 §8): 188 of 956 public functions accepted, where 0.3.0 accepted 5; 182 are idsmith's short validators, and outside idsmith it is still 5 of 126. design/00 §2 cites it.
- **The plan to 0.20.0** (design/07 §3, §5), taken by decision: `format!` (0.16.0), control flow (0.17.0), std on `Option` / `Result` / `Vec` with `unwrap` and `assert!` as panics (0.18.0), `BTreeMap<String, V>` and `BTreeSet<String>` with `HashMap` kept out (0.19.0), and type parameters with the derivable bounds (0.20.0); traits after them.
- **design/02 caught up with 0.13.1**: narrowing `as`, `i128` / `u128`, floats and their methods, the string methods (`trim*`, `to_ascii_*case`, `find`, `split` with a closure), the consumers (`find`, `max` / `min` and their `_by` / `_by_key` forms), and byte strings, which it still called unavailable; two rewrites it still advised are gone.

## 0.15.0 — 2026-10-13

`check` and `build` now refuse a release profile that wraps on overflow (`--allow-overflow-wrap` keeps the warning), `purecrate-ts test` runs the differential comparison on your own crate, and the wire reads and writes `#[serde(rename)]`, `rename_all`, and internally and adjacently tagged enums as serde does. The release history that was design/07 §8 now lives here, under each version's Rationale heading.

### Added

- **`purecrate-ts test <crate>`** runs the differential comparison on your own crate: every public function and method is called in Rust (compiled with overflow checks and debug assertions) and in the generated TypeScript (under node) on the same arguments, drawn from each parameter's type with a seed (`--seed`, `--cases`), and it fails naming each call whose result or panic differs. Integers are drawn at the edges of their width, strings from the crate's own literals and a fixed alphabet, open structs and enums field by field, and closed types through their public constructors. `--keep <dir>` leaves the harness and the driver. A crate that uses `uuid` is refused. On every example it finds no difference; on `usize` past 2^53 − 1 it reports the stated non-equivalence (design/01 §8.4).

- **`#[serde(rename)]`, `#[serde(rename_all)]`, `#[serde(tag)]`, and `#[serde(tag, content)]`** are read and written as serde_derive does, with every schema library and `toJson` (design/04 §3.6): `rename` on a field or a variant, `rename_all` (all eight of serde's rules) on a struct, an enum, or a struct variant, internally and adjacently tagged enums with their sequence forms. The TS domain types keep the Rust names. `check` refuses what serde cannot read back (two fields or variants under one JSON name, a field under the tag's name, an internally tagged tuple variant or newtype of anything but a struct with named fields). Tested against the real derive on 2,953 JSON texts per library, 343 of which serde reads. The adapters gain `internallyTagged` and `adjacentlyTagged`, the runtime `internalTag`, `adjacentTag`, and `Json.withTag`.

### Changed

- **Other `#[serde(...)]` attributes are refused with the list of those modeled**, where the message used to name `try_from` alone; `untagged`, `flatten`, `default`, `skip*`, `alias`, `deny_unknown_fields`, and `transparent` stay out (design/04 §4).
- **`check` and `build` refuse a crate whose `[profile.release]` does not set `overflow-checks = true`** (theirs or their workspace's), where they used to warn: a `--release` server built from it wraps on overflow, where the generated TypeScript panics as a debug build does. `--allow-overflow-wrap` makes it the warning it was. A single `.rs` file with no manifest has no profile and is not refused.

## 0.14.0 — 2026-10-12

The output is laid out by oxfmt, which `build` and `check --out` now run and need. Then oxlint over the generators' output, which the sweep does not lint, found what follows; each is fixed with a fixture (`lint_holes`).

### Changed

- **`build` and `check --out` lay the generated files out with oxfmt** (0.71.0, width 100), run as a program: `PURECRATE_OXFMT`, else `node_modules/.bin/oxfmt` in or above the output's directory, the input crate, or the working directory, else `oxfmt` on `PATH`; without one they stop and write nothing (`check` alone does not need it). Another oxfmt version is used with a warning, since it may lay a line out otherwise. The copied runtime and adapters are left as written. The differential tests lay out what they run the same way.
- **The printer no longer breaks lines itself**: `tidy`'s line rules, which imitated oxfmt shape by shape, are gone; oxfmt breaks them. Every example's and fixture's output is byte for byte what it was. A discriminant table too long for its line is printed one variant a line, which oxfmt keeps open.

### Fixed

- **Refused by oxlint:** what an unread value leaves for its effects kept a comparison, a `!`, an `is_some()` / `is_none()`, a `Some` / `Ok` / `Err`, or an integer method that cannot panic (`checked_*`, `saturating_*`, `min`, ..) standing as a statement (`Int.i32.checkedSub(a, b) !== null;`), and an `if` whose two sides both run printed as a `?:` statement; `a || f()` left alone is now `if (!a) { f(); }`. A name read only to write itself (`f = c && f`) stayed declared and unread. A `for` whose body an unread `let` emptied, or whose `?` the flow decides, printed an empty body. `if !c {} else { .. }` printed `if (!(!c))`. An unread closure parameter renamed for shadowing lost its `_` (`_a$1` printed `a2`). An `else` after a branch that returns once the flow decides its test, an `else` that is one guard (`if (c) return v;`) or one `if` whose test spans lines, an arm left empty (an `else` or an `if` with nothing to do once the flow decides its code away), a decided `if` that declares nothing printed as a lone block, and a closure or a test left as a statement by an unread `let` the flow dropped.
- **Refused by oxlint, in float and width code:** a type assertion on what a function taking any `number` or `bigint` is passed (`Math.abs(1.5 as F32)`, `Number(7n as I64)`, `BigInt(..)`, `Int.u8.cast(300 as I32)`, `Int.f32.ofBig(..)`, `Eq.deep(p.c, 3 as U8)`), on a value asserted again through `number` (`fround(x) as F32 as number as F64`, `91 as U8 as number as U32`, now `91 as U32`), and on the sides of a choice asserted so; an unread `f64` sum, `-`, float conversion or constant, array copy, tuple, array, struct literal, or field read left as a statement.
- **Refused by oxlint, after the flow:** `x = x`, where an assignment kept the name on a side (`x = if c { y } else { x }` printed as statements, `x = !!x`, `x = x.map(|v| v)`) or the flow left only that side; a `for` the flow emptied whose bounds were named before it; a temporary named as a `for` variable declared in its header, which it hid; a name read only to decide its own `x = x || r` (now `if (!x) x = r;`), or only by its own writes; `(c ? a * -1 : Math.fround(b)) as number as F64`, whose sides are already plain numbers.
- **Laid out other than oxfmt:** a `const`, `let`, or `return` of a long sum under an assertion (`(a + b) as F64`) opened the group on lines of its own, where oxfmt keeps it open after its first operand; so did a `const`, `let`, or an arrow's body of a long choice under an assertion, which oxfmt keeps open on its test.
- **Laid out other than oxfmt, around a function called at once:** a struct's or a variant's fields printed the function's lines from the left margin; a call given one as an argument did not open its arguments; `if (c) return v;` kept `v` on its line though it spans lines; `return a && f(..)` with the call opened did not open the `return`. A labelled `for` or `while` head too long for its line broke inside a call instead of opening.
- **Refused by tsc:** `p?` left as a statement (what an unread `x = p?` leaves) narrows `p` in TS, which the flow did not follow, so a later `p?` tested a case TS had ruled out; a closure parameter read only in code the flow decided away was left unread (a function's parameter already took a `_`).

### Rationale: the layout is oxfmt's

<!-- supersedes #rationale-the-rest-of-the-audit-and-layout -->

Why: oxlint over the generators' output, then oxfmt over it, found 1,082 of about 5,300 files laid out otherwise than oxfmt; fixing the shapes one by one brought it to 850. `tidy::wrap` imitated oxfmt rule by rule from the examples, and each new shape was a new rule. oxc's formatter is still not published as a crate (crates.io holds a `0.0.0` placeholder), so `build` and `check --out` run oxfmt as a program.

| Item | Verified by |
| --- | --- |
| `build` and `check --out` lay the generated files out with oxfmt 0.71.0 at width 100, and stop without one | `build.rs` (`build_without_oxfmt_writes_nothing`); `verify.sh` checks the version against `examples/package.json` |
| `tidy`'s line breaking removed (about 1,800 lines); a long discriminant table printed one variant a line, which oxfmt keeps open | every example's and fixture's output (1,430 files) byte for byte unchanged with and without the old rules |
| The differential tests and the generators run the laid-out package | `support::write_package` |

- **Cost.** `build` needs Node and oxfmt beside the binary, and its output follows oxfmt's version: a new oxfmt is a new layout, taken by moving the pin and regenerating the examples.
- **Left to the printer**, since oxfmt keeps it as it finds it: an object, array, or argument list written open; blank lines; parentheses; a comment's place.

## 0.13.2 — 2026-10-11

Four seeded generators over 0.13.1's additions (floats, collections, strings, and every integer width with folded consts), each run over seeds 1 to 120, an independent review of 0.13.1's diff, and the statements generator widened to blocks, shadowing, and the names the translator makes up found what follows; each is fixed with a fixture or a test.

### Fixed

- **Wrong values, with no error:** `const M: i128 = !0;` folded as unsigned and printed 2^127 − 1 (`IntTy::is_signed` left out `i128`); `!x.is_infinite()` printed `!Math.abs(x) === Infinity`, always `false`; `v[i] op= x` and `v[i] = x` evaluated the place before an `x` that needed statements, so the first panic differed; `Eq.deep` took one object on both sides as equal, so a struct holding NaN compared equal to its `clone()`. A `let` in a Rust block stayed in force past the block, as a statement since before 0.13.1 and in a branch, a loop body, or a block value until now (`{ let a = 100; } h += a` added 100), or was dropped with its reads left (`ReferenceError`); two blocks side by side declared one name twice (`SyntaxError`, the module did not load), as did a block narrowing printed flat beside a later binding of one of its names, and a printer temporary beside a `check` one; a closure bound in a block that folds into an expression was dropped (`ReferenceError`). `v[i] = v[i] + x` written out evaluated `x` first; `split` with a closure ran it over the whole string before the consumer read; JSON `-0` into an `i128` was refused.
- **Refused or crashed where rustc accepts:** `-x`, `abs`, and negative consts on `i128`; `const A: i8 = -128;`; `i128::MIN / -1` in a const crashed `check`; a function's name or a local closure as a string pattern. A `?` in the test of an `if` or the scrutinee of a `match` that is a `let`'s value (`let v = match x? { .. }`, `if matches!(x?, 0..=9)`); both run first, and the `?` goes before the statement as on the left of `&&`. A local named `Array` broke `collect` (`Array.from`) and one named `NaN` or `Infinity` failed oxlint; `check` refuses all three now, as it does TS's reserved words. So it does a crate type named `Record`, which a discriminant table's `Record<..>` read in the same file (tsc: "`Record` is not generic").
- **Refused by tsc or oxlint:** a variant literal bound to a temporary lost its `kind` literal type; a `String` only pushed to and dropped; casts on `round` / `castFloat` arguments and on a length used as an index; a negated `f32` literal; an `f64` literal more precise than a double; an unused conversion or array copy left as a statement. A `match` in a closure on a place its arm had decided, and a `p?` there; `x = w?` with `w` known `None` followed by `return undefined`; `const d = !c || r.is_ok()`, which decides `r` only when it fails; a second `r?` where a later closure read `r`; an arm left empty printed as `{ }` before an `else`; an empty struct's type printed as `{}`; `if c { true } else { b }` as a value printed `c ? true : b`, which TS does not narrow by, where narrowing took it as `c || b`. `(r.kind === "Ok") !== false`, which TS narrows by as by the test (and oxlint refuses: a comparison to a `bool` literal prints as the test or its negation), and `let res = { ..; r };` with `r` narrowed, which TS narrows `res` from, were taken as deciding nothing.

### Documentation

- design/01 §3: a sort whose key or comparator may panic more than one way can panic first on a different pair past about 20 elements.

## 0.13.1 — 2026-10-11

Every capability the five new examples asked for, taken by decision in one patch series ([below](#rationale-what-the-five-examples-asked-for)): they stood at 2.2× to 3.5× their idiomatic references, and in each case the excess was something the subset lacked rather than the domain.

### Added

- **`v.remove(i)`, `v[i] = x`, and `v[i] op= x`** on a local `let mut v: Vec<T>`, as `push` and `insert` are: `Slice.remove` / `Slice.set`, with Rust's panic at or past the end (JS would grow the array). Rust evaluates the value before the place; where both may panic, the value is bound first. A field's or an element's array stays refused.
- **`x as T` between integer types** that std's `From` does not join, wrapping as Rust's `as` does (`Int.<t>.cast`); into `usize`, a result above 2^53−1 panics, as every `usize` that large does. A widening `From` takes stays refused as `as`, and the message names `T::from(x)`.
- **`u128` and `i128`**, as `bigint`s with 128-bit checks: operators, bitwise operators and shifts, the integer methods, `from`, `as`, `parse`, and the wire schemas. A literal at 2^127 or above is refused (literals are held as `i128`).
- **Byte strings** `b".."`: `Str.bytes("..")` where the bytes are UTF-8, else an array of byte literals; as a `&[u8]` const or in an expression.
- **Floats**: `round` (half away from zero, `-0.0` kept), `floor`, `ceil`, `trunc`, `abs`, `is_nan`, `is_finite`, `is_infinite`, and `f64::NAN` / `INFINITY` / `NEG_INFINITY`; `x as i64` and the other float-to-integer casts (toward zero, saturating, NaN to 0); integer-to-float `as`, rounded once to the nearest, ties to even (`Int.f32.ofBig` above 2^53, so as not to round twice); `x as f32` from `f64`; `f64::from` / `f32::from` where std has the `From`.
- **`==` and `!=` on a derived `PartialEq`**: the crate's structs and enums, `Option`, `Result`, `Vec`, and tuples, compared part by part (`Eq.deep`), NaN unequal to itself and `-0.0` equal to `0.0` as in Rust. A value holding `ParseIntError` or `uuid::Error` stays refused.
- **Strings**: `to_ascii_lowercase` / `to_ascii_uppercase`, `trim` / `trim_start` / `trim_end` by Rust's whitespace (not JS `trim`'s), `trim_matches` and its one-sided forms with a `char`, a closure, or (one-sided) a `&str`, `find` as a UTF-8 byte offset, and `split` with a closure.
- **Sorting** a local `let mut` `Vec`: `sort()`, `sort_by(|a, b| ..)`, `sort_by_key(|x| ..)`, stable as JS `sort` is. `sort_unstable` stays refused.
- **Consumers**: `find`, `max` / `min`, `max_by_key` / `min_by_key`, `max_by` / `min_by` (the last of the greatest, the first of the least, as std); `cloned()` / `copied()` on an `Option` are the value.

### Fixed

- A `let` of an enum-typed place was cast back to its type even where nothing had narrowed it (a parameter, a `?`'s value), which oxlint refuses as unnecessary; only a place a `match` tests, a name its arm binds, or a copy of one is cast now.
- A `while (..) {` whose test does not fit opened as oxfmt does not; it opens as an `if`'s.
- `[expr/method-call]` on a `Vec` lists `insert` beside `push`.
- `-x` on an `f64` printed `-` on a brand, which oxlint refuses; a value is multiplied by `-1`, which flips the sign exactly, and a literal is bare (`-90.0 as F64`).
- A companion `of` whose head fits up to `=> ({` opened its parameters where oxfmt opens a short object body.
- An opened `if (` / `while (` test broke only where it did not fit; oxfmt breaks it at every top-level `||` (else `&&`), and a comparison that still does not fit after its operator. A range `for` whose declarators do not fit puts the second under the first.
- `xs[i as usize]` with `i: u32` printed a cast `Slice.at` does not need, which oxlint refuses.
- `[expr/cast]` says what `as` allows (a discriminant, `u8`/`u16`/`u32` to `usize`) and that no float converts to or from an integer, instead of advising `T::from(x)`; `f64::NAN` and other std paths are no longer called undefined functions.

- A `u128` const that folds past an `i128` (`(1 << 80) - 1` kept its low bits with `1i128 << 128`) crashed `check`; it folds, and one at 2^127 or more is refused with a message.
- `u128::from(u64)` printed `x as number as U128`; it is `x as bigint as U128`.
- `Slice.range` bounds, `Int.<t>.cast`'s argument, and a float passed to `Math` or `Int.f64.round` carried casts oxlint refuses as unnecessary; a literal or a length is read bare, and a float's `as F64` dropped.
- A one-sided `trim_start_matches` kept the other side's helper in the runtime, which tsc refuses as unread.
- Layouts oxfmt prints otherwise: a logical chain whose last operand opens a closure's block (as a `return`, a `let`, or an arrow's whole body), an arrow body under `=>` that breaks at `&&`, a last-argument closure whose body is a `?:`, `if (..) continue;`, and float casts as array elements.

### Documentation

- The authoring skill caught up with 0.13.0: `Vec::insert`, `x as usize` from `u8` / `u16` / `u32`, and `for _ in`; `Vec::remove` named as refused.
- The authoring skill says what floats take and that none converts to or from an integer.

### Examples

- **jsonpatch**: RFC 8259 text, RFC 6901 JSON Pointer, and RFC 6902 JSON Patch, written from the authoring skill alone; checked against RFC 6902 Appendix A, RFC 6901 §5, and an idiomatic reference over serde_json (design/07 §2).
- **geo**: Google's Encoded Polyline and Geohash on `f64`, written from the authoring skill alone; with no float rounding or conversion in the subset, it rounds exactly by subtracting powers of two.
- **ulid**: the ULID specification (base32, the 16-byte form, monotonic generation), written from the authoring skill alone; no hole, at 3.4× its idiomatic reference for want of narrowing and `u128`.
- **negotiate**: RFC 9110 §12 proactive negotiation, written from the authoring skill alone; its test cites RFC 9110 Erratum 7138.
- **raft**: the Raft paper's Figure 2 as a pure `step`, written from the authoring skill alone, with a simulation checking the four safety properties.
- **jsonpatch, geo, ulid, negotiate** rewritten with this release's additions against their unchanged tests: geo 2.2× → 1.6×, ulid 3.4× → 1.6×, negotiate 2.6× → 1.6×, jsonpatch 4.9× → 4.1× (design/07 §2.2).

### Rationale: what the five examples asked for

<!-- derived-from ./design/07-roadmap.md#22-line-counts-against-idiomatic-rust -->

Why: the five examples written from the authoring skill alone on 2026-10-11 met no rejection, but four stayed over 2× ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)), each for something the subset lacked rather than the domain. Every candidate they raised was taken at once, in a patch series, by decision, overriding the `remove` / index assignment and `split` lines of [design/07 §6](design/07-roadmap.md#6-not-doing) as 0.9.0 overrode growing a `Vec`.

| Item | Verified by |
| --- | --- |
| `v.remove(i)`, `v[i] = x`, `v[i] op= x` on a local `let mut` `Vec`; Rust's order of evaluation where both sides may panic | `vec_edit_equivalence.rs` |
| `x as T` between integer types `From` does not join, wrapping | `int_cast_equivalence.rs` (all 54 pairs), the widths generator |
| `u128` / `i128`; byte strings, as a `&[u8]` const or an expression | `wide_int_equivalence.rs`, `wire.rs`, `wire_write.rs` |
| Float methods, constants, and conversions, rounded once | `float_conv_equivalence.rs` (bit for bit) |
| `==` / `!=` on a derived `PartialEq`, part by part | `deep_eq_equivalence.rs` |
| `to_ascii_*case`, `trim*` by Rust's whitespace, `trim*_matches`, `find`, `split` with a closure | `str_more_equivalence.rs` |
| Stable `sort`, `sort_by`, `sort_by_key` on a local; `find`, `max` / `min` and their `_by_key` / `_by` forms | `sort_pick_equivalence.rs` |

- **Measurement.** Rewritten with them against unchanged tests: geo 2.2× → 1.6×, ulid 3.4× → 1.6×, negotiate 2.6× → 1.6×, jsonpatch 4.9× → 4.1× (the patch layer 3.5× → 2.8×, over the threshold for want of `&mut`, which stays out: [design/07 §3](design/07-roadmap.md#3-candidates)). [design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust) has the detail.
- **Holes met on the way**, each fixed with a fixture: a `u128` const crashed `check`'s folding; `u128::from(u64)` printed `as number`; casts on slice bounds, on `Int.<t>.cast`'s argument, and on a float passed to `Math` were unnecessary assertions to oxlint; a one-sided trim kept an unread helper; and seven layouts oxfmt prints otherwise (`fixtures/layout.rs`).
- **Stated gaps**, in [01 §3](design/01-equivalence.md#3-known-non-equivalences): a `u128` literal or const at 2^127 or more is refused (literals are `i128`s); a comparator that is not a total order may panic in Rust's sort and never in JS's.
- **Open:** the seeded generators draw the integer casts only; the rest is covered by the hand-written tests above.

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

The rest of what tsc refused in generated code ([rationale below](#rationale-the-rest-of-what-tsc-refused)): every generated function of seeds 1 to 120, in all five generators, now type-checks. Nothing on the stable surface changes; the examples' output is byte for byte the same.

### Fixed

- **`null ?? a`** where a `match` on `{ let t = None; t }` or on `Ok(r.value)` was left to run; it takes its arm.
- **Unreachable code** past a test decided in a `matches!` whose scrutinee is a call (`matches!(x.checked_mul(b), Some(_) if false)`), and past `v = x?` that always leaves.
- **A temporary TS typed from itself in a loop** (`const value = opt !== null ? 3 : null` read by its own loop's test): `o.map(f).is_some()` is folded into the `match` it tests.
- Narrowing follows what those print: `const v = r.kind === "Ok"` narrows `r` through `if (v)`, as TS does; a `match` used as a test narrows on both sides; code the fold thinks unreachable is still folded, as TS still checks it.
- A file whose folds print a `switch` imports `assertNever`.

### Rationale: the rest of what tsc refused

<!-- derived-from #rationale-narrowing-as-ts-does-it -->

Why: 0.10.2 left 17 of seeds 1 to 120 of generated bodies refused by tsc, none for narrowing ([0.10.2](#rationale-narrowing-as-ts-does-it)): `??` on a value TS knows is `null`, unreachable code, a temporary TS types from itself in a loop.

| Item | Verified by |
| --- | --- |
| `match { let t = None; t } { .. }` takes its arm, as does a `match` on `Ok(r.value)` (a payload that only reads) | `none_scrutinee`, `built_from_field` |
| A test that is a `match` every arm of which that may run gives one value runs its scrutinee and is decided; `v = x?` that always leaves ends its block | `decided_guard_runs`, `assigned_try_leaves` |
| `o.map(f).is_some()` is that `match` of `true` / `false` (through the `let`s binding its receiver, and a made binding read once as what such a test takes): no temporary for TS to type from itself | `mapped_test_in_loop` |
| What those print, TS narrows by, and so does the fold: a `match` used as a test on the side that fails, `const v = r.kind === "Ok"` read as the test it holds ("aliased conditions"), `if c { true } else { b }` as `c \|\| b` (not other `?:`) | `aliased_condition`, `if_test_as_or` |
| A side or what follows a statement the state says is never reached is still printed and checked: it runs from the state before | `never_taken_side`, `past_leaving_if` |
| `assertNever` is offered to every file's imports and pruned where unread (a fold may print a `switch` the IR did not hold) | the generated sweeps |

Seeds 1 to 120 of all five generators type-check and agree with Rust on every value (26,400 functions, 316,800 calls).

- **Open:** nothing tsc refuses in the sweeps. The fold mirrors how the printer prints a test; a printer change to tests needs the sweeps rerun (`scripts/gen-sweep.sh`, run locally; reduce a failing seed with `scripts/gen-reduce.py`; README, Generated sweeps).
- **The examples' output is unchanged** (`check --out` in `scripts/verify.sh`).

## 0.10.2 — 2026-10-04

Narrowing as TS's control flow does it ([rationale below](#rationale-narrowing-as-ts-does-it)). Nothing on the stable surface changes; the examples' output is byte for byte the same.

### Fixed

- **Output tsc refused where TS had narrowed a value and the printer had not**, none of it a wrong value: past an `if` / `else` whose sides each leave on `Err`, inside and past a loop, past `if c { return }`, inside `if matches!(o, Some(_) if g)`, `if c || c`, or `if t == ","`, after `let v = true`. The printer now follows each place's possible cases as TS does, joining them where control flow meets and finding a loop's head by running its body until they settle. Generated bodies that type-check, seeds 1 to 120: 60 -> 103; transitions and text: all.
- A test decided though part of it must run (`100 / a > 0 && c`, `c` known `false`) runs that part, then the side taken.
- An `if` run for one side's effect prints as a statement, not `c ? undefined : f()`.

### Rationale: narrowing as TS does it

<!-- derived-from #rationale-the-generator-across-the-subset -->

Why: 0.10.1 left 60 of seeds 1 to 120 of generated bodies refused by tsc, 47 of them only where TS narrows and the printer did not fold ([0.10.1](#rationale-the-generator-across-the-subset)). The fold kept a stack of what enclosing arms, `?`s, and jumps had decided, with no state where control flow joins or at a loop's head, and dropped a fact for the rest of a block wherever anything later wrote the place.

| Item | Verified by |
| --- | --- |
| `join/flow.rs`: each place's possible cases flow forward, refined on each side of a test and per `match` arm (less what earlier unguarded arms took), joined where control flow meets, found at a loop's head by running the body until the state settles, forgotten at a write, kept in a closure for names nothing writes. A side a known case contradicts is never taken | `narrowing_equivalence.rs`: past jumping arms and leaving `if`s, through loops, `joined_sides` |
| Facts from `?` statements (not `let x = p?`, which tests a copy), `is_some`, `matches!` (a `match` whose other arms give `false`), shared sides of `\|\|`, `bool` places, `let v = true`, and a place compared with a literal (`t == ","`) | `decided_matches`, `decided_or`, `decided_literal_binding`, `decided_string` |
| Folds: `p?` and `let x = p?` of a known case or a built `Ok(e)`; a decided test whose part must still run (`100 / a > 0 && c`) runs it, then the side taken; `&&` / `\|\|` with a decided left side; `known_bool` of `&&`, `\|\|`, `if`, and a `match` of `bool`s where nothing in them can panic | `decided_try`, `decided_and_runs_left` (panicking for `a == 0` as Rust does), `decided_left` |
| A loop's head that does not settle within six rounds is what enters, less every place under a name the loop writes; states compare in one order, so settling is seen | `late_write` |
| Narrowing reruns after the cleanup until it settles; a loop's body runs for its effects; a `while true` nothing breaks out of ends its block; an `if` run for one side's effect prints as a statement | the generated sweeps; `scripts/verify.sh` lints the fixtures' output |

Seeds 1 to 120, values agreeing in all five generators: bodies type-check on 103 (60 before), transitions and text on all (118 and 119 before); none is refused for narrowing alone.

- **Open:** 17 seeds of bodies, refused for `??` on a value TS knows is `null` (TS2871, TS2869; 9), unreachable code (TS7027; 6), a temporary TS types from itself in a loop (TS7022; 3). None is narrowing as TS does it; each wants a fold or an annotation of its own. A side or arm the state says is never taken is printed as it stands, while TS checks it with the place as `never`; much of the `??` class is likely there (an `unwrap_or` on a known `None` or `Some` is one fold away).
- **The examples' output is unchanged** (`check --out` in `scripts/verify.sh`).

## 0.10.1 — 2026-10-04

Tests generated across the subset, and what they found ([rationale below](#rationale-the-generator-across-the-subset)). Nothing on the stable surface changes; the examples' output is byte for byte the same.

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

### Rationale: the generator across the subset

<!-- derived-from #rationale-tests-generated-from-seeds-and-what-they-asked-for -->

Why: the generator of 0.10.0 drew expressions only, over `i32`, `bool`, `Option<i32>`, and `Result<i32, i32>`. Two of the four disagreements the audits found were in statements and crate types, outside it. Four generators now cover the rest of the subset; each runs eight seeds on every `cargo test`, and seeds 1 to 120 were swept.

| Generator | What it draws | Seeds 1 to 120 |
| --- | --- | --- |
| `generated_equivalence.rs` (0.10.0) | expressions over four types | values agree; 120 type-check (7,200 functions) |
| `generated_statements_equivalence.rs` | bodies: `let` / `let mut`, assignment and `op=`, `if` / `match` statements, range `for`, `while`, `break`, `continue`, early `return`, `?` in a range's ends and a loop's test | values agree; 60 type-check (4,800 functions) |
| `generated_patterns_equivalence.rs` | transitions over a crate's `State` / `Event` / `Acc`: `match` on a pair or one value, `if let` on an `Option`, `matches!`; variants, tuple and struct fields, nested `Option`s, literals, ranges, `\|`, names that hide parameters, guards | values agree; 118 type-check |
| `generated_widths_equivalence.rs` | `u8`, `i16`, `u32`, `i64`, `u64`: operators, shifts past the width, checked / wrapping / saturating forms, `pow`, `abs`, `T::from`, comparisons | values agree; 120 type-check |
| `generated_text_equivalence.rs` | `&str`, `String`, `Vec<i32>`: the allowed methods, slices off char boundaries, `split`, `parse`, adaptors into consumers and `collect`, `push` | values agree; 119 type-check |

What they found, each with a fixture that fails before its fix:

| Found | Kind | Fixture |
| --- | --- | --- |
| `match o.ok_or(x.ok_or(e)?)` returned from an inline function only and took the `Err` arm (Rust `Err(e)`, TS `Ok(..)`) | wrong value, in 0.10.0 | `order_of_eval.rs` |
| `if let Some(0) = o { .. } else { .. }` threw "unexpected variant" for `Some(5)`, where Rust takes the `else` | wrong result, in 0.10.0 | `control.rs` |
| `Set(Some(_)) if g => .., Set(v) => ..` overflowed the printer's stack (`let v = v` followed as an alias forever), in `check` and `build` | crash, in 0.10.0 | `guards.rs` |
| A group broken inside (`=> (c ? .. : ..)`) ended with `,` (`(x,)`) | syntax error | `tidy.rs` unit test |
| Two temporaries named after one local (`xResult` twice in a block) | syntax error | `order_of_eval.rs` |
| A payload's `match` (`matches!(o.unwrap_or(3), 0..=9)` under `Some(_)`) decided by the option being `Some` | wrong value, a regression of this work before release | `narrowing.rs` |
| What folding leaves: unread values (`const optOr`, empty `if {} else {}`), unreachable code, `r?` on a known `Err`, places past jumping arms and leaving `if`s, decided `bool`s, `c ? r : r`, a parameter read only by an unread `as_str` | refused by tsc | `narrowing.rs` |
| The same folds' leftovers lint refuses: an unread comparison as a statement, `if (true)`, a block declaring nothing, `Result` imported as a value it is not used as | refused by oxlint (`scripts/verify.sh`) | `narrowing.rs`, `order_of_eval.rs`, an `imports.rs` unit test |

- A sweep compares values where tsc refuses the package (`PURECRATE_GEN_TYPES=report`, only with `PURECRATE_GEN_SEED`); every run's seeds still type-check (`support::assert_values_equivalent`).
- **Open:** the seeds tsc refuses. Bodies: 47 of 60 only for narrowing TS does where control flow joins or loops ([design/07 §3](design/07-roadmap.md#3-candidates), narrowing as TS does it); 13 also or only for `??` on a value TS knows is `null` (TS2871, TS2869), unreachable code (TS7027), a temporary whose type TS infers in a loop from itself (TS7022), or an unread one (TS6133). Patterns 2 and text 1, narrowing too (TS2322, TS2339, TS2367). `?` in `matches!`'s first argument inside a test is refused (`[check/position]`) although that operand always runs; safe, and written around with a `let`. `if let` takes `Option` and `Result` only (`[pattern/if-let-variant]`).
- **The examples' output is unchanged.** Fixtures' output changed only where a decided `bool` or test folded (`guards.rs`, `bool_patterns.rs`).

## 0.10.0 — 2026-10-04

Tests generated from seeds, and what they found; parentheses decided on a tree of the printed operators; rustfmt and clippy in CI. A minor release: the runtime's `Result.ok` and `Result.err` signatures change, which the stable surface lists ([rationale below](#rationale-tests-generated-from-seeds-and-what-they-asked-for)). Regenerate committed output: each copied runtime changes by two lines.

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

### Rationale: tests generated from seeds, and what they asked for

<!-- derived-from #rationale-fixes-from-an-audit-of-090 -->

Why: an evaluation of 0.9.1 found that every audit since 0.8.1 turned up output that disagreed with Rust, each only in a shape no fixture spelled, and that the printer decided parentheses by reading its own text back. Hand-written cases could not keep up; generated ones can. A minor, because the runtime's signatures change ([design/07 §9](design/07-roadmap.md#9-generated-api-stability)).

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

## 0.9.1 — 2026-10-04

Fixes from an audit of 0.9.0: four outputs that disagreed with Rust, two shapes oxlint refused, and documents that lagged the subset. No change to what is accepted, but `ok()` of a `Result` holding an `Option`, which is now refused ([rationale below](#rationale-fixes-from-an-audit-of-090)).

### Fixed

- **A choice under `is_some`, `===`, or a member is parenthesized.** `r.ok().is_some()` printed `r.kind === "Ok" ? r.value : null !== null ? ..`, which JS reads with the comparison inside the else branch: `Ok(0)` gave 0 where Rust gives 1. Likewise `o.map(f).is_none()`, `!o.map(f).is_some()`, `o.is_some() == p.map(f).is_some()`, and `o.unwrap_or(vec![]).len()`.
- **`ok()` of a `Result<Option<T>, E>` is refused** (`[check/nested-option]`): both `Ok(None)` and `Err(_)` were `null`.
- **A struct pattern's guard leaves names its own bindings hide.** `Q { a, xs } if xs.iter().any(|a| *a > 3)` tested `q.a > 3` inside the closure.
- **A `//` comment stays one line in JS.** A lone CR, U+2028, or U+2029 in a carried comment ended the line for JS, and the rest ran as code; each is a space now.
- **`else if` for an `else` that is one `if`** (oxlint's `no-lonely-if`), **and no `c ? true : false`** (`no-unneeded-ternary`). An equality inside an equality is parenthesized, as oxfmt prints it.
- **Documents:** the skill and 02's tables no longer call `cmp` on `Vec`s, `map` / `filter`, or chained `chars()` unavailable; 01, the overview, and 03's counter sample match the output; a refused method's message lists the 0.9.0 methods.

### Rationale: fixes from an audit of 0.9.0

<!-- derived-from #rationale-lists-built-in-a-function-and-the-examples-against-their-specifications -->

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

## 0.9.0 — 2026-10-04

Lists built in a function, iterator stages, struct patterns, and `clone`; the examples checked against their specifications, with the generator bugs that rewriting them found. The accepted subset grows ([rationale below](#rationale-lists-built-in-a-function-and-the-examples-against-their-specifications)).

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

### Rationale: lists built in a function, and the examples against their specifications

<!-- derived-from #rationale-the-rest-of-the-audit-and-layout -->

Why: a review of the examples against their specifications (RFC 4226/6238, ISO 13616, SemVer 2.0.0, Stripe's PaymentIntent) found defects, and rewriting them found generator bugs; the authoring gaps it listed (a `Vec` grown in a function, adaptors, struct patterns, `clone`) were taken without [design/07 §1](design/07-roadmap.md#1-how-additions-are-chosen)'s threshold being met ([design/07 §3](design/07-roadmap.md#3-candidates)).

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

## 0.8.2 — 2026-10-04

The rest of the audit's fixes, output that reads more as written by hand, and a layout that matches oxfmt for long names outside the examples. No change to what is accepted ([rationale below](#rationale-the-rest-of-the-audit-and-layout)).

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

### Rationale: the rest of the audit, and layout

<!-- derived-from #rationale-fixes-from-an-audit -->

Why: the four defects 0.8.1 left open ([0.8.1](#rationale-fixes-from-an-audit)), readability items found reading the output beside its Rust, and the audit's finding that the layout rules were fitted to the examples' lines. No change to what is accepted.

| Item | Verified by |
| --- | --- |
| A `for` range's bounds in order; an operand `?` that typing made a `match` leaves the function; a reassigned scrutinee read before the assignment | `order_of_eval_equivalence.rs` |
| Two arms binding one name to different fields | `guards_equivalence.rs` |
| Destructured fields, a flipped negated test, the matched place returned, a variant rebuilt from its fields as the place | `readability_equivalence.rs` |
| Layout: width in columns, a lone import, a break after `=`, an arrow's one-field object, a `?:` chain's test, a hugged last arrow, the wire module's generic type | `fixtures/layout.rs` through `oxfmt --check` in `scripts/verify.sh`; `line_width.rs` |

- **The layout check has an oracle outside the examples now.** `fixtures/layout.rs` gives every construct a name long enough to break it; the shapes it found differing from oxfmt were the ones fixed here. `tidy::wrap` is still a chain of line rules, not oxfmt's printer (oxc's formatter is not published as a crate), so a shape the fixture lacks may still differ; the fixture is where to add it.
- **A block in an argument is lifted** before its statement (`let value;` then `if`), which also removes an inline function from the output; the lifted `let` has no annotation, which TS infers from its assignments.

## 0.8.1 — 2026-10-03

An audit of the generator for rules fitted to the examples' shapes found output that Rust does not mean; this release fixes those that 0.8.0 printed for inputs outside the examples. No change to what is accepted ([rationale below](#rationale-fixes-from-an-audit)).

### Fixed

- **`??` beside `?:`, `||`, or `&&` is parenthesized.** `match o { Some(v) => v, None => if c { 1 } else { 2 } }` printed `o ?? c ? 1 : 2`, which JS reads as `(o ?? c) ? 1 : 2`: `Some(5)` gave `1`. `o ?? a || b` and `c || o ?? d` were syntax errors. `??` now has its own precedence, below `||` and parenthesized beside it.
- **A comment above an operand no longer drops its parentheses.** Since 0.8.0, `c && { // .. \n a || b }` printed `c && a || b`. A comment that is not above a statement is left out before printing.
- **An empty arm where the value is returned returns.** `M::A => match s { "" => {}, _ => .. }` in a unit function merged the empty arm into the next test, and `case "A"` ran into `case "B"`: a panic Rust does not have. The merge is kept only where nothing is handed on.
- **No `return` after a statement that never falls through.** A `match` whose arms all return, then `;`, and a `while true` no `break` leaves, were followed by `return undefined;`, which the generated tsconfig rejects as unreachable.
- **A tuple read in part is not taken for the whole.** 0.8.0's `Some((a, b)) => (a, Some(b))` → `x ?? d` matched the printed text, so `Some((a, b, _)) => (a, b)` became `x ?? [0, 0]`, the three-element tuple. The rewrite is withdrawn; semver's `parse` binds `option` again.

### Rationale: fixes from an audit

<!-- derived-from #rationale-generated-typescript-that-reads-like-the-source -->

Why: reading the generator for rules written to the examples' shapes, with each suspected input built and run. No change to what is accepted.

| Item | Verified by |
| --- | --- |
| `??` beside `?:`, `\|\|`, `&&` parenthesized; a commented operand keeps its grouping; a tuple read in part not taken for the whole | `precedence_equivalence.rs` |
| An empty arm under a returned value returns; no `return` after what never falls through | `empty_arm_equivalence.rs` |

- **Open at release**, each reproduced and fixed after it ([CHANGELOG](CHANGELOG.md), Unreleased): a `?` in a `for` range's end ran before a start that can panic; a `?` inside `map_err(..)?` or an `unwrap_or` argument in an operand position stayed inside an inline function; two arms binding one name to different fields declared it twice; a `match` arm that reassigns its scrutinee read the new value. tsc rejected all but the first.
- **Layout.** `tidy::wrap` is a chain of line rules fitted to the examples; outside them its breaks often differ from oxfmt's, and it measures width in bytes.

## 0.8.0 — 2026-10-03

Generated TypeScript that reads closer to the Rust it came from: no `$` in any name, flat guards instead of `else` chains, the source's `//` comments kept, bodies in paragraphs, byte literals that name their character, and fewer temporaries; a runtime copy that keeps only what the package uses; and the last four patterns 0.7.0 refused (`A | B(1)`, `Some((1, b))`, `.map(PreId::Numeric)`, `str::parse` into an integer). Packages generated with 0.7.0 must be regenerated, and callers that read a one-field tuple variant updated (Breaking, below) ([rationale below](#rationale-generated-typescript-that-reads-like-the-source)).

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

### Rationale: generated TypeScript that reads like the source

<!-- derived-from #rationale-lists-from-text -->

Why: the four patterns 0.7.0 still refused, and an exception to taking language capabilities first, as in 0.6.0 ([0.6.0](#rationale-generated-typescript)): reading the generated packages beside their Rust, what a reviewer tripped on was the machine in the output (`$`-names, `else` chains, temporaries a combinator left), what the source said and the output dropped (`//` comments, `b'@'` printed as `64`), and a runtime copy of 36 KB whatever the package used. The naming changes reach the stable surface ([design/07 §9](design/07-roadmap.md#9-generated-api-stability)), so this is a minor.

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
- **Breaking** for callers: `.content[0]` on a one-field variant is `.value`, and the index exports `Int` only when the public surface holds a number ([CHANGELOG](CHANGELOG.md)).

## 0.7.0 — 2026-10-02

`split_once`, and a `Vec` collected once from `s.split(c)`, which is what kept semver over twice the idiomatic Rust after ordering; patterns nested in a case; and two rounds of review fixes (panics as `Panic`, `Mutex` refused, generated-code layout, CI and release hardening). Packages generated with 0.6.0 must be regenerated (Breaking, below) ([rationale below](#rationale-lists-from-text)).

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

### Rationale: lists from text

<!-- derived-from ./design/07-roadmap.md#22-line-counts-against-idiomatic-rust -->

Why: after `Ordering`, semver was still the example over 2×, and what remained was parsing ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)): `split_once`, and lists built with `collect`. A `Vec` whose length is the input's, read once from text, is not a sequence that grows with the state, which is why `collect` stays out everywhere else ([design/07 §6](design/07-roadmap.md#6-not-doing)).

| Item | Verified by |
| --- | --- |
| `s.split(c).collect()` and `s.split(c).map(f).collect()` into `Vec<T>` or `Result<Vec<T>, E>`, the target named by a turbofish, a `let` type, or the return type | `collect_equivalence.rs` |
| A `Result` stops at the first `Err`, so a later piece that would panic does not run | `collect_equivalence.rs` |
| `str::split_once` with a `char` or a `&str`, including an empty needle | `collect_equivalence.rs` |
| `Some((a, b))`, and the same tuple of names in `Ok` / `Err` or a variant's fields, including under a guard | `tuple_match.rs`, `collect_equivalence.rs` |
| A pattern nested in a case: a literal, a range, a variant, or `Some` / `Ok` / `Err` in a variant's field or a payload, to any depth (`PasswordChecked { verified: false, .. }`, `Some(Event::Pay { amount })`) | `nested_patterns_equivalence.rs`, `tuple_match.rs` |

- **What is refused.** Collecting a `Vec`, `chars()`, or anything but `split(c)` with a `char`; `collect` with no target type; `split` on a `&str` (an empty separator differs in JS); a variant constructor passed as `.map(PreId::Numeric)` (built with no fields, or, for a unit variant, not a closure or a function name). A tuple holding anything but names and `_` is still `[pattern/nested]`, as is a tuple nested in a tuple pattern and a side of `|` that tests inside its variant (`A | B(1)`).
- **Nested patterns** go through the decision tree that tuple `match`es and guards already use ([03 §3.3.1](design/03-output.md#331-tuple-match)): where a case is chosen, a field some arm tests becomes one more element, matched further in. rustc checks exhaustiveness, so `check` no longer counts cases for such a `match`; every `switch` the tree prints still names every case. oidc's lockout arm reads `PasswordChecked { verified: false, .. }` as its idiomatic code does, which changes no line count: a guard `if !verified` was one line too, and moving `second_factor: SecondFactor::Totp(..)` into the tuple arm would repeat the arm's head. The evidence for them was readability, not the threshold. A `bool` arm now prints as `x` / `!x`, not `x === true`.
- **`str::parse` stays out.** Rust's `u64` parse accepts a leading `+`. Matching that, and the cases it rejects, is not what brings semver under the threshold.
- **Measurement.** semver rewritten with them: 166 → 138 lines, 2.2× → 1.8× ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)). Pre-release and build identifiers are `Vec`s. The generated `parse` prints `split` as the array, `Iter.tryCollect` for a `Result`, and `Str.splitOnce`; the turbofish is not a second binding. `Some((x, y))` reads the two strings as `[0]` and `[1]`.

## 0.6.0 — 2026-10-01

Generated TypeScript that reads as written by hand: camelCase names, JSDoc from `///`, runtime helpers in place of inline functions, and lines broken at 100 characters. The wire module gives a form only to what serde does, reads text with `fromJson`, and reports a refused value in serde's words. Packages generated with 0.5.0 must be regenerated, and their callers updated (Breaking, below). Measured by reading the examples' output ([rationale below](#rationale-generated-typescript)).

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

### Rationale: generated TypeScript

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
- **Breaking** for callers: the names, the index's exports, and `--schema` on a crate with no serde derive ([CHANGELOG](CHANGELOG.md)).

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

### Rationale: ordering

Why: semver, written from the authoring skill alone, was the first example over 2× after rustfmt normalization, and its ordering was most of the gap ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)): 57 lines against 25, with `std::cmp::Ordering`, `cmp`, and `String` order all refused.

| Item | Verified by |
| --- | --- |
| `std::cmp::Ordering` as a fieldless enum, named through `use std::cmp::Ordering;` or the full path | `ordering_equivalence.rs`, `ordering.rs` in `check` |
| `cmp` on integers, `char`, `bool`, `String` / `&str`, `Uuid` | `ordering_equivalence.rs` |
| `<` `<=` `>` `>=` on strings, by code point | `ordering_equivalence.rs` |
| `is_eq` … `is_ge`, `reverse`, `then`, `then_with`; `==` on `Ordering` | `ordering_equivalence.rs` |

- **Representation and rewrites** are in [01 §6.6](design/01-equivalence.md#66-stdcmpordering) and [§7.10](design/01-equivalence.md#710-cmp-and-orderings-methods). The one new runtime part is `Str.cmp`, which a package carries only when it compares strings.
- **Refused:** `impl Ord` / `PartialOrd`, `cmp` on floats, tuples, `Vec`, `Option`, and the crate's types, and `Ordering` as a field (serde has no form for it).
- **Measurement.** semver rewritten with it: 195 → 166 lines, 2.6× → 2.2× by the script, 1.6× adjusted ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)). The skill-only draft's 2.8× was the first draft over the threshold since normalization; like the first drafts before it, it came under once its gap's capability was in, on the adjusted count. By the script it stayed over (2.2×) until parsing ([0.7.0](#rationale-lists-from-text)).

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

### Rationale: smaller output

From evaluating 0.4.0 ([bench/payment](bench/payment/README.md)). No change to what is accepted.

- Guards compile into the tuple decision tree: payment's generated code 47.9 → 23.5 KB, oidc's 64.4 → 48.8 KB, back to their sizes before the 0.4.0 rewrites.
- Each package's runtime keeps only what its code uses: payment's bundle 3.1 → 2.3 KB gzipped; first call 1.18 → 0.52 ms with the regular expression built lazily.
- Fixed-seed random cases join the differential tests ([01 §8](design/01-equivalence.md#8-verification)).

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

### Rationale: iteration, part two

<!-- derived-from ./design/90-acceptance-survey.md#7-re-measured-with-030-2026-09-30 -->

Why: oidc's lexical helpers were still 2.2× idiomatic Rust ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)), and in the corpus re-measured with 0.3.0 ([90 §7](design/90-acceptance-survey.md#7-re-measured-with-030-2026-09-30)) the leading causes were `chars()` used as a value and slicing ranges. The corpus shows where idiomatic code goes, not what to accept ([design/07 §6](design/07-roadmap.md#6-not-doing)); each item was to be kept only if rewriting oidc, payment, or invoice used it.

| Item | Verified by |
| --- | --- |
| Iterator consumers that yield a scalar: `all`, `any`, `count`, `position`, `sum`; `for (i, x) in xs.iter().enumerate()` | `consumers_equivalence.rs` |
| Byte slicing `&s[a..b]`, `&s[a..]`, `&s[..b]` and on slices; `strip_prefix` / `strip_suffix` | `slicing_equivalence.rs` |
| Tuple destructuring in `let`, closure parameters, `for` | `destructure_equivalence.rs` |
| `const` inside functions | `local_consts_equivalence.rs` |
| Integer helpers `min`, `max`, `abs`, `pow`, `checked_*`, `saturating_*`, `wrapping_*` | `int_methods_equivalence.rs` |
| A consumer after `s.split(c)`; `bool` literal patterns | `consumers_equivalence.rs`, `bool_patterns_equivalence.rs` |

- **Consumers** on `s.chars()`, `s.bytes()`, and `xs.iter()`, with a function name as the predicate as well. None builds an array, so the reason iterator `map`/`filter`/`collect` stay out ([design/07 §6](design/07-roadmap.md#6-not-doing)) does not apply.
- **Slicing** is specified in [01 §6.1](design/01-equivalence.md#61-strings): positions are UTF-8 bytes, and a position off a char boundary or past the end throws with Rust's message, the character escaped as `Debug` escapes it.
- **`const` in a block** is a typed `let` at the top of the block instead of folded ([01 §7](design/01-equivalence.md#7-rewritten-constructs); the value is the same, since rustc rejects one that overflows). Associated consts wait for a use ([design/07 §3](design/07-roadmap.md#3-candidates)).
- **Integer helpers** against Rust's debug-build result, on every integer type, `usize` in Rust's 64 bits (payment's idiomatic code calls `min`).
- **A consumer after `split`, `bool` patterns.** Both were refused in the rewrites: oidc's `has_token` is now `list.split(' ').any(|t| t == word)`, and its `step` matches `(true, None, _)`.
- **Measurement.** oidc, payment, and invoice rewritten and measured again ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)). Guards, tuple `match`, the `Option` methods, `all` / `any`, `min`, and `pow` carried the reduction; the other items went unused by these three. Kept anyway: each is implemented and tested, preserves meaning exactly, and answers a leading cause in the corpus (slicing ranges and `chars()` as a value); [design/07 §1](design/07-roadmap.md#1-how-additions-are-chosen) still decides what comes next.

## 0.3.0 — 2026-09-30

Match guards and `Option` combinators, the gaps invoice and payment measured against idiomatic Rust ([design/07 §8](design/07-roadmap.md#8-releases)), and more readable wire schemas.

### Added

- Match guards (`p if c =>`), also on arms that bind and in `matches!`. A guarded arm is tried in order and does not count toward exhaustiveness, as in rustc; the output is an `if` chain of standalone matches.
- `Option::unwrap_or`, `ok_or`, and `map` with a closure or a function name. `unwrap_or(e)` and `ok_or(e)` evaluate `e` first, as Rust does; `map` refuses a closure containing `?` or `return`.
- `survey --all-causes` lists every cause that keeps an item out, not only the first, to estimate a rewrite. The default output is unchanged.

### Changed

- **Breaking:** the zod adapter and the generated zod schemas target zod 4.6 (peer `^4.6.0`). Users on zod 3 stay on purecrate-ts 0.2.
- zod and valibot schemas are printed dependencies first, with `lazy` only for types in a cycle, and long unions and objects one arm or field per line. A refused `try_from` names the error's variant in all three libraries. arktype reports a bad nested field at its path instead of one error at the type. What each schema reads or refuses is unchanged. Output committed with 0.2.0 and checked with `check --out` must be regenerated.
- Dependencies are crates.io requirements, vendored by source replacement (`scripts/vendor.sh`); syn and proc-macro2 move to their latest 2.x and 1.x.

### Rationale: guards and `Option`

| Item | Why | Verified by |
| --- | --- | --- |
| Match guards | invoice reached for them three times, the idiomatic payment uses two ([design/07 §2.4](design/07-roadmap.md#24-what-the-measurements-changed)) | `guards_equivalence.rs` |
| `Option::unwrap_or`, `ok_or`, `map` | most of payment's remaining gap ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)) | `option_methods_equivalence.rs` |
| `survey --all-causes` | every cause per function, to estimate a rewrite ([91 §4](design/91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)) | `survey.rs` |
| Wire schemas in dependency order | on request, not from an example's gap | `wire.rs`, `wire_write.rs` |
| zod 4.6 | the adapter's peer | `wire.rs` |
| arktype errors keep their location | — | `errors_keep_their_location` in `wire_write.rs` |

- **Match guards.** A guarded arm is tried in order and does not count toward exhaustiveness, in the TS check as in rustc's. Also on binding arms and in `matches!`. The printing changed twice: arms after a guard first sat inside the `_` of a `switch` on the same value, which TS narrowed so that the later `switch` could not name the other variants; then each arm became a standalone `match` in an `if` chain, which after the 0.4.0 rewrites made payment's `step.ts` 31.6 KB. Since 0.4.1 guards sit at the leaves of the tuple decision tree ([01 §7.2](design/01-equivalence.md#72-match-guards)).
- **`Option` methods.** `unwrap_or(e)` evaluates `e` first, as Rust does. `map` also takes a function name.
- **`survey --all-causes`.** An expression or statement the parser cannot take is recorded and stood in for, so the rest of the item is still lowered and type-checked. The default output is unchanged.
- **Wire schemas.** zod and valibot print dependencies first with `lazy` only for recursive types, one arm or field per line when long, through the adapters' `unitVariant` and `optionalField`. A refused `try_from` names the error's variant in all three libraries. `toJson` writes a fieldless enum in one line. Behavior unchanged; mutually recursive and later-declared types added to the tests.
- **zod 4.6.** Peer `^4.6.0`; `z.ZodType<T, In>` without `ZodTypeDef`, issues with `code: "custom"`. A breaking change for users on zod 3, who stay on purecrate-ts 0.2.
- **arktype errors.** Nested errors stay at their path instead of being replaced by one at the type.

## 0.2.0 — 2026-09-30

Iteration, the largest gap the examples measured against idiomatic Rust ([design/07 §8](design/07-roadmap.md#8-releases)).

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

### Rationale: iteration

Why: the largest gap measured so far. oidc's lexical helpers were at 3.2× idiomatic Rust ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)), and Windmill's scope and pipeline parsers ([91 §2](design/91-real-use-candidates.md#2-fit-measured)) were all index bookkeeping over bytes and tokens that idiomatic code writes as iteration.

| Item | Verified by |
| --- | --- |
| `for` over a `Vec`, slice, or bytes | `for_each_equivalence.rs` |
| `while`, `break`, `continue` | `while_break_equivalence.rs` |
| `for t in s.split(c)` with a `char` separator | `for_each_equivalence.rs` |

- **`for` over a `Vec`, slice, or bytes.** `for x in &xs` and `for x in xs.iter()` over a `Vec` or slice, `for b in s.bytes()`; also `for x in xs` on an owned `Vec` and `s.as_bytes()`. Printed as `for..of`, one binding, the body rules of range `for`. The named adaptors are refused with a message.
- **`while`, `break`, `continue`.** No labels, no `loop` with a value. Every loop a jump leaves gets a label, since a bare JS `break` in the `switch` a `match` prints as leaves the `switch` (a run without labels fails the test). A `?` in the condition turns the loop into `while (true)` that computes the condition first. The generated tsconfig also sets `allowUnreachableCode: false`, and no `break;` follows a jump in a `case`.
- **`for t in s.split(c)`.** Added on oidc's evidence ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)). Any code point splits alike in UTF-8 and UTF-16. A `&str` separator is refused, since an empty one does not.
- **oidc's scanners rewritten with them**: the lexical helpers went from 99 to 68 lines ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)).

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

### Rationale: the first release

<!-- derived-from ./design/91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts -->

The subset of [02](design/02-authoring.md) as the examples of [design/07 §2.1](design/07-roadmap.md#21-what-each-example-hit) shaped it, plus what the examples and the first real use asked for before release.

**From the examples:**

| Capability | Asked for by | Verified by |
| --- | --- | --- |
| std methods one at a time: `Vec::is_empty`, `Option::is_some` / `is_none` | three of four real sites, and oidc | `std_methods_equivalence.rs` |
| Tuple scrutinees, `match (state, event)` | one function per state kept payment over the threshold after `_` | `tuple_match_equivalence.rs` |
| Bitwise operators and shifts on the fixed-width integers (not `usize` or `bool`) | oidc and Stoat's permission flags (both worked around); any spec written in them | `bits_equivalence.rs` |
| `vec![a, b]` of a fixed length | oidc's `amr`, a list claim that TS callers expect as an array | `vec_build_equivalence.rs` |
| Crate-level `const` and enum discriminants | oidc (`const`) and Stoat (18 permission bits as functions, `as` on the enum) | `flags_equivalence.rs` (a permission set in Stoat's shape) |

- **Tuple scrutinees.** Tuple arms of `_`, bindings, and arm patterns; `|` of whole tuples binding nothing; not nested tuples. Printed as nested `switch`es, one per element, so TS checks exhaustiveness too rather than trusting rustc's. The cost is copies of arms that several cases reach.
- **`vec![a, b]`** and `vec![]` print as array literals; `vec![x; n]` stays rejected ([design/07 §6](design/07-roadmap.md#6-not-doing)).
- **`const` and discriminants.** Crate-level `const` of integers, floats, `bool`, `char`, `&str`, folded at check time into `consts.ts`. Discriminants on fieldless enums with `#[repr(<int>)]`, and `e as T` when `T` holds every discriminant. Found on the way: with consts accepted, `Some(MAX) =>` would bind `MAX` rather than compare with it, so a const used as a pattern is refused; and one file per const collided with the usual `MAX_LEN` / `fn max_len` pair, so all consts share `consts.ts`.

**From measuring real dual implementations** ([91 §4](design/91-real-use-candidates.md#4-what-the-measurement-asked-of-purecrate-ts)). The first real use (Oxide `Name`) passes without these, but its rewrite was longer and less idiomatic for their absence.

| Capability | Verified by |
| --- | --- |
| `Result<Self, Self::Error>` in `impl TryFrom<T>` | — |
| `for c in s.chars()` (not iterator `for` in general), code points as `char`; replaces hand-decoded UTF-8 | `for_chars_equivalence.rs` |
| `uuid::Uuid` | `uuid_equivalence.rs` |

- **`TryFrom`.** `check` and `build` already accepted it. `survey` skipped trait impls, so a `#[serde(try_from)]` type looked as if its `impl TryFrom` were missing (`item/serde-attr`); it now reads the impl as `X::try_from`.
- **`uuid::Uuid`.** Added on one example's evidence and on how common UUID identifiers are in Rust domain code. Oxide `Name` hand-wrote `Uuid::parse_str` for the two forms its earlier checks let through, a copy that would drift if the check order or the crate changed. A deliberate exception to waiting for a second example ([01 §6](design/01-equivalence.md#6-strings-char-usize-std-methods)).

**Defects found by real use.** From the same measurement:

| Defect | Fix |
| --- | --- |
| a non-`pub` method reachable on the exported companion, a hole in closed types ([01 §4](design/01-equivalence.md#4-closed-types)) | emitted as `T$name`, off the companion |
| `mod r#impl;` looked up as `r#impl.rs` (skipped by `survey`, an error in `check`) | fixed |
| `build --out` emptying a directory it had not written | now refused |

From the Oxide `Name` vendoring ([91 §5](design/91-real-use-candidates.md#5-oxide-name-done-locally)):

| Problem | Change | Verified by |
| --- | --- | --- |
| the runtime | copied into packages: first as `--bundle-runtime`, then for every package, with string-keyed brands so packages still exchange values | — |
| `assertNever`'s unused parameter | fixed | — |
| bindings the Rust leaves unused | the generated `tsconfig.json` sets `noUnusedLocals` and `noUnusedParameters`, so every differential test type-checks under them; fixed what that turned up: unused `let`s, arm and `if let` bindings, write-only `let mut`s, parameters, `for` variables, the `?` temporary of `x?;`, and imports gathered from the IR but not printed | `unused_equivalence.rs` |
| a consumer's CI had no published `purecrate-ts` to run `check --out` with | each release tag carries Linux and macOS binaries, and `cargo install --git … --tag` builds the same version | — |
