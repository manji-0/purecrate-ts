# Generated TS follows kamae-ts style

Date: 2026-09-27
Reference: [iwasa-kosui/kamae-ts](https://github.com/iwasa-kosui/kamae-ts)

PureCrate's output matches kamae-ts's domain layer (Discriminated Union / pure transitions / Companion / Result). Zod, Sensitive, and port separation are outside the generated scope. Those are the job of the consumer (the boundary) of the converted package.

## 1. What we adopt

| kamae-ts | Generation rule |
| --- | --- |
| The discriminant is always `kind` | `tag` / `type` / `status` are not emitted |
| `type`; no `interface` | Avoid declaration merging |
| `Readonly<{ ... }>` | Field reassignment is blocked by the type |
| Group type and functions into a same-named Companion | `export type T` + `export const T = { ... } as const` |
| One concept per file | `event.ts` / `state.ts` / `step.ts`. The only barrel is `index.ts` |
| Function-property syntax | `apply: (s, e) => r`. `apply(s, e)` method syntax is not emitted |
| Pure transitions | The input type is the start state, the return value the end state. Prefer shapes where invalid transitions can be rejected by types |
| Expected failures are Result | `{ kind: "Ok"; value } \| { kind: "Err"; error }` |
| Errors are also `kind` unions | The crate's error enum as-is |
| `assertNever` | The `default` of a `switch` |
| Avoid classes / method syntax | `impl` goes to Companion function properties |

## 1.1 State and sequences (2026-09-28)

In kamae, state is a `Readonly` value per stage. A transition returns the next state. What happened is not accumulated into state; the caller handles it as a separate value. Persistence writing state and events together happens outside the generated package.

Sequences whose elements grow or shrink are not mutable arrays inside state. A recursive enum returns a new sequence. This corresponds to kamae's `[...lines, line]` and `filter`.

```rust
pub enum Lines {
    Empty,
    Cons(Line, Box<Lines>),
}

pub fn cons(line: Line, lines: Lines) -> Lines {
    Lines::Cons(line, Box::new(lines))
}
```

`Vec<T>` is the type for a sequence whose length the caller decides, read via indexing and `len`. Growing a `Vec`, removing elements, or replacing elements inside a transition is not in v0.

## 2. What we do not generate

The parts of kamae-ts that go beyond a closed pure crate.

- Zod / Valibot / ArkType (boundary for external input)
- `Sensitive<T>` (PII; the crate has no such type)
- Dependencies on neverthrow / fp-ts (v0; input/output types are closed within the crate)
- repository / use case / ports
- Generating time or IDs. If a transition needs them, it takes them as arguments (same as kamae-ts's `now: Date`)

If v1 chooses a Result library, a generation option may swap in neverthrow. The default is our own `result.ts`.

## 3. Built-in Result

`src/result.ts`:

```ts
export type Result<T, E> =
  | Readonly<{ kind: "Ok"; value: T }>
  | Readonly<{ kind: "Err"; error: E }>;

export const Result = {
  ok: <T, E>(value: T): Result<T, E> => ({ kind: "Ok", value }),
  err: <T, E>(error: E): Result<T, E> => ({ kind: "Err", error }),
  isOk: <T, E>(r: Result<T, E>): r is Readonly<{ kind: "Ok"; value: T }> =>
    r.kind === "Ok",
  isErr: <T, E>(r: Result<T, E>): r is Readonly<{ kind: "Err"; error: E }> =>
    r.kind === "Err",
} as const;
```

`?` maps to the following.

```ts
if (r.kind === "Err") return r;
const value = r.value;
```

## 4. File layout (counter)

```
src/
  assert-never.ts
  event.ts
  state.ts
  step.ts
  index.ts
```

`event.ts`:

```ts
export type Event =
  | Readonly<{ kind: "Inc" }>
  | Readonly<{ kind: "Dec" }>
  | Readonly<{ kind: "Reset" }>;

export const Event = {
  Inc: (): Event => ({ kind: "Inc" }),
  Dec: (): Event => ({ kind: "Dec" }),
  Reset: (): Event => ({ kind: "Reset" }),
} as const;
```

`state.ts`:

```ts
export type State = Readonly<{
  n: number;
}>;

export const State = {
  of: (n: number): State => ({ n }),
} as const;
```

`step.ts`:

```ts
import { assertNever } from "./assert-never.ts";
import type { Event } from "./event.ts";
import type { State } from "./state.ts";

export const step = (state: State, event: Event): State => {
  switch (event.kind) {
    case "Inc":
      return { n: state.n + 1 };
    case "Dec":
      return { n: state.n - 1 };
    case "Reset":
      return { n: 0 };
    default:
      return assertNever(event);
  }
};
```

`assert-never.ts`:

```ts
export const assertNever = (x: never): never => {
  throw new Error("unexpected variant");
};
```

`index.ts` contains only re-exports.

Free functions are `export const name = (...) =>`. `export function` is not used (keeps notation consistent with Companion / function properties).

## 5. Partial unions

If the reachable start states in Rust are a subset of the variants, the generator may emit a partial union.

```ts
export type Cancellable = Waiting | EnRoute | InTrip;
```

v0 emits it only when there is an explicit `type` alias. Automatic partial unions via inference are v1.

## 6. Mapping impl

```rust
impl State {
    pub fn bump(self) -> State { State { n: self.n + 1 } }
}
```

```ts
export const State = {
  of: (n: number): State => ({ n }),
  bump: (state: State): State => ({ n: state.n + 1 }),
} as const;
```

The receiver is the first parameter. `this` is not emitted. Shared references (`&self`, `&T`, `&str`, `&[T]`) map the same as values. Generated TS does not mutate values. `Cell` and `RefCell` are rejected and `Mutex<T>` is erased to `T`, so there is no need to distinguish references from values. `&mut` is rejected.

`Self` is replaced with the impl's type name before reading. `Type::method(x)` becomes the Companion function-property call `Type.method(x)`. Receiver syntax `x.method(y)` has the type checker find the receiver's type `T` and becomes `T.method(x, y)`. It resolves to the crate's own inherent impl. std methods are rejected, except `Vec` indexing and `len`.

## 6.1 newtype

A one-element tuple struct becomes a branded version of its inner type. The runtime value is the inner value itself, which also matches serde's JSON representation (a newtype serializes as its inner value).

```rust
pub struct Meters(i32);
impl Meters {
    pub fn plus(&self, other: &Meters) -> Self { Self(self.0 + other.0) }
}
```

```ts
declare const MetersBrand: unique symbol;
export type Meters = number & { readonly [MetersBrand]: true };

export const Meters = {
  of: (value: number): Meters => value as Meters,
  plus: (self: Meters, other: Meters): Meters => Meters.of(Int.i32.add(self, other)),
} as const;
```

`.0` becomes the value itself. The brand key is a `unique symbol` so that keys do not collide for a newtype of a newtype. Newtypes whose inner type is `Option`, `()`, or `!` (including via aliases) are rejected: `null & { ... }` becomes `never` and cannot carry a brand. Tuple structs with two or more elements are out of scope for v0.

## 6.2 Closures

A closure bound with `let` and called inside a function body becomes a typed arrow function.

```rust
pub fn scaled(x: i32) -> i32 {
    let k: i32 = 3;
    let scale = |v: i32| v * k;
    scale(x)
}
```

```ts
export const scaled = (x: number): number => {
  const k: number = 3;
  const scale: ((_0: number) => number) = ((v: number): number => Int.i32.mul(v, k));
  return scale(x);
};
```

- **Only immutable bindings are captured.** A Rust closure holds the captured value, but a JS closure sees the variable itself, so later reassignments would be visible. Closures that read or write a `let mut` are rejected (rebinding the current value with `let` makes it accepted).
- **Parameter types come from annotations or the expected type.** If undetermined, `|v: T|` is required. The return type is inferred from the body.
- **`?` and `return` exit the closure.** Therefore a return type annotation (`|..| -> T { .. }`) is required. `?` hoisting is done per closure body and does not leak into the outer function's statements.
- **Locals shadow items.** After `let inc = |v| ..`, `inc(x)` calls the closure. A TS `const` shadows a same-named import across the whole block (including uses before the declaration), so a local with the same name as an item is emitted renamed, e.g. `inc$1`.
- Closures as function parameters, return values, or fields (`impl Fn`, `fn` types) are out of scope for v0. Closures passed to std methods are accepted under that method's allowance (TODO 32–34).

## 7. Test data

When writing tests for the output, narrow literals with `as const satisfies Type` as in kamae-ts. This is not about the converter itself, but expected values in examples use this form.

```ts
const ev = { kind: "Inc" } as const satisfies Event;
```
