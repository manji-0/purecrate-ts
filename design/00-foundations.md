# PureCrate → TypeScript package conversion — foundational design

Date: 2026-09-26
Updated: 2026-09-27
Status: draft v0 (public surface, flattening, and version plan settled)

## 1. Goal

Build a mechanism that converts a Rust crate satisfying the following constraints into a **single TypeScript package**.

- The public surface's inputs and outputs are closed over type definitions within the crate
- Public functions have no side effects
- Algebraic data types (struct / enum = union) are first-class
- The typical public function is a pure transition ` (State, Event) -> State | Result<State, Error> `

The output is TS source (types + implementation) consumable via npm. Rust / WASM is not required at runtime.

## 2. Non-goals (v0)

- Full compilation of arbitrary Rust crates
- Reproducing I/O, async, threads, `unsafe`, or trait objects
- Automatic binding to existing JS ecosystem types (DOM, Node fs, etc.)
- Generating WASM binaries (kept as a future second backend)
- Conversion that keeps references or lifetimes in the public API

## 3. Terminology

| Term | Meaning |
| --- | --- |
| PureCrate | The subset of Rust crates this mechanism accepts |
| Public surface | `pub` types and functions (reachable private implementation is generated but not exported) |
| Transition function | A pure function whose inputs and outputs are only crate-defined ADTs. In particular state × event → state |
| IR | The intermediate representation at the center of the conversion pipeline. Depends on neither Rust nor TS syntax |
| Package | The output. `package.json` + `.ts` implementation + re-exports |

## 4. Design principles

1. **Types are the contract.** Reject public functions whose input/output types are not closed within the crate.
2. **Map enums to closed discriminated unions.** An exhaustive `switch` must be possible on the TS side.
3. **Functions are referentially transparent.** Same input, same output. Globals, randomness, time, and I/O are forbidden.
4. **Only owned values cross the boundary.** No `&T` / lifetimes in public signatures.
5. **Reject before generating.** Anything outside the subset is not partially generated; emit a diagnostic and fail.
6. **Output is machine-readable and deterministic.** The same input crate always yields the same TS.

## 5. PureCrate subset (v0)

### 5.1 Allowed types

- Primitives: `bool`, `i8` `i16` `i32`, `u8` `u16` `u32`, `f32` `f64`, `String`
- `i64` / `u64` are allowed but become `bigint` in TS (not collapsed to `number`)
- `Option<T>`, `Result<T, E>`, `Vec<T>`
- Tuples (elements of allowed types only)
- Named-field `struct` (owned fields only)
- `enum` (unit / tuple / struct variants)
- `type` aliases
- Unit `()`

`Box<T>`, `Arc<T>`, and `Mutex<T>` are treated as `T`. No runtime indirection, sharing, or mutual exclusion is generated. Owned recursive enums are written with `Box`. The output keeps a comment saying what it is used for in Rust and that TS ignores it because it is single-threaded.

Forbidden: reference fields, lifetimes, `Rc`/`Cell`/`RefCell`, slices, trait objects, user-defined generics, `HashMap` / `BTreeMap`, external crate types.

### 5.2 Allowed functions

```
pub fn name(arg: OwnedType, ...) -> OwnedType
```

- The body is an expression language: literals, `let`, `if` / `if let`, `match`, constructors, field access, tuples, calls to allowed functions, `?` (`Result` and `Option`; the error type must match the function's, no `From` conversion), `return` (statement position only)
- Local `let mut`, assignment and compound assignment (`x += e`), and expression statements are allowed. Assignment to fields and `mut` parameters are rejected. Bindings are renamed to names unique within the function before generation (shadowing becomes `x$1`)
- Owned or logically pure methods in `impl Type { pub fn ... }` may be normalized into free functions whose first parameter is the receiver, and exported

Forbidden: `async`, `unsafe`, macros (outside the allow-list), closures capturing `let mut`, function-typed parameters and return values, control via `panic!`, `println!`, mutable statics, foreign functions.

Allowed macros (v0): `unreachable!` only (TS `assertNever`). `vec!`, `format!`, `todo!`, and `panic!` are rejected. A `Vec<T>` is built with an array literal `[a, b]` whose expected type is `Vec<T>`, and read via indexing and `len`. Sequences that grow inside state are written as recursive enums (design/02 §1.1).

### 5.3 Modules and public surface

- The public surface adopts `pub` automatically. `#[purecrate::export]` is not needed (reserved only for future narrowing)
- Covered: `pub` struct / enum / type alias / fn / `pub` methods on `impl`
- Even non-`pub` types and functions are included in generation if reachable from a public function's body or a public type's fields (internal implementation)
- `pub(crate)` / `pub(super)` are the same as private
- In-crate `mod`s are not kept in TS. They are folded into a flat namespace
- `pub use` makes the folded target's name the public name
- If type or function names collide after folding, reject (module paths are not embedded in TS names)

### 5.4 Dependencies

- No external crate types other than `purecrate` itself appear on the public surface
- External crates in internal implementation are also forbidden in v0 (to close the analysis boundary)

## 6. Pipeline

```
source crate
  → parse (syn / rustc_ast equivalent; v0 uses syn)
  → public surface extraction
  → subset check (type closure, side effects, references)
  → IR
  → TS printing
  → package assembly (package.json, tsconfig, index)
```

The converter itself is a Rust CLI + library:

```
purecrate-ts build <crate-path> --out <dir> [--name <crate>]
purecrate-ts check <crate-path> [--out <dir>] [--name <crate>]
```

`<crate-path>` is either a crate directory (reads `src/lib.rs`) or a single `.rs`. When `--name` is omitted, `[package] name` from `Cargo.toml` is used, else the directory name. `check` without `--out` only checks; with it, it also verifies byte equality with the output (lists differences, missing, and extra files, and exits with code 1).

Parsing does not depend on rustc. Type inference is done in-house in a limited form (annotations required as a rule; local inference only).

## 7. IR

The IR has two parts: "type definitions" and "function definitions".

### 7.1 Types

```
Ty =
  | Prim(Bool|I32|U32|I64|U64|F32|F64|String|Unit)
  | Option(Ty)
  | Result(Ty, Ty)
  | Vec(Ty)
  | Tuple([Ty])
  | Named(Path)
  | Never

Adt =
  | Struct { name, fields: [(name, Ty)] }
  | Enum { name, variants: [Variant] }
  | Alias { name, ty }

Variant =
  | Unit { name }
  | Tuple { name, elems: [Ty] }
  | Struct { name, fields: [(name, Ty)] }
```

### 7.2 Functions

```
Fn = { name, params: [(name, Ty)], ret: Ty, body: Expr }

Expr =
  | Lit | Var | Let | Assign
  | If | Match
  | Call { callee, args }
  | Construct { adt, variant?, fields }
  | Field | Tuple | Array
  | Return | Unreachable
```

`Match` is normalized into exhaustive enum coverage. Guards may be out of scope for v0, but simple `if` guards are candidates for allowance.

## 8. Type mapping

| Rust | TypeScript |
| --- | --- |
| `bool` | `boolean` |
| `i8`..`i32`, `u8`..`u32` | Per-width branded `number` (`I32` etc.). Boundary: `Int.i32.of` |
| `f32`, `f64` | Branded `number` (`F32`, `F64`). Boundary: `Int.f32.of` / `Int.f64.of`. `f32` arithmetic uses `Math.fround` |
| `i64`, `u64` | Branded `bigint` (`I64`, `U64`) |
| `String`, `&str` | `string` (length and indexing reproduce UTF-8 byte units; design/04 §1.5) |
| `char` | Branded `string` of one code point (planned; design/04 §1.5) |
| `usize` | `Usize`. Checked from 0 to 2^53−1 (design/04 §1.5). The type of `Vec` indices and `len` |
| `isize` | Unsupported |
| `()` | `undefined` |
| `Option<T>` | `T \| null` |
| `Result<T,E>` | `Readonly<{ kind: "Ok"; value: T }> \| Readonly<{ kind: "Err"; error: E }>` |
| `Vec<T>` | `ReadonlyArray<T>` |
| `(A,B)` | `readonly [A, B]` |
| `struct S { a: T }` | `export type S = Readonly<{ a: T }>` + companion `const S` |
| `enum` | `kind` discriminated union + companion (next section) |

Numeric types are not mixed at the public boundary. There is no opt-in to lower `i64` to `number`.

### 8.1 Semantics of numeric operations

The reference for equivalence is a Rust debug build (design/04 §1.3, §5). `check::accept` infers a type for each expression and rewrites it into the following forms before printing.

| Rust | Generated TS |
| --- | --- |
| Integer `+ - * / %`, unary `-` | `Int.<type>.add(a, b)` etc. `/` truncates; overflow and division by zero throw with the same message as Rust's panic. `-0` is normalized to `0` |
| `f32` `+ - * / %` | `Math.fround(a op b)`. For `f32` literals the converter rounds from decimal to `f32` in one step and emits that value as an exact decimal (`Math.fround(lit)` goes through `f64` and so rounds twice) |
| `f64` arithmetic | JS operators as-is |
| `i64`/`u64` literals | `5n` |

`Int` lives in the output's `int.ts`; the name `Int` and file name `int` are reserved. `Math.fround` is emitted as `globalThis.Math.fround`, so `Math` is not reserved. Inference is bidirectional and closed within the expression tree; it does not use types that rustc determines from later uses, nor the `i32`/`f64` defaults. Numeric literals whose type is not determined are rejected, asking for a suffix (`1i64`) or a `let x: T` annotation. Comparisons whose result differs between JS and Rust (struct `==`, ordering of `String`) are also rejected. Ordering comparisons on `String` and `char` stay rejected until a code-point-order comparison function (design/04 §1.5) is added.

## 9. enum → union

The default is the same as kamae-ts: **`kind` internal tag**. `type` / `status` / `tag` are not used.

```rust
enum Cmd {
    Quit,
    Move(i32, i32),
    Paint { color: String },
}
```

```ts
export type Cmd =
  | Readonly<{ kind: "Quit" }>
  | Readonly<{ kind: "Move"; content: readonly [number, number] }>
  | Readonly<{ kind: "Paint"; color: string }>;

export const Cmd = {
  Quit: (): Cmd => ({ kind: "Quit" }),
  Move: (a: number, b: number): Cmd => ({ kind: "Move", content: [a, b] }),
  Paint: (color: string): Cmd => ({ kind: "Paint", color }),
} as const;
```

Generated helpers:

- A Companion Object with the same name as the type (variant construction, associated functions)
- `assertNever(x: never): never` for exhaustiveness checking (throws as an unexpected fault if reached)

serde's externally / adjacently tagged representations are v1. v0 is fixed to `kind`.

## 10. Function generation rules

- `match e { ... }` → `switch (e.kind)` + variant bindings. Missing arms are rejected in the check phase
- `if let Enum::V { .. } = e` → `kind` test + narrowing
- `?` → `if (r.kind === "Err") return r` (for `Option`, `if (r === null) return null`). A `?` inside an expression is hoisted into a preceding `const`, preserving evaluation order
- A `match` / `if` used as a `let` value or on the right side of an assignment is lowered to `let x: T;` plus an assignment in each arm
- `Option` branches on `=== null`
- Struct update syntax `S { a: 1, ..s }` → `({ ...s, a: 1 })`. Omitted fields come from `s`. For `?` in fields and in `s`, the written-out fields exit the function first. `..` on enum variants and newtypes is rejected
- `impl` methods become Companion function properties `Type.method: (self, ...) => ...` (method syntax is not used)

To preserve referential transparency, generated TS does not mutate arguments. Updates return new objects.

## 11. Transition function convention

As a language feature it is just a pure function. The following form is the documented standard form.

```rust
pub fn step(state: State, event: Event) -> Result<State, Error> { ... }
```

TS:

```ts
export function step(state: State, event: Event): Result<State, Error>
```

The `Transition` trait is not required in v0. If needed, it can later be added as a marker on the IR.

## 12. Output package

```
<out>/
  package.json
  tsconfig.json
  src/
    assert-never.ts
    result.ts          # built-in Result type + companion
    event.ts           # one concept per file (example)
    state.ts
    step.ts
    index.ts           # re-exports only
```

Rust modules are flattened, but the TS side follows kamae-ts with **one concept per file**. Grab-bag files like `types.ts` / `fns.ts` are not emitted. File names are the kebab-case of the public name.

`package.json` has `type: "module"`, and `exports` points to `src/index.ts` (or `dist` after emit). The package name defaults to the input crate name in kebab-case.

A stamp at the top of each generated file:

```
/* generated by purecrate-ts. do not edit. */
```

`check` detects drift by byte equality (or equality after normalization) with the existing output.

## 13. Checks (acceptance conditions)

All must hold before conversion.

1. Public function types are a closure over in-crate ADTs + allowed built-ins
2. Function bodies contain only allowed expressions
3. No reachability to forbidden paths (`std::fs`, `std::net`, `std::time::SystemTime`, randomness, etc.)
4. enum matches are exhaustive
5. Public signatures have no references or lifetimes
6. Recursive types are accepted by erasing `Box<T>` to `T`. `Arc<T>` and `Mutex<T>` are also erased to `T`. The output keeps a comment that single-threaded TS ignores them. Generated TS expresses them via forward references in type aliases
7. Type names and free function names are unique after flattening

On failure, return file, line, and rejection reason. No partial files are left behind (`--out` is replaced only on success).

## 14. Repository layout (this project)

```
purecrate-ts/
  crates/
    ir/          # IR data types. Minimal dependencies
    syntax/      # syn parsing → IR
    check/       # subset check (names, resolution, exhaustiveness) and removal of unreachable private items
    emit_ts/     # IR → TS strings
    pack/        # package assembly
    cli/         # build / check. tests/ has goldens and Rust/TS equivalence tests
  examples/
    counter/     # minimal transition crate
    counter-ts/  # its TS package (output; golden)
  scripts/
    verify.sh    # cargo test + tsc on output and runtime (TypeScript 6 and 7)
```

Public functions of each crate are also kept pure where possible. File I/O is confined to `cli` and `pack`.

## 15. Minimal example (acceptance criterion)

Input:

```rust
pub enum Event { Inc, Dec, Reset }

pub struct State { pub n: i32 }

pub fn step(state: State, event: Event) -> State {
    match event {
        Event::Inc => State { n: state.n + 1 },
        Event::Dec => State { n: state.n - 1 },
        Event::Reset => State { n: 0 },
    }
}
```

The output TS must have the same input/output types and return the same values as Rust for any `State` × `Event`. This is the completion condition for v0.

## 16. Settled version plan

- Public surface: automatic `pub` adoption (2026-09-27)
- Modules: flattened. Collisions are check errors (2026-09-27)
- Errors: domain `Result` is `{ kind: "Ok" | "Err" }`. Expected failures are values. Only `assertNever` throws, as an unexpected fault
- Generics: v0 has only `Option` / `Result` / `Vec`. User-defined type parameters are v1
- `HashMap` / `BTreeMap`: forbidden in v0. In v1, `ReadonlyMap<string, V>` only when the key is `String`
- WASM: reserved as a second backend emitted from the same IR. Not implemented in v0. The IR is not tied to TS printing

Public surface and v1 scope: [design/01](./01-surface-flatten-roadmap.md). The full set of current constraints and the order of upcoming additions: [design/08](./08-limits-and-roadmap.md). v1 is introduced once an example appears that cannot be expressed without type parameters.

## 17. Position relative to existing tools

`ts-rs` / `tsify` / `typeshare` are strong at generating **type declarations**. This mechanism packages, in addition to types, the **implementation of pure functions** into a TS package. WASM bindings are an alternative implementation and not the main path in v0.
