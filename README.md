# purecrate-ts

Rust で書いた純粋なドメイン関数を、WASM なしの TypeScript パッケージに変換する。生成物は普通の TS の値で、`tsc --strict` が通り、受理した入力では Rust の debug ビルドと同じ結果を返す。

任意の Rust をコンパイルするものではない。対象は、[PureCrate の制約](design/07-authored-constraints.md)の中で新しく書くコードである。状態と事象を ADT で表し、`fn step(state, event) -> Result<State, Error>` のような遷移を共有する用途を想定している。設計の全体は [design/00-foundations.md](design/00-foundations.md)。

## 必要条件

- Rust（edition 2021）。依存は `vendor/` にあり、`cargo --offline` でビルドできる。`check` と `build` は入力を `rustc` でもコンパイルするので、実行時にも `rustc` が要る（`RUSTC` で差し替え可）。
- 生成物の型検査と、Rust との差分テストには Node と `npx` が要る。型検査は TypeScript 6 と 7 の両方で行う（`npx -p typescript@6` と `@7` を取りに行く）。

## 使い方

```sh
cargo run --offline -p purecrate-ts -- build examples/counter --out /tmp/counter-ts
```

`<crate-path>` はクレートのディレクトリ（`src/lib.rs`）か、単一の `.rs` ファイル。`--name` を省くと `Cargo.toml` のパッケージ名を使う。

```text
purecrate-ts build <crate-path> --out <dir> [--name <crate>] [--schema zod|valibot|arktype]
purecrate-ts check <crate-path> [--out <dir>] [--name <crate>] [--schema zod|valibot|arktype]
purecrate-ts survey <crate-path>... [--json]
```

`check` は受理できない定義を `path:line:col` と理由コードで拒否し、ファイルを書かない。サブセットの検査を通ったあと、入力を rustc にかけ、コンパイルできなければ `[rustc/E0382]` のように rustc のエラーコードで拒否する。`check` が通れば、入力はライブラリとしてコンパイルできる。`--out` を付けると、既存の生成物とのバイト一致も見る。`survey` は公開関数と公開型が、参照先ごと受理できるかを JSON で出す。

数値のブランドはパッケージ `purecrate` にある。`--schema` を付けたときだけ、そのライブラリ向けのワイヤ用スキーマを `src/purecrate-wire.ts` に出す。serde の既定 JSON を、ドメインのブランド型へ読む。指定していないライブラリのスキーマは出さない。

生成したパッケージは編集しない。変えるときは Rust を変えて作り直す。

## テスト

```sh
./scripts/verify.sh
```

`cargo test --offline`、examples/counter の生成物とのドリフト検出、examples/order の `check`、ランタイムパッケージ（`packages/`）と counter の生成物への TypeScript 6・7 の `tsc` を順に走らせる。差分テストは、同じ入力を Rust と生成 TS（Node）の両方で実行して比べる。

## 受理するもの

v0 が書けるのは、おおよそ次である。詳細と、生成 TS を呼ぶ側の制約は [design/07-authored-constraints.md](design/07-authored-constraints.md)。制約の全体と、その先に足す順序は [design/08-limits-and-roadmap.md](design/08-limits-and-roadmap.md)。

- struct、enum（`kind` 判別ユニオン）、1 要素のタプル構造体（newtype）
- `Option`、`Result`、`?`、`if let`、網羅的な `match`
- 局所的な `let mut`。更新は新しい値を返す。`&mut` は拒否する
- 整数演算は Rust の debug ビルドに合わせる。オーバーフローとゼロ除算は throw する。`i64` / `u64` は `bigint`
- 幅の違う整数は、std に `From` がある拡大だけ `i64::from(x)` で変換する
- 固定の文字列は `String::from("…")` で作る。`String` と `&str` は `==` で比べる
- 不変の束縛だけを捕捉するローカルクロージャ
- 構造体更新 `S { a: e, ..base }`
- 増減する列は再帰 enum。`Vec` は、外で長さが決まった列を添字と `len` で読む

10 進小数型は入れない。金額は最小単位の整数 newtype（`struct Yen(i64)`）で書く。状態型は可変配列を持たない。

## 設計メモ

| 文書 | 内容 |
| --- | --- |
| [design/00-foundations.md](design/00-foundations.md) | サブセット、型の写像、パイプライン |
| [design/02-kamae-ts-emit.md](design/02-kamae-ts-emit.md) | 生成 TS の形 |
| [design/04-objective-means-demand.md](design/04-objective-means-demand.md) | 目的、同値性、意味論の決定 |
| [design/05-type-sharing-scope.md](design/05-type-sharing-scope.md) | JSON との境界 |
| [design/06-acceptance-survey.md](design/06-acceptance-survey.md) | 既存クレートを測った記録。以後の指標ではない |
| [design/07-authored-constraints.md](design/07-authored-constraints.md) | 新しく書くときの制約と、TS 側に残る制約 |
| [design/08-limits-and-roadmap.md](design/08-limits-and-roadmap.md) | 制約の全体と、足す順序 |

## ライセンス

クレートの `license` は MIT。
