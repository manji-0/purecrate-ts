# purecrate-ts

Share **behavior**, not just types, between Rust and TypeScript.

purecrate-ts translates pure domain functions written in Rust into an ordinary TypeScript package, without WASM. The output is plain `Readonly` values and functions that pass `tsc --strict`. For every accepted input they return the same result as a Rust debug build, and differential tests check this. Anything whose meaning cannot be preserved is rejected with its location, and no output is written.

The usual sharing setup is a server that uses the same types and functions as its wire format and domain logic. Servers are built with `--release`, where `overflow-checks` is off by default, so on overflow Rust wraps while the generated TypeScript throws. Equivalence with that server holds only when the crate (or workspace) sets `[profile.release] overflow-checks = true` (and `debug-assertions` if anything depends on them). `check` and `build` warn when the release profile does not.

It is not a compiler for arbitrary Rust. You write new domain code within [the PureCrate constraints](design/02-authoring.md): states and events as ADTs, and transitions such as `fn step(state, event) -> Result<State, Error>`. Start with [design/00-overview.md](design/00-overview.md).

## Install

Download the binary for your platform from the [latest release](https://github.com/manji-0/purecrate-ts/releases/latest) (Linux x86_64 and aarch64, macOS x86_64 and arm64), or build it from a tag:

```sh
cargo install --git https://github.com/manji-0/purecrate-ts --tag v0.9.1 purecrate-ts
```

The binary carries the runtime and the schema adapters; it needs only `rustc` on the `PATH` (see below). Changes are listed in [CHANGELOG.md](CHANGELOG.md).

## Requirements

- Rust (edition 2021) to build from source. Dependencies are vendored; build with `cargo --offline`. `check` and `build` also run the input through `rustc`, so `rustc` is needed at run time too (override with `RUSTC`); the input may use `serde`, for which `check` gives rustc a stand-in. The input's edition comes from `Cargo.toml` (`[package]` or inherited `[workspace.package]`; 2015 if unset). A standalone file uses 2021 unless you pass `--edition`.
- Node and `npx` to type-check the output and to run differential tests. Type checking runs on TypeScript 6 and 7.

## Usage

```sh
purecrate-ts build examples/counter --out /tmp/counter-ts
# from a clone, without installing:
cargo run --offline -p purecrate-ts -- build examples/counter --out /tmp/counter-ts
```

```text
purecrate-ts build  <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype] [--publishable]
purecrate-ts check  <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema zod|valibot|arktype] [--publishable]
purecrate-ts survey <crate-path>... [--json] [--all-causes]
```

- `<crate-path>` is a crate directory (`src/lib.rs`) or a single `.rs` file; module files it declares (`mod x;`) are read too. `--name` defaults to the `Cargo.toml` package name.
- `build` replaces `--out` whole, removing files an earlier build left, but keeps `node_modules/` so an `npm install` in the package survives a rebuild (`dist/` is dropped: it is stale). It refuses a directory that is not empty and was not written by `build`.
- `check` writes nothing. It rejects out-of-subset input as `path:line:col` plus a reason code, then rustc errors as e.g. `[rustc/E0382]`. With `--out`, it also compares the result byte for byte with an existing output. `check` and `build` warn when `[profile.release]` does not set `overflow-checks = true`.
- The output is an npm package. `npm run build` emits `dist` (it also runs before `npm pack` and `npm publish`). The runtime and, with `--schema`, the adapter are copied into `src/`; the schema library is the only peer dependency. `version` and `license` come from `Cargo.toml`. The generated `package.json` says `"private": true`, so `npm publish` refuses it; `--publishable` leaves that out. See [Distribution](#distribution).
- `--schema` emits `src/purecrate-wire.ts`, which reads serde's default JSON into the domain's branded types and writes it back with `toJson.T(x)`, the same bytes serde_json writes. Read JSON text with `fromJson.T(text)`, the inverse of `toJson.T`, which goes through `parseJson` so that `i64`/`u64` above 2^53 stay exact, and throws on malformed text or a refused value; never `JSON.parse` then the schema.
- `survey` reports, for each public function and type, whether it is accepted with everything it refers to, and the first cause when it is not. `--all-causes` lowers each item past what it cannot take and lists every cause, the type check's included, to estimate a rewrite.
- Never edit generated packages. Change the Rust and regenerate.
- A brand exists only in types, so `5 as I32` or `s as Email` in your own code goes unchecked. Build values with `Int.i32.of`, the wire schemas, or the crate's functions, and keep `as` out with a lint such as `@typescript-eslint/consistent-type-assertions` (`assertionStyle: "never"`), the generated directory left out. The generated code casts only where the value is already what the type says ([design/03 §1.1](design/03-output.md#11-casts)).

## What you can write

- structs, enums (`kind` discriminated unions), newtypes, `Option`, `Result`, `?`, `if let`, exhaustive `match` (with `A | B` arms binding nothing, a last `_`, guards `p if c`, and struct patterns `P { method: M { kind: K::B, .. } }`), `match (state, event)` on tuples, and `true` / `false` arms
- byte literals `b'@'`, integer literal and range patterns in `match` (ending in `_`) and `matches!(b, b'0'..=b'9')`
- local `let mut` (updates return new values), local closures, struct update `S { a, ..base }`, `for i in a..b`, `const` items in function bodies, tuple patterns in `let (a, b) = t;`, `|(a, b)|`, and `for (k, v) in &pairs`
- `for x in &xs` over a `Vec` or slice, `for c in s.chars()`, `for b in s.bytes()`, `for t in s.split(c)`, and `.enumerate()` of any of them; `while` with `break` and `continue`
- `map`, `filter`, `copied`, and `cloned` over `xs.iter()`, `s.chars()`, `s.bytes()`, and `s.split(c)`, lazy as in Rust, then `collect` (into `Vec<T>` or `Result<Vec<T>, E>`, `_` allowed in the turbofish), `all`, `any`, `position`, `count`, or integer `sum`
- `Option` read with `is_some`, `is_none`, `unwrap_or`, `ok_or`, and `map`; `Result` with `ok`, `map`, and `map_err`; integers read from text with `s.parse::<T>()`
- integer arithmetic with debug-build semantics (overflow and division by zero throw); `i64`/`u64` as `bigint`; bitwise operators and shifts; widening with `i64::from(x)`; `min`, `max`, `abs`, `pow`, and `checked_*` / `saturating_*` / `wrapping_*`. A `--release` server matches this only with `[profile.release] overflow-checks = true`
- crate-level `const` items, folded into `consts.ts`; enum discriminants (`#[repr(u64)] enum Perm { View = 1 << 0, .. }`) read with `p as u64`
- `Vec` read by index, `len`, slices `&xs[a..b]`, and `cmp`; built as `vec![a, b]`, by `collect`, by `clone`, or grown in a function by `let mut v: Vec<T> = Vec::new(); v.push(x)` (only a local; the caller's arrays are never written); recursive enums for sequences too
- `char` as a branded one-code-point string: literals, ranges in `match` / `matches!`, ordering by code point, `u32::from(c)`, `char::from(b)`, `char::from_u32(n)`, the ASCII methods
- `String::from("…")`, `clone` (and `as_ref` / `as_deref` on an `Option`), string `==`, `len` (UTF-8 bytes), `is_empty`, `starts_with` / `ends_with` / `contains` / `strip_prefix` / `strip_suffix` with a string needle, slices `&s[a..b]` at byte positions, string contents through `s.as_bytes()`
- structs with private fields stay closed: TS gets values only from your public constructors
- modules, inline or in files: flattened, with exports following Rust's public surface
- `#[derive(Serialize, Deserialize)]` on the same types, so the server uses them as its wire format; `#[serde(try_from = "T")]` with `impl TryFrom<T>` reads a closed type through its constructor on both sides; `impl Error` is allowed and not translated, and an `impl Display` writing a fixed text becomes `to_string` (`toString` in TS)

There is no decimal type. Write money as an integer newtype in the smallest unit (`struct Yen(i64)`). For the full rules, see [design/02](design/02-authoring.md). For what TS callers must observe, see [design/03 §5](design/03-output.md#5-caller-contract).

## Distribution

A generated package carries everything it runs on. The runtime (`packages/boundary`) is copied into it as `src/purecrate-runtime.ts`, and with `--schema` the adapter (`packages/boundary-zod`, `-valibot`, `-arktype`) as `src/purecrate-<lib>.ts`, at the revision of the `purecrate-ts` binary that generated the code, so the two cannot fall out of step. The only dependency left is the schema library itself (`zod`, `valibot`, or `arktype`), a peer dependency the consumer installs as usual.

Use the output either way:

- **Vendor the sources.** Commit the output (as one commits an OpenAPI client) and import `src/index.ts`. Generated files import with `.ts` extensions (`from "./purecrate-runtime.ts"`). The consuming project's tsconfig must allow that: `allowImportingTsExtensions` with `noEmit` or a bundler, or `rewriteRelativeImportExtensions` when emitting (`tsc` reports TS5097 otherwise). The generated package's own `tsconfig.json` already sets the former. If the project cannot set them, install the package instead and import through `exports` (the `purecrate-source` condition resolves to the `.ts` sources; `types` / `default` to `dist` after `npm run build`). Check the committed copy in CI with `purecrate-ts check <crate> --out <dir>`, which fails when it differs from what `build` would write; pin the release binary (or `cargo install --git … --tag`) there so the check and the committed output come from the same version.
- **Install it as a package.** `npm pack` it (the `prepack` script builds `dist`) and install the tarball, or push it to a private registry.

```sh
(cd <generated-package> && npm install --no-save typescript@6 && npm pack --pack-destination ../tarballs)
# In the project that uses it:
npm install tarballs/<name>-<version>.tgz
```

Generated packages are `"private": true` by default, which stops an accidental `npm publish` but not `npm pack` or installing the tarball; build with `--publishable` (and pass it to `check --out` too, which compares bytes) when the package is meant for a private registry. Several generated packages can live in one project: each carries its own copy of the runtime, and every brand is keyed by string (`I32` as `{ readonly "purecrate.I32": true }`, a crate newtype as `{ readonly "payment.Amount": true }`), so a value from one package is the same type in another. Closed types of your crate have no `of` on the companion and do not export `unsafeMakeAmount` (marked `@internal`) from `index.ts`; that is a convention, backed by a consumer `as` lint, not a type-level guarantee (a string brand does not stop `import { unsafeMakeAmount } from "./gen/src/amount.ts"` when the sources are vendored). `crates/cli/tests/it/package.rs` runs the two-package flow.

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

CI (`.github/workflows/verify.yml`) runs `cargo fmt --check` (`rustfmt.toml`: 120 columns) and `cargo clippy -D warnings` in a job of their own, and the script in another, on every push inside `nix develop`, after `npm ci` in the three adapter packages and in `examples`. The script runs both lints too (CI sets `PURECRATE_SKIP_LINT`), then `cargo test --offline` (goldens, differential tests that run the same inputs through Rust and the generated TS, and wire tests against the vendored serde_json), `check` with drift detection on every example against its committed output (`examples/<name>/ts/plain`, and `examples/<name>/ts/<lib>` with each schema library where the example derives serde; `scripts/examples.sh` regenerates them), and `tsc` on TypeScript 6 and 7 for the runtime packages and those outputs.

The tests of each crate are one binary (`crates/*/tests/it`, one module per file), so they link once and run in parallel. `scripts/output-snapshot.sh` prints one digest of everything `build` and `survey` write for every example and test fixture; a refactor that must not change the output keeps it.

`.github/workflows/release.yml` runs `scripts/verify.sh` first, then builds the release binaries when a `vX.Y.Z` tag is pushed (the tag must match the workspace version, and `CHANGELOG.md` must have its section, which becomes the release notes); the x86_64 macOS binary, cross-built on the arm64 runner, is smoke-tested under Rosetta. Run the workflow by hand to verify, build, and smoke-test every target without publishing.

`scripts/line-counts.py` counts each example's logic against its idiomatic reference, both formatted by rustfmt ([design/07 §2.2](design/07-roadmap.md#22-line-counts-against-idiomatic-rust)). `bench/payment/measure.sh` compares the generated TS with wasm-bindgen on the same source; it needs the network and a `wasm32-unknown-unknown` target ([bench/payment](bench/payment/README.md)).

## Stability

Commit the generated output and check it in CI with `purecrate-ts check <crate> --out <dir>`, pinning the same version that wrote it. Within a minor series, **export names, type shapes, the wire format, and the runtime API** stay the same; **formatting, internal helpers (`unsafeMakeX`, temps), local names, and which runtime members a copy keeps** may change. A change to the stable surface is a minor bump (a major after 1.0). The table is in [design/07 §9](design/07-roadmap.md#9-generated-api-stability).

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
| [07-roadmap](design/07-roadmap.md) | How additions are chosen, evidence from examples, next steps, generated API stability |
| [90-acceptance-survey](design/90-acceptance-survey.md) | Archive: measurements of existing crates |
| [91-real-use-candidates](design/91-real-use-candidates.md) | Record: dual Rust/TS implementations found in public projects, their fit to the subset, and the first real-use target |

## License

MIT.
