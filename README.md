# purecrate-ts

Converts pure domain functions written in Rust into a TypeScript package without WASM. The output consists of ordinary TS values, passes `tsc --strict`, and for accepted inputs returns the same results as a Rust debug build.

It does not compile arbitrary Rust. The target is new code written within [the PureCrate constraints](design/07-authored-constraints.md). The intended use is representing states and events as ADTs and sharing transitions such as `fn step(state, event) -> Result<State, Error>`. For the overall design, see [design/00-foundations.md](design/00-foundations.md).

## Requirements

- Rust (edition 2021). Dependencies are in `vendor/` and build with `cargo --offline`. `check` and `build` also compile the input with `rustc`, so `rustc` is needed at run time too (override with `RUSTC`). The input's edition is read from `Cargo.toml` (`[package] edition`, or `[workspace.package]` when inherited; 2015 if unspecified, as with cargo). A standalone file without `Cargo.toml` uses 2021, overridable with `--edition`.
- Type-checking the output and differential tests against Rust need Node and `npx`. Type checking runs on both TypeScript 6 and 7 (fetching `npx -p typescript@6` and `@7`).

## Usage

```sh
cargo run --offline -p purecrate-ts -- build examples/counter --out /tmp/counter-ts
```

`<crate-path>` is a crate directory (`src/lib.rs`) or a single `.rs` file. If `--name` is omitted, the package name from `Cargo.toml` is used.

```text
purecrate-ts build <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype]
purecrate-ts check <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype]
purecrate-ts survey <crate-path>... [--json]
```

`check` rejects unacceptable definitions with `path:line:col` and a reason code, and writes no files. After the subset check passes, it runs the input through rustc and, if it does not compile, rejects with rustc's error code, e.g. `[rustc/E0382]`. If `check` passes, the input compiles as a library. With `--out`, it also checks byte equality with existing output. `survey` outputs JSON saying whether public functions and public types are acceptable together with everything they reference.

The output is an npm package. `npm run build` emits JavaScript and declarations to `dist`, which `exports` points to (it runs automatically before `npm pack` and `npm publish`). The runtime `purecrate` and the schema adapters are `peerDependencies`, and `version` is taken from the crate's `Cargo.toml`. `purecrate` is not yet published to npm, so for now pack it from `packages/` and install it.

Numeric brands live in the `purecrate` package. Only with `--schema` are wire schemas for that library emitted to `src/purecrate-wire.ts`. They read serde's default JSON into the domain's branded types. Schemas for libraries not specified are not emitted. serde_json writes `i64` / `u64` as JSON numbers, so read JSON text with `purecrate`'s `parseJson`, not `JSON.parse`. Integers above 2^53 then become `bigint` without loss.

Do not edit generated packages. To change them, change the Rust and regenerate.

## Testing

```sh
./scripts/verify.sh
```

Runs, in order: `cargo test --offline`, drift detection against the examples/counter output, `check` on examples/order, and `tsc` on TypeScript 6 and 7 for the runtime packages (`packages/`) and the counter output. Differential tests run the same inputs in both Rust and the generated TS (Node) and compare.

## What is accepted

Roughly, v0 can express the following. For details and constraints on callers of the generated TS, see [design/07-authored-constraints.md](design/07-authored-constraints.md). For the full set of limits and the order in which to lift them, see [design/08-limits-and-roadmap.md](design/08-limits-and-roadmap.md).

- structs, enums (`kind` discriminated unions), single-element tuple structs (newtypes)
- `Option`, `Result`, `?`, `if let`, exhaustive `match`
- Local `let mut`. Updates return new values. `&mut` is rejected
- Integer arithmetic matches a Rust debug build. Overflow and division by zero throw. `i64` / `u64` are `bigint`
- Integers of different widths are converted with `i64::from(x)`, only for widenings that have a `From` in std
- Fixed strings are built with `String::from("…")`. `String` and `&str` are compared with `==`
- Local closures that capture only immutable bindings
- Struct update `S { a: e, ..base }`
- Growing sequences are recursive enums. `Vec` is read by index and `len` for sequences whose length is fixed outside

No decimal types. Write money as an integer newtype in the smallest unit (`struct Yen(i64)`). State types do not hold mutable arrays.

## Design notes

| Document | Contents |
| --- | --- |
| [design/00-foundations.md](design/00-foundations.md) | Subset, type mapping, pipeline |
| [design/02-kamae-ts-emit.md](design/02-kamae-ts-emit.md) | Shape of the generated TS |
| [design/04-objective-means-demand.md](design/04-objective-means-demand.md) | Objective, equivalence, semantic decisions |
| [design/05-type-sharing-scope.md](design/05-type-sharing-scope.md) | Boundary with JSON |
| [design/06-acceptance-survey.md](design/06-acceptance-survey.md) | Record of measuring existing crates. Not a metric going forward |
| [design/07-authored-constraints.md](design/07-authored-constraints.md) | Constraints when writing new code, and constraints remaining on the TS side |
| [design/08-limits-and-roadmap.md](design/08-limits-and-roadmap.md) | Full set of limits, and the order to lift them |

## License

The crate's `license` is MIT.
