# Architecture

Status: current (2026-10-11, 0.13.1)

<!-- derived-from ./00-overview.md#3-how-it-holds-together -->

## 1. Pipeline

```
crate source (root and module files)
  → parse (syn) into IR, rejecting unknown syntax at its location
  → flatten modules, resolve names, collect the public surface and what it reaches
  → type every expression (in-house bidirectional inference) and rewrite arithmetic
  → check exhaustiveness and the subset rules; replace a last `_` arm with the cases it takes
  → rustc --crate-type lib --emit=metadata (pass/fail only)
  → print TS from the IR
  → assemble the package (package.json, tsconfig, index, runtime re-exports)
```

`--out` is replaced only on success, and only if it is absent, empty, or an earlier build's output (`src/index.ts` beginning with the generated header); any other directory is refused untouched, since replacing it would delete its files. `node_modules/` from the previous `--out` is moved onto the new tree (`check --out` already skips it); `dist/` is not, it would be stale. Each rejection carries `path:line:col` and a reason code (`Reason::code()`, e.g. `expr/method-call`). Problems inside a body point to the statement, the trailing expression, or the `match` arm; the parser marks these with `Expr::At`, removed after checking.

## 2. Crates

| Crate | Role |
| --- | --- |
| `ir` | IR data types. No dependencies, no I/O |
| `syntax` | syn → IR; `survey` |
| `check` | names, resolution, reachability, typing, exhaustiveness, `_` expansion, renaming (`check::accept`) |
| `emit_ts` | IR → TS strings, operators as a tree that sets their parentheses (`tx.rs`); wire schemas and `toJson` (`schema.rs`) |
| `pack` | package assembly |
| `cli` | `build`, `check`, `survey`. Tests: goldens, differential tests, package and wire tests |
| `canon` | test-only proc-macro: canonical value printing ([01 §8](./01-equivalence.md#8-verification)) and derive-equivalent `Serialize` impls ([04 §6](./04-wire.md#6-reading-and-writing-text)) |

- **Runtime packages.** The TS runtime and the three schema adapters are written by hand in `packages/`; `pack` embeds their sources and copies them into every generated package, the runtime cut to what the package uses ([03](./03-output.md)).
- **Purity.** File I/O is confined to `cli` and `pack`; everything else is pure.
- **Dependencies.** Dependencies are ordinary crates.io requirements, read from `vendor/` through source replacement (`.cargo/config.toml`) and built with `--offline`.

  | Used by | Crates |
  | --- | --- |
  | build | syn, quote, proc-macro2, unicode-ident |
  | tests only | serde_core, serde_json, itoa, memchr, ryu, uuid |

- **Vendoring.** `scripts/vendor.sh` regenerates `vendor/` from `Cargo.lock`. It cuts the packages no release target builds down to their manifests (uuid's wasm32 dependencies, the `serde_derive` that serde_core names only to pin its version) and the rest to their sources.

### 2.1 Inside the crates

Files follow the stages of their crate, not IR node kinds:

| Crate | Files |
| --- | --- |
| `syntax` | `item.rs` items and the lowering context, `expr.rs` expressions and blocks, `expr/pattern.rs` patterns, `match`, guards and `matches!`, `ty.rs` types, `survey.rs` |
| `check` | one file per pass (`names`, `resolve`, `exhaustive`, `position`, `wire`, `rest`, `rename`, `lift`, `unused`, `consts`, `tuple`, `reach`); typing in `types.rs` (entry, `Typer`, scopes) with `types/ops.rs` operators and literals, `types/patterns.rs` `match`, `types/calls.rs` calls, closures, construction and `?`, `types/methods.rs` method dispatch, the std allow-list and `as`, `types/iter.rs` iterator chains, `types/option_result.rs` `Option` and `Result` combinators, `types/prim.rs` integer, `char` and `str` methods |
| `emit_ts` | `lib.rs` package assembly and per-file output, `context.rs` what the printers share for one crate, `index.rs` `index.ts`, `items.rs` declarations, `join.rs` guards the decision tree tested inside an arm joined into the arm's test, and a `match` on a value an enclosing arm narrowed resolved to the arm it takes, before printing (`join/flow.rs` the narrowing state, `join/known.rs` what it decides, `join/fold.rs` what deciding leaves to fold), `stmt.rs` statements with `stmt/loops.rs`, `stmt/switch.rs`, `stmt/branches.rs` (the `if` chain), `stmt/let_else.rs` and `stmt/patterns.rs`, `expr.rs` expressions, literals and types, `imports.rs` what a file imports, `schema.rs` wire schemas, built as a small TS syntax tree (`js.rs`) laid out by a layout document and its printer (`doc.rs`, the group / indent / line model oxfmt formats with); the domain files are printed as text and broken by `tidy.rs` (`tidy/scan.rs` reads the text, the other `tidy/` files each break one kind of line) |

`stmt.rs` and `expr.rs` call each other on purpose: a statement holds expressions, and an expression that needs statements prints as an arrow function around them. The child modules of `types`, `emit_ts`, `stmt`, `join`, and `tidy` share their parent's items through `use super::*`, so the graph shows few edges between them.

Passes that only collect (reachability, emit's imports, `survey`'s references) handle the variants they care about and walk the rest through `Expr::children` and `Expr::own_types`, both matched exhaustively in `ir`. Passes that transform or check (typing, positions, lifting, renaming, printing) match every variant themselves, so a new variant is a compile error in each until it is handled.

## 3. rustc as the final gate

**What.** `check` and `build` run the input through rustc after the subset check and reject its errors as `[rustc/E0382]` etc. A passing `check` means the input compiles as a library. rustc is therefore required at run time (`RUSTC` overrides the binary).

**Why.** The subset check erases borrows and does not track moves or lifetimes, so alone it would accept programs rustc rejects (one such hole: a string literal in a `String` position).

**serde.** The crate may name `serde` for its derives; rustc gets a stand-in whose `Serialize`/`Deserialize` derives expand to nothing ([04 §3.2](./04-wire.md#32-serde-in-the-input)). The `serde` and `uuid` stand-ins are built once per `rustc -vV` and purecrate-ts version into a per-user cache (`$XDG_CACHE_HOME/purecrate-ts/rustc-stubs`, or `~/.cache/...`). rustc's metadata for the input goes in a unique 0700 directory under the process temp dir, created exclusively and removed when `check` returns.

**Edition.** The input's edition comes from `Cargo.toml` (`[package]` or inherited `[workspace.package]`, 2015 if unset); a standalone file defaults to 2021 (`--edition`).

**Type information is not read.** The typing that decides output is the in-house inference; whether to switch is open ([07 §7](./07-roadmap.md#7-open-questions)). rust-analyzer or `rustc_public` would be more precise but heavy to depend on and maintain.

## 4. IR

The IR knows neither Rust nor TS syntax, which keeps a second backend possible.

```
Ty   = Prim(Bool|Int|Float|Usize|String|Str|Unit) | Option(Ty) | Result { ok, err }
     | Vec(Ty) | Tuple([Ty]) | Named(Name) | Fn { params, ret }   (local closures)
     | Ignored { wrapper: Box|Arc, inner }                         (erased, kept for comments)
     | Never
Item = Struct | Enum | Alias | Fn
VariantFields = Unit | Tuple([Ty]) | Struct([Field])
Fn   = { name, owner: Option<Name>, vis: Pub | Internal, params, ret, body: Expr }
Expr = Lit | Var | Let | Assign | If | Match | Call | MethodCall | Construct
     | Field | Tuple | Array | Return | Unreachable | Comment | …
```

- Names are already flattened. `Vis::Pub` is the public surface, `Vis::Internal` what it reaches.
- `owner = Some(State)` makes the function a property of the companion `State`.
- `Item::file_stem` gives the kebab-case file name.
- `Option` / `Result` / `Vec` are built-in `Ty` constructors; user ADTs are items.
- `purecrate_ir::counter_example()` is the canonical IR of the acceptance example.

See `crates/ir/src` for the exact definitions; the sketch above is not maintained field by field.

## 5. CLI

```text
purecrate-ts build  <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype] [--publishable]
purecrate-ts check  <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema …] [--publishable]
purecrate-ts survey <crate-path>... [--json]
```

`<crate-path>` is a crate directory (`src/lib.rs`) or a single `.rs`. `--name` defaults to the `Cargo.toml` package name, then the directory name. `check --out` also compares bytes with an existing output, listing differing, missing, and extra files. `survey` reports, per public function and type, whether it is accepted together with everything it references ([90](./90-acceptance-survey.md)).

`scripts/verify.sh` runs `cargo test --offline`, drift detection on every example's committed output (`examples/<name>/ts/plain`, and `ts/<lib>` with each schema library where the example derives serde), and `tsc` on TypeScript 6 and 7 for the runtime packages and those outputs.
