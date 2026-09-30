# Generated TypeScript

Status: current (2026-09-30)

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
| `Box<T>`, `Arc<T>`, `Mutex<T>` | `T`, with a comment |
| `struct S { a: T }` | `Readonly<{ a: T }>` + companion; branded if closed |
| newtype `S(T)` | `T & { readonly [SBrand]: true }` |
| `enum` | `kind` union + companion |

## 3. Shapes

### Enums

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

### Structs, methods, newtypes

```rust
pub struct Meters(i32);
impl Meters { pub fn plus(&self, other: &Meters) -> Self { Self(self.0 + other.0) } }
```

```ts
declare const MetersBrand: unique symbol;
export type Meters = I32 & { readonly [MetersBrand]: true };

// not exported from index.ts
export const Meters$of = (value: I32): Meters => value as Meters;

export const Meters = {
  plus: (self: Meters, other: Meters): Meters => Meters$of(Int.i32.add(self, other)),
} as const;
```

A newtype's runtime value is its content, which is also serde's JSON for it. `.0` is the value itself. The brand key is a `unique symbol` so newtypes of newtypes do not collide. `Meters` above is closed (its field is not `pub`), so there is no `of`; with `pub struct Meters(pub i32)` the companion would have `of`.

Methods become companion properties with the receiver first. `Self` is replaced by the type name. `Type::m(x)` becomes `Type.m(x)`; `x.m(y)` is resolved from `x`'s inferred type to `T.m(x, y)`. Only the crate's own inherent methods resolve, plus the std allow-list ([01 §6](./01-equivalence.md#6-strings-char-usize-std-methods)): `Vec::len` as `.length`, `Vec::is_empty` as `.length === 0`, `Option::is_some` / `is_none` as `!== null` / `=== null`, indexing, and the `str` and `char` methods.

### Control flow

- `match` → `switch (e.kind)` with `default: return assertNever(e)`. `if let` → `kind` test with narrowing. `Option` branches on `=== null`. A `match` on a tuple is split into nested `match`es, one element at a time, choosing the first element the first remaining arm tests (`check::tuple`): `switch (event.kind) { case "Reset": … case "Tick": switch (state.kind) { … default: return assertNever(state); } … default: return assertNever(event); }`. Every enum, `Option`, and `Result` element is matched with every case named, so TS checks exhaustiveness; integer, `char`, and string elements are `if`/`else` on one arm's pattern at a time. Elements that are not places go into `const`s first, in order (`$e1`, `$e2`); field and payload bindings are read into `$f`/`$v` names, then into the arm's own names. A body several cases reach is copied into each; cases with the same code and no bindings share a `case` list. A binding of a place with an enum, `Option`, or `Result` type prints `const s = state as State`, since an annotation would keep the narrowing of an enclosing `switch`.
- Guarded arms → an `if` chain: per arm in order, one match tests pattern and guard and another takes the body, with the scrutinee bound once. `unwrap_or`, `ok_or`, and `map` → the `match` std writes, with the receiver and an eager argument bound first.
- `for` over a `Vec`, `chars()`, `bytes()`, or `split(c)` → `for..of`; `while` → `while`. A loop that a `break` or `continue` leaves gets a label, since a bare `break` inside the `switch` a `match` prints as would leave the `switch`.
- `?` → `if (r.kind === "Err") return r;` (for `Option`, `if (r === null) return null;`). A `?` inside an expression is hoisted into a preceding `const` to preserve evaluation order.
- A `match` / `if` used as a value becomes `let x: T;` plus an assignment per arm.
- `S { a: 1, ..s }` → `({ ...s, a: 1 })`; a `?` in an explicit field exits before `s` is evaluated.
- Bindings are renamed to be unique per function (shadowing gives `x$1`). A local with the same name as an item is renamed, because a TS `const` shadows an import across the whole block.

### Closures

```ts
const scale: ((_0: I32) => I32) = ((v: I32): I32 => Int.i32.mul(v, k));
```

`?` and `return` exit the closure, so they need a return annotation in Rust; hoisting stays inside the closure body.

### Counter, in full

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

The committed golden is [examples/counter-ts](../examples/counter-ts/src).

## 4. Package

```
<out>/
  package.json  tsconfig.json
  src/
    index.ts          re-exports only
    result.ts  assert-never.ts
    int.ts  str.ts    re-export Int / Str and brands from `purecrate`
    <concept>.ts      one per public concept, kebab-case
    consts.ts         every `const` of the crate, folded to its value
    purecrate-wire.ts only with --schema
```

Each file starts with `/* generated by purecrate-ts. do not edit. */`. The output is byte-deterministic.

`tsconfig.json` is `strict` with `noUnusedLocals`, `noUnusedParameters`, and `allowUnreachableCode: false`, so the sources also pass in a consumer project that turns those on. A binding the Rust leaves unused is not printed: an unused `let` or write keeps its value as a statement (it may panic, as in Rust), an unused pattern binding becomes `_`, and an unused parameter or `for` variable gets the leading `_` TS exempts. Imports are those the printed code mentions.

`package.json` has `type: "module"`; `exports` points to `dist` (and `<package>/wire` with `--schema`). `npm run build` (also run by `prepack`) compiles with TypeScript 6 or 7, rewriting `.ts` imports to `.js`, so consumers need no TS loader and can resolve under `nodenext` or `bundler`. `version` comes from `Cargo.toml`. `private` is `true` unless `--publishable` is given: generated packages belong to private code, so `npm publish` should refuse by default, while `npm pack` and installing the tarball still work. The runtime (`packages/boundary`) is copied in as `src/purecrate-runtime.ts` and, with `--schema`, the adapter as `src/purecrate-<lib>.ts`, at the generator's revision; the schema library is the only `peerDependencies` entry. Decided 2026-09-30, after vendoring into Oxide's console showed the earlier peer-installed runtime had no place in a project that commits generated code ([91 §5](./91-real-use-candidates.md#5-oxide-name-done-locally)); it also removes the version skew between generator and runtime, and nothing has to be installed that is not on npm. The runtime's brands are keyed by string (`{ readonly "purecrate.I32": true }`), so packages that each carry a copy exchange values; a crate's own closed types keep `unique symbol` brands. `crates/cli/tests/it/package.rs` packs two generated packages, installs them into a separate project with only `zod`, passes one's `I32` to the other, runs on node, and type-checks under both resolutions with both TS versions. Inside this repository the adapters read the runtime's sources via the `purecrate-source` condition.

## 5. Caller contract

What callers of a successfully generated package must observe.

**Calling.** `State.bump(state)`, not `state.bump()`. Import flat names from the package root. Variants can be built with `Cmd.Move(a, b)` or as literals. A public function's parameter may be renamed (`inc$1`); calls are positional, so nothing changes.

**Failure.** Expected failure is a value: `Result` or `null`. `undefined` means `()`, not absence. Only overflow, division by zero, out-of-bounds indexing, and `assertNever` throw.

**Numbers.**

| Rust | TS | Caller's job |
| --- | --- | --- |
| `i8`–`i32`, `u8`–`u32`, `usize` | `I32` etc. | Bring values in with `Int.i32.of`. Raw `number` arithmetic results cannot be passed back; use `Int.i32.div` etc. |
| `f32`, `f64` | `F32`, `F64` | `Int.f32.of` / `Int.f64.of` |
| `i64`, `u64` | branded `bigint` | Do not mix with `number`. Read serde_json text with `parseJson`, not `JSON.parse` |

**Strings.** `===` matches Rust equality. JS `.length` and `[i]` are UTF-16 units, not Rust byte lengths; the output never emits them.

**Closed types.** No `of`; obtain values from public functions (`Email.parse`). Object literals and raw primitives do not type-check as the closed type (verified with `@ts-expect-error` consumers under TS 6 and 7). A value produced with `as Email` is outside the equivalence guarantee.

**Aliasing.** Arguments are not mutated, so the pre-call state remains usable. Nothing is frozen; mutating after stripping `Readonly` is outside the guarantee.

**JSON.** In-memory enums use `kind`; serde's default JSON does not. Go through the `--schema` wire schema, not `JSON.parse` output directly ([04](./04-wire.md)).

**Editing.** Never edit the package. Change the Rust and regenerate.
