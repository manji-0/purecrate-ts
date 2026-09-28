# New code written within the constraints

Date: 2026-09-27
Status: decided
Prerequisites: design/00, design/04, current `check::accept` and `emit_ts`

## 0. Decision

The target is not to pass published Rust through unchanged. It is **new domain code written within PureCrate's constraints**.

Success is judged by the following two:

1. Definitions written under those constraints can still express the capabilities Rust should have for this purpose. Capability means writing pure transitions with ADTs, exhaustive matching, `Result` / `Option`, and debug-build integer semantics (design/04 §1.2). It does not mean mapping all of Rust.
2. The constraints that remain on callers of the generated TS are enumerated.

The acceptance rate on the existing corpus (design/06) is kept as the rationale for not choosing that target. It is not used as a metric. `Option` / `Result` methods, iterator adapters, `String` methods, `format!`, and re-measuring the corpus are work for getting existing code through, so they are discontinued. The same behavior can be written with `match`, `?`, and string `==`.

"Loss" is split into three layers.

| Layer | Meaning | Example |
| --- | --- | --- |
| Capability | The behavior cannot be expressed in any way rustc accepts | Cannot read elements of a list |
| Notation | No shorter way to write the same behavior | `match` instead of `.map` |
| Annotation | The author must make explicit a type rustc would infer | Suffixes on integer literals |

## 1. Capabilities that can be written under the constraints

The following can be written while preserving the meaning of newly written transition functions. They match the Rust debug build in differential tests.

| Capability | Written as | Generated TS |
| --- | --- | --- |
| Closed ADTs | struct, enum, newtype (whose content is not `Option`, `()`, or `!`) | `Readonly` objects, `kind` unions, brands |
| Exhaustiveness | `match` on a single enum. Each variant gets exactly one arm | `switch` and `assertNever` |
| Expected failure | `Result` / `Option`, `?`, early `return`, `if let` | `kind` or `null`. Failure is a value |
| Transition | `fn step(state, event) -> Result<State, Error>`. `&self` methods are accepted as values, and receiver syntax becomes a companion call | Functions that do not mutate arguments |
| Local update | `let mut` and assignment to local variables. Field assignment and `&mut` parameters are rejected | Returns a new value |
| Integers | `+ - * / %` on `i8`–`i32`, `u8`–`u32`. Truncating division. Overflow and division by zero throw with the same message as the panic | `Int.<type>.*` |
| Wide integers | `i64` / `u64` | `bigint` |
| Integer widening | Only widenings for which std has `From`. `i64::from(q)` | Value unchanged. `BigInt(q)` only when it becomes `bigint` |
| String construction | `String::from("…")`. Literals are `&str` and cannot be placed in a `String` position | The literal itself |
| String equality | `==` / `!=` between `String` and `&str` (either order) | `===` / `!==` |
| String contents | Read the UTF-8 byte sequence from `s.as_bytes()` via `&[u8]` indexing, `len`, and recursion (2026-09-29) | `Str.bytes(s)`. A `ReadonlyArray<U8>` of code points encoded as UTF-8 |
| Local closures | Capture only immutable bindings; bind with `let` and call. `?` and `return` exit the closure | Typed arrow functions |
| Recursive calls | Named functions calling themselves or other functions | Plain function calls |
| Integer-range iteration | `for i in a..b { .. }`. Both ends are the same integer type and are evaluated once before the loop. `i` is immutable. The body may contain `let mut` updates, early `return`, and `?`. `a..=b`, iterators, `break` / `continue`, labels, `while`, and `loop` are rejected (2026-09-29) | `for (let i = a, $e = b; i < $e; i = (i + 1) as T)` |

`&self` is collapsed to a value because the output does not mutate arguments and interior mutability (`Cell`, `RefCell`) is rejected. `Mutex<T>` is erased to `T`, and `lock` does not become a method. The observable result is the same as a function taking a value.

`fn apply(&mut self, event)` cannot be written. `fn apply(self, event) -> Self` can. The latter suffices for state machine transitions.

## 2. Capabilities the constraints drop

### 2.1 Sequences are not mutable arrays in state

A state type holds only the phase and the fields that phase needs. Past events are not accumulated in the state. They are passed outside the transition (design/02 §1.1).

Sequences whose elements grow and shrink are written as recursive enums. `Lines::Cons(line, Box::new(lines))` corresponds to kamae's `[...lines, line]`. To remove one element, recursively return a new list that skips that element.

`Vec<T>` is a type for reading a sequence whose length is fixed outside the function. Values are built with array literals `[a, b]`. `xs[i]` and `xs.len()` are accepted. The index type and the return of `len` are `usize`, which in TS is a `number` checked from 0 to 2^53−1. Out of bounds throws `index out of bounds: the len is N but the index is I`. `map` / `filter` / `collect` and length-changing operations (append, remove, replace) are not included. `vec!` is rejected.

### 2.2 Owned recursive data

rustc rejects an enum that contains itself by value as infinitely sized. Owned trees are written with `Box`. `Box<T>` is accepted by erasing it to `T`. `Box::new(v)` becomes `v`. No runtime indirection is emitted.

`Arc<T>` and `Mutex<T>` are likewise erased to `T`. `Arc::new(v)` and `Mutex::new(v)` become `v`. `lock` and `clone` are not accepted as methods. Generated types and expressions get a comment stating the use in Rust and that TS ignores the wrapper because it is single-threaded.

```rust
pub enum Ast {
    Num(i32),
    Add(Box<Ast>, Box<Ast>),
}
```

The generated TS type is a type alias in which `Ast` refers to itself. `Rc`, `Cell`, and `RefCell` stay rejected. Even single-threaded, collapsing writes through shared references or interior mutability into values changes the meaning.

### 2.3 Nested `Option` cannot be written

`Option<T>` is `T | null`. `Option<Option<T>>` is rejected because the two levels of `null` collapse. Newtypes wrapping `Option`, `()`, or `!` are also rejected. `null & { readonly [Brand]: true }` becomes `never`.

A domain that distinguishes "unset" from "explicitly empty" is written as an enum, not a nested `Option`.

```rust
pub enum Patch {
    Unset,
    Clear,
    Set(i32),
}
```

This is a constraint that removes from the Rust definition a distinction TS cannot represent. Written as an enum, the same distinction can be represented.

### 2.4 Modules do not become namespaces

`mod`s within the crate are flattened. If type names or free function names collide after flattening, the input is rejected. `billing::State` and `shipping::State` cannot both be written. Public names must be unique across the crate.

`const` and `static` are not translated. Make named constants into functions.

### 2.5 Struct update

`State { n: state.n + 1, ..state }` becomes `({ ...state, n: state.n + 1 })`. Omitted fields come from `state`. The only side effect allowed in this position is `?`, and a `?` in an explicitly written field exits the function before the `..` base. `..` on enum variants and newtypes is rejected. Rust also rejects functional record update on enums.

## 3. Notation and annotation

The capability remains. New code is written in the following forms.

| Form you would want to write | Form to write instead |
| --- | --- |
| `xs.iter().map(\|x\| f(x)).collect()` | Read a passed `Vec` with indexing and recursion. Return new sequences as recursive enums (§2.1) |
| `opt.map(\|x\| x + 1)`, `and_then` | `match` or `?` |
| `format!("{}", n)` | For transitions that need strings, the caller formats. Domain functions return numbers and ADTs |
| `state == other` (struct / enum) | An `eq` method. JS structural comparison does not match Rust, so the operator is rejected |
| `s < t` (`String`) | Rejected for now. Until code-point-order comparison is added, use an enum or integer if ordering is needed |
| `for x in xs`, `while`, `loop`, `break` / `continue` | Integer-range `for i in a..b` and early `return` (§1). Iteration that cannot be written that way uses recursion of a named function |
| Guarded `match`, nested patterns, `let else` | `if` inside the arm, one level of `match` at a time |
| User-defined generics, traits, `HashMap` | List concrete types. State that needs key lookup is out of scope for v0 (`HashMap` is v1, design/00 §16) |
| Untyped integer literals, closures without parameter types, `?` in closures without return types | Suffixes (`1i32`), `\|v: T\|`, `\|v: T\| -> R { .. }` |

`usize` is a `number` checked from 0 to 2^53−1 (design/04 §1.5). The representation of `char` is decided, but the implementation still rejects it. Write sequences of characters as `String`.

## 4. Constraints on callers of the generated TS

What callers must observe for functions whose translation succeeded.

### 4.1 How to call

- Methods are not methods on the value. Not `state.bump()` but `State.bump(state)`. The receiver is the first argument. No `this` is emitted.
- Structs, tuples, and `Vec` are `Readonly`. Updates happen via new values returned by functions.
- Enums branch on `kind`. Variants can be built either as `Cmd.Move(a, b)` or as `{ kind: "Move", content: [a, b] }`. A non-exhaustive `switch` is caught by `assertNever`.
- Module paths do not remain. Import flat names from the package's `index.ts`.
- If a public function's parameter name collides with an item name in the same crate, it changes, e.g. to `inc$1`. Calls are positional, so the result does not change. The name you read may differ from the Rust parameter name.
- Do not edit generated files. To change them, change the Rust and regenerate.

### 4.2 Two kinds of failure

- Expected failure is a value. `Result` is `{ kind: "Ok", value } | { kind: "Err", error }`. Absence in `Option` is `null`. `undefined` is `()`, not absence.
- Unexpected failure is a throw: integer overflow, division by zero, out-of-bounds indexing (after §2.1), `assertNever`. Domain failures are not carried by throw.
- Integer division inside generated functions truncates. If the caller divides two `I32`s with `/`, the result type is `number` and cannot be passed back to an `I32` parameter. For the same computation as Rust, use `Int.i32.div` or call a generated function.

### 4.3 Shapes of numbers and strings

| Rust | TS value | What the caller adds |
| --- | --- | --- |
| `i8`–`i32`, `u8`–`u32`, `usize` | Branded `number` (`I32`, `Usize`, etc.) | Bring in with `Int.i32.of`. Results of raw `number` arithmetic cannot be passed back |
| `f32`, `f64` | `F32`, `F64` | Bring in with `Int.f32.of` / `Int.f64.of`. `F32` and `F64` are distinct types |
| `i64`, `u64` | Branded `bigint` | Do not mix with `number`. `JSON.parse` loses precision for integers above 2^53, so read serde_json JSON with `parseJson` |
| `String` | `string` | `===` matches Rust equality. `.length` and `[i]` are in UTF-16 units, not Rust byte lengths or byte indices. The current subset does not emit those operations |
| newtype | The content value's type intersected with a brand | The runtime value is the content itself. The brand disappears through JSON. If the content is `pub`, construct with `Meters.of`. If non-`pub`, it is a closed type obtained from the crate's public functions (§4.7) |
| struct with private fields | Branded `Readonly` object (§4.7) | No `of`. Construct by calling a Rust public function, e.g. `Email.parse` |
| `Vec<T>` | `ReadonlyArray<T>` | Indexing and `len` are readable. Append, remove, `map` / `filter` are not emitted. Not frozen at runtime |

### 4.4 Ownership disappears only at the type level

In Rust, `step` takes `state` by value, so the original binding cannot be used after the call. Generated TS does not mutate arguments, so after the call the original object remains in its pre-call state. The previous state can be retained.

If you strip `Readonly` and mutate an object, the mutation is visible through other aliases pointing to the same object. The output does not `Object.freeze`.

### 4.5 The wire format is separate from in-memory values

Generated enums use a `kind` internal tag. This does not match serde's default JSON (external tagging, unit variants as strings) ([design/05](./05-type-sharing-scope.md) §2.2). The result of `JSON.parse` cannot be passed directly as a function argument. Only with `--schema` are wire schemas for public structs and enums emitted to `src/purecrate-wire.ts`. They are read-only; the side that writes JSON from domain values does not exist yet ([design/08](./08-limits-and-roadmap.md) §5.3).

`#[serde(...)]` is rejected. Field names are emitted as the Rust names.

### 4.6 How to consume the package

The generated package is distributed as an npm package. Sources are `src/*.ts` and use `.ts`-extension imports. `npm run build` (run by `prepack` before `npm pack` and `npm publish`) emits JavaScript and declarations into `dist` with TypeScript 6 or 7. `.ts` imports are rewritten to `.js` by `rewriteRelativeImportExtensions`. `exports` points to `dist`, so consumers can load it from node without a TypeScript loader, and tsc can read it under either `nodenext` or `bundler`. With `--schema`, wire schemas are read from `<package>/wire`. `version` is the version from the crate's `Cargo.toml` (2026-09-29).

The runtime `purecrate` and schema adapters such as `purecrate-zod` are `peerDependencies`. Brand types (`I32`, etc.) are distinguished by `purecrate`'s `unique symbol`, so even with two generated packages, values can be passed between them only if there is a single runtime. The runtime and adapters also ship `dist` in the same shape. Inside this repository, sources are read without building via the condition `purecrate-source` (node's `--conditions`, tsc's `customConditions`).

The test packs the generated package, installs it into a separate project, runs it with node, and passes type checking with TypeScript 6 and 7 tsc under both `nodenext` and `bundler` (`crates/cli/tests/package.rs`). `purecrate` is not yet published to npm.

Reserved names are `Result`, `Int`, `Str`, numeric brands (`I32`, etc.), `assertNever`, `Readonly`, `ReadonlyArray`, `globalThis`, and the file stems `index` / `result` / `assert-never` / `int` / `str`; also the discriminant field name `kind` and the companion's `of`. Domain types cannot use these names. Generated code reads `Math`, `Number`, `Error`, and `BigInt` as e.g. `globalThis.Error`, so a domain `Error` type is allowed. `__proto__` as a field, variant, or method name is rejected, because in an object literal it sets the prototype. Enums with no variants are rejected, because they become neither a TS union nor a wire format.

### 4.7 Closed types are built from public functions (2026-09-29)

<!-- constrained-by ./04-objective-means-demand.md#16-sharing-validation-and-public-constructors -->

A struct with even one non-`pub` field in Rust becomes a closed type. For closed types, callers must observe the following:

- The companion has no `of`. Use values returned by Rust public functions. `pub fn parse(raw: String) -> Result<Email, EmailError>` becomes `Email.parse`.
- The type carries a brand. Object literals and raw `string`s do not become the closed type as-is. A value typed with `as Email` is outside the equivalence guarantee (design/04 §1.3.1).
- Fields remain readable as before. They cannot be mutated.
- A closed-type value read from the wire has been checked for shape only, as with Rust's `Deserialize`. Invariants are not checked (design/05 §7.7).

That consumers cannot construct these is verified by type-checking consumer files annotated with `@ts-expect-error` under TypeScript 6 and 7 (`crates/cli/tests/closed_equivalence.rs`).

From the Rust author's side, whether a field is `pub` decides whether TS allows `of`. For a type with invariants, make its fields non-`pub` and write a checked public function.

## 5. What to do after this evaluation

Close only capability holes. Do not build allow-lists that raise the acceptance rate of existing code. The order of additions is in [design/08](./08-limits-and-roadmap.md).

`usize` is included for `Vec` indexing and `len`. `Box<T>`, `Arc<T>`, and `Mutex<T>` are accepted by erasing them to `T`, leaving a cautionary comment in the output.

No decimal type is included (2026-09-27). Putting a type named `Decimal` over `number` leaves the arithmetic in binary floating point. External crates such as `rust_decimal` are not accepted either. Money is written on the Rust side as an integer newtype in the smallest unit.

```rust
pub struct Yen(i64);
```

The runtime value in the generated TS is `bigint`, with that newtype's brand at the type level. Rounding of fractions is written as methods on this type using integer arithmetic. Its content is non-`pub`, so it is a closed type (§4.7). To make it constructible from TS, write a checked public function like `Yen::new` in [examples/order](../examples/order/src/lib.rs).
