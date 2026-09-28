# Public surface, flattening, and version plan

Date: 2026-09-27
Status: Final

## 1. Public surface

`pub` items of the input crate become the package's exports as-is.

Included:

- `pub struct` / `pub enum` / `pub type`
- `pub fn` (free functions)
- `impl T { pub fn ... }` (receivers are normalized to the first parameter)
- Non-public types and functions reachable from the above (emitted as implementation details, without TS `export`)

Excluded:

- `pub(crate)` / `pub(super)` / private items not reachable from the public surface
- `const` / `static` (v0; deferred because they require value folding)
- Trait definitions themselves

Field visibility: non-`pub` fields are also emitted to TS. Rust module boundaries disappear, so only `readonly` is added, meaning the generated side cannot rewrite them. Information hiding via module privacy is not reproduced in v0.

<!-- constrained-by ./04-objective-means-demand.md#16-sharing-validation-and-public-constructors -->

Revised 2026-09-29: construction of a struct with non-public fields is closed, as in Rust. Fields remain readable via `readonly` as before. However, the type gets a brand, and its companion does not emit `of`. The only way to obtain a value from outside the crate is through public functions. Reading fields is not hidden.

## 2. Flattening

Module paths are not used in generated names.

```
crate
  src/lib.rs        pub struct State
  src/event.rs      pub enum Event
  src/step.rs       pub fn step
```

All of these become the flat names `State` / `Event` / `step`, and per rule 6 are emitted to `state.ts` / `event.ts` / `step.ts`.

Rules:

1. The defined name (or the name at the `pub use` target) is the public name
2. If the same public name appears twice, reject. The diagnostic lists both module paths
3. Only names that are neither Rust keywords nor TS reserved words are allowed. On collision, reject (do not rename)
4. A method `impl State { pub fn apply }` becomes the Companion `State.apply` (a function property). It does not collide with a free function `apply`
5. For names, only collisions between free functions and between types are checked
6. Each public concept is emitted to its own kebab-case file (`State` → `state.ts`). No catch-all files. If file names collide, reject even between a type and a function (type `Command` and function `command` both map to `command.ts`)

Reached non-public items are flattened too. If there are two non-public `fn helper`s, reject as with a public name collision. No automatic prefix is added. Names must be unique in the input crate.

## 3. Generics (decided)

The only type constructors allowed in v0 are the three built-ins.

- `Option<T>`
- `Result<T, E>`
- `Vec<T>`

User-written `struct Foo<T>` / `fn id<T>(x: T) -> T` are rejected.

Reasons:

- The core of a transition function is `match` over closed ADTs; map that correctly first
- User generic types require choosing between monomorphization and TS generics on the generated side
- Keep v0 checks (type closure, exhaustiveness) simple

To add in v1:

- Unbounded type parameters (just `T`)
- Emit them as generics on the TS side too (no monomorphization)
- Bounds, `where`, and associated types are rejected even in v1

## 4. Map (decided)

v0 rejects `HashMap` / `BTreeMap`.

Reason: Rust key equality is by value; JS `Map` object keys are by identity. Lowering to `Record<string, V>` is only possible when keys are strings. Adding it halfway would change the meaning of transitions.

v1: `ReadonlyMap<string, V>` only when the key is `String`. Insertion order is not made to match Rust `HashMap` (public functions depending on order are candidates for rejection).

## 5. WASM (decided)

The v0 artifact is a TS source package only.

The IR is data that does not know its print target. Room is left to add `emit_wasm` later. WASM is not part of v0's completion criteria. Mapping the same IR to WASM is optional work after TS printing has stabilized on the counter example.

## 6. Result is not an exception

`?` maps to an early `return { kind: "Err", error }`. Domain functions do not `throw`. A failed transition is also a value. Only `assertNever` throws, as an unexpected fault when exhaustiveness is broken (kamae-ts's "the unexpected is an exception").

## 7. Counter example (acceptance input)

No attributes. Splitting into modules yields the same result after flattening.

```rust
pub enum Event {
    Inc,
    Dec,
    Reset,
}

pub struct State {
    pub n: i32,
}

pub fn step(state: State, event: Event) -> State {
    match event {
        Event::Inc => State { n: state.n + 1 },
        Event::Dec => State { n: state.n - 1 },
        Event::Reset => State { n: 0 },
    }
}
```

The expected public TS is kamae-ts form. Details and file splitting are in `02-kamae-ts-emit.md`.
