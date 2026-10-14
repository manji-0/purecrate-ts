# Writing Rust within the constraints

Status: current (2026-10-15, 0.21.0)

<!-- constrained-by ./01-equivalence.md -->

## 1. What "constraint" costs

The constraints exist so that everything accepted can be translated with its meaning intact. Their cost is described in three layers, and only the first counts as a real loss.

| Layer | Meaning | Example |
| --- | --- | --- |
| Capability | The behavior cannot be written in any accepted form | Cannot read a list's elements |
| Notation | The behavior can be written, only longer | `match` instead of `.map` |
| Annotation | A type rustc would infer must be written | `1i32` |

The goal is **no capability loss** for pure transitions with ADTs, exhaustive matching, `Result` / `Option`, and debug integer semantics. Notation loss is tracked by line count against idiomatic Rust ([06 §4](./06-strategy.md#4-success-and-withdrawal-criteria)).

## 2. What can be written

| Capability | Written as | Generated TS |
| --- | --- | --- |
| Closed ADTs | struct, enum, newtype (content not `Option`, `()`, `!`) | `Readonly` objects, `kind` unions, brands |
| Exhaustiveness | `match` on one enum: arms naming a variant, `A \| B` binding nothing, and a last `_` | `switch` listing every case + `assertNever` |
| Guards | `p if c =>` on any arm, a binding arm (`n if n > 3`) included, and in `matches!`; a guarded arm does not count toward exhaustiveness | a decision tree: each value tested once, the guard at its arm's leaf |
| Transition tables | `match (state, event)`: tuple arms whose elements are `_`, a binding, or an arm pattern | nested `switch`es, one per element, each listing every case + `assertNever` |
| Slice patterns | `[]`, `[x]`, `[x, .., y]`, `[x, rest @ ..]`, `[rest @ .., x]` on a `&[T]`, `xs.as_slice()`, `&xs[..]`, or `*xs`, in `match`, a tuple arm, `if let`, `let`-`else`, `while let`, and `matches!`, with guards; elements any arm pattern ([§3.5](#slice-matches), [01 §7.24](./01-equivalence.md#724-slice-patterns)) | an `if` chain on `xs.length`, then `Slice.at(xs, i)`; `rest` a new array, `Slice.range(xs, i, xs.length - j)` |
| Character classes | `b'@'` (a `u8`); integer literals and ranges in `match` and `matches!` (`matches!(b, b'0'..=b'9' \| b'_')`) | the number, with the character it wrote before it (`/* '@' */ 64`); an `if` chain tried in order |
| Characters | `char`, `'a'`; literals and ranges in `match` / `matches!`; `==`, `<`; `u32::from(c)`, `char::from(b)`, `char::from_u32(n)`; ASCII methods (`is_ascii_digit`, `to_digit(10)`, …) | `Char` (branded `string`); ordering and ranges through `Char.code` |
| Ordering | `use std::cmp::Ordering;`; `a.cmp(&b)` on integers, `char`, `bool`, strings, `Uuid`, and `Vec`s of these (element by element, then the shorter first); `<` `<=` `>` `>=` on strings (code points); `Ordering::Less` in patterns, `==`, `is_lt()` and the other predicates, `reverse()`, `then(o)`, `then_with(\|\| ..)` | a fieldless enum; `cmp` as `Ord.cmp`, or `Ord.cmpStr` (by code point) on `char`, strings, and `Uuid`, and `Ord.cmpList(a, b, Ord.cmp)` on `Vec`s; `then` as `Ord.then` |
| UUIDs | `uuid::Uuid` (or `Uuid` after `use uuid::Uuid;`); `Uuid::parse_str(s)` / `try_parse(s)` returning `Result<Uuid, uuid::Error>`; `Uuid::nil()`; `==`, `<` | `Uuid` (branded canonical `string`); `Uuid.parseStr`; `===`, `<` |
| Expected failure | `Result` / `Option`, `?`, early `return`; `if let P = e` and `let P = e else { .. };` on any pattern a `match` arm may be that can fail (`Some(x)`, `E::V { a, .. }`, `0..=9`), not a struct pattern | values, not throws; `let`-`else` as a guard (`if (s.kind !== "V") return ..;`) |
| Optional values | `o.is_some()`, `is_none()`; `unwrap_or(d)`, `ok_or(e)`, `or(o)`, `map_or(d, f)` (the value argument evaluated first, as in Rust); `map(f)`, `and_then(f)`, `filter(p)`, `is_some_and(p)`, `is_none_or(p)`, `unwrap_or_else(f)`, `ok_or_else(f)`, `or_else(f)`, `map_or_else(d, f)` (the function run only on its side; a closure without `?` or `return`, a function name, or a one-field tuple variant); on a `Result` `ok`, `err`, `is_ok`, `is_err`, `is_ok_and`, `is_err_and`, `unwrap_or`, `unwrap_or_else`, `map`, `map_err`, `map_or`, `and_then`, `or_else` ([01 §7.1](./01-equivalence.md#71-option-and-result-methods)) | the `match` std writes, the receiver bound once |
| Panics | `panic!(..)` and `unreachable!(..)` with no message or `format!`'s; `assert!(c)` (the condition names, literals, operators, calls, fields, indexing) and `assert!(c, "..", args)`; `o.unwrap()` and `o.expect(m)` on an `Option` ([01 §7.17](./01-equivalence.md#717-panics)) | `Panic.raise(m)` with Rust's message; `assert!` an `if` |
| Results | `r.ok()`, `r.map(f)`, `r.map_err(f)` (`f` as for `Option::map`); `r.map_err(f)?` | the `match` std writes; `map_err(..)?` returns `Err(f(e))` without building the mapped `Result` |
| Integers from text | `s.parse::<T>()` (or a `let` of `Result<T, ParseIntError>`), `T` an integer type: an optional `+`, `-` when signed, ASCII digits, in range. `ParseIntError` (`std::num::ParseIntError`) carries nothing | `Int.<t>.parse(s)` |
| Transition | `fn step(state, event) -> Result<State, Error>`; `&self` and `&T` are read as values | functions that never mutate arguments |
| Local update | `let mut`, assignment and `+=` on locals; `v.push(x)`, `v.insert(i, x)`, `v.remove(i)`, and `v[i] = x` on a local `let mut v: Vec<T>` (`Vec::new()`, `vec![..]`, or any `Vec`, copied when bound unless new) | new values; the grown local is an `Array<T>`, `v.push(x)` ([§3.1](#31-state-is-a-value-sequences-are-rebuilt)) |
| Copies | `clone()` on any type, `as_ref()` on an `Option`, `as_deref()` on an `Option<String>` or `Option<Vec<T>>`, `as_slice()` on a `Vec` | `[...xs]` for a `Vec`, else the value itself |
| Tuple patterns | `let (a, mut b, _) = t;` (annotated or not), `\|(a, b)\| ..`, `for (k, v) in &pairs` and `for &(k, v) in pairs.iter()`: each element `_`, a name, `mut` a name, or `&` one of these; tuples do not nest. `t.0`, `t.1` read an element | one `const` per element; a `mut` element a `let`; `t[0]` |
| Integers | `+ - * / %`, bitwise `& \| ^ !`, and shifts `<< >>` (and their `op=`) on `i8`–`i32`, `u8`–`u32` with debug semantics; bitwise and shifts not on `usize` | `Int.<ty>.*` |
| Integer methods | `min`, `max`, `abs` (signed), `pow(e: u32)`, and `checked_*`, `saturating_*`, `wrapping_*` of `add`, `sub`, `mul`, `pow`, and (`checked_`, `wrapping_`) `div`, `rem`, `neg`, on every integer type | `Int.<ty>.checkedAdd(a, b)` etc., from the exact result |
| Wide integers | `i64` / `u64`, and `i128` / `u128` (a literal below 2^127) | `bigint` |
| Widening | `i64::from(x)`, only where std has `From` | unchanged or `BigInt(x)` |
| Narrowing | `x as T` between integer types std's `From` does not join, wrapping as Rust's `as` does; into `usize`, a result past 2^53 − 1 panics ([§3.2](#32-numbers-are-sized-integers-and-floats)) | `Int.<t>.cast(x)` |
| Floats | `f32` / `f64`, their operators, `round` (half away from zero), `floor`, `ceil`, `trunc`, `abs`, `is_nan`, `is_finite`, `is_infinite`, `f64::NAN` / `INFINITY` / `NEG_INFINITY`; `as` to and from integers ([§3.2](#32-numbers-are-sized-integers-and-floats)) | `number`; `Math`'s methods where IEEE fixes them as Rust's, `Int.f64.round` ([01 §5.4](./01-equivalence.md#54-floats)) |
| Strings | `String::from("…")`; `==` / `!=` between `String` and `&str`; `len` (UTF-8 bytes), `is_empty`, `starts_with` / `ends_with` / `contains` / `strip_prefix` / `strip_suffix` with a `&str`; `split_once` with a `char` or a `&str`; `eq_ignore_ascii_case` with a `&str`; `to_ascii_lowercase` / `to_ascii_uppercase`; `trim` / `trim_start` / `trim_end` (Rust's whitespace) and `trim_matches` with its one-sided forms (a `char`, a closure `|c: char| ..`, or one-sided a `&str`); `find` (a UTF-8 byte offset); `split` with a closure; `to_lowercase` / `to_uppercase` (full Unicode); `repeat(n)`; `join(sep)` on a `Vec` or slice of `String` / `&str`; `x.to_string()` on what `format!("{}", x)` prints; slicing `&s[a..b]`; built by `String::new()` and `push(c)` / `push_str(t)` on a local `let mut`, or by `collect::<String>()` over `char`s, `&s[a..]`, `&s[..b]` at byte positions; string literals in `match` and `matches!` (on `s.as_str()` for a `String`); contents via `s.as_bytes()` indexed as `&[u8]` | literal; `===`; `Str.len(s)`, `startsWith` etc.; `Str.splitOnce(s, p)`; `Str.eqIgnoreAsciiCase(s, t)`; `Str.slice(s, a, b)`; an `if` chain of `===`; `Str.bytes(s)`; `s.toLowerCase()`, `s.repeat(n)`, `xs.join(sep)`, `` `${n}` `` ([01 §7.21](./01-equivalence.md#721-cutting-and-building-text)) |
| Formatting | `format!` with `{}`, `{x}` (a local captured), `{0}`, `{name}`, a width (`{:5}`), an alignment and fill (`{:<5}`, `{:*^5}`), and zero padding on an integer (`{:05}`), over integers, `bool`, `char`, `String`, `&str`, and `Uuid` (no width on a `Uuid`) | a template literal, `Str.pad` / `Str.padZero` where padded ([01 §6.8](./01-equivalence.md#68-format)) |
| Local closures | bound with `let`, capturing only immutable bindings | typed arrow functions |
| Recursion | named functions calling themselves or each other | plain calls |
| Integer ranges | `for i in a..b`, `a..=b`, `(a..b).rev()`, `(a..=b).rev()` (same integer type at both ends, evaluated once, start first; `i` immutable; body may use `let mut`, `return`, `?`); `(a..b).step_by(k)` with `.rev()` before it, after it, or both (`k` of 0 panics; no step past the end wraps), in a `for` and as a sequence below ([01 §7.22](./01-equivalence.md#722-iterator-adaptors-and-ranges)) | `for (let i = a, end = b; i < end; …)` (`<=` for `..=`; from `b - 1` down for `rev`); a literal end is read in place (`i < 4`); with `step_by`, `for (const i of Iter.range(a, b).stepBy(k))` |
| A string's chars | `for c in s.chars()` (`s` a `String` or `&str`, evaluated once; `c` a `char`; same body rules) | `for (const c of s)` |
| Collections and bytes | `for x in &xs`, `xs.iter()`, `xs` (a `Vec` or slice, evaluated once; `x` each element), `for b in s.bytes()` (`b` a `u8`), `for t in s.split(p)` (`p` a `char` or a `&str`, not the literal `""`; `t` each `&str` piece, empty ones included), `for w in s.split_whitespace()`, `for l in s.lines()`; same body rules. `.rev()` and `.zip(ys)` on any of these, and `for (i, x) in <any of these>.enumerate()` adds a `usize` index. `for` also walks any sequence of the consumers' row below (`xs.windows(n)`, `xs.iter().filter_map(f)`, …), its stages running as the loop asks for each item. Other adaptors (`skip`, `take`, …) are refused. `xs.get(i)`, `first()`, `last()`, `contains(&x)` read a `Vec` ([01 §7.18](./01-equivalence.md#718-reading-a-vec-get-first-last-contains)). `for (k, v) in &m` and `for k in &s` walk a `BTreeMap` / `BTreeSet` in key order, as `m.iter()`, `m.keys()`, `m.values()`, `s.iter()` do into a consumer or `collect` | `for (const x of xs)`; `for (const b of Str.bytes(s))`; `for (const t of s.split(c))` (`Str.splitStr(s, p)` for a `&str` that is not a literal), `Str.splitWhitespace(s)`, `Str.lines(s)`; `Slice.rev(xs)`, `Slice.zip(xs, ys)`; a counter beside the loop |
| Scalar consumers | `all`, `any`, `position` (a closure `\|x\| ..` without `?` or `return`, or a function name), `count`, `sum` (integers only), `fold(init, \|acc, x\| ..)` (`init` typed: `0u64`, or a typed result), `find(|x| ..)`, `max()` / `min()`, `max_by_key` / `min_by_key`, and `max_by` / `min_by` (the last of the greatest, the first of the least, as std) on `s.chars()`, `s.bytes()`, `s.split(p)`, `s.split_whitespace()`, `s.lines()`, `xs.iter()`, `xs.into_iter()`, `xs.windows(n)`, `xs.chunks(n)` (`n` of 0 panics), and a range `(a..b)` / `(a..=b)` (with `rev()` and `step_by(k)`), with `rev()` or `zip(ys)` right after, through any `map(f)`, `filter(p)`, `filter_map(f)` (`f` returning an `Option`), `flat_map(f)` (`f` returning a `Vec` or an `Option`), `flatten()` (over `Vec` or `Option` items), `copied()`, `cloned()`; `sum::<T>()` or an annotated result | `Iter.all(xs, (x) => ..)` etc., the loop std runs, stopping where std stops; `sum` adds with the type's checked `add`, panicking on overflow. A stage is `Iter.map` / `Iter.filter` / `Iter.filterMap` / `Iter.flatten`, lazy, so each item runs every stage before the next, as in Rust ([01 §7.13](./01-equivalence.md#713-map-and-filter-over-a-sequence)); `Slice.windows(xs, n)`, `Slice.chunks`; `Iter.range(a, b).rev().stepBy(k)`, computed as it is walked ([01 §7.22](./01-equivalence.md#722-iterator-adaptors-and-ranges)) |
| Lists | `collect()` of any sequence a consumer takes (above), into `Vec<T>` or `Result<Vec<T>, E>` (`c` a `char`; `f` a closure of one parameter without `?` or `return`, or a function name). The target is `collect::<..>()` (where `_` stands for what `f` returns: `collect::<Result<Vec<_>, _>>()`), a typed `let`, or the return type. A `Result` stops at the first `Err`. `s.split_once(p)` with a `char` or a `&str` | a new array: the pieces of `s.split(c)`, `[...xs]`, the array's own `.map(f)` / `.filter(p)` for one stage, else `Array.from` of the lazy stages, or `Iter.tryCollect`; `Str.splitOnce` |
| Constants | `const NAME: T = expr;` at crate level, `T` an integer, float, `bool`, `char`, or `&str`; `expr` of literals, other consts, `E::A as T`, and integer operators | one `consts.ts`: `export const NAME: T = <folded value>` |
| Local constants | `const NAME: T = expr;` inside a function body or block, visible in the whole block; not in a pattern | a `const` at the top of the block |
| Associated constants | `const NAME: T = expr;` in an inherent `impl` block (a generic type's too), read as `Self::NAME` there and `T::NAME` anywhere; `T` and `expr` as a crate-level const's; not in a pattern ([01 §7.23](./01-equivalence.md#723-associated-consts-and-paths-through-modules)) | a member of the type's companion, `NAME: <folded value>`; a non-`pub` one `tyNAME`, exported from the type's file only |
| Flags | discriminants on a fieldless enum (`A = 1 << 3`, implicit ones counting on), `#[repr(u64)]` and the other integer reprs; `e as T` where `T` holds every discriminant | a table indexed by `kind`; `E::A as T` is the literal |
| Loops | `while cond { .. }` (`cond` runs before every pass, `?` in it included), `while let P = e { .. }` (`e` runs before every pass), `loop { .. }`, its value `let x = loop { .. break v; .. };`; `break` (with a value in a `loop`) and `continue` without a label, as statements | `while`, `for (;;)`; a `loop`'s value assigned before each `break`, or returned; a label on a loop only where a jump inside a `switch` leaves it |
| Owned trees | `Box<T>` (also `Arc<T>`) erased to `T` | self-referential type alias, with a comment |
| Invariants | non-`pub` fields + a checked public constructor | closed type, no `of` ([01 §4](./01-equivalence.md#4-closed-types)) |

Why `&self` can be a value: the output never mutates arguments, and interior mutability (`Cell`, `RefCell`) is rejected, so observation is the same as passing by value. `&mut` is rejected; `fn apply(&mut self, e)` is written `fn apply(self, e) -> Self`.

## 3. Rules

### 3.1 State is a value; sequences are rebuilt

<!-- constrained-by ./07-roadmap.md#6-not-doing -->

1. A transition takes the state and returns the next one. No `&mut`, no field assignment, no `mut` parameters.
2. Past events are not accumulated in state. The caller keeps them.
3. A sequence that grows or shrinks with the state is a `Vec` built anew by the transition, or a recursive enum, returned as a new value.
4. A `Vec` in a state, a field, or a parameter is read, never grown. A function body grows its own: a local `let mut v: Vec<T>`, by `v.push(x)` ([Building a `Vec`](#building-a-vec)).
5. `Rc`, `Cell`, `RefCell`, and `Mutex` are rejected.

#### History and logs

The transition returns what happened next to the next state. The caller appends it to its own list (an ordinary growing array or table, outside the crate). The state keeps only the summary the rules read (a count, a last timestamp, a version).

Why: the state still determines the result alone. Avoid a field that points at a log someone else updates (a history ID whose contents the rules read): it is an input the signature does not show.

```rust
pub struct Order { pub status: Status, pub version: u32, pub failed_attempts: u8 }

pub fn step(order: Order, cmd: Command) -> Result<(Order, OrderEvent), OrderError>
```

In TS this is `Result<readonly [Order, OrderEvent], OrderError>`.

- A rule that needs the whole history takes it as a parameter, `history: &[OrderEvent]`, read by index like any `Vec` the caller supplies.
- Exception: a history that is itself the domain state, such as an undo stack. That is a sequence in state, and a recursive enum.

#### Sequences that grow or shrink

Return the new sequence from the transition: a `Vec` the function builds (a local grown by `push`, or `collect`), which is the counterpart of kamae's `[...lines, line]`, or a recursive enum. `examples/order` adds a line with `push`:

```rust
let mut out: Vec<Line> = Vec::new();
for l in lines { /* .. */ out.push(l); }
```

```rust
pub enum Lines { Empty, Cons(Line, Box<Lines>) }
```

#### Reading a `Vec` or slice

- `Vec<T>` is read with `xs[i]`, `xs.len()`, and `&xs[a..b]` (also `a..` and `..b`). Positions are `usize`. Out of bounds throws Rust's message.
- It is also read with `for x in &xs` ([§2](#2-what-can-be-written)).
- `&[u8]` parameters are accepted and read the same way.

#### Building a `Vec`

<!-- constrained-by ./07-roadmap.md#6-not-doing -->

- The crate builds a `Vec` as a list of its elements: `vec![a, b]`, or `vec![]` where the type is known. Its length is fixed in the source. Use it for a list the caller expects as an array, such as a JSON claim.
- It also builds one from text, once: `s.split(c).collect()` or `s.split(c).map(f).collect()`, into the `Vec<T>` or `Result<Vec<T>, E>` the context names. `c` is a `char` or a `&str` (not the literal `""`); `s.split_whitespace()` and `s.lines()` collect the same way ([01 §7.21](./01-equivalence.md#721-cutting-and-building-text)). The length is the input's, so this is not a sequence that grows with the state. A `Result` stops at the first `Err`.
- From a `Vec`, a string's `chars()` / `bytes()`, `s.split(p)`, `s.split_whitespace()`, or `s.lines()`, through any `map(f)` and `filter(p)`, with `collect()`; and `v.clone()`.
- A local grows: `let mut v: Vec<T> = Vec::new();` (or `vec![..]`, or any `Vec`), then `v.push(x)` or `v.insert(i, x)`, and is edited in place by `v.remove(i)`, `v[i] = x`, and `v[i] op= x`. Only a local declared `let mut` is written; a field (`s.items.push(x)`), an element's array (`v[i][j] = x`), and a parameter are not, and no closure captures such a local.
- It is sorted in place by `v.sort()`, `v.sort_by(|a, b| ..)`, and `v.sort_by_key(|x| ..)`, stably, and its runs of equal elements (by `==`, or by a key's) are cut to their first by `v.dedup()` and `v.dedup_by_key(|x| ..)`.
- Rejected: `vec![x; n]`, `Vec::from`, `to_vec`, `extend`, `pop`, `truncate`, `sort_unstable`, and the other mutating methods.

Why the rest of the output stays immutable: a local that is pushed to is bound to an array of its own. `Vec::new()`, `vec![..]`, `.clone()`, and `.collect()` make a new array; any other value (a parameter, a field, what a function returns) is copied when bound (`let v: T[] = [...xs]`), since in TS the caller may still hold it. So the arrays in states, fields, and parameters are never written, and `clone` of anything but a `Vec` is the value itself ([01 §7.14](./01-equivalence.md#714-growing-and-editing-a-vec)).
- `[a, b]` is an array, which rustc does not accept as a `Vec`. `[T; N]` types are rejected.
- A sequence that grows or shrinks with the state is a `Vec` the function builds, or a recursive enum (above).

#### Shared and interior mutability

`Rc`, `Cell`, and `RefCell` stay rejected. Why: even single-threaded, collapsing shared writes into values changes results.

`Box` and `Arc` can be read with `*x`. `Mutex` is refused (`[type/mutex]`): it signals shared mutable state, which a pure-function subset does not have; `lock` was already out, so a `Mutex` field could only be moved around, and erasing it would hide that.

### 3.2 Numbers are sized integers and floats

1. There is no decimal type (neither a TS `Decimal` over `number` nor `rust_decimal`). Money is an integer newtype in the smallest unit, and rounding is integer arithmetic on that type:

   ```rust
   pub struct Yen(i64);   // closed: construct via a checked `Yen::new`
   ```

2. `i32` and `f64` are both `number` at runtime, but `I32` and `F64` in types.
3. Undetermined literals need a suffix or `let x: T`. A `let` of an unsuffixed integer (`let mut s = 0;`) takes the type of its first use that names one, as rustc infers it: `s += i`, `s < n`, a call's parameter, or the function's result ([01 §7.22](./01-equivalence.md#722-iterator-adaptors-and-ranges)). A `let` of an unsuffixed integer (`let mut s = 0;`) takes the type of its first use that names one, as rustc infers it: `s += i`, `s < n`, a call's parameter, or the function's result ([01 §7.22](./01-equivalence.md#722-iterator-adaptors-and-ranges)).
4. Widening via `T::from(x)` is limited to what std provides: `u8`/`u16`/`u32` to wider unsigned or signed, `i8`/`i16`/`i32` to wider signed, and only `u8`/`u16` to `usize`. Every other conversion between integer types is `x as T`, which wraps as Rust's does (`Int.<t>.cast(x)`, the low bits read signed or not; into `usize`, Rust's 64 bits, then the 2^53 check every `usize` makes); `as` on a pair `From` takes is refused, so a lossless conversion keeps its one spelling. `u8`/`u16`/`u32` `as usize` stays accepted, as Rust has no `usize::from(u32)`.
5. A float converts the same way: `f64::from(x)` from `f32` and the integers std's `From` takes (`i8`–`i32`, `u8`–`u32`; for `f32`, `i8`/`i16`/`u8`/`u16`), and `x as T` otherwise: a float to an integer toward zero and saturating, NaN to 0; an integer to a float, or `f64` to `f32`, rounded once to the nearest, ties to even.
6. `.into()` and `try_from` are rejected.

### 3.3 Names are unique across the crate

Modules are flattened. Module paths never appear in TS names.

- A crate may be split into inline modules and module files (`mod x;` as `x.rs` or `x/mod.rs`). `#[path]` is not followed.
- A path through the crate's modules names the item alone: `crate::money::Yen`, `super::Yen`, `self::money::Yen`, and `money::Yen` are `Yen`, for a function, a type, a const, an associated const (`money::Yen::MAX`), and a variant (`money::Coin::Yen`), inside `matches!` and the other macros too. Rust's privacy is rustc's: a call to an item another module keeps private is refused as rustc refuses it (`[rustc/E0603]`).
- Which items are exported is in [§3.4](#34-public-surface).

Rules:

1. The defined name is the public name. `pub use a::B as C` is rejected: it would export a name the item does not have.
2. Two types or two free functions with the same name are rejected, listing both paths. This includes non-public items reached from the public surface. Nothing is auto-prefixed.
3. Functions, methods, consts, parameters, and locals print in camelCase: an `_` between a letter or digit and a lowercase letter goes and the letter is raised (`compare_pre_ids` → `comparePreIds`; `_unused`, `type_`, `a_1`, and UPPER_SNAKE consts stay). Fields, types, and variants keep the Rust name: a field is the JSON key serde writes. Names that are Rust keywords or TS reserved words, or that print as one or as a name the package defines (`parse_json` → `parseJson`), are rejected, not renamed; so are two methods of one type that print alike (`get_n`, `getN`).
4. Methods live in companions (`State.apply`) and do not collide with a free `apply`.
5. Each concept gets its own kebab-case file, so a type `Command` and a function `command` collide on `command.ts` and are rejected. Consts are the exception: they all go into `consts.ts` ([§3.4](#34-public-surface)), but for an associated const, which is in its type's file. An associated const is a companion member, so one named like a variant or `of` is rejected, as such a method is.

#### Reserved by the output

| Kind | Reserved |
| --- | --- |
| Names | `Result`, `Int`, `Str`, `Slice`, `Ord`, `Iter`, `Char`, the numeric brands, `assertNever`, `Array`, `Readonly`, `ReadonlyArray`, `Record`, `globalThis`, `Uuid` ([§3.7](#37-types)); `NaN` and `Infinity`, which no binding may shadow |
| File stems | `index`, `purecrate-runtime`, `purecrate-wire`, `purecrate-zod` / `-valibot` / `-arktype`; `consts` when the crate has a `const` |
| Field | `kind` |
| Companion member | `of` |

- `__proto__` as a field, variant, or method name is rejected.
- A domain `Error` type is fine. Generated code uses `globalThis.Error`.

### 3.4 Public surface

#### Exported

Exports need no attribute. An item is exported as in Rust's public surface: `pub` with every enclosing module `pub`, or named by a `pub use` (a `pub use m::*` exports module `m`'s items).

- `pub struct` / `enum` / `type` / `fn`.
- `pub fn` in inherent `impl`s. The receiver becomes the first parameter.
- `pub const`, exported from `consts.ts`. That file holds every const of the crate, so `MAX_LEN` and `fn max_len` do not collide.
- `pub const` in an inherent `impl`, as a member of the type's companion (`Grid.LIMIT`); a non-`pub` one is not on the companion, as a non-`pub` method is not.

#### Generated without `export`

- Non-public items reachable from the exported ones.
- `pub(crate)` / `pub(super)` count as private.

#### Not translated

| Item | Instead |
| --- | --- |
| `static` | a crate-level `const` |
| associated consts of a trait or a trait impl | a const in an inherent `impl`, or a crate-level `const` |
| trait definitions and trait impls | none, except the three rows below |
| `impl TryFrom<T> for X` | translated: becomes the method `X.try_from`. It must have `type Error` and `fn try_from` only |
| `impl Display` writing a fixed text per value | translated: becomes the method `X.toString` ([01 §7.11](./01-equivalence.md#711-impl-display-with-a-fixed-text)): `f.write_str("..")`, `write!(f, "..")` without arguments, or a `match self` of those or of string literals |
| any other `impl Display`, `impl std::error::Error` | skipped |

Why the others are skipped: a server needs them (serde's `try_from` requires `Display` on the error), and a `Display` that formats arguments has no translation in v0.

### 3.5 `match` arms name variants

<!-- derived-from ./07-roadmap.md#2-evidence-from-examples -->

Each arm names what it takes. The accepted patterns depend on what is matched. Arms are tried in order, as in Rust.

#### Arm patterns

| Matched value | An arm is | Last `_` |
| --- | --- | --- |
| enum, `Option`, `Result` | one variant (`Some` / `None`, `Ok` / `Err` included), binding its fields to names or `_`; or several variants of the same enum joined by `\|`, binding nothing (`Event::Pay(_) \| Event::Ship { .. } =>`) | allowed |
| `bool` | `true`, `false`, or both joined by `\|` | not required where the arms name both |
| integer | a literal (`b'@'`, `-1`), a range with a literal at both ends (`b'a'..=b'z'`, `0..10`), or several joined by `\|` | required, even where the ranges cover every value |
| `char` | a literal (`'@'`), a range with a literal at both ends (`'a'..='z'`), or several joined by `\|` | required |
| `&str` | a string literal, or several joined by `\|` (`"card" \| "credit_card" =>`). A `String` is matched through `s.as_str()`, as rustc requires | required |
| tuple | see [Tuple matches](#tuple-matches) | not required where the arms cover every case |
| slice (`&[T]`) | see [Slice matches](#slice-matches) | not required where the arms cover every length and element |

#### Struct patterns

A field of a variant, or an element of a tuple arm, may be a struct pattern, nested as deep as the types are: `Status::Processing { method: PaymentMethod { kind: MethodKind::BankDebit, .. } }`. Each of its fields is `_`, a name, another struct pattern, or a pattern that binds nothing (`K::B`, `0..=9`, `"x"`); `..` leaves the rest. Also in `matches!`; not in `if let` or `let`-`else`.

The arm is the same arm with the struct bound to a name (printed after the struct: `paymentMethod`), a test of each refutable field before its own guard, and a `const` of each bound field at the head of its body. Why: the decision tree, exhaustiveness, and printing stay as they are. Cost: as a guarded arm, it does not count toward exhaustiveness, so a `match` that covers a field's every value by struct patterns still needs a last `_` (rustc would not). A field pattern that binds inside a refutable one (`M { kind: K::A(x) }`) is refused.

#### The `_` arm

- `_` is the last arm. It takes every case no other arm names.
- A `_` after arms covering everything is accepted and dropped (rustc warns).
- A `match` whose only arm is `_` is rejected.
- The TS `switch` still lists every case by name (`_` becomes `case "A": case "B":`), so TS checks exhaustiveness too.
- As in Rust, a variant added later falls into `_` silently. Write every arm where that matters.

#### Tuple matches

`match (state, event)`. An arm is one of:

- a tuple whose elements are each `_`, a binding, a tuple, or any pattern from [Arm patterns](#arm-patterns): `(State::Paid { at }, Event::Refund(r)) =>`, `(_, Event::Reset) =>`, `(State::A | State::B, _) =>`, `(0, Some(n)) =>`, `(a, (true, n)) =>`;
- several tuples joined by `|`, when they bind nothing;
- a last `_`.

No `_` is required where the arms cover every case. The first arm that matches wins, as in Rust.

Evaluation: the tuple is not built. Elements that are places (`state`, `self.phase`) are matched as they are. Anything else is evaluated first, left to right.

Output:

- The output matches one element at a time.
- Every enum, `Option`, or `Result` element gets a `switch` (or `if`) that names every case and ends in `assertNever`. So TS checks exhaustiveness of each element on its own, not only rustc.
- An arm that several cases reach is printed once per case: `(_, Event::Reset)` appears under every state.
- Cases that reach the same code and bind nothing share one `case` list.

#### Slice matches

`match xs` on a `&[T]` (`xs.as_slice()` or `&xs[..]` of a `Vec`; `*xs` to bind the elements by value). An arm is a slice pattern, `|` of them binding nothing, or a last `_`:

- `[]`, `[a, b]`: exactly that many elements;
- `[a, ..]`, `[.., z]`, `[a, .., z]`: at least that many, `..` taking the rest;
- `[a, rest @ ..]`, `[rest @ .., z]`, `[a, rest @ .., z]`: `rest` the elements `..` takes, a `&[T]`.

Each element is `_`, a name, or any pattern a tuple's element may be, with guards after. Also in `if let`, `let`-`else` (with `mut` on an element), `while let`, `matches!`, and as an element of a tuple arm (`match (xs, ys)`).

No `_` is required where the arms cover every length, as rustc decides it, and every element within each length. `check` names a length left out (``match on a slice is missing `[_, _, ..]` ``); an element left out asks for a last `_`, also where ranges cover a whole integer type (`[0..=255]` on `&[u8]`), which rustc would accept.

Output: an `if` chain on `xs.length`, shortest first, then the elements as values of a decision tree (`Slice.at(xs, 0)`, `Slice.at(xs, xs.length - 1)`). `rest` is `Slice.range(xs, 1, xs.length - 1)`, a new array where Rust's is a view: the same values, at the cost of a copy.

#### Guards

`p if c =>` is accepted on any arm above, and on a binding arm (`n if n > 3 =>`, not in a tuple match).

- Arms are tried in order. A guard runs only when its pattern matched, as in Rust.
- The arms without guards must be exhaustive by themselves. rustc checks this.
- A `?` or `return` inside a guard is refused. Bind the `?` result with `let` first ([§4](#4-rewrites)).

- A guarded arm covers nothing: `A if g => 1, _ => 2` sends an `A` whose guard fails to `_`.

Output: the same nested `switch`es as a tuple `match` (a single value as a tuple of one), with an `if` on the guard where the arm's pattern has matched and the arms that can still match in its `else`. Each value is switched on once per path, so TS narrows nothing it would contradict.

#### Patterns inside a case

A variant's field, the payload of `Some`, `Ok`, or `Err`, or a tuple's element may hold any arm pattern or a tuple, to any depth: `PasswordChecked { verified: false, .. }`, `Paid(Method::Card, Some(0))`, `Ok(Some(Dir::Up))`, `Some(1..=9)`, `Some((1, b))`, `(a, (true, n))`, with bindings beside them and guards after them. A side of `|` may test inside its case (`Started | Paid(Method::Card, Some(0)) =>`); the sides still bind nothing. Arms are tried in order, as in Rust; rustc checks they are exhaustive.

Output: the same nested `switch`es as a guarded `match`; the field becomes one more value switched on where its case was chosen (`const verified = event.verified; if (!verified) …`). A tuple's elements are tested where they sit (`x[1][0]`). A `|` whose sides test inside is tried side by side, each with the arm's code; a test whose two ways run the same code is left out.

#### `matches!`

- `matches!(x, p)` is `match x { p => true, _ => false }`, with the same arm rules.
- `matches!(x, p if c)` is `match x { p => c, _ => false }`.

#### Not accepted

- `|` arms that bind names
- `ref` or `mut` on `rest @ ..`, and two `..` in one slice pattern
- binding-only arms without a guard
- float literal patterns
- half-open ranges (`5..`) and ranges bounded by a path (`i32::MIN..=0`)
- a type annotation on `let`-`else` (annotate the value)

### 3.6 Strings

#### Building a `String`

- A string literal is `&str` and cannot stand where `String` is expected. Write `String::from("a")`.
- `x.to_string()` is what `format!("{}", x)` prints, on the types `format!` takes (floats refused as there); a string, a `char`, or a `Uuid` prints as itself. `.to_owned()` and `.into()` are rejected, to keep one spelling.
- `s.repeat(n)` and `xs.join(sep)` (`xs` of `String` / `&str`, `sep` a `&str`) build one too ([01 §7.21](./01-equivalence.md#721-cutting-and-building-text)).
- `format!("INV-{n:05}")` builds one from numbers, text, `char`s, `bool`s, and `Uuid`s ([§2](#2-what-can-be-written)). An argument other than a local, a literal, or a field of one is placed once, in the order written; bind it with `let` otherwise. Floats, `{:?}`, `{:x}`, a precision, and the crate's own types are refused.
- `s.clone()` or `String::from(&s)` copies a `String`, `o.clone()` an `Option<String>`; `o.as_ref()` and `o.as_deref()` read an `Option` in place. In TS each is the value itself: nothing writes a string.

#### Methods

- Allowed: `len`, `is_empty`, `starts_with`, `ends_with`, `contains`, `strip_prefix`, `strip_suffix`, `split_once`, `eq_ignore_ascii_case`, `to_ascii_lowercase`, `to_ascii_uppercase`, `trim`, `trim_start`, `trim_end`, `trim_matches`, `trim_start_matches`, `trim_end_matches`, `find`, `to_lowercase`, `to_uppercase` (full Unicode, as JS's), `split_whitespace`, `lines`, `repeat`, `to_string`, and `String::as_str`; slicing `&s[a..b]` at UTF-8 byte positions, which panics off a char boundary as Rust does.
- The needle of `starts_with`, `ends_with`, `contains`, and `strip_*` is a `&str` (`s.starts_with("pm_")`, `s.contains(&t)`), not a `char` or closure. `split_once` also takes a `char`, and its pair is `Some((a, b))`.
- `s.chars()`, `s.bytes()`, `s.split(p)` (a `char`, a closure, or a `&str` other than the literal `""`), `s.split_whitespace()`, and `s.lines()` as a `for` iterable, or through `map` / `filter` into a consumer or `collect` ([§2](#2-what-can-be-written)).
- Other methods are rejected until an example needs them ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)).

#### Reading contents

Read contents through `as_bytes()`: index, `len`, `is_empty`, `u8` comparisons with byte literals (`b[i] == b'@'`), `matches!` on byte ranges, recursion or range `for`.

Byte string literals (`b"pm_"`) are a `&[u8]`, in an expression or as a crate-level `const`.

### 3.7 Types

#### Type constructors

Only `Option`, `Result`, `Vec`, `BTreeMap<String, V>` and `BTreeSet<String>` (from 0.19.0, [01 §7.19](./01-equivalence.md#719-btreemapstring-v-and-btreesetstring)), and the erased `Box` / `Arc` are type constructors, besides tuples `(A, B)` (read with `match`, a tuple pattern, or `.0`) and slices `&[T]` (read like a `Vec`). From 0.20.0 a struct, an enum, an `impl` block, and a function may have type parameters bounded by `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Debug` ([01 §7.20](./01-equivalence.md#720-type-parameters)): a `T` may be moved, cloned, compared with `==`, and put in a `Vec`, an `Option`, or a struct, not ordered; a generic type deriving serde has a schema of its parameters' schemas. There are no traits.

#### Rejected types

| Type | Why | Instead |
| --- | --- | --- |
| `HashMap` / `HashSet` | Rust leaves their iteration order unspecified, so a function that iterates one has no one result to match | `BTreeMap<String, V>` / `BTreeSet<String>` |
| `BTreeMap` / `BTreeSet` with keys other than `String` | JS `Map` compares an object key by identity, and a number key's JSON form is a string the wire would have to parse | a `String` key (`id.to_string()` where the type has `format!`), or a `Vec` of pairs |
| `BTreeMap<String, Option<T>>` and of `()` | `get` of a missing key and of that value would both be `null` / `undefined` | an enum value |
| `Rc` / `Cell` / `RefCell` / `Mutex` | shared or interior mutability; `Mutex` is shared mutable state, which the subset does not have (`[type/mutex]`) | |
| `Option<Option<T>>` | both `None`s become `null`; serde's default JSON is `null` for both as well, so the server cannot distinguish them either | an enum such as `Patch { Unset, Clear, Set(i32) }` |
| newtypes over `Option`, `()`, or `!`, and a generic newtype at one (`W<Option<u32>>`) | `null & brand` is `never` | an enum such as `Patch { Unset, Clear, Set(i32) }` |
| enums with no variants | | |
| unit structs (`struct S;`) | serde writes `struct S;` as `null` and `struct S {}` as `{}`; only the latter is kept | `struct S {}` |

#### Attributes

| Attribute | Status |
| --- | --- |
| `#[derive(Serialize, Deserialize)]`, `use serde::…` | pass: the types are the server's wire format too ([04 §3.2](./04-wire.md#32-serde-in-the-input)) |
| `#[serde(try_from = "T")]` on a struct | accepted ([04 §5](./04-wire.md#5-closed-types-on-the-wire)) |
| `#[serde(rename)]`, `#[serde(rename_all)]`, `#[serde(tag)]`, `#[serde(tag, content)]` | accepted where serde reads them ([04 §3.6](./04-wire.md#36-names-and-tags)) |
| any other `#[serde(...)]` | rejected |
| `#[cfg]`, `#[cfg_attr]` | rejected |
| `#[cfg(test)]` items | skipped |
| `derive`, `doc`, lint attributes | pass |

#### External crates

No external crate but `serde` and `uuid` is allowed. Of `uuid`, only `Uuid` and `Error` ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)). The name `Uuid` is reserved.

## 4. Rewrites

| Instead of | Write |
| --- | --- |
| `r.unwrap()`, `r.expect(m)` on a `Result`, `assert_eq!(a, b)` | `?`, `unwrap_or`, or a `match` with `panic!("..")`; `assert!(a == b, "..")` (their messages print a value with `{:?}`) |
| `o.xor(p)`, `o.zip(p)`, `o.take()`, `unwrap_or_default`, other `Option`/`Result` methods | a `match` |
| `?` inside a guard, in `matches!`'s first argument inside a test (`if matches!(x?, p)`), or on the right of `&&` / `\|\|`; `?` or `return` in an arm of a `match` bound by a tuple `let` (`let (a, b) = match ..`); `\|` arms that bind names | bind the `?` result with `let` first (the tuple too, then take it apart); split the match |
| `f.to_string()`, `format!("{:?}", x)`, `format!("{:x}", n)`, `format!("{}", f)` on a float | return a float or an ADT and let the caller format it |
| `s.split("")` | `s.chars()`: Rust gives an empty piece at each end, JS neither |
| `xs.join(sep)` on items other than `String` / `&str`, or with a `char` separator | make the text first (`.map(\|x\| format!("{x}")).collect::<Vec<String>>()`); a `&str` separator |
| `a == b` on a value holding `ParseIntError` or `uuid::Error` | compare with a `match`; `==` on the crate's types, `Option`, `Result`, `Vec`, and tuples is the derived one (from 0.13.1) |
| `a & b`, `a \| b`, `a ^ b` on `bool` | `a && b`, `a \|\| b`, `a != b` |
| `x & 1` on `usize` | `u32` or `u64` for bit fields; `%` and `/` for lengths |
| `static N: u32 = 3;` | a crate-level `const N: u32 = 3;` |
| `T::N` (an associated const) in a pattern | a guard, `x if x == T::N` |
| `x as u64` where std widens | `u64::from(x)`: a widening `From` takes has that one spelling |
| `Uuid::parse_str(s).is_ok()`, `Uuid::new_v4()` | `matches!(Uuid::parse_str(s), Ok(_))`; take new IDs as parameters (generation is the caller's) |
| `a.cmp(&b)` on floats, tuples, a `Vec` of the crate's types, or the crate's types; `impl Ord` | compare the parts and chain them with `then` / `then_with` |
| labelled `break` or `continue` | a flag the outer loop tests, or a function with early `return` |
| `.map(f).rev()`, `.filter(p).zip(..)`, `.skip(n)`, `xs.iter().step_by(n)` | `rev` or `zip` right after the source (`xs.iter().rev().map(f)`); `.enumerate()` and a test on the index; a stepped range of indices, `(0..xs.len()).step_by(n).map(\|i\| xs[i])` |
| `.flat_map(\|s\| s.chars())`, `flatten` over another iterator; `let r = (0..n);`, `(0..n).contains(&x)`, `(0..).step_by(k)` | `f` returning a `Vec` (`s.chars().collect::<Vec<char>>()`) or an `Option`; a range only before an iterator method or in a `for`; `0 <= x && x < n`; both ends |
| `.nth(n)`, `.last()` on an iterator | `xs.get(n)`, `xs.last()` on a `Vec`; `for c in s.chars()` with a `let mut` counter and early `return`; `.position(p)` for the first match |
| untyped literal / closure param / `?` in closure | `1i32`, `\|v: T\|`, `\|v: T\| -> R { .. }` |

Closures cannot capture `let mut` (a JS closure would see later reassignments; rebind with `let` first), and cannot be parameters, return values, or fields.
