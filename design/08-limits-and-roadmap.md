# 限界とロードマップ

日付: 2026-09-28
状態: 現行の全体像

<!-- constrained-by ./07-authored-constraints.md -->
<!-- derived-from ./04-objective-means-demand.md -->
<!-- constrained-by ./05-type-sharing-scope.md -->
<!-- constrained-by ./01-surface-flatten-roadmap.md -->

## 0. この文書の位置

対象は、[PureCrate の制約](./07-authored-constraints.md)の中で新しく書くコードである。既存クレートの受理率を上げる計画ではない。計測は [design/06](./06-acceptance-survey.md) に残し、指標には使わない。

ここまでで、純粋な遷移を書くときに Rust 側が受け入れる制約と、生成した TS を呼ぶ側の制約が、一枚で言えるところまで来た。個別の決定は [design/07](./07-authored-constraints.md)、[design/04](./04-objective-means-demand.md)、[design/01](./01-surface-flatten-roadmap.md)、[design/05](./05-type-sharing-scope.md) にある。この文書はその全体と、その先に足す順序である。

足す条件は一つである。制約の中で新しく書いた例が、その能力なしでは書けなくなったときだけ足す。コーパスの拒否件数では順序を決めない。

## 1. 今書けるもの

状態と事象を ADT で書き、`fn step(state, event) -> State` のように次の値を返す遷移である。カウンタ例が受け入れ基準である（[design/00](./00-foundations.md) §15）。

書けるもの:

- struct、enum（`kind` 判別ユニオン）、1 要素のタプル構造体（newtype）
- `Option`、`Result`、`?`、`if let`、網羅的な `match`
- 局所的な `let mut`。更新は新しい値を返す
- 不変の束縛だけを捕捉するローカルクロージャ
- 構造体更新 `S { a: e, ..base }`
- 増減する列は再帰 enum。`Box<T>` で所有する木を書く
- `Vec` は、外で長さが決まった列を添字と `len` で読む
- 整数と浮動小数の幅を、TS のブランド型で分ける
- std に `From` がある整数の拡大を `i64::from(x)` で書く
- 固定の文字列を `String::from("…")` で作る
- `--schema zod|valibot|arktype` で、serde の既定 JSON をドメインの値へ読む

## 2. Rust で書く側の制約

生成物が Rust の debug ビルドと同じ結果になるために、書き方をこちらに寄せる。能力の定義は [design/07](./07-authored-constraints.md) §0 である。

### 2.1 状態は次の値として返す

遷移は `state` を受け取って新しい `State` を返す。`&mut self`、フィールドへの代入、引数の `mut` は書けない。局所の `let mut` は、その関数の中だけで使える。

イベントは状態に積まない。過去の列が要るなら、遷移の外に置く（[design/02](./02-kamae-ts-emit.md) §1.1）。

増減する列は `Lines::Cons(line, Box::new(lines))` のように再帰 enum で、新しいリストとして返す。`Vec` に `push` しない。`Vec` は、関数の外で長さが決まった列を `[a, b]` で作り、`xs[i]` と `xs.len()` で読む。`map` / `filter` / `collect` と `vec!` は入れない。

### 2.2 数は幅のある整数と浮動小数だけ

10 進小数型は入れない。`Decimal` も `rust_decimal` も受理しない。金額は最小単位の整数 newtype（`struct Yen(i64)`）で書く。

`i32` と `f64` は、実行時はどちらも `number` だが、型の上では `I32` と `F64` で混ざらない。型が決まらない数値リテラルは、接尾辞か `let x: T` を書いて決める。素の `+` の結果はブランドを失うので、ドメインに戻す計算は `Int.i32.add` のように幅を指定した演算で書く。

幅の違う整数は、std に `From` がある拡大だけ `to::from(x)` で変換できる。`u8` / `u16` / `u32` から広い符号なしと、より広い符号付きへ、`i8` / `i16` / `i32` から広い符号付きへ、である。`usize` は std と同じく `u8` と `u16` からだけ受ける。値は変わらない。TS では `number` から `bigint` になるときだけ `BigInt(x)` を出す。縮小、`as`、`.into()`、`try_from` は拒否する。[examples/order](../examples/order/src/lib.rs) は数量を `u32` で持ち、`i64::from(line.qty)` で単価に掛ける。

同値性の基準は Rust の debug ビルドである。オーバーフローとゼロ除算は throw する。release のラップには合わせない。`usize` が 2^53 以上のとき、再帰が深いとき（約 1 万段）などは同値性の外である（§4、[design/04](./04-objective-means-demand.md) §1.3.1）。

非公開フィールドとスマートコンストラクタで守る不変条件は、TS には残らない。生成物はすべての struct に `of` を出すので、TS からは Rust の `new` が拒否する値も作れる。TS 側で値を作るときも不変条件を守るなら、`of` ではなく、Rust の検査つきコンストラクタ（`pub fn new(..) -> Result<Self, E>` は `Percent.new` になる）を呼ぶ。

### 2.3 名前はクレート全体で一意

モジュールパスは生成名に残らない。`billing::State` と `shipping::State` は同時に書けない。自由関数名も同じである（[design/01](./01-surface-flatten-roadmap.md) §2）。型と関数も、kebab-case のファイル名が重なれば衝突する。型 `Command` と関数 `command` は、どちらも `command.ts` になるので同時に書けない。

ユーザーが書いた型パラメータは拒否する。許可する型コンストラクタは `Option`、`Result`、`Vec`、それに消える `Box` / `Arc` / `Mutex` だけである。`HashMap` と `BTreeMap` は拒否する。キーの等価が Rust と JS で違うからである。

`#[serde(...)]` は拒否する。黙って別名の JSON を受理しない。フィールド名は Rust の名前のままである。

### 2.4 共有と内部可変は値に潰さない

`Rc`、`Cell`、`RefCell` は拒否する。シングルスレッドでも、共有先の書き換えや内部可変は、値に潰すと結果が変わる。

`Box<T>`、`Arc<T>`、`Mutex<T>` は `T` に消す。`::new(v)` は `v` になる。生成物には、Rust では何のために使うかと、TS はシングルスレッドなので無視する、というコメントを残す。`lock` と `clone` はメソッドにならない。所有する再帰データは `Box` で書く。`Box` と `Arc` は `*x` で中身を読める。`Mutex` は `lock` なしでは読めないので、作って保持することしかできない。

クロージャは関数の中だけで使う。`let mut` を捕捉しない。引数・戻り値・フィールドには置かない。

### 2.5 `match` の腕は一つのバリアントを名指す

腕に書けるのは、enum のバリアント、`Some` / `None`、`Ok` / `Err` だけである。次は書けない。

- `match (state, event)` のようなタプルの scrutinee。状態ごとの関数に分け、その中で事象を `match` する
- `_ =>` と `A | B =>`。受理しない遷移は、バリアントごとに腕を書く。腕の数は状態数と事象数の積で増える
- 束縛だけの腕（`lines => ...`）。バリアントを名指して値を組み直す

### 2.6 文字列は `String::from` で作り、`==` で比べる

文字列リテラルの型は `&str` である。`String` の位置には置けない。`String::from("a")` と書く。TS ではリテラルそのものになる。`"a".to_string()`、`.to_owned()`、`.into()` は拒否する。書き方を一つに保つためである。

`String` は `&str` の位置に置ける。`==` と `!=` は `String` と `&str` をどちらの順でも比べる。`clone` はできないので、一つの `String` を二か所に置くには、もう一度 `String::from` で作る。

### 2.7 仕様はあるが、まだ書けない

次は決定済みで、実装は拒否のままである。例がこれらなしでは書けなくなったときに足す（§5）。

- `char`、文字列のバイト長・バイト添字、`String` の大小比較。仕様は [design/04](./04-objective-means-demand.md) §1.5。今は文字の列を `String` の等価だけで扱う。順序が要るなら enum か整数にする。
- `isize`
- `while` / `for`、`match` のリテラルパターン
- std のメソッド許可リスト。`Vec::len` と添字だけが、その先取りとして入っている
- `const` / `static`

struct と enum の `==` は拒否する。JS の構造比較は Rust と一致しない。比較は `eq` メソッドで書く。

## 3. TS で呼ぶ側の制約

変換が通った関数について、呼び出し側が守ることである。詳細は [design/07](./07-authored-constraints.md) §4。

- メソッドは `State.bump(state)` である。`this` は出ない。構造体と配列は `Readonly` で、更新は戻り値で行う。
- 想定した失敗は `Result` の値である。throw するのはオーバーフロー、ゼロ除算、範囲外の添字、`assertNever` だけである。
- 数はブランドである。外から入れるときは `Int.i32.of` のように検査して入れる。生の `number` の演算結果は、ブランドの引数に戻せない。
- `i64` / `u64` は `bigint` である。serde_json の JSON を読むときは、`JSON.parse` ではなく `parseJson` を使う。`JSON.parse` の結果を渡すと、2^53 を超える値はスキーマが拒否する。
- enum のメモリ上の形は `kind` である。serde の既定 JSON とは違う。JSON を関数に渡すときは、`--schema` が出したワイヤ用スキーマを通す（[design/05](./05-type-sharing-scope.md) §7.5）。
- 欠けた `Option` フィールドと JSON の `null` は `None` である。`undefined` は `()` であり、不在ではない。
- Rust では呼び出し後に元の `state` は使えない。生成 TS は引数を書き換えないので、元のオブジェクトは残る。`Object.freeze` はしない。残った元の値や、型を外した書き換えは、同値性の約束の外である。
- 同値性が約束されるのは、Rust の値を写した TS の値を渡したときだけである（[design/04](./04-objective-means-demand.md) §1.3.1）。`of` で不変条件を破った値や、`as I32` で検査を飛ばした値を渡したときの結果は約束しない。
- 生成ファイルは編集しない。変えるときは Rust を変えて作り直す。

## 4. 同値性の残り

受理した入力では、戻り値、想定した `Result`、debug の整数演算を Rust と合わせる。定義域は Rust の値を TS に写した像である（[design/04](./04-objective-means-demand.md) §1.3.1）。次は、その外である。

| 穴 | 扱い |
| --- | --- |
| `usize` が 2^53 以上 | 明示した非同値。長さと添字はこの範囲に届かない |
| 再帰の深さ | TS（Node 24 の既定スタック）は約 1.2 万段で `RangeError`。Rust の debug は主スレッドで 5 万段を通り、10 万段では abort する。同値性の外で、失敗の仕方も違う |
| JSON の入れ子の深さ | serde_json は 128 段を超える入れ子を拒否する。ワイヤ用スキーマに上限はない。境界の同値性は 128 段以下 |
| 非公開フィールドの不変条件 | TS の `of` で破れる。像の外 |
| 差分テストの比較範囲 | 値全体ではなく、テストが選んだ射影（`kind` と `content`、駆動関数が返すスカラー）を比べる。struct バリアントのフィールドは比べていない（[design/04](./04-objective-means-demand.md) §1.4.1） |
| release ビルドのラップ | 合わせない。基準は debug |
| `i64` の JSON 数 | スキーマは安全な整数の数値、`bigint`、数字の文字列を受ける。大きな値は `parseJson` でテキストから読む。`JSON.parse` の結果では 2^53 を超える値を拒否する（[design/05](./05-type-sharing-scope.md) §7.6） |
| 非有限の `f64` | serde_json は `NaN` を `null` として書く。拒否するか、往復の非対称として文書化するかは未決（[design/05](./05-type-sharing-scope.md) §6） |
| 文字列の `.length` と `[i]` | 生成物はこれらの演算を出さない。呼ぶ側が JS の UTF-16 単位を Rust のバイト長だと思ってはいけない |
| 型推論の実装 | 出力を決める型付けは自作の双方向推論である。rustc の型情報に切り替えるかは未決（[design/04](./04-objective-means-demand.md) §5） |
| コンパイルできない入力 | `check` と `build` はサブセット検査の後に rustc をかけ、エラーを `path:line:col: [rustc/E0382]` の形で報告して拒否する。サブセット検査は借用を消し、move とライフタイムを追わないので、その範囲は rustc が塞ぐ。`check` の実行には rustc が要る（`RUSTC` で差し替え可）。起動できなければ失敗する |

## 5. ロードマップ

順序は、新しい例が止まった場所で決める。拒否件数の多い構文からではない。

### 5.1 済

カウンタ型の遷移、再帰的な列、newtype、`Result`、数値のブランド、`--schema` による JSON からドメイン値への読み取り。

制約の中で書いた二つ目の例として、注文ライフサイクル（[examples/order](../examples/order/src/lib.rs)）がある。下書き・確定・支払い・出荷・取消の五状態で、明細は再帰 enum、金額は `Yen(i64)` である。4 手の全列を Rust と生成 TS で比べている（`crates/cli/tests/order_equivalence.rs`）。

### 5.2 例が止めたもの

examples/order を書いたとき、次の二つで止まった。どちらも足した。

1. **情報を失わない整数の拡大**（§2.2）。数量を `u32` に戻せた。20 通りの拡大を、元の型の両端で差分テストしている（`crates/cli/tests/widen_equivalence.rs`）。縮小（`try_from`）と `as` は、例が求めるまで拒否のままにする。
2. **文字列リテラルからの `String`**（§2.6）。ドメインの遷移ではなく、コードから `Command` を組み立てる差分テストの駆動関数で止まっていた。同時に、リテラルを `String` の位置に置く入力を拒否した。rustc が拒否するものを `check` が受理していた穴である。

記法の損失（§2.5 の腕の制約）は、能力を落としていないので、ここには入れない。`_ =>` の腕数が遷移表を読めなくするほど増えたら、そのときに扱う。

### 5.3 例が必要になったら足す

1. **境界の書き出し。** 読み取りはある。同じ JSON をドメインから書く側（encode）はまだない。サーバーが生成物と同じ形で JSON を返す例が先に要る。`i64` の読み取りは `parseJson` で済んだ（2026-09-28）。書き出しでは、`bigint` を JSON の数値として書く必要がある。`NaN` の扱いはそのときに [design/05](./05-type-sharing-scope.md) §6 を閉じる。
2. **文字と文字列。** 検証をこのサブセットの中で書く例が必要になったとき。UTF-8 バイト単位を再現し、JS の `.length` には写さない。メソッドは一つずつ、差分テスト付きの許可リストで足す。
3. **繰り返し。** 再帰と `match` で書けない例が出てから `while` / `for` を足す。状態機械は今の形で足りている。
4. **その例が呼ぶ std メソッド。** 一致させられないものは拒否したままにする。イテレータの `map` / `filter` / `collect` は、状態の列を配列で伸ばす書き方なので足さない。

### 5.4 型の表現が足りなくなったら（v1）

[design/01](./01-surface-flatten-roadmap.md) のままである。

- 境界なしの型パラメータ。TS 側もジェネリクスのまま出す。`where` と関連型は入れない。
- キーが `String` の `HashMap` / `BTreeMap` だけを `ReadonlyMap<string, V>` にする。挿入順は Rust と一致させない。

どちらも、型パラメータなし・Map なしでは表せない例が現れてからである。

### 5.5 やらない

- 既存クレートが通ることを目標にした許可リスト。需要と成否は、第三者の仕様から書いた例と撤退の閾値で測る（[design/04](./04-objective-means-demand.md) §1.4.2）
- 10 進小数、成長する `Vec`、状態の中のイベントログ
- 核パッケージへのスキーマライブラリの依存。zod / valibot / arktype は、指定した一つだけを別パッケージで使う
- v0 の完了条件としての WASM。IR は第二バックエンドを拒まないが、今の経路は TS ソースである
- `Rc` / `Cell` / `RefCell`。値に潰すと観測が変わる

## 6. 文書の役割

| 文書 | 役割 |
| --- | --- |
| [design/08](./08-limits-and-roadmap.md) | 制約の全体と、足す順序 |
| [design/07](./07-authored-constraints.md) | 新しく書くときの能力と、TS 側に残る制約 |
| [design/04](./04-objective-means-demand.md) | 同値性の定義と、意味論の決定 |
| [design/05](./05-type-sharing-scope.md) | JSON との境界 |
| [design/01](./01-surface-flatten-roadmap.md) | 公開面、平坦化、ジェネリクスと Map の版 |
| [design/06](./06-acceptance-survey.md) | 既存クレートを測った記録。以後の指標ではない |
