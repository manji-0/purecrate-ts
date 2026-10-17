# Architecture

Status: current (2026-10-16, 0.23.0)

<!-- derived-from ./00-overview.md#3-how-it-holds-together -->

## 1. Pipeline

```
crate source (root and module files)
  → refuse a [profile.release] without overflow-checks (cli; --allow-overflow-wrap warns instead)
  → parse (syn) into IR, flattening modules, rejecting unknown syntax at its location (syntax)
  → check::accept: subset checks, typing, rewrites (check; below)
  → rustc --crate-type lib --emit=metadata (pass/fail only)
  → keep the public surface and what it reaches (check::prune_unreachable)
  → print TS from the IR and assemble the package: package.json, tsconfig, index, runtime copies (emit_ts, pack)
  → lay the files out with oxfmt (cli)
  → write --out
```

`check::accept` (`crates/check/src/lib.rs`) runs, in order:

1. `order::derive`: a `cmp` for each type that derives `PartialOrd` / `Ord`, written as the derive's code reads, so the rest checks it as ordinary code.
2. The checks that need no types, all run so every problem is reported: `names` (unique, file-safe names), `resolve` (every reference exists with the shape it assumes), `exhaustive` (`match` arms), `position` (`return` and `?` where the printer can keep them), `wire` (`try_from`'s `TryFrom` impl).
3. `types::elaborate`: in-house bidirectional inference, one expression tree at a time, and the rewrite of numeric code into `Int` calls; on the way it folds constants (`consts`), lowers tuple and slice `match`es into decision trees (`tuple`), judges map keys (`keys`), and passes trait dictionaries.
4. `traits::forward`: the trait defaults an `impl` does not write, as members of the type's companion.
5. `complete`: nothing is left for the printer to guess (a literal's width, a `?`'s kind).
6. The rewrites: `rest` (a last `_` arm becomes the cases it takes), `rename` (one name per binding in each JS scope), `grow` (the local `Vec`s a function writes), `lift` (each `?` into its own `let`), `binds` (a decision tree's bindings merged with the arm's), `unused` (bindings Rust leaves unused dropped), `std_dicts` (the std trait dictionaries a call passes, as crate consts).

A translated `impl Display` that fails these is dropped rather than refused, and `accept` runs again without it (design/01 §7.25); a function that printed it then fails in turn.

`--out` is replaced only on success, and only if it is absent, empty, or an earlier build's output (`src/index.ts` beginning with the generated header); any other directory is refused untouched, since replacing it would delete its files. `node_modules/` from the previous `--out` is moved onto the new tree (`check --out` already skips it); `dist/` is not, it would be stale. Each rejection carries `path:line:col` and a reason code (`Reason::code()`, e.g. `expr/method-call`). Problems inside a body point to the statement, the trailing expression, or the `match` arm; the parser marks these with `Expr::At`, removed after checking.

## 2. Crates

| Crate | Role |
| --- | --- |
| `ir` | IR data types. No dependencies, no I/O |
| `syntax` | syn → IR, modules flattened; `survey` |
| `check` | the subset checks, typing, and rewrites of §1 (`check::accept`), and reachability (`prune_unreachable`) |
| `emit_ts` | IR → TS strings, operators as a tree that sets their parentheses (`tx.rs`); wire schemas and `toJson` (`schema.rs`) |
| `pack` | package assembly, the runtime copy cut to what the package uses (`trim.rs`) |
| `cli` | `build`, `check`, `test` (`difftest/`: the Rust harness and the node driver), `survey`; rustc and oxfmt as programs. Tests: goldens, differential tests, the eight seeded generators (`generated_*_equivalence.rs`), package and wire tests |
| `canon` | test-only proc-macro: canonical value printing ([01 §8](./01-equivalence.md#8-verification)) and derive-equivalent `Serialize` impls ([04 §6](./04-wire.md#6-reading-and-writing-text)) |

- **Runtime packages.** The TS runtime and the three schema adapters are written by hand in `packages/`; `pack` embeds their sources and copies them into every generated package, the runtime cut to what the package uses ([03 §4.4](./03-output.md#44-runtime-copied-in)).
- **Purity.** File I/O is confined to `cli`; everything else is pure. `cli` also runs programs: `rustc` on the input, oxfmt over the output (`build` and `check --out`; `oxfmt.rs`), since oxc's formatter is not published as a crate, and for `test` the compiled harness and node.
- **Dependencies.** Dependencies are ordinary crates.io requirements, read from `vendor/` through source replacement (`.cargo/config.toml`) and built with `--offline`.

  | Used by | Crates |
  | --- | --- |
  | build | syn, quote, proc-macro2, unicode-ident |
  | run time, outside Cargo | `rustc`; oxfmt 0.71.0 from npm (`PURECRATE_OXFMT`, else `node_modules/.bin` in or above the output's directory, the input, or the working directory, else `PATH`) |
  | tests only | serde, serde_derive (the real derive, for `wire_attrs.rs`), serde_core, serde_json, itoa, memchr, ryu, uuid |

- **Vendoring.** `scripts/vendor.sh` regenerates `vendor/` from `Cargo.lock`. It cuts the packages no release target builds (uuid's wasm32 dependencies) down to their manifests, and the rest to their sources.

### 2.1 Inside the crates

Files follow the stages of their crate, not IR node kinds:

| Crate | Files |
| --- | --- |
| `syntax` | `item.rs` items and the lowering context, `modules.rs` a crate of several modules as one flat list, `traits.rs` traits and `impl`s, `serde_attrs.rs` the `#[serde(..)]` keys modeled, `std_ordering.rs` `std::cmp::Ordering`, `expr.rs` expressions and blocks, `expr/pattern.rs` patterns, `match`, guards and `matches!`, `expr/format.rs` `format!`, `expr/panic.rs` `panic!`, `unreachable!`, `assert!`, `ty.rs` types, `survey.rs` |
| `check` | the passes of §1, one file each (`order`, `names`, `resolve`, `exhaustive`, `position`, `wire`, `traits`, `complete`, `rest`, `rename`, `grow`, `lift`, `binds`, `unused`, `std_dicts`, `reach`); what typing calls (`consts` constant folding, `tuple` decision trees, `keys` map keys, `defs` the crate's definitions by name, with aliases expanded); typing in `types.rs` (entry, `Typer`, scopes) with `types/ops.rs` operators and literals, `types/patterns.rs` `match`, `types/calls.rs` calls, closures, construction and `?`, `types/methods.rs` method dispatch, the std allow-list and `as`, `types/prim.rs` integer, `char` and `str` methods, `types/iter.rs` iterator chains, `types/option_result.rs` `Option` and `Result` combinators, `types/access.rs` `get` / `first` / `last` / `contains`, `types/tree.rs` `BTreeMap` / `BTreeSet`, `types/format.rs` `format!`'s arguments, `types/ordering.rs` `cmp` and `Ordering`, `types/generic.rs` type parameters, `types/traits.rs` calls through a bound's dictionary, `types/convert.rs` `From` and `Default` |
| `emit_ts` | `lib.rs` package assembly and per-file output, `context.rs` what the printers share for one crate, `index.rs` `index.ts`, `items.rs` declarations, `lengths.rs` the array lengths a `pub` function checks on entry, `join.rs` guards the decision tree tested inside an arm joined into the arm's test, and a `match` on a value an enclosing arm narrowed resolved to the arm it takes, before printing (`join/flow.rs` the narrowing state, `join/known.rs` what it decides, `join/fold.rs` what deciding leaves to fold, `join/query.rs` questions about an expression), `stmt.rs` statements with `stmt/loops.rs`, `stmt/switch.rs`, `stmt/branches.rs` (the `if` chain), `stmt/let_else.rs`, `stmt/patterns.rs`, and `stmt/query.rs`, `expr.rs` expressions, literals and types, `tx.rs` operators as a tree, `destructure.rs` field reads joined into one `const { a, b } = x`, `paragraph.rs` blank lines between statements, `plain.rs` plain names for the `$` names the compiler made, `imports.rs` what a file imports, `schema.rs` wire schemas, built as a small TS syntax tree (`js.rs`) laid out by a layout document and its printer (`doc.rs`, the group / indent / line model oxfmt formats with); the domain files are printed as text, which `tidy.rs` reads where the printer still decides by it (`tidy/scan.rs` the text, `tidy/operators.rs` a top-level `&&` / `||`), and laid out by oxfmt in `cli` |
| `cli` | `main.rs` the commands, `args.rs` options and `Cargo.toml`, `files.rs` the module files `mod x;` reaches, `rustc.rs` rustc and its stand-ins, `oxfmt.rs`, `drift.rs` `check --out`, `survey.rs`, `difftest/` `test` |

`stmt.rs` and `expr.rs` call each other on purpose: a statement holds expressions, and an expression that needs statements prints as an arrow function around them. The child modules of `types`, `emit_ts`, `stmt`, `join`, and `tidy` share their parent's items through `use super::*`, so the graph shows few edges between them.

Passes that only collect (reachability, emit's imports, `survey`'s references) handle the variants they care about and walk the rest through `Expr::children` and `Expr::own_types`, both matched exhaustively in `ir`. Passes that transform or check (typing, positions, lifting, renaming, printing) match every variant themselves, so a new variant is a compile error in each until it is handled.

## 3. rustc as the final gate

**What.** `check` and `build` run the input through rustc after the subset check and reject its errors as `[rustc/E0382]` etc. A passing `check` means the input compiles as a library. rustc is therefore required at run time (`RUSTC` overrides the binary).

**Why.** The subset check erases borrows and does not track moves or lifetimes, so alone it would accept programs rustc rejects (one such hole: a string literal in a `String` position).

**serde and uuid.** The crate may name `serde` for its derives; rustc gets a stand-in whose `Serialize`/`Deserialize` derives expand to nothing ([04 §3.2](./04-wire.md#32-serde-in-the-input)), and a stand-in `uuid` with the part of its API the subset accepts. Both are built once per `rustc -vV` and purecrate-ts version into a per-user cache (`$XDG_CACHE_HOME/purecrate-ts/rustc-stubs`, or `~/.cache/...`). rustc's metadata for the input goes in a unique 0700 directory under the process temp dir, created exclusively and removed when `check` returns.

**Edition.** The input's edition comes from `Cargo.toml` (`[package]` or inherited `[workspace.package]`, 2015 if unset); a standalone file defaults to 2021 (`--edition`).

**Type information is not read.** The typing that decides output is the in-house inference; whether to switch is open ([07 §7](./07-roadmap.md#7-open-questions)). rust-analyzer or `rustc_public` would be more precise but heavy to depend on and maintain.

## 4. IR

The IR knows neither Rust nor TS syntax, which keeps a second backend possible.

```
Ty   = Prim(Bool | I8..I128 | U8..U128 | Usize | F32 | F64 | String | Str | Char
            | Uuid | UuidError | ParseIntError | Unit)
     | Option(Ty) | Result { ok, err } | Vec(Ty) | Array { elem, len }
     | Map { key, value } | Set(Ty) | Tuple([Ty])
     | Named(Name) | App { name, args } | Param(Name)                (generics)
     | Fn { params, ret }                                             (local closures)
     | Ignored { wrapper: Box|Arc, inner }                            (erased, kept for comments)
     | Never
Item = Struct | Enum | Alias | Fn | Const | Trait
VariantFields = Unit | Tuple([Ty]) | Struct([Field])
Fn   = { name, owner: Option<Name>, vis: Pub | Internal, params, ret, body: Expr, .. }
Expr = Lit | Var | Let | Assign | If | Match | Call | MethodCall | Construct
     | Field | Index | Cast | Tuple | Array | Closure | Binary | Unary
     | For | ForEach | While | Break | Continue | Seq | Try | Return
     | Unreachable | Comment | Ignored | At
```

- Names are already flattened. `Vis::Pub` is the public surface, `Vis::Internal` what it reaches.
- `owner = Some(State)` makes the function a property of the companion `State`.
- `Item::file_stem` gives the kebab-case file name.
- `Option` / `Result` / `Vec` / arrays / maps / sets are built-in `Ty` constructors; user ADTs are items.
- `purecrate_ir::counter_example()` is the canonical IR of the acceptance example.

See `crates/ir/src` for the exact definitions; the sketch above is not maintained field by field.

## 5. CLI

```text
purecrate-ts build  <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype] [--publishable] [--allow-overflow-wrap]
purecrate-ts check  <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema …] [--publishable] [--allow-overflow-wrap]
purecrate-ts test   <crate-path> [--name <crate>] [--edition <year>] [--cases <n>] [--seed <n>] [--skip <fn>].. [--timeout <s>] [--keep <dir>] [--allow-overflow-wrap]
purecrate-ts survey <crate-path>... [--json] [--all-causes]
```

`<crate-path>` is a crate directory (`src/lib.rs`, else `src/main.rs`) or a single `.rs`. `--name` defaults to the `Cargo.toml` package name, then the directory name. `check` and `build` refuse a crate whose (or whose workspace's) `[profile.release]` does not set `overflow-checks = true`, since the output panics on overflow as a debug build does; `--allow-overflow-wrap` makes that a warning. `check --out` also compares bytes with an existing output, listing differing, missing, and extra files. `test` calls the crate's public functions in Rust and in the generated TS on drawn arguments and compares the results ([01 §8.4](./01-equivalence.md#84-a-users-crate-purecrate-ts-test)). `survey` reports, per public function and type, whether it is accepted together with everything it references ([90](./90-acceptance-survey.md)).

`scripts/verify.sh` checks that the install docs and the runtime packages carry the workspace version and that oxfmt's pinned version is the one installed, then runs `cargo test --offline`, drift detection on every example's committed output (`examples/<name>/ts/plain`, and `ts/<lib>` with each schema library where the example derives serde), `tsc` on TypeScript 6 and 7 for the runtime packages and those outputs, and oxlint and `oxfmt --check` over the generated output. `scripts/gen-sweep.sh` runs the seeded generators over many seeds, locally.
