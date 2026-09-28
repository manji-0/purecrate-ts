# Roadmap

Status: current (2026-09-29)

<!-- constrained-by ./02-authoring.md -->
<!-- constrained-by ./06-strategy.md#4-success-and-withdrawal-criteria -->

## 1. How additions are chosen

**Add a capability only when an example written within the constraints cannot be written without it.** Rejection counts in a corpus do not set the order ([90](./90-acceptance-survey.md) is not a metric). Two exceptions skip the wait:

- **Holes** — anything that makes accepted input compute a wrong value, or lets `check` accept what rustc rejects, is fixed immediately.
- **Domain fixes** — changes that tighten the domain of equivalence (closed types) rather than add capability.

Every addition comes with a differential test.

## 2. Evidence from examples

| Example | Source | Got stuck on | Added |
| --- | --- | --- | --- |
| counter | author | — | acceptance criterion |
| vending, control | author | — | test fixtures for `?` and early `return` (all 1296 sequences) |
| order (5 states, `Yen`, `Sku`) | author | quantities as `u32` × `i64` price; `String` literals in a test driver | lossless widening (`widen_equivalence.rs`, 20 widenings at both ends); `String::from` (and closed the literal-in-`String` hole) |
| signup (WHATWG email, NIST SP 800-63B-4 length) | third party | nothing readable from a string but `==`; `Ok(())` rejected as `expected (), found ()` and printed as `[]`; `email` vs `Email` file collision | `str::as_bytes`; `Lit::Unit` fix; renamed the driver (known rule) |
| iban (ISO 13616-1, ISO 7064 MOD 97-10) | third party | nothing; but line count over threshold | integer-range `for` |

Semantic cross-checks beyond Rust-vs-TS: signup's acceptance matches WHATWG's own regular expression on node; iban matches an idiomatic-Rust implementation on published valid IBANs, one-character mutations, and malformed input.

Line counts (non-blank, non-comment) against idiomatic Rust, threshold 2×:

| Example | Idiomatic | Recursion only | With range `for` |
| --- | --- | --- | --- |
| Email (WHATWG) | 28 | 79 (2.8×) | 52 (1.9×) |
| Password (NIST) | 17 | 28 (1.6×) | 24 (1.4×) |
| IBAN | 24 | 57 (2.4×) | 45 (1.9×) |

All are inside the threshold with little margin. Most of the remaining gap is spelling character classes as numeric comparisons. **The next candidate if the threshold is hit again: literal/range patterns in `match` and byte literals.**

## 3. Next, when an example needs it

1. **Boundary encoding** — write serde JSON from domain values (`bigint` as a JSON number, decide `NaN`) ([04 §6](./04-wire.md#6-open-questions)).
2. **`#[serde(try_from)]`** — uphold closed-type invariants on the wire ([04 §5](./04-wire.md#5-closed-types-on-the-wire)).
3. **Character-class notation** — literal/range patterns, `b'@'` (see §2).
4. **Strings and `char`** — as specified in [01 §6](./01-equivalence.md#6-strings-char-usize-std-methods), one method at a time.
5. **Iteration** — `while`, `break`/`continue`, iterator `for`, when range `for` plus recursion is not enough.
6. **std methods** the example calls, via the allow-list. Iterator `map`/`filter`/`collect` are not added: they are how state sequences grow as arrays.
7. **Match ergonomics** — if missing `_ =>` makes transition tables unreadable.

## 4. Specified but not yet implemented

`char`; `String::len`, byte slicing, `String` ordering; `isize`; `while`, `loop`, `break`/`continue`, `a..=b`, iterator `for`; literal patterns; byte literals; the std allow-list beyond `Vec::len`, indexing, `str::as_bytes`; `const`/`static`.

## 5. v1: when type expressiveness runs out

Waits for an example that cannot be written without it.

- Unbounded type parameters, emitted as TS generics (no monomorphization). No bounds, `where`, or associated types.
- `HashMap`/`BTreeMap` with `String` keys only, as `ReadonlyMap<string, V>`. Insertion order is not matched; functions depending on it are rejection candidates.

## 6. Not doing

- Allow-lists aimed at passing existing crates.
- Decimals; growable `Vec`; event logs inside state.
- A schema-library dependency in the core runtime.
- WASM. The IR does not preclude a second backend, but the path is TS source.
- `Rc` / `Cell` / `RefCell`.

## 7. Open questions

- Should output typing come from rustc's type information instead of the in-house inference ([05 §3](./05-architecture.md#3-rustc-as-the-final-gate))?
- `NaN` over JSON, and Hermes support for `JSON.parse` source text ([04 §6](./04-wire.md#6-open-questions)).
- A shared error type with field paths for validation ([01 §4](./01-equivalence.md#4-closed-types)).
- Demand: see [06 §5](./06-strategy.md#5-validating-demand-next).
