---
name: purecrate-authoring
description: How to write Rust that purecrate-ts accepts and translates to TypeScript with identical behavior. Use this skill whenever the user writes, ports, or fixes Rust domain logic (state machines, validators, `fn step(state, event) -> Result<State, Error>`, money or ID newtypes, wire types with serde) meant for `purecrate-ts check` / `build`, or when `check` rejects code with a reason code like `[expr/method-call]`, `[check/needs-annotation]`, `[pattern/arm]`, or `[rustc/E0382]`. Also use it when adding an example under `examples/`, even if the user only says "share this logic with the frontend" or "make this Rust usable from TS without WASM".
---

# Writing Rust for purecrate-ts

purecrate-ts translates a small, pure subset of Rust into a TypeScript package that returns the same result as a Rust debug build. Anything whose meaning it cannot preserve is rejected with a location, and nothing is written. So the job is: write the domain logic in the accepted subset, run `check`, and fix what it names. Do not try to make arbitrary Rust pass; the subset is the point.

The authoritative rules are in `design/02-authoring.md`. Read it when this skill is not enough (§2 capability table, §3.5 `match` arms, §4 rewrites). Working code to copy from is in `examples/` (`counter`, `order`, `signup`, `iban`, `invoice`, `payment`).

## Loop

1. Write the crate (`src/lib.rs`, or a single `.rs`; `mod x;` files are read too).
2. Run `cargo run --offline -p purecrate-ts -- check <crate-path>`. It writes nothing. A failure prints `path:line:col: [reason/code] message`; rustc errors print as `[rustc/E0382]`. `check` also needs `rustc` on the machine.
3. Fix the first diagnostics, rerun. Let the diagnostic drive: it is cheaper than guessing which construct is out.
4. `build <crate-path> --out <dir> [--schema zod|valibot|arktype]` emits the package. Never edit generated output; change the Rust and regenerate.

## Shape the domain like this

The constraints match a functional style, so lean into it rather than fighting it.

- **State and events are ADTs.** Structs, enums (become `kind` unions), newtypes. A transition takes the state and returns the next one: `fn step(state: State, event: Event) -> Result<State, Error>`. No `&mut`, no field assignment, no `mut` parameters. `&self` and `&T` are fine (read as values). Write `fn apply(self, e) -> Self`, not `apply(&mut self, e)`.
- **One function per state** instead of `match (state, event)`. Each ends in `_ => Err(..InvalidTransition)`. `order` and `payment` show the pattern.
- **Growing sequences are recursive enums** (`enum Lines { Nil, Cons(Line, Box<Lines>) }`), returned as new values. `Vec<T>` is only for a length fixed by the caller: build with `[a, b]`, read with `xs[i]` and `xs.len()`. No `push`, `map`/`filter`/`collect`, `vec!`, `is_empty` on `Vec`.
- **Invariants live in closed types**: non-`pub` fields plus a checked public constructor (`Yen::new(v) -> Result<Yen, E>`). TS then gets values only from your constructor. This is what makes validation shared, so prefer it to public fields.
- **Money is an integer newtype in the smallest unit** (`struct Yen(i64)`), rounding is integer arithmetic. There is no decimal, no floats-for-money.
- **Expected failures are `Result`/`Option`** with `?` and early `return`; nothing throws except integer overflow, division by zero, and out-of-range indexing, which panic exactly as a Rust debug build does.
- **Wire types**: `#[derive(Serialize, Deserialize)]` is accepted so the server uses the same types. For a closed type use `#[serde(try_from = "i64")]` with `impl TryFrom<i64>` calling your constructor; `impl Display` / `Error` are allowed and ignored. Other `#[serde(...)]` attributes are rejected.

## Accepted, at a glance

- Integers `i8`–`i32`, `u8`–`u32` (`Int.*`, checked), `i64`/`u64` as `bigint`, `f32`/`f64`, `usize` (safe-integer range). Widening only via `i64::from(x)` where std has `From`. No `as`, `.into()`, `try_from` (except the serde form above).
- Strings: `String::from("…")`, `==`/`!=`, `len` (UTF-8 bytes), `is_empty`, `starts_with`/`ends_with`/`contains` with a `&str`, contents via `s.as_bytes()` indexed as `&[u8]`. A bare literal is `&str` and cannot stand where `String` is expected.
- `char`: literals, `==`, `<`, ranges in `match`/`matches!`, `u32::from(c)`, `char::from(b)`, `char::from_u32(n)`, ASCII methods. `s.chars()` is not available yet, so iterate bytes.
- Control: `if`, `if let`, exhaustive `match`, `for i in a..b` (integer range, early `return`/`?` allowed), recursion, local `let mut`, local closures over immutable bindings, struct update `S { a, ..base }`.
- `match` arms: one variant binding fields or `_`; `A | B` binding nothing; a last `_`. Literal and range patterns on integers, `char`, and `&str` also need a last `_`. `matches!(x, p)` follows the same rules.

## Not accepted, and what to write

| Instead of | Write |
| --- | --- |
| `iter().map(..).collect()`, `for x in xs`, `while`, `loop`, `break` | index + recursion, or a range `for` with early `return` |
| `opt.map(..)`, `and_then`, `unwrap_or` | `match` or `?` |
| `match (state, event)` | one function per state |
| match guards `p if c`, nested patterns, `|` arms that bind names | an `if` inside the arm; split the match |
| `a == b` on structs/enums | an `eq` method |
| `s < t` on `String` | an enum or integer |
| `format!`, `.to_string()`, `.to_owned()`, `.into()`, `clone` | return numbers/ADTs and let the caller format; rebuild with `String::from` |
| generics, traits, `HashMap`, `Rc`/`Cell`/`RefCell`, `const`/`static` | concrete types, functions, recursive enums |
| `Option<Option<T>>`, newtype over `Option`/`()`, unit struct `struct S;` | an enum such as `Patch { Unset, Clear, Set(i32) }`; `struct S {}` |
| `&mut` anything | take `self`, return the new value |

## Small things that trip people

- **Names are unique across the whole crate**, modules are flattened, and each type or function becomes a kebab-case file. A type `Group` and a function `group` collide on `group.ts` and are rejected; so is `Command` vs `command`. Rename the helper, not the type. Rust keywords and TS reserved words are rejected, not renamed. The output reserves `Result`, `Int`, `Str`, `Char`, `kind`, `of`, and a few more (`design/02 §3.3`).
- **Integer suffixes are only for when `check` asks** (`[check/needs-annotation]`), and it asks in three places you can predict: a `let mut` accumulator that nothing has typed yet (`let mut sum: u32 = 0;`, the most common one, e.g. when porting `.sum()` to a range `for`), a range whose bounds are used only as indices (`for i in 0..2usize`), and a bare `0` returned as `i64`. Where the type is determined by context, write the bare literal; extra suffixes are noise.
- **`_` hides new variants.** As in Rust, a variant added later falls into `_` silently. Where that matters, name every arm.
- **Reading a string means `as_bytes()`**: `b[i] == b'@'`, `matches!(b[i], b'0'..=b'9')`, with `b.len()` for the bound. Byte string literals (`b"pm_"`) do not exist; use `starts_with("pm_")`.
- **rustc runs too.** A program the subset accepts but rustc rejects fails as `[rustc/E…]`; fix it as you would in ordinary Rust.
- **Tests and callers**: for what TS callers must observe (values are `Readonly`, `i64` as `bigint`, `Result` values not throws), see `design/03-output.md §5`. When you add a capability or example, add a differential test (`crates/cli/tests/*_equivalence.rs`) that runs the same inputs through Rust and the generated TS.

## Adding an example

Write it within the constraints first, from a third-party spec if possible, and record where `check` stopped you in `design/07-roadmap.md §2`. A capability is added to purecrate-ts only when an example cannot be written without it; do not widen the subset just to make one program pass.
