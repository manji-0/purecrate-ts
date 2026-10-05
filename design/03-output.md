# Generated TypeScript

Status: current (2026-10-05, 0.12.0)

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
| One concept per file | `state.ts`, `event.ts`, `step.ts`; `index.ts` only re-exports. A function the package does not export, whose callers all sit in one other file, is printed in that file, unexported, after a blank line: a helper of a helper follows it (`Crate::homes`). Why: a 5-line `is-digit.ts` sent a reader of its one caller to another file; where a helper lands depends only on the source, never on how many files there are, and files stay flat, as the modules are ([02 §3.3](./02-authoring.md#33-names-are-unique-across-the-crate)) |
| Expected failure is `Result` | only `assertNever` (a plain `Error`) and `Panic` (overflow, division by zero, indexing) throw |
| Time and IDs are arguments | the domain never generates them |
| `return` ends a branch | where a value is returned, a branch that returns is an `if` the next one follows, not an `else` (`if (s === "") return ..;` then the rest); a branch of one `return` sits on its `if`'s line. A function name passed as a function (`.map(f)`, `Iter.all(xs, f)`) is `f`, not an arrow around it |
| Lines up to 100 columns | a longer line opens a comma-separated bracket, a `if (cond) return`, a long `&&` / `||` / `?:`, or an arrow body (`emit_ts::tidy::wrap`), as oxfmt (`examples/.oxfmtrc.json`, width 100) would: an arrow whose head fits keeps its parameters, opening an object, block, or array body and moving any other body after `=>`; a condition breaks at its top-level `||` before a call in it opens; a top-level `?:` splits before the `&&` or a bracket in its branches, each branch one indent past its `?` / `:`, and a `?:` in the middle operand stands without parentheses. Parentheses follow operator precedence. `crates/cli/tests/it/line_width.rs` fails on any generated domain line over the limit. Width is counted in columns, a wide East Asian character taking two, as oxfmt counts it; an `import` of one name and a template literal stay on their line, as oxfmt leaves them. A value oxfmt finds poorly breakable (a name or field chain called with nothing or one short operand) breaks after `=`; a last argument that is an arrow keeps its head on the call's line where its body fits below. `crates/cli/tests/fixtures/layout.rs` makes each of these break with long names, and verify.sh checks it with oxfmt |
| Functions, methods, parameters, locals in camelCase | `compare_pre_ids` → `comparePreIds`, `Yen::try_from` → `Yen.tryFrom`; constructor parameters too (`lastError`). Fields, types, variants, and UPPER_SNAKE consts keep the Rust name (a field is the JSON key) ([02 §3.3](./02-authoring.md)) |
| Lint and format | every example's output, the wire module included, passes oxlint, type-aware, with `examples/.oxlintrc.json`: the correctness, suspicious, and perf categories plus `consistent-type-imports`, `no-import-type-side-effects`, `no-unnecessary-type-assertion`, `no-else-return`, `no-lonely-if`, and the like, less `no-unsafe-type-assertion` (a brand is a cast; [casts.rs](../crates/cli/tests/it/casts.rs) admits each kind); each test fixture's output passes it too, less what [`scripts/fixtures.oxlintrc.json`](../scripts/fixtures.oxlintrc.json) lists as written on purpose. Every generated file, of the examples and of each test fixture that builds, is laid out as oxfmt lays it out (`examples/.oxfmtrc.json`, width 100): `scripts/verify.sh` runs `oxfmt --check` over them. The wire module is built as a syntax tree and printed with oxfmt's model (`emit_ts::doc`); the domain files are broken by `emit_ts::tidy`, rule by rule: a union on one line where it fits; a chain of `?:` split whole, a `?:` body of an arrow parenthesized on its line; a long `?:` whose test is a comparison after the `=`; a call or parameter list whose one item is not an object, array, or arrow opened around it; a string in the quote that needs fewer escapes |
| `///` comments are JSDoc | on the type, each struct field, each variant's constructor, each function and method, `const`, and alias; an editor shows the Rust documentation on hover. A comment on an `impl` block has nowhere to go; one on `impl Display` documents `toString` |
| A body reads in paragraphs | statements of one line run on; a statement over several lines (a block, or `let x: T;` and the `if` or `switch` that assigns it) has a blank line before and after it, except next to the one-line `const` its first line reads (`const m = next(cmd);` `switch (m.kind) {`) and before a `break;` or `continue;`; a `//` comment has a blank line before it. None at a block's start or end, nor between `switch` cases. Counted before `tidy::wrap` breaks a line (`emit_ts::paragraph`) |
| Reads as a person would write it | consecutive `const a = x.a;` of one place are `const { a, b } = x;` (`emit_ts::destructure`, after names are final); `!c ? a : b` is `c ? b : a`; an arm that returns the unit variant it matched returns the place, and unit arms that do the same share one arm; a variant built again from every field its arm bound, in order, is the place (`emit_ts::join`). Each keeps the value: nothing in the output changes a value once built |
| A byte literal names its character | `b'.'` is `/* '.' */ 46`, in a comparison, a pattern's test, and an argument (`Int.u8.sub(c, /* '0' */ 48 as U8)`), escaped as Rust escapes it (`/* '\'' */ 39`). Before the number, where oxfmt leaves a comment; one after it would move past a `)` or `;` |
| `//` comments in a body stay | the `//` lines above a statement, a block's tail, or a `match` arm are printed above what it becomes, so the reasons written in the source (`// RFC 6749 §10.12: …`) read in the output too. The nearest run of them is taken, across blank lines between it and the code; code, `///`, `//!`, or `/* */` ends it. A `//` after the code on a statement's last line is moved above it; the `//` lines before a block's `}`, after its last statement, end the block; the `//` lines directly above a crate `const` are printed above it in `consts.ts`, which then starts a paragraph. An arm's comment comes after the bindings its pattern makes (`const n = k.value;`). A comment separates the statements on either side of it, so a rewrite that joins two statements stops at it (`let c = b + 1;` `// …` `c` keeps `const c` rather than `return` its value). Above a value printed inside an expression (`1 + { … }`) there is no line for it, and it is left out; above a function's value it opens the body as a block (`=> {` `// …` `return …;` `}`). The parser reads them from the block's source text, as `Expr::Comment` first in a `Seq` (`crates/cli/tests/it/comments.rs`) |

### 1.1 Casts

A brand exists only in types, so TS lets any `as` make a number an `I32` or a string a `Yen`. The generated code casts only where the value is already what the type says, for a reason outside TS; `crates/cli/tests/it/casts.rs` reads the output of every example and test fixture and fails on any `as` of no kind below, and on a cast to one of the crate's brands outside its constructor.

| Kind | Example | Why the value is what the type says |
| --- | --- | --- |
| Literal | `1 as I32`, `10n as I64`, `1.0 as F64` | rustc refuses an integer literal its type cannot hold |
| `char` literal | `"." as Char` | a Rust `char` literal is one scalar value |
| Length | `xs.length as Usize` | a JS length is an integer below 2^32 |
| Widening | `(x as number as U32)`, `(globalThis.BigInt(x) as I64)` | `check` takes `T::from(x)` only where std has `From`, which is lossless |
| `for` counter | `i = (i + 1) as Usize` | `i` is below the exclusive end, so `i + 1` is at most the end |
| Discriminant | `otpDigitsDiscriminants[d.kind] as U8`, the table a `const` of the file (`const otpDigitsDiscriminants = { Six: 6, .. } satisfies Record<OtpDigits["kind"], number>;`), widened in the same cast (`as Usize`); `({ A: 1 as U8 } as Record<string, U8>)[e.kind] as U8` where `e` is neither a place nor a call, whose `kind` may have widened | the table holds the folded discriminants, each in range ([01 §7.7](./01-equivalence.md#77-const-and-discriminants)), and `satisfies` checks its keys are `E`'s |
| Float | `(a * b as F64)`, `(Math.fround(x) as F32)` | every `number` is an `f64`; `fround` gives an `f32` |
| Constructor | `unsafeMakeYen = (value: I64): Yen => value as Yen`, a newtype's `of` | the crate's own constructor, which Rust lets the crate call; a closed type's is not exported |
| Declared type | `{ kind: "A" } as Event` | a variant literal given the union type; a place whose type is already the target is not cast ([casts.rs](../crates/cli/tests/it/casts.rs) fails on identity) |
| Not a cast to a brand | `as const`, `import { A as A$ }`, arktype's `ctx.error(..) as never` | — |

A cast is parenthesized only where an operator beside it would take it apart (`a + (1 as I32)`) and, as oxfmt prints it, in a branch of `?:` (`apart ? p : (0n as I64)`); as an argument, an element, a field value, or a `const`'s value it stands bare (`Int.i32.add(n, 1 as I32)`, `Six: 6 as U8`), and so does an arrow passed as an argument (`Iter.all(xs, (b: U8): boolean => ..)`). A test that `is_empty`, `is_some`, or a `match` prints (`s.length === 0`, `k.kind === "A"`) is parenthesized by precedence like any comparison. Since 0.10.0 the printer decides this on a tree of the operators it writes (`emit_ts::tx`), not by reading its printed text back: each operand is parenthesized where its operator binds looser than its parent's or would regroup, a `!` of a comparison turns the comparison round, and a negated `&&` chain is the `||` of each side turned round. A shift amount and a literal `Slice.at` index read bare too (`Int.u32.shl(x, 24)`, `Slice.at(parts, 0)`): the runtime takes them as plain `number`s. A comparison reads a literal or a length without its brand (`n === 1`, `parts.length < 2`, `b >= 48 && b <= 57`): `===` and `<` compare the values, which a brand does not change. JS `split` takes the separator as a plain string (`s.split(".")`).

A caller's own code can write `5 as I32` all the same; no type stops it. Values from outside belong in `Int.i32.of`, the wire schemas, or the crate's functions, and a lint such as oxlint's `typescript/consistent-type-assertions` with `assertionStyle: "never"` (the generated directory left out) keeps the rest of the code from casting.

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
| `Box<T>`, `Arc<T>` | `T`, the type marked `/* Box */ T`; `Box::new(x)` is `x`. `Box` is heap indirection for a recursive type, `Arc` shared ownership across threads: a single-threaded program with values never mutated observes neither. `Mutex` is refused (`[type/mutex]`) |
| `struct S { a: T }` | `Readonly<{ a: T }>` + companion; branded if closed |
| newtype `S(T)` | `T & { readonly "<crate>.S": true }` |
| `enum` | `kind` union + companion |

**`Option`.** Nested `Option` and a newtype over `Option` are refused (`[check/nested-option]`, `[check/newtype-inner]`), because `T | null` cannot tell `None` from `Some(None)` and `null & brand` is `never`. Both show up in PATCH-style domain code (unset vs. clear vs. set); the spelling is a hand-written enum such as `Patch { Unset, Clear, Set(i32) }` ([02 §3.7](./02-authoring.md#37-types)). This is a consequence of the representation, not of Rust semantics. serde's default JSON is `null` for both `None`s of an `Option<Option<T>>`, so a server using the same types cannot distinguish them either unless it uses `serde_with`-style handling; that is why the rejection stays rather than printing only the nested occurrence as `{ kind: "Some", value } | { kind: "None" }`.

## 3. Shapes

### 3.1 Enums

```rust
enum Cmd { Quit, Add(i32), Move(i32, i32), Paint { color: String } }
```

```ts
export type Cmd =
  | Readonly<{ kind: "Quit" }>
  | Readonly<{ kind: "Add"; value: I32 }>
  | Readonly<{ kind: "Move"; content: readonly [I32, I32] }>
  | Readonly<{ kind: "Paint"; color: string }>;

export const Cmd = {
  Quit: (): Cmd => ({ kind: "Quit" }),
  Add: (value: I32): Cmd => ({ kind: "Add", value }),
  Move: (_0: I32, _1: I32): Cmd => ({ kind: "Move", content: [_0, _1] }),
  Paint: (color: string): Cmd => ({ kind: "Paint", color }),
} as const;
```

A one-field tuple variant is `{ kind: "Add"; value: I32 }`, read as `event.value`; its constructor's parameter is `value`. Two or more fields are `content`, read as `event.content[0]`, with parameters `_0`, `_1`. The JSON is serde's either way (`{"Add": 1}`, `{"Move": [1, 2]}`). Until 0.8.0 one field was also `content: [T]`.

A partial union (`type Cancellable = Waiting | EnRoute`) is emitted only from an explicit Rust `type` alias.

Constructor parameters of struct variants and of `S.of` are camelCase (`lastError`); the object keys they write stay the Rust field names (`last_error`), which are the JSON keys. Honouring `#[serde(rename_all = "camelCase")]` so the field and the key both become camelCase is a candidate, still refused ([07 §9](./07-roadmap.md#9-generated-api-stability)).

### 3.2 Structs, methods, newtypes

```rust
pub struct Meters(i32);
impl Meters { pub fn plus(&self, other: &Meters) -> Self { Self(self.0 + other.0) } }
```

```ts
declare const Meters$brand: unique symbol;
export type Meters = I32 & { readonly [Meters$brand]: true };

/**
 * Makes `Meters` values without a check.
 *
 * [..] `index.ts` does not export it.
 * @internal
 */
export const unsafeMakeMeters = (value: I32): Meters => value as Meters;

export const Meters = {
  plus: (self: Meters, other: Meters): Meters => unsafeMakeMeters(Int.i32.add(self, other)),
} as const;
```

**Newtypes.**

- A newtype's runtime value is its content. This is also serde's JSON for it.
- `.0` is the value itself.
- A closed type's brand key is a `unique symbol` its own file declares and does not export (`Meters$brand`), so no code outside the package can write it: an object literal with a closed struct's fields, the brand key written out or not, is not one. An open newtype's brand is a string (`{ readonly "geo.Id": true }`), so newtypes of newtypes do not collide and two copies of the package exchange its values; two copies do not exchange closed values, as each may hold its own invariants. Closedness still is not complete: `unsafeMakeMeters` is a file export, and a vendored import of it builds a value without a cast.
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
| `s.split_once(p)` | `Str.splitOnce(s, p)` (`p` a `char` or `&str`; `None` is `null`) |

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
| `s.split(c).collect()` / `.map(f).collect()` | `s.split(c)` as the array, `.map(f)` into `Vec<T>`, or `Iter.tryCollect(xs, f)` into `Result<Vec<T>, E>` |
| `while` | `while` |
| a loop that a `break` or `continue` leaves | the loop gets a label ([3.3.3](#333-loop-labels)) |
| `?` on `Result` | `if (r.kind === "Err") return r;` |
| `?` on `Option` | `if (r === null) return null;` |
| `match` / `if` used as a value | `let x: T;` plus an assignment per arm; where one side returns (`let x = match o { Some(v) => v, None => return e }`, or on a `Result`), the exit first and then `const x = o` with the payload read in place (`if (o === null) return e;`, `if (r.kind === "Err") return Result.err(f(r.error));`), a call matched this way bound first (`const result = Email.parse(raw);`); a `match` on a call whose arms are expressions, the call bound first and one `?:` on it |
| `let (a, b) = v` where `v` is not a place | `const [a, b] = v`, annotated only where an element may be an object literal |
| `match` (or `matches!`) on a place inside an expression, each arm an expression | `?:` on each arm's test, `||` / `&&` where arms are `true` / `false`, bindings read from the place: `(o !== null ? o : 0)`, `(k.kind === "A")`. A `matches!` of literals alone on a value that is not a place is `["00", "01"].includes(e)`, which reads `e` once; any other `match` on such a value, an inline function that evaluates it once; in an `if` condition, where it is evaluated first, a `const` before the `if` instead |
| a `match` on a place whose arms are all `true` / `false` | the test, `return (b >= 48 && b <= 57);`, not a `switch` |
| `S { a: 1, ..s }` | `({ ...s, a: 1 })` |

#### 3.3.1 Tuple match

A `match` on a tuple is split into nested `match`es, one element at a time (`check::tuple`). At each level it chooses the first element that the first remaining arm tests. A `match` with guards, or with a pattern nested in a case, is split the same way, a single value as a tuple of one; a field some arm tests inside (`verified: false`) becomes one more element where its case was chosen, read into a name for the field (`verified`; `field` / `value` / `error` for a positional field or a payload).

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
- Integer, `char`, `bool`, and string elements are `if`/`else` on one arm's pattern at a time; a `bool` test prints as `x` or `!x`.
- Elements that are not places go into `const`s first, in order (`elem`, `elem2`).
- A field or payload an arm binds is read once, into the arm's own name (`const conversion = method.conversion;`); a guard, and another arm reaching the same case, read that name (`check::binds`). `Some(m)` on a variable binds nothing: the arm reads the narrowed variable (`if (method !== null)` then `method === "S256"`, not `const m = method;`), unless the arm binds or assigns that variable or a closure in it reads it. A made name (`field`, `value`) remains only where no arm names the value.
- A body that several cases reach is copied into each. Cases with the same code and no bindings share a `case` list. A `_` (or the remaining variants of a tuple element) prints as `default:`; `assertNever` is only the `default` of a `switch` that names every variant. Hoisting an arm that ignores an earlier element (`(_, Event::Cancel)`) is a candidate.
- A binding of a place with an enum, `Option`, or `Result` type prints `const s = state as State`. An annotation would keep the narrowing of an enclosing `switch`.

#### 3.3.2 Guards and Option methods

- A `match` with guards prints as a tuple `match` does (§3.3.1), a single value as a tuple of one. Where an arm's pattern has matched, `if (guard) { body } else { .. }`, the `else` holding the arms after it that can still match. The guard reads the arm's bindings from their places.
- `unwrap_or`, `ok_or`, and `map` become the `match` that std writes. The receiver and an eager argument are bound first when they may panic or have an effect; a name, a literal, or a field is read in the arm (`x ?? d`, `if (x === null) return Result.err(e)`). `o.map(f).unwrap_or(d)` with such a `d` is one test, no `Option` held between (`o === null || c.step > o`). `Some(s) => Some(s), None => None` is `x`; on a call, the call is read there, with no binding. A tuple is not taken for itself from its elements (`Some((a, b)) => (a, b)`): the printer cannot see whether the arm read all of it. `??` has its own precedence, below `||`, and is parenthesized beside `||`, `&&`, and `?:` (JS rejects `a ?? b || c`).
- `let x = o.ok_or(e)?` is a guard instead: `if (o === null) return Result.err(e);` and `const x = o` when `o` and `e` cannot panic. Otherwise the receiver and `e` are bound first, then the same test. The `match` would build a `Result` only for `?` to take it apart.
- A `?` inside an expression is hoisted in front of its statement: `const fResult = f(x);`, `if (fResult.kind === "Err") return fResult;` (`=== null` for an `Option`, held in `fOpt`), and the expression reads `fResult.value` (`fOpt`). `let x = e?` holds `e` in `xResult` and binds the payload to `x`. A `?` on a name is the test on that name.
- A name the generator makes says what it holds: `fResult` for the `Result` of a call to `f`, `o` and `oOr` for the receiver and argument of `o.unwrap_or(..)` / `o.ok_or(..)`, `xOpt` and `xOr` for those of `let x = f(..).ok_or(..)?`, `xResult` for `let x = r.map_err(f)?` (`x2Result` for a shadow printed `x2`), the matched value of a `match` on a call after its enum (`otpCheck`), or `result`, `option`, `value`; a loop's end is `end`, its label `loop`. No `$` is printed (§3.3.5).
- `if c { return v; }` as a statement prints on one line, `if (c) return v;`, when `c` and `v` each fit on one.

#### 3.3.3 Loop labels

A loop that a `break` or `continue` leaves gets a label. A `match` prints as a `switch`, and a bare `break` inside that `switch` would leave the `switch`, not the loop.

#### 3.3.4 Evaluation order

- A `?` inside an expression is hoisted into a preceding `const`. This preserves evaluation order.
- In `S { a: 1, ..s }`, a `?` in an explicit field exits before `s` is evaluated.

#### 3.3.5 Renaming

- Bindings are numbered (`x2`, counting the first as one) only when the name is already live in the same JS scope (a prior `let` in the function body, a parameter, or an import). Match arms, `if`/`else` blocks, and loop bodies reuse the Rust name; adding an arm does not renumber the others.
- A name the generator makes is printed plain (`emit_ts::plain`): while the code is built it starts with `$`, which no Rust identifier has, and last it takes its plain spelling unless an identifier read or declared in the innermost block holding its uses has it, or another made name of an overlapping block took it; then the next number (`result2`). So it never captures or hides a source name. Until 0.8.0 the `$` was printed (`$v_major`, `$m_3_$t`, `x$1`).
- A local with the same name as an item is renamed, because a TS `const` shadows an import across the whole block: a type or `const` always, a function only where a function of the same file calls or reads it, since only then does the file import it (a parameter `total` keeps its name where nothing in its file calls `total`; `let total = total(&lines)` is `total2`); a function the package does not export always, so that a name read after renaming is the helper when it is a helper's.

### 3.4 Closures

```ts
const scale = (v: I32): I32 => Int.i32.mul(v, k);
```

`?` and `return` exit the closure, so they need a return annotation in Rust. Hoisting stays inside the closure body.

### 3.5 Counter, in full

```ts
export const step = (state: State, event: Event): State => {
  switch (event.kind) {
    case "Inc":
      return { n: Int.i32.add(state.n, 1 as I32) };
    case "Dec":
      return { n: Int.i32.sub(state.n, 1 as I32) };
    case "Reset":
      return { n: 0 as I32 };
    default:
      return assertNever(event);
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

A test TS has already decided by narrowing is folded, not printed, since TS refuses it (`"Err"` and `"Ok"` have no overlap, `null ?? d`, unreachable code): a `match` on a place inside an arm that matched it, or after a `?` or a `map_err(f)?` on it; a `match` on `None`, `Some(e)`, `Ok(e)`, or `Err(e)`, also through a `let`; `is_some()` of one; two literals compared. Each holds only while nothing writes the place (`let mut o` assigned in an arm or after). The bindings and `let`s that folding leaves unread go as above (`emit_ts::join`, `crates/cli/tests/fixtures/narrowing.rs`).

`Result.ok(v)` is `Result<T, never>` and `Result.err(e)` is `Result<never, E>` where nothing names the other type parameter (the runtime's defaults, since 0.10.0), so a `?:` of the two, or of one and a `Result`, is the `Result` it reads as.

### 4.3 `package.json` and build

| Field or script | Value |
| --- | --- |
| `type` | `"module"` |
| `exports` | `dist` (and `<package>/wire` with `--schema`); under the `purecrate-source` condition, the `.ts` sources |
| `version` | from `Cargo.toml` |
| `license` | from `Cargo.toml` (`MIT` if unset), so a `--publishable` package is not shown as unlicensed |
| `private` | `true` unless `--publishable` is given |
| `sideEffects` | `false` |
| `engines` | `"node": ">=21"` with `--schema` (`parseJson` needs `JSON.parse` source text) |
| `peerDependencies` | only the schema library |
| `files` | `dist` and `src` |
| `npm run build` | `tsc -p tsconfig.build.json`, TypeScript 6 or 7; also run by `prepack` |

The build rewrites `.ts` imports to `.js`. Consumers need no TS loader and can resolve under `nodenext` or `bundler`.

`private` defaults to `true` because generated packages belong to private code. `npm publish` should refuse by default. `npm pack` and installing the tarball still work.

### 4.4 Runtime, copied in

**What.** The runtime (`packages/boundary`) is copied in as `src/purecrate-runtime.ts`. With `--schema`, the adapter is copied in as `src/purecrate-<lib>.ts`. Both are copied at the generator's revision. The schema library is the only `peerDependencies` entry.

**Only what the package uses.** The runtime marks its parts with region and needs comments; `pack` keeps a part when the package's other files use it, and leaves the markers out (`crates/pack/src/trim.rs`). A use is a member read through a namespace (`Int.<ty>.<op>`, down to each operator and method; `Str.<member>`, `Json`) or an identifier read in code (`Result`, `I32`, `Char`, `Uuid`). What the kept copy reads in turn is kept too, until it reads nothing new (`trim::trim_closed`): `Int.i32.add` keeps `small`, which keeps `panic` and `I32`. An `Int` width is kept when the code reads it or the index exports its brand; `Panic` and `assertNever` always are. A namespace whose members are all trimmed is dropped (`export const Iter = {}` is not kept), and so is all of what the index exports to callers. The index re-exports `Result` and each of `I8`…`F64` only when the public surface holds that type, `Int` when it holds one of these, `Char` when it holds a `char`, `Uuid` when it holds a `Uuid`, and `parseJson` with `--schema` (`trim::exported`). Why: the runtime's `Int` is one object, so a bundler cannot drop the widths and methods a package does not use, and a caller that loads `src/` or `dist/` directly gets no bundler at all. Two packages re-exported from one barrel then only share the names both surfaces actually use. payment's copy is 4.5 KB of the runtime's 36 KB, counter's 3.2 KB.

**Brands.** The runtime's brands are keyed by string, `purecrate.` and the type (`{ readonly "purecrate.I32": true }`), and so are a crate's open newtypes, by the crate's name and the type (`{ readonly "invoice.Id": true }`): packages that each carry a copy of the runtime exchange values, and so do two copies of one crate's package, as two installed versions would be; the key reads in a hover or a type error. A closed type (newtype or struct) is keyed by a `unique symbol` its file declares and does not export, so a literal outside the package cannot be one (a string key can be written out); two copies of the package do not exchange closed values.

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
- Overflow, division by zero, out-of-range shifts, and out-of-bounds indexing throw `Panic` (a subclass of `Error`). `message` is Rust's panic text, so `e.message` still matches a debug build. `instanceof Panic` works across copies of the runtime (`Symbol.for("purecrate.Panic")`). `assertNever` still throws a plain `Error` (`"unexpected variant"`): that is a generator bug, not a domain panic.

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
- `unsafeMakeEmail` is a file export, not an index export, and `@internal`. An installed package cannot import it through `exports`; vendored sources can (`import { unsafeMakeEmail } from "./gen/src/email.ts"`). Closedness is that convention plus an `as` lint, not a unique-symbol seal.

### 5.6 Aliasing

Arguments are not mutated, so the pre-call state remains usable. Nothing is frozen. Mutating after stripping `Readonly` is outside the guarantee.

### 5.7 JSON

In-memory enums use `kind`; serde's default JSON does not. Go through the `--schema` wire schema, not `JSON.parse` output directly ([04](./04-wire.md)).

### 5.8 Editing

Never edit the package. Change the Rust and regenerate.
