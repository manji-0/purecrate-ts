# Writing Rust within the constraints

Status: current (2026-09-29)

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
| Character classes | `b'@'` (a `u8`); integer literals and ranges in `match` and `matches!` (`matches!(b, b'0'..=b'9' \| b'_')`) | the number; an `if` chain tried in order |
| Characters | `char`, `'a'`; literals and ranges in `match` / `matches!`; `==`, `<`; `u32::from(c)`, `char::from(b)`, `char::from_u32(n)`; ASCII methods (`is_ascii_digit`, `to_digit(10)`, …) | `Char` (branded `string`); ordering and ranges through `Char.code` |
| Expected failure | `Result` / `Option`, `?`, early `return`, `if let` | values, not throws |
| Transition | `fn step(state, event) -> Result<State, Error>`; `&self` and `&T` are read as values | functions that never mutate arguments |
| Local update | `let mut`, assignment and `+=` on locals | new values |
| Integers | `+ - * / %` on `i8`–`i32`, `u8`–`u32` with debug semantics | `Int.<ty>.*` |
| Wide integers | `i64` / `u64` | `bigint` |
| Widening | `i64::from(x)`, only where std has `From` | unchanged or `BigInt(x)` |
| Strings | `String::from("…")`; `==` / `!=` between `String` and `&str`; `len` (UTF-8 bytes), `is_empty`, `starts_with` / `ends_with` / `contains` with a `&str`; string literals in `match` and `matches!` (on `s.as_str()` for a `String`); contents via `s.as_bytes()` indexed as `&[u8]` | literal; `===`; `Str.len(s)`, `startsWith` etc.; an `if` chain of `===`; `Str.bytes(s)` |
| Local closures | bound with `let`, capturing only immutable bindings | typed arrow functions |
| Recursion | named functions calling themselves or each other | plain calls |
| Integer ranges | `for i in a..b` (same integer type at both ends, evaluated once, `i` immutable; body may use `let mut`, `return`, `?`) | `for (let i = a, $e = b; i < $e; …)` |
| Owned trees | `Box<T>` (also `Arc<T>`, `Mutex<T>`) erased to `T` | self-referential type alias, with a comment |
| Invariants | non-`pub` fields + a checked public constructor | closed type, no `of` ([01 §4](./01-equivalence.md#4-closed-types)) |

Why `&self` can be a value: the output never mutates arguments, and interior mutability (`Cell`, `RefCell`) is rejected, so observation is the same as passing by value. `&mut` is rejected; `fn apply(&mut self, e)` is written `fn apply(self, e) -> Self`.

## 3. Rules

### 3.1 State is a value; sequences are recursive enums

A transition takes the state and returns the next one. No `&mut`, no field assignment, no `mut` parameters. Past events are not accumulated in state; the caller keeps them.

Sequences that grow or shrink are recursive enums returned as new values, the counterpart of kamae's `[...lines, line]`:

```rust
pub enum Lines { Empty, Cons(Line, Box<Lines>) }
```

`Vec<T>` is only for sequences whose length is fixed outside the function: built with `[a, b]`, read with `xs[i]` and `xs.len()` (both `usize`; out of bounds throws Rust's message). No `push`, `map`/`filter`/`collect`, or `vec!`.

`Rc`, `Cell`, and `RefCell` stay rejected: even single-threaded, collapsing shared writes into values changes results. `Box` and `Arc` can be read with `*x`; `Mutex` has no `lock`, so it can only be built and held.

### 3.2 Numbers are sized integers and floats

No decimal type (neither a TS `Decimal` over `number` nor `rust_decimal`). Money is an integer newtype in the smallest unit, and rounding is integer arithmetic on that type:

```rust
pub struct Yen(i64);   // closed: construct via a checked `Yen::new`
```

`i32` and `f64` are both `number` at runtime but `I32` and `F64` in types. Undetermined literals need a suffix or `let x: T`. Widening via `T::from(x)` is limited to what std provides: `u8`/`u16`/`u32` to wider unsigned or signed, `i8`/`i16`/`i32` to wider signed, and only `u8`/`u16` to `usize`. Narrowing, `as`, `.into()`, and `try_from` are rejected.

### 3.3 Names are unique across the crate

Modules are flattened; module paths never appear in TS names. A crate may be split into inline modules and module files (`mod x;` as `x.rs` or `x/mod.rs`; `#[path]` is not followed), since 2026-09-29; before that `check` required one file although this section already described flattening.

- A path through the crate's modules names the item alone: `crate::money::Yen`, `super::Yen`, and `money::Yen` are `Yen`.
- An item is exported as in Rust's public surface: `pub` with every enclosing module `pub`, or named by a `pub use` (a `pub use m::*` exports module `m`'s items). Other items are generated without `export` if reachable.

1. The defined name is the public name. `pub use a::B as C` is rejected: it would export a name the item does not have.
2. Two types or two free functions with the same name are rejected, listing both paths. This includes non-public items reached from the public surface. Nothing is auto-prefixed.
3. Names that are Rust keywords or TS reserved words are rejected, not renamed.
4. Methods live in companions (`State.apply`) and do not collide with a free `apply`.
5. Each concept gets its own kebab-case file, so a type `Command` and a function `command` collide on `command.ts` and are rejected.

Reserved by the output: `Result`, `Int`, `Str`, `Char`, the numeric brands, `assertNever`, `Readonly`, `ReadonlyArray`, `globalThis`; the file stems `index`, `result`, `assert-never`, `int`, `str`; the field `kind` and the companion member `of`. `__proto__` as a field, variant, or method name is rejected. A domain `Error` type is fine (generated code uses `globalThis.Error`).

### 3.4 Public surface

`pub` items become exports with no attribute: `pub struct` / `enum` / `type` / `fn`, and `pub fn` in inherent `impl`s (receiver becomes the first parameter). Non-public items reachable from these are generated without `export`. `pub(crate)` / `pub(super)` count as private. Not translated: `const`, `static` (use functions), trait definitions and trait impls, except: `impl TryFrom<T> for X` becomes the method `X.try_from` (it must have `type Error` and `fn try_from` only), and `impl Display` / `impl std::error::Error` are skipped: a server needs them (serde's `try_from` requires `Display` on the error), and nothing translated can call them.

### 3.5 `match` arms name variants

<!-- derived-from ./07-roadmap.md#2-evidence-from-examples -->

An arm is one of:

- one variant, `Some`/`None`, or `Ok`/`Err`, binding its fields to names or `_`;
- several variants of the same enum joined by `|`, binding nothing (`Event::Pay(_) | Event::Ship { .. } =>`);
- `_`, as the last arm, taking every case no other arm names. A `_` after arms covering everything is accepted and dropped (rustc warns); a `match` whose only arm is `_` is rejected;
- on an integer: a literal (`b'@'`, `-1`), a range with a literal at both ends (`b'a'..=b'z'`, `0..10`), or several joined by `|`. The last arm must be `_`, even where the ranges cover every value. Arms are tried in order, as in Rust.
- on a `char`: a literal (`'@'`), a range with a literal at both ends (`'a'..='z'`), or several joined by `|`. The last arm must be `_`.
- on a `&str`: a string literal, or several joined by `|` (`"card" | "credit_card" =>`). The last arm must be `_`. A `String` is matched through `s.as_str()`, as rustc requires.

`matches!(x, p)` is `match x { p => true, _ => false }`, with the same arm rules; a guard (`p if c`) is rejected.

The TS `switch` still lists every case by name (`_` becomes `case "A": case "B":`), so TS checks exhaustiveness too. As in Rust, a variant added later falls into `_` silently; write every arm where that matters.

Not accepted: tuple scrutinees (`match (state, event)` — split into one function per state), `|` arms that bind names, binding-only arms, guards, nested patterns, `bool` and float literal patterns, half-open (`5..`) ranges and ranges bounded by a path (`i32::MIN..=0`), `let else`.

### 3.6 Strings

A string literal is `&str` and cannot stand where `String` is expected; write `String::from("a")`. `.to_string()`, `.to_owned()`, and `.into()` are rejected to keep one spelling; there is no `clone`, so build it again. `len`, `is_empty`, `starts_with`, `ends_with`, `contains`, and `String::as_str` are allowed; the needle is a `&str` (`s.starts_with("pm_")`, `s.contains(&t)`), not a `char` or closure. Other methods are rejected until an example needs them ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)). Read contents through `as_bytes()`: index, `len`, `u8` comparisons with byte literals (`b[i] == b'@'`), `matches!` on byte ranges, recursion or range `for`. Byte string literals (`b"pm_"`) are not available; use `starts_with`.

### 3.7 Types

Only `Option`, `Result`, `Vec`, and the erased `Box`/`Arc`/`Mutex` are type constructors. No user type parameters, traits, `HashMap`/`BTreeMap` (key equality differs between Rust and JS). `Option<Option<T>>` is rejected (both `None`s become `null`), as are newtypes over `Option`, `()`, or `!` (`null & brand` is `never`); write an enum such as `Patch { Unset, Clear, Set(i32) }` instead. Enums with no variants are rejected, and so are unit structs (`struct S;`; serde writes it as `null`, `struct S {}` as `{}`, and only the latter is kept). `#[derive(Serialize, Deserialize)]` and `use serde::…` pass: the types are the server's wire format too ([04 §3](./04-wire.md#3-current-design)). Of `#[serde(...)]`, only `#[serde(try_from = "T")]` on a struct is accepted ([04 §5](./04-wire.md#5-closed-types-on-the-wire)); the rest, `#[cfg]`, and `#[cfg_attr]` are rejected. `#[cfg(test)]` items are skipped; `derive`, `doc`, and lint attributes pass. No external crate but `serde` is allowed.

## 4. Rewrites

| Instead of | Write |
| --- | --- |
| `xs.iter().map(f).collect()` | index + recursion or range `for`; return new sequences as recursive enums |
| `opt.map(..)`, `and_then` | `match` or `?` |
| `format!("{}", n)` | return numbers and ADTs; the caller formats |
| `a == b` on structs/enums | an `eq` method (JS structural comparison differs) |
| `s < t` on `String` | an enum or integer until code-point comparison exists |
| `for x in xs`, `while`, `loop`, `break` | range `for` with early `return`, or recursion |
| `for c in s.chars()` | `s.as_bytes()` read by index in a range `for`, with `b'@'` and `matches!(b, b'0'..=b'9')` (`s.chars()` waits for iterator `for`; a single `char` value can be written as `'@'`, `c.is_ascii_digit()`) |
| `match (s, e)` | one function per state, each ending in `_ => Err(..)` |
| untyped literal / closure param / `?` in closure | `1i32`, `\|v: T\|`, `\|v: T\| -> R { .. }` |

Closures cannot capture `let mut` (a JS closure would see later reassignments; rebind with `let` first), and cannot be parameters, return values, or fields.
