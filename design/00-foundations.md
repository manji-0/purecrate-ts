# PureCrate → TypeScript パッケージ変換 — 基礎設計

日付: 2026-09-26
更新: 2026-09-27
状態: 草案 v0（公開面・平坦化・版計画を確定）

## 1. 目的

Rust クレートのうち、次の制約を満たすものを **単一の TypeScript パッケージ** に変換する仕組みを作る。

- 公開面はクレート内の型定義だけで入出力が閉じている
- 公開関数は副作用を持たない
- 代数的データ型（struct / enum = ユニオン）が第一級
- 典型的な公開関数は純粋遷移 ` (State, Event) -> State | Result<State, Error> `

変換結果は npm で消費できる TS ソース（型 + 実装）とする。実行時に Rust / WASM を必須としない。

## 2. 非目的（v0）

- 任意の Rust クレートの完全コンパイル
- I/O・非同期・スレッド・`unsafe`・トレイトオブジェクトの再現
- 既存 JS エコシステム型（DOM, Node fs 等）との自動結合
- WASM バイナリの生成（将来の第二バックエンドとして残す）
- 参照・ライフタイムを公開 API に残したままの変換

## 3. 用語

| 用語 | 意味 |
| --- | --- |
| PureCrate | 本仕組みが受理する Rust クレートの部分集合 |
| 公開面 | `pub` な型・関数（到達した非公開実装は生成するが export しない） |
| 遷移関数 | 入力・出力がクレート定義 ADT のみである純粋関数。特に状態 × 事象 → 状態 |
| IR | 変換パイプライン中央の中間表現。Rust 構文にも TS 構文にも依存しない |
| パッケージ | 生成物。`package.json` + `.ts` 実装 + 再エクスポート |

## 4. 設計原則

1. **型が契約である。** 公開関数の入出力型がクレート内で完結していないものは拒否する。
2. **enum は閉じた判別ユニオンに写す。** TS 側で網羅 `switch` が可能であること。
3. **関数は参照透過。** 同じ入力なら同じ出力。グローバル・乱数・時刻・I/O を禁止。
4. **所有値だけが境界を越える。** 公開シグネチャに `&T` / ライフタイムを置かない。
5. **拒否は生成より先。** サブセット外は部分生成せず、診断を出して失敗する。
6. **生成物は機械可読で決定的。** 同じ入力クレートから常に同じ TS を出す。

## 5. PureCrate サブセット（v0）

### 5.1 許可する型

- プリミティブ: `bool`, `i8` `i16` `i32`, `u8` `u16` `u32`, `f32` `f64`, `String`
- `i64` / `u64` は許可するが TS では `bigint`（`number` に潰さない）
- `Option<T>`, `Result<T, E>`, `Vec<T>`
- タプル（要素は許可型のみ）
- 名前付きフィールド `struct`（所有フィールドのみ）
- `enum`（unit / tuple / struct バリアント）
- `type` 別名
- ユニット `()`

禁止: 参照フィールド、ライフタイム、`Box`/`Rc`/`Arc`/`Cell`/`RefCell`/`Mutex`、スライス、トレイトオブジェクト、ユーザー定義ジェネリクス、`HashMap` / `BTreeMap`、外部クレート型。

### 5.2 許可する関数

```
pub fn name(arg: OwnedType, ...) -> OwnedType
```

- 本体は式言語: リテラル、`let`、`if` / `if let`、`match`、コンストラクタ、フィールドアクセス、タプル、許可関数の呼び出し、`?`（`Result` のみ）
- 局所 `mut` と再代入は許可（SSA / 代入文に正規化してから生成）
- `impl Type { pub fn ... }` の所有または論理的に純粋なメソッドは、第一引数がレシーバの自由関数に正規化して公開してよい

禁止: `async`, `unsafe`, マクロ（許可リスト以外）、クロージャの捕獲、`panic!` による制御、`println!`、静的可変、外部関数。

許可マクロ（v0）: `vec![]`、`Some`/`None`/`Ok`/`Err`、`todo!` は拒否、`unreachable!` は TS `never` 分岐。

### 5.3 モジュールと公開面

- 公開面は `pub` を自動採用する。`#[purecrate::export]` は不要（将来の絞り込み用に予約するだけ）
- 対象: `pub` な struct / enum / type alias / fn / `impl` 上の `pub` メソッド
- 非 `pub` でも、公開関数の本体または公開型のフィールドから到達する型・関数は生成に含める（内部実装）
- `pub(crate)` / `pub(super)` は非公開と同じ
- クレート内 `mod` は TS に残さない。平坦な名前空間へ畳む
- `pub use` は畳み先の名前を公開名にする
- 畳み後に型名・関数名が衝突したら拒否する（モジュールパスを TS 名に埋め込まない）

### 5.4 依存

- `purecrate` 自身以外の外部クレート型を公開面に出さない
- 内部実装での外部クレートも v0 は禁止（解析境界を閉じるため）

## 6. パイプライン

```
ソースクレート
  → 解析 (syn / rustc_ast 相当。v0 は syn)
  → 公開面抽出
  → サブセット検査（型閉包・副作用・参照）
  → IR
  → TS 印刷
  → パッケージ組み立て (package.json, tsconfig, index)
```

変換器自体は Rust CLI + ライブラリ:

```
purecrate-ts build <crate-path> --out <dir> [--name <crate>]
purecrate-ts check <crate-path> [--out <dir>] [--name <crate>]
```

`<crate-path>` はクレートのディレクトリ（`src/lib.rs` を読む）か単一の `.rs`。`--name` 省略時は `Cargo.toml` の `[package] name`、なければディレクトリ名。`check` は `--out` なしで検査のみ、ありで生成物とのバイト一致も見る（差分・欠落・余分を列挙し終了コード 1）。

解析は rustc に依存しない。型推論は限定的に自前で行う（注釈必須を原則とし、局所推論のみ）。

## 7. IR

IR は「型定義」と「関数定義」の二部。

### 7.1 型

```
Ty =
  | Prim(Bool|I32|U32|I64|U64|F32|F64|String|Unit)
  | Option(Ty)
  | Result(Ty, Ty)
  | Vec(Ty)
  | Tuple([Ty])
  | Named(Path)
  | Never

Adt =
  | Struct { name, fields: [(name, Ty)] }
  | Enum { name, variants: [Variant] }
  | Alias { name, ty }

Variant =
  | Unit { name }
  | Tuple { name, elems: [Ty] }
  | Struct { name, fields: [(name, Ty)] }
```

### 7.2 関数

```
Fn = { name, params: [(name, Ty)], ret: Ty, body: Expr }

Expr =
  | Lit | Var | Let | Assign
  | If | Match
  | Call { callee, args }
  | Construct { adt, variant?, fields }
  | Field | Tuple | Array
  | Return | Unreachable
```

`Match` は enum の網羅に正規化する。ガードは v0 対象外でもよいが、単純な `if` ガードは許可候補。

## 8. 型写像

| Rust | TypeScript |
| --- | --- |
| `bool` | `boolean` |
| `i8`..`i32`, `u8`..`u32`, `f32`, `f64` | `number` |
| `i64`, `u64` | `bigint` |
| `String` | `string` |
| `()` | `undefined` |
| `Option<T>` | `T \| null` |
| `Result<T,E>` | `Readonly<{ kind: "Ok"; value: T }> \| Readonly<{ kind: "Err"; error: E }>` |
| `Vec<T>` | `ReadonlyArray<T>` |
| `(A,B)` | `readonly [A, B]` |
| `struct S { a: T }` | `export type S = Readonly<{ a: T }>` + companion `const S` |
| `enum` | `kind` 判別ユニオン + companion（次節） |

数値は公開境界で混在させない。`i64` を `number` に落とすオプトインは持たない。

### 8.1 数値演算の意味論

同値性の基準は Rust の debug ビルド（design/04 §1.3、§5）。`check::accept` が式ごとに型を推論し、印刷前に次の形へ書き換える。

| Rust | 生成 TS |
| --- | --- |
| 整数の `+ - * / %`、単項 `-` | `Int.<型>.add(a, b)` など。`/` は切り捨て、オーバーフローとゼロ除算は Rust の panic 文と同じ文言で throw。`-0` は `0` に正規化 |
| `f32` の `+ - * / %` | `Math.fround(a op b)`。`f32` リテラルは `Math.fround(lit)` |
| `f64` の演算 | JS の演算子そのまま |
| `i64`/`u64` のリテラル | `5n` |

`Int` は生成物の `int.ts` にあり、名前 `Int`・`Math` とファイル名 `int` は予約する。推論は式木の中で閉じた双方向推論で、rustc が後続の使用から決める型や `i32`/`f64` への既定値は使わない。型が決まらない数値リテラルは、接尾辞（`1i64`）か `let x: T` の注釈を求めて拒否する。JS と Rust で結果が変わる比較（struct の `==`、`String` の大小比較）も拒否する。

## 9. enum → ユニオン

既定は kamae-ts と同じ **`kind` 内部タグ**。`type` / `status` / `tag` は使わない。

```rust
enum Cmd {
    Quit,
    Move(i32, i32),
    Paint { color: String },
}
```

```ts
export type Cmd =
  | Readonly<{ kind: "Quit" }>
  | Readonly<{ kind: "Move"; content: readonly [number, number] }>
  | Readonly<{ kind: "Paint"; color: string }>;

export const Cmd = {
  Quit: (): Cmd => ({ kind: "Quit" }),
  Move: (a: number, b: number): Cmd => ({ kind: "Move", content: [a, b] }),
  Paint: (color: string): Cmd => ({ kind: "Paint", color }),
} as const;
```

生成補助:

- 型と同名の Companion Object（バリアント構築・関連関数）
- 網羅検査用 `assertNever(x: never): never`（到達したら予期しない故障として throw）

serde の externally / adjacently tagged は v1。v0 は `kind` に固定する。

## 10. 関数生成規則

- `match e { ... }` → `switch (e.kind)` + バリアント束縛。欠落腕は検査フェーズで拒否
- `if let Enum::V { .. } = e` → `kind` 判定 + 狭め
- `?` → `if (r.kind === "Err") return r`
- `Option` は `=== null` で分岐
- struct 更新構文 `S { a: 1, ..s }` → スプレッド `{ ...s, a: 1 }`
- `impl` メソッドは Companion の関数プロパティ `Type.method: (self, ...) => ...`（メソッド記法は使わない）

参照透過を保つため、生成 TS は引数を変異しない。更新は新しいオブジェクトを返す。

## 11. 遷移関数の慣習

言語機能としてはただの純粋関数。次の形を文書上の標準形とする。

```rust
pub fn step(state: State, event: Event) -> Result<State, Error> { ... }
```

TS:

```ts
export function step(state: State, event: Event): Result<State, Error>
```

トレイト `Transition` は v0 で必須にしない。必要なら後で IR 上の印として付ける。

## 12. 出力パッケージ

```
<out>/
  package.json
  tsconfig.json
  src/
    assert-never.ts
    result.ts          # 組み込み Result の type + companion
    event.ts           # 1概念1ファイル（例）
    state.ts
    step.ts
    index.ts           # 再エクスポートのみ
```

Rust モジュールは平坦化するが、TS 側は kamae-ts に合わせ **1概念1ファイル** にする。`types.ts` / `fns.ts` のような寄せ集めは出さない。ファイル名は公開名の kebab-case。

`package.json` は `type: "module"`、`exports` で `src/index.ts`（または emit 後の `dist`）を指す。パッケージ名は入力クレート名を kebab-case にしたものを既定とする。

生成ファイル先頭にスタンプ:

```
/* generated by purecrate-ts. do not edit. */
```

`check` は既存生成物とのバイト一致（または正規化後一致）でドリフトを検出する。

## 13. 検査（受理条件）

変換前にすべて満たすこと。

1. 公開関数の型が、クレート内 ADT + 許可組み込みの閉包になっている
2. 関数本体が許可式のみ
3. 禁止パス（`std::fs`, `std::net`, `std::time::SystemTime`, 乱数 等）への到達がない
4. enum match が網羅
5. 公開シグネチャに参照・ライフタイムがない
6. 再帰型は許可（`Box` なしで IR の Named 参照）。生成 TS は type alias の前方参照で表現
7. 平坦化後の型名・自由関数名が一意

失敗時はファイル・行・拒否理由を返す。部分ファイルを書き残さない（`--out` は成功時のみ置換）。

## 14. リポジトリ構成（本プロジェクト）

```
purecrate-ts/
  crates/
    ir/          # IR データ型。依存最小
    syntax/      # syn 解析 → IR
    check/       # サブセット検査（名前・解決・網羅）と到達しない非公開の除去
    emit_ts/     # IR → TS 文字列
    pack/        # パッケージ組み立て
    cli/         # build / check。tests/ にゴールデンと Rust/TS 同値テスト
  examples/
    counter/     # 最小遷移クレート
    counter-ts/  # その TS パッケージ（生成物。ゴールデン）
  scripts/
    verify.sh    # cargo test + 生成物の tsc
```

各クレートの公開関数も、可能なら純粋にする。ファイル I/O は `cli` と `pack` に閉じる。

## 15. 最小例（受け入れ基準）

入力:

```rust
pub enum Event { Inc, Dec, Reset }

pub struct State { pub n: i32 }

pub fn step(state: State, event: Event) -> State {
    match event {
        Event::Inc => State { n: state.n + 1 },
        Event::Dec => State { n: state.n - 1 },
        Event::Reset => State { n: 0 },
    }
}
```

出力 TS が同一の入出力型を持ち、任意の `State` × `Event` で Rust と同じ値を返すこと。これが v0 の完了条件。

## 16. 確定した版計画

- 公開面: `pub` 自動採用（2026-09-27）
- モジュール: 平坦化。衝突は検査エラー（2026-09-27）
- エラー: ドメインの `Result` は `{ kind: "Ok" | "Err" }`。想定失敗は値。`assertNever` だけ予期しない故障として throw
- ジェネリクス: v0 は `Option` / `Result` / `Vec` のみ。ユーザー定義の型パラメータは v1
- `HashMap` / `BTreeMap`: v0 禁止。v1 はキーが `String` のときだけ `ReadonlyMap<string, V>`
- WASM: 同じ IR から出す第二バックエンドとして予約する。v0 では実装しない。IR を TS 印刷に固定しない

詳細は `01-surface-flatten-roadmap.md`。v1 はカウンタ例が通ったあと、型パラメータなしでは表せない例が必要になった時点で入れる。

## 17. 既存ツールとの位置

`ts-rs` / `tsify` / `typeshare` は **型宣言** の生成に強い。本仕組みは型に加え **純粋関数の実装** ごと TS パッケージ化する。WASM バインドは代替実装であり、v0 の主経路ではない。
