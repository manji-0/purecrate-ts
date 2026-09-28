# Acceptance rate measurement (2026-09-27)

Measurement for design/04 §3.5-1. We ran `purecrate-ts survey` over publicly available Rust code and counted how many public functions and public types are accepted as-is.

## 0. Conclusions

1. **Public functions in existing code are almost never accepted.** Across the 8 domain-focused entries, 1 of 956 functions (0.1%).
2. **About half of public types are accepted.** 44 of 87 types (51%). Sharing types only (design/05) is realistic even for existing code.
3. **The first barrier for functions is references in signatures (`&T`, `&self`); the next is standard library methods.** A prototype that treats references as values did not increase acceptance; the top rejection reason shifted to `expr/method-call` (`collect`, `len`, `to_string`, etc.) (§3).
4. **We decided the target is "new code written within the PureCrate constraints"** (design/07). The acceptance rates in this document remain as the rationale for not converting existing code as-is. Priorities from here on are owned by design/07.

## 1. Method

### 1.1 Corpus

`corpus/manifest.tsv` pins the repository, commit, and survey root. `scripts/survey-corpus.sh` fetches and measures, writing results to `corpus/results.jsonl`.

| Entry | Category | Root |
| --- | --- | --- |
| rust-ddd-example | DDD | whole crate |
| rust-ddd-example.domain | DDD | `src/domain` |
| zero-to-production | Validation | whole crate |
| zero-to-production.domain | Validation | `src/domain` |
| idsmith | Validation (checksums for IBAN and various IDs) | whole crate |
| eventually.bank-accounting.domain | DDD | `examples/bank-accounting/src/domain.rs` |
| eventually.light-switch.domain | State machine | `examples/light-switch/src/domain.rs` |
| little-raft | State machine (Raft) | `little_raft` |
| poker | Game rules | whole crate |
| cozy-chess.types | Game rules (bitboards) | `types` |

The "domain scope" in the totals counts the 8 entries that cover only the domain part, excluding whole applications (the 2 `.domain` entries, idsmith, the 2 eventually entries, little-raft, poker, cozy-chess.types). The 2 whole-application entries include HTTP and DB layers, so they are for reference only.

### 1.2 Judgement

- Units are public functions (`pub fn` as free functions and in inherent impls) and public types (struct, enum, type alias). Trait impls, `const`, `static`, and `trait` are not judged, only counted.
- Each unit is passed to `check::accept` together with the transitive closure of the types and functions it references. This is the same judgement `build` would make when emitting only that unit.
  - **Accepted**: the whole closure passes.
  - **Rejected**: the unit itself is out of scope.
  - **Dragged down**: the unit passes, but something it references is out of scope.
- Reasons are counted by the reason codes introduced in TODO 25 (`Reason::code()`). Method names, macro names, paths, etc. are recorded alongside as `detail`.

### 1.3 Measurement limitations

- **Only the first rejection reason for each unit is known.** The parser stops at the first out-of-scope construct per unit and looks at the signature before the body. So removing a top reason reveals the next hidden reason. §3 confirms this with a prototype.
- Modules are flattened by name for judgement. Same-named types in different modules are rejected as conflicts (this did not affect counts in this corpus).
- Path aliases via `use` are not resolved. Module qualification such as `shapes::Shape` is counted as `type/qualified-path`.

## 2. Results (current acceptance scope)

### 2.1 By entry

| Entry | Functions (accepted / total) | Types (accepted / total) |
| --- | --- | --- |
| rust-ddd-example | 0 / 19 | 5 / 11 |
| rust-ddd-example.domain | 0 / 2 | 1 / 1 |
| zero-to-production | 0 / 51 | 5 / 28 |
| zero-to-production.domain | 0 / 2 | 0 / 3 |
| idsmith | 0 / 830 | 20 / 36 |
| eventually.bank-accounting.domain | 0 / 7 | 5 / 9 |
| eventually.light-switch.domain | 0 / 4 | 8 / 9 |
| little-raft | 0 / 5 | 2 / 8 |
| poker | 1 / 72 | 8 / 16 |
| cozy-chess.types | 0 / 34 | 0 / 5 |
| **Domain scope total** | **1 / 956 (0.1%)** | **44 / 87 (51%)** |

idsmith has many functions (830) and dominates the total. Excluding idsmith, functions are still 1 / 126.

### 2.2 Function rejection reasons (domain scope, first reason per unit)

| Count | Reason code | Entries | Main contents |
| --- | --- | --- | --- |
| 785 | `type/reference` | 3 | `&str` / `&T` parameters (mostly idsmith) |
| 50 | `item/ref-receiver` | 6 | `&self` |
| 32 | `type/self` | 5 | `Self` in return types and construction |
| 20 | `expr/method-call` | 3 | Calls to the crate's own methods (poker's `is_pair`, `is_straight`, etc.) |
| 16 | `expr/block-item` | 2 | `const` or nested functions inside a function |
| 10 | `expr/operator` | 2 | Bitwise operations (cozy-chess) |
| 9 | `item/generics` | 3 | |
| 8 | `type/disallowed` | 2 | `usize`, `char` |

Excluding idsmith, the top reasons are `type/self` (22), `expr/method-call` (20), `expr/block-item` (16), `item/ref-receiver` (15). **More important than the counts: in 5 of the 7 non-idsmith entries, `&self` is the first barrier.**

### 2.3 Type rejection reasons (domain scope)

| Count | Reason code | Main contents |
| --- | --- | --- |
| 11 | `item/cfg` | idsmith's feature switches |
| 11 | `item/generics` | |
| 5 (+2 dragged down) | `item/tuple-struct` | newtypes (`struct EntityId(Uuid)` etc.) |
| 5 | `check/undefined-type` | External types (`Decimal`), types defined by macros |
| 4 | `type/disallowed` | `usize`, `char`, `HashMap` |

## 3. Estimate: treating shared references as values

We measured the same corpus with an **uncommitted prototype** that reads `&T` (other than `&mut`) as `T`, `&self` as `self`, and the expressions `&x` / `*x` as `x`. This reinterpretation very likely does not change meaning in this subset: generated TS does not mutate values, and interior mutability (`Cell`, `RefCell`, etc.) is already rejected at the type level.

| | Current | Prototype |
| --- | --- | --- |
| Functions accepted | 1 / 956 | 1 / 956 |
| Types accepted | 44 / 87 | 46 / 87 |

Acceptance barely moves. The top rejection reason changed.

| Count | Reason code | Main contents |
| --- | --- | --- |
| 437 | `expr/method-call` | `collect`×295, `len`×51, `to_string`×33, `replace`×11, `to_uppercase`×10, `unwrap_or`×8 |
| 305 | `type/reference` | The remainder is `&mut` (idsmith's RNG parameters) |
| 51 | `type/qualified-path` | Module qualification such as `super::GenOptions` |
| 33 | `type/self` | |
| 16 | `expr/macro` | `format!`×12 |
| 16 | `expr/block-item` | |
| 14 | `type/array` | Slices `&[T]` |

References are merely the first barrier in the signature; bodies are written with **iterators and methods on strings and `Option`**.

## 4. Interpretation

- **Type sharing works for existing code.** Half of public types pass as-is, and the main causes for the rest (`cfg`, generics, newtypes, `usize`) can be handled within type definitions. This is consistent with design/05's conclusion (build the boundary codec first).
- **Behaviour sharing hardly works for existing code as-is.** Domain functions take `&self` and `&str` and are written with `iter().map().collect()` and `s.len()`. These are Rust idioms and do not compromise purity. What falls outside the subset is the "style of writing", not the "properties".
- String methods (`len`, `to_uppercase`, `replace`) run directly into the open point in design/04 §1.3 (the UTF-8 vs UTF-16 difference). Accepting them requires pinning down the semantic difference with differential tests.

## 5. Next priorities (proposal)

1. **Remove the signature barrier (low cost)**: accept shared references `&T`, `&self`, `&str` as values. Replace `Self` with the impl's type name. Accept newtypes (single-element tuple structs). On its own this does not move the function acceptance rate, but it is a prerequisite for 2 and also raises the type acceptance rate.
2. **Accept std methods via an allow-list (medium to high cost)**: from the top, `Vec` / iterators (`iter`, `map`, `filter`, `collect`, `len`), `Option` (`unwrap_or`, `map`, `is_some`), `String` (`to_string`, `len`, `replace`, `to_uppercase`, `trim`, `is_empty`). Attach differential tests against Rust to each. For strings, decide the unit of length and indexing before adding them.
3. **Remaining type work**: handling `cfg` (read with fixed features?), generics (v1 plan), TS representation of `usize`.

### Question requiring a decision

- Is the target "existing Rust domain code" or "new code written within the PureCrate constraints"? The former makes §5-2 mandatory and greatly increases the surface of semantics to maintain. The latter needs only up to §5-1, with documentation and lints guiding how to write code.

## 6. Progress

### 6.1 After TODO 28 (shared references, `Self`, newtypes, `Type::method` calls)

Newly accepted:

- Shared references `&T`, `&self`, `&str`, and the expressions `&x`, `*x`.
- `&[T]` read as `Vec<T>`.
- Lifetime-only generics (`fn f<'a>`).
- `Self` replaced with the impl's type name.
- Single-element tuple structs (newtypes) and `.0`.
- Method calls via `Type::method(x)`.

| | TODO 27 (current) | After TODO 28 |
| --- | --- | --- |
| Functions accepted | 1 / 956 | 3 / 956 |
| Types accepted | 44 / 87 (51%) | 52 / 87 (60%) |
| Types accepted (excluding idsmith) | 24 / 51 | 30 / 51 |

Top function rejection reasons (first reason):

| Count | Reason code | Main contents |
| --- | --- | --- |
| 454 | `expr/method-call` | `collect`×295, `len`×51, `to_string`×34, `replace`×11, `to_uppercase`×10 |
| 305 | `type/reference` | `&mut` (idsmith's RNG) |
| 51 | `type/qualified-path` | Module qualification such as `super::GenOptions` |
| 19 | `expr/macro` | `format!`×12, `vec!`×4 |
| 19 | `expr/block-item` | `const` inside functions, etc. |

Excluding idsmith, the contents of the 34 `expr/method-call` cases change. The top ones call the crate's own methods with receiver syntax, like `self.hand_rank()` or `self.is_pair()`. std methods (`is_empty`, etc.) come next.

**The next barrier is method calls with receiver syntax.** The crate's own methods can be mapped the same as `Type::method(x)` once the receiver's type is known. This type-directed resolution mechanism is reused as-is for the std method allow-list (TODO 32-34). So it is added as a TODO to do before closures.

### 6.2 After TODO 29 (method calls with receiver syntax)

`x.m(args)` is kept as a `MethodCall` in the IR. Type checking resolves it from the receiver's type to a method in the crate's own inherent impls. Calls that cannot be resolved (methods on std types, etc.) are rejected at type-checking as `expr/method-call`, with the method name kept in detail. survey adds same-named methods of each type in the closure as dependencies, because the receiver's type always appears in some signature inside the closure.

Acceptance does not change (functions 3 / 956, types 52 / 87). What changed is the breakdown of first rejection reasons: reasons hidden behind method calls became visible.

| Count | Reason code | Main contents |
| --- | --- | --- |
| 339 (+16 dragged down) | `expr/closure` | idsmith's `.map(\|c\| ..)` etc. These had stopped before `collect` / `len` |
| 305 | `type/reference` | `&mut` (idsmith's RNG) |
| 51 | `type/qualified-path` | Module qualification |
| 33 | `expr/macro` | `format!`×26, `vec!`×4 |
| 26 | `expr/index` | `s[i]` |
| 21 | `expr/loop` | `for` |
| 19 | `expr/block-item` | `const` inside functions, etc. |

For the 126 functions excluding idsmith, the top reasons became spread out.

| Count | Reason code | Main source |
| --- | --- | --- |
| 18 | `expr/loop` | poker (17) |
| 17 | `expr/block-item` | poker (15) |
| 13 | `expr/operator` | cozy-chess bitwise operations (11) |
| 11 | `item/ref-receiver` | `&mut self`. eventually's aggregates (6), little-raft, etc.; 5 entries |
| 8 | `type/disallowed` | `char`, `usize` |

### 6.3 Next candidates (observations on the plan for TODO 30 onward)

- **Closures (TODO 31) have the largest effect.** Accepting closures moves idsmith's 339 cases on to the std iterator method barrier (TODO 33). Acceptance only moves once both are in place.
- **`&mut self` is a shape common to DDD and event-sourcing aggregates** (`fn apply(&mut self, event)`). It is only 11 cases as a first reason, but spans 5 entries. Rewriting it as a function that takes `self` and returns a new value is likely to map it while preserving meaning. Not yet planned. To be considered as a candidate after the current TODOs.
- Loops, `const` inside functions, bitwise operations, and `char` are each concentrated in 1-2 entries. Low general priority.

### 6.4 After TODO 31 (local closures)

Closures bound with `let` become typed arrow functions (mapping in design/02 §6.2). The 339 `expr/closure` cases went to 0. Acceptance still does not change (functions 4 / 1026, types unchanged). As expected, std methods and character handling behind the closures became visible.

| Count | Reason code | Main contents |
| --- | --- | --- |
| 305 | `type/reference` | `&mut` (idsmith's RNG) |
| 198 | `expr/method-call` | `chars`×341, `len`×190, `to_string`×13 |
| 181 | `check/needs-annotation` | Integer literals compared with a rejected `.len()`. Disappears once the method has a type |
| 110 | `literal/other` | `char` literals such as `'0'` |
| 54 | `expr/index` | `s[i]` |
| 53 | `type/qualified-path` | Module qualification |
| 46 | `expr/macro` | `format!`×39, `vec!`×4 |

The breakdown for the 196 functions excluding idsmith is almost unchanged (`async` 29, `for` 18, `const` inside functions 17, etc.). The next TODOs 32-34 (methods of `Option`/`Result`, `Vec`/iterators, `String`/`char`, plus `format!` and `vec!`) target the `expr/method-call`, `check/needs-annotation`, `literal/other`, and `expr/macro` above together.
