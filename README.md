# purecrate-ts

Share **behavior**, not just types, between Rust and TypeScript.

purecrate-ts translates pure domain functions written in Rust into an ordinary TypeScript package, without WASM. The output is plain `Readonly` values and functions that pass `tsc --strict`. For every accepted input they return the same result as a Rust debug build, and differential tests check this. Anything whose meaning cannot be preserved is rejected with its location, and no output is written.

It is not a compiler for arbitrary Rust. You write new domain code within [the PureCrate constraints](design/02-authoring.md): states and events as ADTs, and transitions such as `fn step(state, event) -> Result<State, Error>`. Start with [design/00-overview.md](design/00-overview.md).

## Requirements

- Rust (edition 2021). Dependencies are vendored; build with `cargo --offline`. `check` and `build` also run the input through `rustc`, so `rustc` is needed at run time too (override with `RUSTC`); the input may use `serde`, for which `check` gives rustc a stand-in. The input's edition comes from `Cargo.toml` (`[package]` or inherited `[workspace.package]`; 2015 if unset). A standalone file uses 2021 unless you pass `--edition`.
- Node and `npx` to type-check the output and to run differential tests. Type checking runs on TypeScript 6 and 7.

## Usage

```sh
cargo run --offline -p purecrate-ts -- build examples/counter --out /tmp/counter-ts
```

```text
purecrate-ts build  <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype]
purecrate-ts check  <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype]
purecrate-ts survey <crate-path>... [--json]
```

- `<crate-path>` is a crate directory (`src/lib.rs`) or a single `.rs` file. `--name` defaults to the `Cargo.toml` package name.
- `check` writes nothing. It rejects out-of-subset input as `path:line:col` plus a reason code, then rustc errors as e.g. `[rustc/E0382]`. With `--out`, it also compares the result byte for byte with an existing output.
- The output is an npm package. `npm run build` emits `dist` (it also runs before `npm pack` and `npm publish`). The runtime `purecrate` and the schema adapters are `peerDependencies`. `version` comes from `Cargo.toml`. `purecrate` is not yet on npm, so pack it from `packages/`.
- `--schema` emits `src/purecrate-wire.ts`, which reads serde's default JSON into the domain's branded types and writes it back with `toJson.T(x)`, the same bytes serde_json writes. Read JSON text with `purecrate`'s `parseJson`, not `JSON.parse`, so that `i64`/`u64` above 2^53 stay exact.
- Never edit generated packages. Change the Rust and regenerate.

## What you can write

- structs, enums (`kind` discriminated unions), newtypes, `Option`, `Result`, `?`, `if let`, exhaustive `match` (with `A | B` arms binding nothing and a last `_`)
- byte literals `b'@'`, integer literal and range patterns in `match` (ending in `_`) and `matches!(b, b'0'..=b'9')`
- local `let mut` (updates return new values), local closures, struct update `S { a, ..base }`, `for i in a..b`
- integer arithmetic with debug-build semantics (overflow and division by zero throw); `i64`/`u64` as `bigint`; widening with `i64::from(x)`
- growing sequences as recursive enums; `Vec` read by index and `len`
- `String::from("…")`, string `==`, `len` (UTF-8 bytes), `is_empty`, `starts_with` / `ends_with` / `contains` with a string needle, string contents through `s.as_bytes()`
- structs with private fields stay closed: TS gets values only from your public constructors
- `#[derive(Serialize, Deserialize)]` on the same types, so the server uses them as its wire format; `#[serde(try_from = "T")]` with `impl TryFrom<T>` reads a closed type through its constructor on both sides; `impl Display` / `Error` are allowed and not translated

There is no decimal type. Write money as an integer newtype in the smallest unit (`struct Yen(i64)`). For the full rules, see [design/02](design/02-authoring.md). For what TS callers must observe, see [design/03 §5](design/03-output.md#5-caller-contract).

## Testing

```sh
./scripts/verify.sh
```

This runs `cargo test --offline` (goldens, differential tests that run the same inputs through Rust and the generated TS, and wire tests against the vendored serde_json), drift detection on examples/counter, `check` on the other examples, and `tsc` on TypeScript 6 and 7 for the runtime packages and the counter output.

`bench/payment/measure.sh` compares the generated TS with wasm-bindgen on the same source; it needs the network and a `wasm32-unknown-unknown` target ([bench/payment](bench/payment/README.md)).

## Design documents

| Document | Contents |
| --- | --- |
| [00-overview](design/00-overview.md) | Claim, focus, current state, map of the documents |
| [01-equivalence](design/01-equivalence.md) | What "same result" means, the domain, known gaps, verification |
| [02-authoring](design/02-authoring.md) | What the Rust author can write, and how |
| [03-output](design/03-output.md) | The shape of the generated TS, and the caller contract |
| [04-wire](design/04-wire.md) | Reading serde JSON into domain values |
| [05-architecture](design/05-architecture.md) | Pipeline, crates, IR |
| [06-strategy](design/06-strategy.md) | Alternatives, demand, success and withdrawal criteria |
| [07-roadmap](design/07-roadmap.md) | How additions are chosen, evidence from examples, next steps |
| [90-acceptance-survey](design/90-acceptance-survey.md) | Archive: measurements of existing crates |

## License

MIT.
