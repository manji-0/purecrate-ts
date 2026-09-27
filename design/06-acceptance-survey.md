# 受理率の計測（2026-09-27）

design/04 §3.5-1 の計測。公開されている Rust コードに `purecrate-ts survey` をかけ、公開関数と公開型がそのまま受理されるかを数えた。

## 0. 結論

1. **既存コードの公開関数は、ほぼ受理できない。** ドメイン寄りの 8 件で 956 関数中 1 件（0.1%）。
2. **公開型は約半分を受理できる。** 87 型中 44 型（51%）。型だけを共有する用途（design/05）は、既存コードでも現実的に成り立つ。
3. **関数の最初の壁はシグネチャの参照（`&T`・`&self`）、次の壁は標準ライブラリのメソッド。** 参照を値として扱う試作では受理数は増えず、拒否理由の首位が `expr/method-call`（`collect`・`len`・`to_string` など）に移った（§3）。
4. **対象は「PureCrate の制約の中で新しく書くコード」と決めた**（design/07）。この文書の受理率は、既存コードをそのまま変換する道を採らなかった根拠として残す。以降の優先度は design/07 が持つ。

## 1. 方法

### 1.1 コーパス

`corpus/manifest.tsv` にリポジトリ・コミット・調査の起点を固定した。`scripts/survey-corpus.sh` が取得と計測を行い、結果を `corpus/results.jsonl` に書く。

| エントリ | 分類 | 起点 |
| --- | --- | --- |
| rust-ddd-example | DDD | クレート全体 |
| rust-ddd-example.domain | DDD | `src/domain` |
| zero-to-production | 検証 | クレート全体 |
| zero-to-production.domain | 検証 | `src/domain` |
| idsmith | 検証（IBAN・各種 ID のチェックサム） | クレート全体 |
| eventually.bank-accounting.domain | DDD | `examples/bank-accounting/src/domain.rs` |
| eventually.light-switch.domain | 状態機械 | `examples/light-switch/src/domain.rs` |
| little-raft | 状態機械（Raft） | `little_raft` |
| poker | ゲームルール | クレート全体 |
| cozy-chess.types | ゲームルール（ビットボード） | `types` |

集計の「ドメイン範囲」は、アプリ全体を除きドメイン部分だけを数えた 8 件（`.domain` の 2 件、idsmith、eventually の 2 件、little-raft、poker、cozy-chess.types）。アプリ全体の 2 件は、HTTP やDB 層を含むので参考値とする。

### 1.2 判定

- 単位は公開関数（自由関数と固有 impl の `pub fn`）と公開型（struct・enum・型別名）。trait impl、`const`、`static`、`trait` は判定せず件数だけ数える。
- 各単位を、それが参照する型と関数の推移閉包と一緒に `check::accept` にかける。`build` がその単位だけを出力しようとしたときの判定と同じ。
  - **受理**: 閉包全体が通る。
  - **拒否**: 単位そのものが範囲外。
  - **巻き込み**: 単位は通るが、参照先が範囲外。
- 理由は TODO 25 で導入した理由コード（`Reason::code()`）で数える。メソッド名・マクロ名・パスなどは `detail` として併記する。

### 1.3 計測上の制約

- **各単位について最初の拒否理由しか分からない。** パーサは単位ごとに最初の範囲外の構文で止まり、シグネチャを本体より先に見る。したがって上位の理由を解消すると、隠れていた理由が次に現れる。§3 はこれを試作で確かめたもの。
- モジュールは名前で平坦化して判定する。別モジュールの同名の型は衝突として拒否される（今回のコーパスでは件数に影響していない）。
- `use` によるパスの別名は解決しない。`shapes::Shape` のようなモジュール修飾は `type/qualified-path` として数える。

## 2. 結果（現行の受理範囲）

### 2.1 エントリ別

| エントリ | 関数（受理 / 総数） | 型（受理 / 総数） |
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
| **ドメイン範囲の計** | **1 / 956（0.1%）** | **44 / 87（51%）** |

idsmith は関数数が多く（830）、合計を支配する。idsmith を除いても関数は 1 / 126。

### 2.2 関数の拒否理由（ドメイン範囲、単位ごとの最初の理由）

| 件数 | 理由コード | 出現エントリ数 | 主な中身 |
| --- | --- | --- | --- |
| 785 | `type/reference` | 3 | `&str`・`&T` 引数（idsmith が大半） |
| 50 | `item/ref-receiver` | 6 | `&self` |
| 32 | `type/self` | 5 | 戻り値や構築の `Self` |
| 20 | `expr/method-call` | 3 | 自前のメソッド呼び出し（poker の `is_pair`・`is_straight` など） |
| 16 | `expr/block-item` | 2 | 関数内の `const` や入れ子の関数 |
| 10 | `expr/operator` | 2 | ビット演算（cozy-chess） |
| 9 | `item/generics` | 3 | |
| 8 | `type/disallowed` | 2 | `usize`・`char` |

idsmith を除くと、上位は `type/self`（22）、`expr/method-call`（20）、`expr/block-item`（16）、`item/ref-receiver`（15）。**idsmith 以外の 7 エントリ中 5 エントリで `&self` が最初の壁になっている**点が、件数より重要。

### 2.3 型の拒否理由（ドメイン範囲）

| 件数 | 理由コード | 主な中身 |
| --- | --- | --- |
| 11 | `item/cfg` | idsmith の feature 切り替え |
| 11 | `item/generics` | |
| 5（＋巻き込み 2） | `item/tuple-struct` | newtype（`struct EntityId(Uuid)` など） |
| 5 | `check/undefined-type` | 外部型（`Decimal`）、マクロで定義された型 |
| 4 | `type/disallowed` | `usize`・`char`・`HashMap` |

## 3. 試算: 共有参照を値として扱った場合

`&T`（`&mut` 以外）を `T`、`&self` を `self`、式の `&x`・`*x` を `x` として読む**未コミットの試作**で同じコーパスを測った。この読み替えは本サブセットでは意味を変えない見込みが高い。生成 TS は値を変異させず、内部可変性（`Cell`・`RefCell` など）は型として拒否済みだから。

| | 現行 | 試作 |
| --- | --- | --- |
| 関数の受理 | 1 / 956 | 1 / 956 |
| 型の受理 | 44 / 87 | 46 / 87 |

受理数はほとんど動かない。拒否理由の首位が入れ替わった。

| 件数 | 理由コード | 主な中身 |
| --- | --- | --- |
| 437 | `expr/method-call` | `collect`×295、`len`×51、`to_string`×33、`replace`×11、`to_uppercase`×10、`unwrap_or`×8 |
| 305 | `type/reference` | 残りは `&mut`（idsmith の乱数生成器引数） |
| 51 | `type/qualified-path` | `super::GenOptions` などモジュール修飾 |
| 33 | `type/self` | |
| 16 | `expr/macro` | `format!`×12 |
| 16 | `expr/block-item` | |
| 14 | `type/array` | スライス `&[T]` |

参照はシグネチャ上の最初の壁にすぎず、本体は **イテレータと文字列・`Option` のメソッド** で書かれている。

## 4. 解釈

- **型の共有は既存コードで成り立つ。** 半数の公開型がそのまま通り、残りの主因（`cfg`、ジェネリクス、newtype、`usize`）は型定義の範囲で対処できる。design/05 の結論（境界コーデックを先に作る）と整合する。
- **振る舞いの共有は、既存コードのままではほぼ成り立たない。** ドメイン関数は `&self` と `&str` を受け取り、`iter().map().collect()` や `s.len()` で書かれている。これらは Rust の慣用であって、純粋性を損なうものではない。サブセットの外にあるのは「書き方」であって「性質」ではない。
- 文字列メソッド（`len`・`to_uppercase`・`replace`）は、design/04 §1.3 の未決点（UTF-8 と UTF-16 の差）に直接ぶつかる。受理するなら、意味の差を差分テストで押さえる必要がある。

## 5. 次の優先順位（提案）

1. **シグネチャの壁を除く（低コスト）**: 共有参照 `&T`・`&self`・`&str` を値として受理する。`Self` を impl の型名に置き換える。newtype（1 要素のタプル構造体）を受理する。単独では関数の受理率を動かさないが、2 の前提になり、型の受理率も上げる。
2. **std のメソッドを許可リストで受理する（中〜高コスト）**: 上位から `Vec`／イテレータ（`iter`・`map`・`filter`・`collect`・`len`）、`Option`（`unwrap_or`・`map`・`is_some`）、`String`（`to_string`・`len`・`replace`・`to_uppercase`・`trim`・`is_empty`）。それぞれに Rust との差分テストを付ける。文字列は長さと添字の単位を決めてから入れる。
3. **型の残り**: `cfg` の扱い（feature を固定して読むか）、ジェネリクス（v1 計画）、`usize` の TS 表現。

### 決定を要する問い

- 対象を「既存の Rust ドメインコード」とするか、「PureCrate の制約内で新しく書くコード」とするか。前者なら §5-2 が必須で、保守する意味論の面積が大きく増える。後者なら §5-1 までで足り、ドキュメントと lint で書き方を案内する。

## 6. 推移

### 6.1 TODO 28 後（共有参照・`Self`・newtype・`Type::method` 呼び出し）

受理したもの:

- 共有参照 `&T`・`&self`・`&str` と、式の `&x`・`*x`。
- `&[T]` を `Vec<T>` として読む。
- ライフタイムだけの総称（`fn f<'a>`）。
- `Self` を impl の型名に置き換える。
- 1 要素のタプル構造体（newtype）と `.0`。
- `Type::method(x)` によるメソッド呼び出し。

| | TODO 27（現行） | TODO 28 後 |
| --- | --- | --- |
| 関数の受理 | 1 / 956 | 3 / 956 |
| 型の受理 | 44 / 87（51%） | 52 / 87（60%） |
| 型の受理（idsmith 除く） | 24 / 51 | 30 / 51 |

関数の拒否理由（最初の理由）の上位:

| 件数 | 理由コード | 主な中身 |
| --- | --- | --- |
| 454 | `expr/method-call` | `collect`×295、`len`×51、`to_string`×34、`replace`×11、`to_uppercase`×10 |
| 305 | `type/reference` | `&mut`（idsmith の乱数生成器） |
| 51 | `type/qualified-path` | `super::GenOptions` などモジュール修飾 |
| 19 | `expr/macro` | `format!`×12、`vec!`×4 |
| 19 | `expr/block-item` | 関数内の `const` など |

idsmith を除くと `expr/method-call` 34 件の中身が変わる。上位は自前のメソッドを `self.hand_rank()`・`self.is_pair()` のようにレシーバ構文で呼ぶもの。std のメソッド（`is_empty` など）はその次にくる。

**次の壁はレシーバ構文のメソッド呼び出し**である。自前のメソッドは、レシーバの型が分かれば `Type::method(x)` と同じに写せる。この型からの解決の仕組みは、std のメソッドの許可リスト（TODO 32〜34）でもそのまま使う。そこで、クロージャより先に行う TODO として追加する。

### 6.2 TODO 29 後（レシーバ構文のメソッド呼び出し）

`x.m(args)` を IR の `MethodCall` として残す。型検査でレシーバの型から、クレート自身の固有 impl のメソッドへ解決する。解決できない呼び出し（std の型のメソッドなど）は、型検査の段階で `expr/method-call` として拒否し、メソッド名を detail に残す。survey は、閉包に含まれる各型について同名のメソッドを依存に加える。レシーバの型は、必ず閉包内のどこかのシグネチャに現れるから。

受理数は変わらない（関数 3 / 956、型 52 / 87）。変わったのは最初の拒否理由の内訳で、メソッド呼び出しの奥にあった理由が見えるようになった。

| 件数 | 理由コード | 主な中身 |
| --- | --- | --- |
| 339（＋巻き込み 16） | `expr/closure` | idsmith の `.map(\|c\| ..)` など。`collect`・`len` の手前で止まっていたもの |
| 305 | `type/reference` | `&mut`（idsmith の乱数生成器） |
| 51 | `type/qualified-path` | モジュール修飾 |
| 33 | `expr/macro` | `format!`×26、`vec!`×4 |
| 26 | `expr/index` | `s[i]` |
| 21 | `expr/loop` | `for` |
| 19 | `expr/block-item` | 関数内の `const` など |

idsmith を除く 126 関数では、首位が分散した。

| 件数 | 理由コード | 主な出所 |
| --- | --- | --- |
| 18 | `expr/loop` | poker（17） |
| 17 | `expr/block-item` | poker（15） |
| 13 | `expr/operator` | cozy-chess のビット演算（11） |
| 11 | `item/ref-receiver` | `&mut self`。eventually の集約（6）、little-raft など 5 エントリ |
| 8 | `type/disallowed` | `char`・`usize` |

### 6.3 次の候補（TODO 30 以降の計画に対する所見）

- **クロージャ（TODO 31）の効果が最も大きい。** idsmith の 339 件は、クロージャを受理すると std のイテレータメソッドの壁（TODO 33）に進む。両方がそろって初めて受理数が動く。
- **`&mut self` は、DDD とイベントソーシングの集約に共通する形**である（`fn apply(&mut self, event)`）。最初の理由では 11 件だが、5 エントリにまたがる。`self` を受け取って新しい値を返す関数に書き換えれば、意味を保ったまま写せる見込みがある。まだ計画にない。現行の TODO の後に候補として検討する。
- ループ・関数内の `const`・ビット演算・`char` は、それぞれ 1〜2 エントリに集中している。汎用の優先度は低い。

### 6.4 TODO 31 後（ローカルのクロージャ）

`let` に束縛したクロージャを型付きのアロー関数にする（写し方は design/02 §6.2）。`expr/closure` の 339 件は 0 になった。受理数はまだ変わらない（関数 4 / 1026、型は変化なし）。予想どおり、クロージャの奥にある std のメソッドと文字の扱いが見えるようになった。

| 件数 | 理由コード | 主な中身 |
| --- | --- | --- |
| 305 | `type/reference` | `&mut`（idsmith の乱数生成器） |
| 198 | `expr/method-call` | `chars`×341、`len`×190、`to_string`×13 |
| 181 | `check/needs-annotation` | 拒否された `.len()` と比べる整数リテラル。メソッドが型を持てば消える |
| 110 | `literal/other` | `'0'` などの `char` リテラル |
| 54 | `expr/index` | `s[i]` |
| 53 | `type/qualified-path` | モジュール修飾 |
| 46 | `expr/macro` | `format!`×39、`vec!`×4 |

idsmith を除く 196 関数の内訳はほぼ変わらない（`async` 29、`for` 18、関数内の `const` 17 など）。次の TODO 32〜34（`Option`/`Result`・`Vec`/イテレータ・`String`/`char` のメソッドと `format!`・`vec!`）は、上の `expr/method-call`・`check/needs-annotation`・`literal/other`・`expr/macro` をまとめて対象にする。
