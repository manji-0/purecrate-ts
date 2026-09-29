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
- `survey` reports one cause per function, which understates rewrite work; an all-causes mode is wanted for estimates.
- `build --out` deletes unrelated files in the output directory (fixed 2026-09-30: a directory `build` did not write is refused).
