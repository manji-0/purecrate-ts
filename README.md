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
purecrate-ts build  <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype | --bundle-runtime] [--publishable]
purecrate-ts check  <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype | --bundle-runtime] [--publishable]
purecrate-ts survey <crate-path>... [--json]
```

- `<crate-path>` is a crate directory (`src/lib.rs`) or a single `.rs` file; module files it declares (`mod x;`) are read too. `--name` defaults to the `Cargo.toml` package name.
- `build` replaces `--out` whole, removing files an earlier build left, but refuses a directory that is not empty and was not written by `build`.
- `check` writes nothing. It rejects out-of-subset input as `path:line:col` plus a reason code, then rustc errors as e.g. `[rustc/E0382]`. With `--out`, it also compares the result byte for byte with an existing output.
- The output is an npm package. `npm run build` emits `dist` (it also runs before `npm pack` and `npm publish`). The runtime `purecrate` and the schema adapters are `peerDependencies`. `version` comes from `Cargo.toml`. The generated `package.json` says `"private": true`, so `npm publish` refuses it; `--publishable` leaves that out. See [Distribution](#distribution).
- `--schema` emits `src/purecrate-wire.ts`, which reads serde's default JSON into the domain's branded types and writes it back with `toJson.T(x)`, the same bytes serde_json writes. Read JSON text with `purecrate`'s `parseJson`, not `JSON.parse`, so that `i64`/`u64` above 2^53 stay exact.
- Never edit generated packages. Change the Rust and regenerate.

## What you can write

- structs, enums (`kind` discriminated unions), newtypes, `Option`, `Result`, `?`, `if let`, exhaustive `match` (with `A | B` arms binding nothing and a last `_`)
- byte literals `b'@'`, integer literal and range patterns in `match` (ending in `_`) and `matches!(b, b'0'..=b'9')`
- local `let mut` (updates return new values), local closures, struct update `S { a, ..base }`, `for i in a..b`
- integer arithmetic with debug-build semantics (overflow and division by zero throw); `i64`/`u64` as `bigint`; widening with `i64::from(x)`
- growing sequences as recursive enums; `Vec` read by index and `len`
- `char` as a branded one-code-point string: literals, ranges in `match` / `matches!`, ordering by code point, `u32::from(c)`, `char::from(b)`, `char::from_u32(n)`, the ASCII methods
- `String::from("…")`, string `==`, `len` (UTF-8 bytes), `is_empty`, `starts_with` / `ends_with` / `contains` with a string needle, string contents through `s.as_bytes()`
- structs with private fields stay closed: TS gets values only from your public constructors
- modules, inline or in files: flattened, with exports following Rust's public surface
- `#[derive(Serialize, Deserialize)]` on the same types, so the server uses them as its wire format; `#[serde(try_from = "T")]` with `impl TryFrom<T>` reads a closed type through its constructor on both sides; `impl Display` / `Error` are allowed and not translated

There is no decimal type. Write money as an integer newtype in the smallest unit (`struct Yen(i64)`). For the full rules, see [design/02](design/02-authoring.md). For what TS callers must observe, see [design/03 §5](design/03-output.md#5-caller-contract).

## Distribution

purecrate-ts is meant for domain code that stays private, so its output is never published to npm, and it would sit badly to keep only the shared runtime there. Everything is delivered as tarballs packed from this repository. The runtime `purecrate` (`packages/boundary`) and the schema adapters (`packages/boundary-zod`, `-valibot`, `-arktype`) are **packed from `packages/`**, at the same revision as the `purecrate-ts` binary that generated the code, and installed together with your generated package:

```sh
mkdir -p tarballs
# The build script needs TypeScript, so install each package's dev dependencies first.
(cd packages/boundary && npm install && npm pack --pack-destination ../../tarballs)        # purecrate
(cd packages/boundary-zod && npm install && npm pack --pack-destination ../../tarballs)    # only with --schema zod
# The generated package needs the runtime it is built against, then packs itself (`npm run build` runs first).
(cd <generated-package> && npm install --no-save typescript@6 ../tarballs/purecrate-0.1.0.tgz \
  && npm pack --pack-destination ../tarballs)
# In the project that uses it:
npm install tarballs/purecrate-0.1.0.tgz tarballs/<name>-<version>.tgz
```


Push the same tarballs to a private registry, or vendor them, if several projects consume them. Generated packages are `"private": true` by default, which stops an accidental `npm publish` but not `npm pack` or installing the tarball; build with `--publishable` (and pass it to `check --out` too, which compares bytes) when the package is meant for a private registry. The runtime packages in `packages/` are `"private": true` as well; remove it from your copy with `npm pkg delete private` before publishing them to a registry of your own. All generated packages in one project must resolve one copy of `purecrate`: brands are `unique symbol`s, so values cross between packages only through a shared runtime. The schema library itself (`zod`, `valibot`, or `arktype`) is an ordinary npm dependency of the consumer. `crates/cli/tests/package.rs` runs exactly this flow.

**Vendoring the sources instead.** A project that commits generated code (as it does an OpenAPI client) can skip the tarballs: `build --bundle-runtime` copies the runtime into the package as `src/purecrate-runtime.ts`, so `src/` stands alone with no dependency. Copy it into the project and import `src/index.ts`. The brands are then that package's own, so values do not cross to another generated package; `--schema` is not available this way, since the adapters import `purecrate`. Check the committed copy with `check --bundle-runtime --out <dir>`.

## Agent skill

[`skills/purecrate-authoring`](skills/purecrate-authoring/SKILL.md) teaches a coding agent how to write Rust within the constraints, with the `check` loop, the accepted and rejected constructs, and the common pitfalls. Install it with the GitHub CLI (2.90 or later):

```sh
gh skill install manji-0/purecrate-ts purecrate-authoring
```

`--agent` picks the agent (Claude Code, Copilot, Cursor, Codex, …) and `--scope user` installs it for every project. In a clone of this repository, Claude Code reads it through the link `.claude/skills/purecrate-authoring`.

## Testing

```sh
./scripts/verify.sh
```

It needs Rust and Node 21+ (CI uses 24). `nix develop`, or direnv with the checked-in `.envrc`, provides rustc 1.98.1 (the release the differential tests' panic messages were measured on) and Node 24; TypeScript and the schema libraries still come from npm.

CI (`.github/workflows/verify.yml`) runs it on every push inside `nix develop`, after `npm ci` in the three adapter packages. It runs `cargo test --offline` (goldens, differential tests that run the same inputs through Rust and the generated TS, and wire tests against the vendored serde_json), drift detection on examples/counter, `check` on the other examples, and `tsc` on TypeScript 6 and 7 for the runtime packages and the counter output.

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
| [91-real-use-candidates](design/91-real-use-candidates.md) | Record: dual Rust/TS implementations found in public projects, their fit to the subset, and the first real-use target |

## License

MIT.
