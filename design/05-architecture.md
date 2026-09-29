# Architecture

Status: current (2026-09-29)

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

`--out` is replaced only on success. Each rejection carries `path:line:col` and a reason code (`Reason::code()`, e.g. `expr/method-call`). Problems inside a body point to the statement, the trailing expression, or the `match` arm; the parser marks these with `Expr::At`, removed after checking.

## 2. Crates

| Crate | Role |
| --- | --- |
| `ir` | IR data types. No dependencies, no I/O |
| `syntax` | syn → IR; `survey` |
| `check` | names, resolution, reachability, typing, exhaustiveness, `_` expansion, renaming (`check::accept`) |
| `emit_ts` | IR → TS strings; wire schemas and `toJson` (`schema.rs`) |
| `pack` | package assembly |
| `cli` | `build`, `check`, `survey`. Tests: goldens, differential tests, package and wire tests |
| `canon` | test-only proc-macro: canonical value printing ([01 §7](./01-equivalence.md#7-verification)) and derive-equivalent `Serialize` impls ([04 §6](./04-wire.md#6-writing-domain-values)) |

TS runtime packages are in `packages/` (`boundary` is named `purecrate`, plus the three schema adapters; they are packed from this repository, not published to npm, see [03](./03-output.md)). File I/O is confined to `cli` and `pack`; everything else is pure. Dependencies are vendored (`vendor/`: syn, quote, proc-macro2, unicode-ident; for tests only, serde_core, serde_json, itoa, memchr, ryu) and built with `--offline`. The vendored manifests point at each other by `path`, with tests, benches, and unused optional dependencies removed.

## 3. rustc as the final gate

The subset check erases borrows and does not track moves or lifetimes, so alone it would accept programs rustc rejects (one such hole: a string literal in a `String` position). Since 2026-09-28, `check` and `build` run the input through rustc after the subset check and reject its errors as `[rustc/E0382]` etc. A passing `check` means the input compiles as a library. rustc is therefore required at run time (`RUSTC` overrides the binary). The crate may name `serde` for its derives; rustc gets a stand-in built on the fly, whose `Serialize`/`Deserialize` derives expand to nothing ([04 §3](./04-wire.md#3-current-design)). The input's edition comes from `Cargo.toml` (`[package]` or inherited `[workspace.package]`, 2015 if unset); a standalone file defaults to 2021 (`--edition`).

rustc's type information is **not** read. The typing that decides output is the in-house inference; whether to switch is open ([07 §7](./07-roadmap.md#7-open-questions)). rust-analyzer or `rustc_public` would be more precise but heavy to depend on and maintain.

## 4. IR

The IR knows neither Rust nor TS syntax, which keeps a second backend possible.

```
Ty   = Prim(Bool|Int|Float|Usize|String|Str|Unit) | Option(Ty) | Result { ok, err }
     | Vec(Ty) | Tuple([Ty]) | Named(Name) | Fn { params, ret }   (local closures)
     | Ignored { wrapper: Box|Arc|Mutex, inner }                   (erased, kept for comments)
     | Never
Item = Struct | Enum | Alias | Fn
VariantFields = Unit | Tuple([Ty]) | Struct([Field])
Fn   = { name, owner: Option<Name>, vis: Pub | Internal, params, ret, body: Expr }
Expr = Lit | Var | Let | Assign | If | Match | Call | MethodCall | Construct
     | Field | Tuple | Array | Return | Unreachable | …
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

`scripts/verify.sh` runs `cargo test --offline`, drift detection on examples/counter, `check` on the other examples (order, signup, iban, payment, invoice), and `tsc` on TS 6 and 7 for the runtime packages and the counter output.
