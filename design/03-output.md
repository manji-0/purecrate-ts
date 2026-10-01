# Generated TypeScript

Status: current (2026-10-01, 0.5.0)

<!-- constrained-by ./01-equivalence.md -->

## 1. Style

The output follows the domain layer of [kamae-ts](https://github.com/iwasa-kosui/kamae-ts): `kind` discriminated unions, pure transitions, companion objects, `Result` as a value. The boundary parts of kamae (schema validation, `Sensitive<T>`, ports, repositories) belong to the consumer, except the optional wire schemas ([04](./04-wire.md)).

| Rule | Consequence |
| --- | --- |
| Discriminant is always `kind` | no `type` / `tag` / `status` |
| `type`, never `interface` | no declaration merging |
| `Readonly<{…}>`, `ReadonlyArray`, `readonly [..]` | no reassignment through types |
| Same-named companion | `export type T` + `export const T = { … } as const` |
| Function properties, `export const f = (…) =>` | no classes, no method syntax, no `this`, no `export function` |
| One concept per file | `state.ts`, `event.ts`, `step.ts`; `index.ts` only re-exports |
| Expected failure is `Result` | only `assertNever` and arithmetic/index panics throw |
| Time and IDs are arguments | the domain never generates them |
| Lines up to 100 characters | a longer line opens its outermost bracket with commas, one item per line (`emit_ts::tidy::wrap`) |
| Functions, methods, parameters, locals in camelCase | `compare_pre_ids` → `comparePreIds`, `Yen::try_from` → `Yen.tryFrom`; fields, types, variants, and UPPER_SNAKE consts keep the Rust name ([02 §3.3](./02-authoring.md)) |
| `///` comments are JSDoc | on the type, each struct field, each variant's constructor, each function and method, `const`, and alias; an editor shows the Rust documentation on hover. A comment on an `impl` block has nowhere to go; one on `impl Display` documents `toString` |

### 1.1 Casts

A brand exists only in types, so TS lets any `as` make a number an `I32` or a string a `Yen`. The generated code casts only where the value is already what the type says, for a reason outside TS; `crates/cli/tests/it/casts.rs` reads the output of every example and test fixture and fails on any `as` of no kind below, and on a cast to one of the crate's brands outside its constructor.

| Kind | Example | Why the value is what the type says |
| --- | --- | --- |
| Literal | `(1 as I32)`, `(10n as I64)`, `(1.0 as F64)` | rustc refuses an integer literal its type cannot hold |
| `char` literal | `"." as Char` | a Rust `char` literal is one scalar value |
| Length | `(xs.length) as Usize` | a JS length is an integer below 2^32 |
| Widening | `(x as number as U32)`, `(globalThis.BigInt(x) as I64)` | `check` takes `T::from(x)` only where std has `From`, which is lossless |
| `for` counter | `i = (i + 1) as Usize` | `i` is below the exclusive end, so `i + 1` is at most the end |
| Discriminant | `({ A: (1 as U8) } as Record<string, U8>)[e.kind] as U8` | the table holds the folded discriminants, each in range ([01 §7.7](./01-equivalence.md#77-const-and-discriminants)) |
| Float | `(a * b as F64)`, `(Math.fround(x) as F32)` | every `number` is an `f64`; `fround` gives an `f32` |
| Constructor | `Yen$of = (value: I64): Yen => value as Yen`, a newtype's `of` | the crate's own constructor, which Rust lets the crate call; a closed type's is not exported |
| Declared type | `state as State`, `{ kind: "A" } as Event`, `o as I64 \| null` | the value has that type already; TS had narrowed it, and the cast widens it back |
| Not a cast to a brand | `as const`, `import { A as A$ }`, arktype's `ctx.error(..) as never` | — |

A caller's own code can write `5 as I32` all the same; no type stops it. Values from outside belong in `Int.i32.of`, the wire schemas, or the crate's functions, and a lint such as `@typescript-eslint/consistent-type-assertions` with `assertionStyle: "never"` (the generated directory left out) keeps the rest of the code from casting.

## 2. Type mapping

| Rust | TypeScript |
| --- | --- |
| `bool` | `boolean` |
| `i8`–`i32`, `u8`–`u32` | branded `number` (`I32` …) |
| `f32`, `f64` | `F32`, `F64` (distinct brands) |
| `i64`, `u64` | branded `bigint` (`I64`, `U64`) |
| `usize` | `Usize`, checked 0..2^53−1 |
| `String`, `&str` | `string` |
| `char` | `Char`, a branded one-code-point `string` |
| `()` | `undefined` |
| `Option<T>` | `T \| null` |
| `Result<T, E>` | `Readonly<{ kind: "Ok"; value: T }> \| Readonly<{ kind: "Err"; error: E }>` |
| `Vec<T>`, `&[T]` | `ReadonlyArray<T>` |
| `(A, B)` | `readonly [A, B]` |
| `Box<T>`, `Arc<T>`, `Mutex<T>` | `T`, the type marked `/* Box */ T`; `Box::new(x)` is `x`. `Box` is heap indirection for a recursive type, `Arc` shared ownership across threads, `Mutex` exclusion between threads: a single-threaded program with values never mutated observes none of them |
| `struct S { a: T }` | `Readonly<{ a: T }>` + companion; branded if closed |
| newtype `S(T)` | `T & { readonly "<crate>.S": true }` |
| `enum` | `kind` union + companion |

## 3. Shapes

### 3.1 Enums

```rust
enum Cmd { Quit, Move(i32, i32), Paint { color: String } }
```

```ts
export type Cmd =
  | Readonly<{ kind: "Quit" }>
  | Readonly<{ kind: "Move"; content: readonly [I32, I32] }>
  | Readonly<{ kind: "Paint"; color: string }>;

export const Cmd = {
  Quit: (): Cmd => ({ kind: "Quit" }),
  Move: (_0: I32, _1: I32): Cmd => ({ kind: "Move", content: [_0, _1] }),
  Paint: (color: string): Cmd => ({ kind: "Paint", color }),
} as const;
```

A partial union (`type Cancellable = Waiting | EnRoute`) is emitted only from an explicit Rust `type` alias.

### 3.2 Structs, methods, newtypes

```rust
pub struct Meters(i32);
impl Meters { pub fn plus(&self, other: &Meters) -> Self { Self(self.0 + other.0) } }
```

```ts
export type Meters = I32 & { readonly "geo.Meters": true };

// not exported from index.ts
export const Meters$of = (value: I32): Meters => value as Meters;

export const Meters = {
  plus: (self: Meters, other: Meters): Meters => Meters$of(Int.i32.add(self, other)),
} as const;
```

**Newtypes.**

- A newtype's runtime value is its content. This is also serde's JSON for it.
- `.0` is the value itself.
- The brand key is a `unique symbol`, so newtypes of newtypes do not collide.
- `Meters` above is closed (its field is not `pub`), so there is no `of`. With `pub struct Meters(pub i32)` the companion would have `of`.

**Methods.** Methods become companion properties with the receiver first. `Self` is replaced by the type name.

| Rust | TS |
| --- | --- |
| `Type::m(x)` | `Type.m(x)` |
| `x.m(y)` | `T.m(x, y)`, with `T` resolved from `x`'s inferred type |
| `Vec::len` | `.length` |
| `Vec::is_empty` | `.length === 0` |
| `Option::is_some` / `is_none` | `!== null` / `=== null` |
| `&s[a..b]` on a string | `Str.slice(s, a, b)`, at UTF-8 byte positions |
| `xs[i]` on a `Vec` or slice | `Slice.at(xs, i)`, with Rust's bounds check and panic message |
| `&xs[a..b]` on a `Vec` or slice | `Slice.range(xs, a, b)` (`null` for an open end), with Rust's range checks |
| `s.strip_prefix(p)` / `strip_suffix` | `Str.stripPrefix(s, p)` / `Str.stripSuffix` |

Only the crate's own inherent methods resolve, plus the std allow-list ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)): the `Vec` and `Option` rows above, indexing, and the `str` and `char` methods.

### 3.3 Control flow

| Rust | TS |
| --- | --- |
| `match` | `switch (e.kind)` with `default: return assertNever(e)` |
| `if let` | `kind` test with narrowing |
| `match` / `if let` on `Option` | branch on `=== null` |
| `match` on a tuple | nested `match`es, one element at a time ([3.3.1](#331-tuple-match)) |
| a tuple pattern in `let`, a closure parameter, or a `for` variable | a one-arm tuple `match`: one `const` per element |
| guarded arms | the tuple `match` tree with an `if` on each guard ([3.3.2](#332-guards-and-option-methods)) |
| `unwrap_or`, `ok_or`, `map` | the `match` std writes ([3.3.2](#332-guards-and-option-methods)) |
| `for` over a `Vec`, `chars()`, `bytes()`, or `split(c)` | `for..of` |
| `for (i, x) in ...enumerate()` | `for..of` with a `usize` counter declared before it and advanced at the top of each pass |
| `all`, `any`, `position`, `count`, `sum` on `chars()`, `bytes()`, `iter()` | `Iter.position(Str.bytes(s), (b: U8): boolean => ..)`: the runtime's loop, stopping where std stops, the closure as an arrow; `sum` as `Iter.sum(xs, Int.i32.add, 0)` |
| `while` | `while` |
| a loop that a `break` or `continue` leaves | the loop gets a label ([3.3.3](#333-loop-labels)) |
| `?` on `Result` | `if (r.kind === "Err") return r;` |
| `?` on `Option` | `if (r === null) return null;` |
| `match` / `if` used as a value | `let x: T;` plus an assignment per arm |
| `match` (or `matches!`) on a place inside an expression, each arm an expression | `?:` on each arm's test, `||` / `&&` where arms are `true` / `false`, bindings read from the place: `(o !== null ? o : 0)`, `(k.kind === "A")`. On a value that is not a place, an inline function that evaluates it once |
| a `match` on a place whose arms are all `true` / `false` | the test, `return (b >= 48 && b <= 57);`, not a `switch` |
| `S { a: 1, ..s }` | `({ ...s, a: 1 })` |

#### 3.3.1 Tuple match

A `match` on a tuple is split into nested `match`es, one element at a time (`check::tuple`). At each level it chooses the first element that the first remaining arm tests.

```ts
switch (event.kind) {
  case "Reset": …
  case "Tick":
    switch (state.kind) { … default: return assertNever(state); }
  …
  default: return assertNever(event);
}
```

- Every enum, `Option`, and `Result` element is matched with every case named, so TS checks exhaustiveness.
- Integer, `char`, and string elements are `if`/`else` on one arm's pattern at a time.
- Elements that are not places go into `const`s first, in order (`$e1`, `$e2`).
- A field or payload an arm binds is read once, into the arm's own name (`const conversion = method.conversion;`); a guard, and another arm reaching the same case, read that name (`check::binds`). A fresh `$f`/`$v` name remains only where no arm names the value.
- A body that several cases reach is copied into each. Cases with the same code and no bindings share a `case` list.
- A binding of a place with an enum, `Option`, or `Result` type prints `const s = state as State`. An annotation would keep the narrowing of an enclosing `switch`.

#### 3.3.2 Guards and Option methods

- A `match` with guards prints as a tuple `match` does (§3.3.1), a single value as a tuple of one. Where an arm's pattern has matched, `if (guard) { body } else { .. }`, the `else` holding the arms after it that can still match. The guard reads the arm's bindings from their places.
- `unwrap_or`, `ok_or`, and `map` become the `match` that std writes. The receiver and an eager argument are bound first.
- `let x = o.ok_or(e)?` is a guard instead: the receiver and `e` bound, then `if ($o === null) return Result.err($oOr);` and `const x = $o`. The `match` would build a `Result` only for `?` to take it apart.
- A `?` inside an expression is hoisted in front of its statement: `const $f = f(x);`, `if ($f.kind === "Err") return $f;` (`=== null` for an `Option`), and the expression reads `$f.value` (`$f`). `let x = e?` binds the payload to `x` instead.
- A name the generator makes starts with `$`, which no Rust name can, and says what it holds where it can: `$f` for the value of a call to `f`, `$o` and `$oOr` for the receiver and argument of `o.unwrap_or(..)` / `o.ok_or(..)`; a second one of a name gets a number (`$f2`, `$o$1`).
- `if c { return v; }` as a statement prints on one line, `if (c) return v;`, when `c` and `v` each fit on one.

#### 3.3.3 Loop labels

A loop that a `break` or `continue` leaves gets a label. A `match` prints as a `switch`, and a bare `break` inside that `switch` would leave the `switch`, not the loop.

#### 3.3.4 Evaluation order

- A `?` inside an expression is hoisted into a preceding `const`. This preserves evaluation order.
- In `S { a: 1, ..s }`, a `?` in an explicit field exits before `s` is evaluated.

#### 3.3.5 Renaming

- Bindings are renamed to be unique per function. Shadowing gives `x$1`.
- A local with the same name as an item is renamed, because a TS `const` shadows an import across the whole block.

### 3.4 Closures

```ts
const scale: ((_0: I32) => I32) = ((v: I32): I32 => Int.i32.mul(v, k));
```

`?` and `return` exit the closure, so they need a return annotation in Rust. Hoisting stays inside the closure body.

### 3.5 Counter, in full

```ts
export const step = (state: State, event: Event): State => {
  switch (event.kind) {
    case "Inc":   return { n: Int.i32.add(state.n, (1 as I32)) };
    case "Dec":   return { n: Int.i32.sub(state.n, (1 as I32)) };
    case "Reset": return { n: (0 as I32) };
    default:      return assertNever(event);
  }
};
```

The committed golden is [examples/counter/ts/plain](../examples/counter/ts/plain/src). Every example keeps its output beside its source: `examples/<name>/ts/plain` without a schema and, for an example that derives serde ([04 §3.2](./04-wire.md#32-serde-in-the-input)), `examples/<name>/ts/<lib>` with each schema library (`scripts/examples.sh` regenerates them all).

## 4. Package

### 4.1 Layout

```
<out>/
  package.json  tsconfig.json  tsconfig.build.json (for npm run build)
  src/
    index.ts          re-exports only
    purecrate-runtime.ts  the runtime, copied in (§4.4), with `Result` and
                      `assertNever`; each file imports what it uses from it
    <concept>.ts      one per public concept, kebab-case
    consts.ts         every `const` of the crate, folded to its value
    purecrate-wire.ts only with --schema
```

- Each file starts with `/* generated by purecrate-ts. do not edit. */`.
- The output is byte-deterministic.

### 4.2 Compiler strictness

`tsconfig.json` is `strict` with `noUnusedLocals`, `noUnusedParameters`, and `allowUnreachableCode: false`. So the sources also pass in a consumer project that turns those on.

A binding the Rust leaves unused is not printed:

| Unused in Rust | Printed as |
| --- | --- |
| `let` or write | its value as a statement (it may panic, as in Rust) |
| pattern binding | `_` |
| parameter or `for` variable | the name with the leading `_` that TS exempts |

Imports are those the printed code mentions.

### 4.3 `package.json` and build

| Field or script | Value |
| --- | --- |
| `type` | `"module"` |
| `exports` | `dist` (and `<package>/wire` with `--schema`); under the `purecrate-source` condition, the `.ts` sources |
| `version` | from `Cargo.toml` |
| `private` | `true` unless `--publishable` is given |
| `peerDependencies` | only the schema library |
| `files` | `dist` and `src` |
| `npm run build` | `tsc -p tsconfig.build.json`, TypeScript 6 or 7; also run by `prepack` |

The build rewrites `.ts` imports to `.js`. Consumers need no TS loader and can resolve under `nodenext` or `bundler`.

`private` defaults to `true` because generated packages belong to private code. `npm publish` should refuse by default. `npm pack` and installing the tarball still work.

### 4.4 Runtime, copied in

**What.** The runtime (`packages/boundary`) is copied in as `src/purecrate-runtime.ts`. With `--schema`, the adapter is copied in as `src/purecrate-<lib>.ts`. Both are copied at the generator's revision. The schema library is the only `peerDependencies` entry.

**Only what the package uses.** The runtime marks its parts with region and needs comments; `pack` keeps a part when the package's other files name it (`Int.<ty>.<op>` for each type's bitwise operators and methods, `Str.<member>`, `Json`), and leaves the markers out (`crates/pack/src/trim.rs`). The types, `Result`, `assertNever`, and each integer type's `of` and arithmetic are always kept, and so is all of what the index exports to callers: `Char` when the public surface holds a `char`, `Uuid` when it holds a `Uuid`, and `parseJson` with `--schema` (`trim::exported`). Why: the runtime's `Int` is one object that generated code always names, so a bundler cannot drop what it does not use, and a caller that loads `src/` or `dist/` directly gets no bundler at all. payment's copy is 10.3 KB of the runtime's 29 KB, counter's 6.5 KB.

**Brands.** Brands are keyed by string: the runtime's by `purecrate.` and the type (`{ readonly "purecrate.I32": true }`), a crate's newtypes and closed structs by the crate's name and the type (`{ readonly "invoice.Yen": true }`). Packages that each carry a copy of the runtime exchange values, and so do two copies of one crate's package, as two installed versions would be; the key reads in a hover or a type error.

**Why.** It removes the version skew between generator and runtime. Nothing has to be installed that is not on npm. A peer-installed runtime, the earlier design, had no place in a project that commits generated code, as vendoring into Oxide's console showed ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)).

**Verified by.** `crates/cli/tests/it/package.rs` packs two generated packages and installs them into a separate project with only `zod`. It passes one's `I32` to the other, runs on node, and type-checks under both resolutions with both TS versions.

**Inside this repository.** The adapters read the runtime's sources via the `purecrate-source` condition.

## 5. Caller contract

What callers of a successfully generated package must observe.

### 5.1 Calling

- `State.bump(state)`, not `state.bump()`.
- Import flat names from the package root.
- Variants can be built with `Cmd.Move(a, b)` or as literals.
- A public function's parameter may be renamed (`inc$1`). Calls are positional, so nothing changes.

### 5.2 Failure

- Expected failure is a value: `Result` or `null`.
- `undefined` means `()`, not absence.
- Only overflow, division by zero, out-of-bounds indexing, and `assertNever` throw.

### 5.3 Numbers

| Rust | TS | Caller's job |
| --- | --- | --- |
| `i8`–`i32`, `u8`–`u32`, `usize` | `I32` etc. | Bring values in with `Int.i32.of`. Raw `number` arithmetic results cannot be passed back; use `Int.i32.div` etc. |
| `f32`, `f64` | `F32`, `F64` | `Int.f32.of` / `Int.f64.of` |
| `i64`, `u64` | branded `bigint` | Do not mix with `number`. Read serde_json text with `parseJson`, not `JSON.parse` |

### 5.4 Strings

`===` matches Rust equality. JS `.length` and `[i]` are UTF-16 units, not Rust byte lengths; the output never emits them.

### 5.5 Closed types

- There is no `of`. Obtain values from public functions (`Email.parse`).
- Object literals and raw primitives do not type-check as the closed type. Verified with `@ts-expect-error` consumers under TS 6 and 7.
- A value produced with `as Email` is outside the equivalence guarantee.

### 5.6 Aliasing

Arguments are not mutated, so the pre-call state remains usable. Nothing is frozen. Mutating after stripping `Readonly` is outside the guarantee.

### 5.7 JSON

In-memory enums use `kind`; serde's default JSON does not. Go through the `--schema` wire schema, not `JSON.parse` output directly ([04](./04-wire.md)).

### 5.8 Editing

Never edit the package. Change the Rust and regenerate.
