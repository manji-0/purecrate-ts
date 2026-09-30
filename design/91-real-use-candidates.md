# Real-use candidates

Status: record (surveyed and measured 2026-09-30)

<!-- derived-from ./06-strategy.md#5-validating-demand-next -->

[06 §5.2](./06-strategy.md#5-validating-demand-next) asks for one real use: a dual implementation in someone else's code, replaced by generated TS. This records where such dual implementations were found, how four of them fit the subset when measured, and which one was chosen.

## 1. Where the same rule is written twice

Wanted: public repositories where one rule is written by hand both in Rust (server) and TS (web frontend). Shared types (ts-rs, specta, typeshare, OpenAPI clients) do not count. Two passes: a code search for comments such as "mirrors the backend", "keep in sync", and a check of about 36 well-known Rust + TS projects. Every candidate below was confirmed by opening both files.

About 30 sites in 20 projects were found. Most show drift between the copies:

| Project | Rule | Drift seen in the copies |
| --- | --- | --- |
| Oxide (omicron / console) | resource `Name`; instance and disk state → allowed actions | TS accepts UUID-shaped names; the state table is synced by hand to pinned commits |
| Windmill | pipeline-annotation parser (TS header: "keep behaviorally identical"), MCP scope policy, dbt status, folder names | retry sign and overflow, whitespace set, missing `no-op` arm |
| Stoat (Revolt) | permission calculator | override order changed on the server (2026-08), not in the SDK |
| atomic-server | identifier parsing, value validation | `\d` is Unicode in Rust's `regex`, ASCII in JS |
| OpenObserve | on-call rotation and routing | comparator missing a term; no DST in TS |
| Sui, Aptos | name and address validation | max length 235 vs 200 |
| Rauthy, Tabby, AppFlowy | password policy, publish names | bytes vs UTF-16 units; characters accepted by one side only |
| Lemmy, Scanopy, lmnr, nasty | titles, predicates, prices, NFS hosts | missing maximums; an open issue caused by drift |

Not usable: projects without a TS frontend, frontends in ReScript or Flutter, projects that already ship the logic as WASM (Pubky, parts of Windmill), private Rust counterparts (Convex, lmnr's rate table), and Tauri apps that can call Rust over IPC.

Reading: dual implementation with drift is common; parity tests are rare (Windmill's pipeline fixtures are the one full example). Many of the rules are validation (names, identifiers, passwords), where [06 §3.2](./06-strategy.md#32-use-cases) places the most demand.

## 2. Fit, measured

Four sites were cut out of their repositories unchanged, run through `survey`, rewritten until `check` passed, and the generated TS was compared with the original Rust on random inputs.

| Candidate | `survey` untouched (fns) | Rewrite | Lines, original → subset | Rust vs generated TS | Hand-written TS vs Rust | Missing |
| --- | --- | --- | --- | --- | --- | --- |
| Oxide `Name` | 0/1 | passes | 64 → 105 (1.6×; 22 lines stand in for `Uuid::parse_str`) | 0 / 36,012 | TS accepts 4,542 names Rust rejects; the OpenAPI regex (and the zod generated from it) another 2,501 | nothing |
| Stoat permissions | 2/18 | passes, after the async `PermissionQuery` becomes plain input structs | ~330 → 245 | 0 / 20,000; rewrite vs original 0 / 200,000 | override order, non-members, voice revokes, groups, saved messages (read from code) | bit operators, `as` on enums, `const` (all spelled around) |
| Windmill MCP scope | 0/8 | listing policy only | 186 → 237 (1.3×; 35 lines emulate Unicode `trim`) | 0 / 20,000 | 1,282 / 20,000, a Rust bug (byte length used as a `char` index) | building `String`s from parts; returning `string[]` |
| Windmill pipeline parser | 0/15 | 6 of ~20 keywords | ~100 → 242 for the part; whole ≥ 2.5× estimated | 0 / 20,000 (part) | retry `+2` and overflow, U+0085/U+FEFF | string construction, `BTreeMap`, growable `Vec`, iterators, `while`/`break`, guards |

The pipeline parser, the strongest case on paper (a hand port kept in line by a shared fixture corpus), is the furthest from the subset: it builds strings and growing collections throughout, which [07 §6](./07-roadmap.md#6-not-doing) excludes, and its frontend already loads sibling parsers as WASM.

## 3. Decision

<!-- constrained-by ./06-strategy.md#4-success-and-withdrawal-criteria -->

1. **Oxide `Name` first.** It passes with no new capability and its public shape unchanged: the check moves into a small crate (`check_name(&str) -> Result<(), NameError>`) called from the existing `Name::try_from`. Three copies disagree today (Rust, console, the OpenAPI pattern), and the console carries a TODO to fuzz them against each other. Both repositories are MPL-2.0, and the console already regenerates its API client from a pinned omicron commit, where a generated validator fits. What this tests is distribution: packing the runtime, pinning across repositories, and mapping `NameError` to the console's messages. The weak point: the TS copy is 15 lines, and a maintainer may prefer adding a UUID check by hand; the case is the three-way drift, not the line count.
2. **Stoat permissions second.** The stronger demonstration (about 100 lines of TS logic, drift after a server change), but it needs the server's permission code split into a pure core first, which is the maintainers' call.
3. Windmill's two sites are kept as evidence of what strings and collections would cost, not as targets.

## 4. What the measurement asked of purecrate-ts

Capabilities, in the order they would have shortened the rewrites:

1. `Result<Self, Self::Error>` in `impl TryFrom<T>` — omicron's idiomatic signature, reported by `survey` as `item/serde-attr` with the impl "not judged". It turned out a `survey` defect: `check` accepts the signature, but `survey` never read trait impls (fixed 2026-09-30).
2. `s.chars()` through iterator `for` — `Name` hand-decodes UTF-8 in 18 lines to report the offending character; the MCP rewrite does the same for `chars().nth(n)`.
3. `is_empty` on `Vec`/slices, `Option::is_some`/`is_none` — in three of the four.
4. Bitwise operators on integers, `u64` included — Stoat's flag sets (a 25-line bit loop instead), after oidc.
5. `const`, and enum discriminants read without `as` — Stoat's 18 permission bits became functions.

Defects found on the way:

- A non-`pub` method (`McpScopeConfig::any`) appears on the exported companion object, so TS callers can reach what Rust callers cannot. Confirmed as a hole in closed types (a private method returning the type builds it unchecked) and fixed the same day ([01 §4](./01-equivalence.md#4-closed-types)).
- `mod r#impl;` and `mod r#trait;` are looked up as `r#impl.rs` and silently skipped (fixed 2026-09-30).
- `survey` reports one cause per function, which understates rewrite work; an all-causes mode is wanted for estimates (added 2026-09-30 as `--all-causes`).
- `build --out` deletes unrelated files in the output directory (fixed 2026-09-30: a directory `build` did not write is refused).

## 5. Oxide `Name`, done locally

<!-- derived-from #3-decision -->

2026-09-30, on local branches of omicron (`c925805`) and console (`14ec752`); nothing is proposed upstream yet.

**omicron.** A new crate, `name-rules`, holds `check_name(&str) -> Result<(), NameError>` (89 lines at first, written to the subset with `for c in s.chars()`, the UUID forms checked by hand; 66 once `uuid::Uuid` was supported and the check became `matches!(Uuid::parse_str(value), Ok(_))`); `Name::try_from` calls it and turns the error into the same message with `Display` (38 lines became 2). The crate's `ts/` is `build --bundle-runtime` output. Its test runs the previous implementation, with the real `uuid::Uuid::parse_str`, against `check_name` on about 120,000 inputs: same result and message on all. `omicron-common` still compiles.

**console.** `tools/generate_api_client.sh` also fetches `name-rules/ts/src/*` at the pinned omicron commit into `app/api/__generated__/name-rules/`, adding the MPL header except to the bundled runtime. `validateName` calls `check_name` and maps each `NameError` to the console's wording with an exhaustive `match`: the rule is gone from the console, the wording stays (15 lines before, 16 after). `tsc`, `oxlint`, `oxfmt --check`, and the unit tests pass (the 4 failures are webkit browser specs that fail the same without the change).

**Measured on 106,000 names** against omicron's Rust: the generated TS agrees on every result, including the offending character. The hand-written copy accepted all 6,000 UUID-shaped names the API rejects, and on 33,852 others reported a different first error than the API (uppercase first letters, length in UTF-16 units instead of bytes). The counts describe a corpus built to reach every branch (the 6,000 UUIDs were put in on purpose), not how often users meet the difference. In the console the difference is a delay, not a wrong result: the API rejects the name on submit and the form shows its message. The OpenAPI pattern disagrees with the API more widely (it accepts uppercase and the 32-hex form), which reaches every client generated from it; nothing was reported upstream. Two console tests changed accordingly (`Abc` now says the first character is wrong, as the API does), and UUID and byte-length cases were added.

What the vendoring asked of purecrate-ts, and of the consumer:

- **The runtime was not installable.** A project that commits generated code has no tarball step. `--bundle-runtime` was added: the runtime is copied into `src/`, and the sources stand alone. The same day this became the only way (the runtime and adapter are copied into every package, with string-keyed brands so values still cross between packages).
- **`noUnusedParameters`** failed on `assertNever`'s parameter in every package; renamed `_x`. Generated code still carried any binding the Rust leaves unused (rustc only warns), which a consumer with `noUnusedLocals` rejects; fixed 2026-09-30 ([07 §3](./07-roadmap.md#3-next)).
- **License headers.** The console requires its MPL header on every `.ts`; the bundled runtime is MIT and had to be excluded from that check by path.
- **Lint.** oxlint's `number-arg-out-of-range` flags the runtime's `toExponential(99)` (valid since ES2018); the console ignores the vendored runtime.
- **Keeping `ts/` current in omicron** needs `purecrate-ts check --out name-rules/ts` in its CI, so omicron's CI would have to install purecrate-ts, which is not published anywhere.

Lines: omicron −36 in `common`, +89 for the rule and +138 of equivalence tests; the console loses the rule but not its wording, so its line count does not drop. The gain is agreement, not size: three copies (Rust, console, and the OpenAPI pattern, which still accepts uppercase and the 32-hex form) became two, and the console's is now checked rather than kept by hand.
