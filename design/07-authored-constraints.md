# 制約の中で新しく書くコード

日付: 2026-09-27
状態: 決定
前提: design/00、design/04、現行の `check::accept` と `emit_ts`

## 0. 決定

対象は、公開されている Rust を無変更で通すことではない。**PureCrate の制約の中で新しく書くドメインコード**である。

成否は次の二つで見る。

1. その制約で書いた定義が、この目的で Rust が持つべき能力をまだ表せること。能力とは、純粋な遷移を ADT・網羅マッチ・`Result` / `Option`・debug ビルドの整数意味で書けること（design/04 §1.2）。Rust 全体を写せることではない。
2. 生成された TS を呼ぶ側に、どの制約が残るかが列挙されていること。

既存コーパスの受理率（design/06）は、この対象を採らなかった根拠として残す。指標にはしない。`Option` / `Result` のメソッド、イテレータアダプタ、`String` のメソッド、`format!`、コーパスの再計測は、既存コードを通すための作業なので打ち切る。同じ振る舞いは `match` と `?` と文字列の `==` で書ける。

「損なう」は三層に分ける。

| 層 | 意味 | 例 |
| --- | --- | --- |
| 能力 | rustc が受理する書き方で、その振る舞いを表せない | リストの要素を読めない |
| 記法 | 同じ振る舞いをより短く書く手段がない | `.map` の代わりに `match` |
| 注釈 | rustc が推論する型を、こちらが明示させる | 整数リテラルの接尾辞 |

## 1. 制約下でも書ける能力

次は、新しく書く遷移関数の意味を保ったまま書ける。差分テストで Rust の debug ビルドと一致する。

| 能力 | 書く形 | 生成 TS |
| --- | --- | --- |
| 閉じた ADT | struct、enum、newtype（中身が `Option`・`()`・`!` でないもの） | `Readonly` なオブジェクト、`kind` ユニオン、ブランド |
| 網羅 | 単一 enum の `match`。腕は各バリアントちょうど一度 | `switch` と `assertNever` |
| 想定内の失敗 | `Result` / `Option`、`?`、早期 `return`、`if let` | `kind` または `null`。失敗は値 |
| 遷移 | `fn step(state, event) -> Result<State, Error>`。`&self` のメソッドは値として受理され、レシーバ構文はコンパニオン呼び出しになる | 引数を変異しない関数 |
| 局所的な更新 | `let mut` と局所変数への代入。フィールド代入と `&mut` 引数は拒否 | 新しい値を返す |
| 整数 | `i8`〜`i32`、`u8`〜`u32` の `+ - * / %`。切り捨て除算。オーバーフローとゼロ除算は panic と同じ文言で throw | `Int.<型>.*` |
| 広い整数 | `i64` / `u64` | `bigint` |
| 整数の拡大 | std に `From` がある拡大だけ。`i64::from(q)` | 値はそのまま。`bigint` になるときだけ `BigInt(q)` |
| 文字列の構築 | `String::from("…")`。リテラルは `&str` で、`String` の位置には置けない | リテラルそのもの |
| 文字列の等価 | `String` と `&str` の `==` / `!=`（どちらの順でも） | `===` / `!==` |
| 文字列の中身 | `s.as_bytes()` の UTF-8 バイト列を、`&[u8]` の添字と `len` と再帰で読む（2026-09-29） | `Str.bytes(s)`。コードポイントを UTF-8 に符号化した `ReadonlyArray<U8>` |
| 局所クロージャ | 不変の束縛だけを捕捉し、`let` に束縛して呼ぶ。`?` と `return` はクロージャから抜ける | 型付きアロー関数 |
| 再帰呼び出し | 名前のある関数が自分や他の関数を呼ぶ | そのままの関数呼び出し |
| 整数範囲の繰り返し | `for i in a..b { .. }`。両端は同じ整数型で、ループの前に一度だけ評価する。`i` は不変。本体で `let mut` の更新、早期 `return`、`?` を書ける。`a..=b`、イテレータ、`break` / `continue`、ラベル、`while`、`loop` は拒否（2026-09-29） | `for (let i = a, $e = b; i < $e; i = (i + 1) as T)` |

`&self` を値に潰すのは、生成物が引数を変異せず、内部可変性（`Cell`、`RefCell`）を拒否しているからである。`Mutex<T>` は `T` に消え、`lock` はメソッドにならない。観測できる結果は、値を受け取る関数と同じになる。

`fn apply(&mut self, event)` は書けない。`fn apply(self, event) -> Self` は書ける。状態機械の遷移は後者で足りる。

## 2. 制約が落とす能力

### 2.1 列は状態の可変配列にしない

状態型は、段階と、その段階に必要なフィールドだけを持つ。過去のイベントは状態に積まない。遷移の外で渡す（design/02 §1.1）。

要素が増減する列は再帰 enum で書く。`Lines::Cons(line, Box::new(lines))` が、kamae の `[...lines, line]` に当たる。1 要素抜くときは、再帰でその要素を飛ばした新しいリストを返す。

`Vec<T>` は、関数の外で長さが決まっている列を読む型である。値は配列リテラル `[a, b]` で作る。`xs[i]` と `xs.len()` は受理する。添字の型と `len` の戻りは `usize` で、TS では 0 から 2^53−1 まで検査する `number` である。範囲外は `index out of bounds: the len is N but the index is I` で throw する。`map` / `filter` / `collect` と、長さを変える操作（追加、削除、置換）は入れない。`vec!` は拒否する。

### 2.2 所有する再帰データ

rustc は自分自身を値で含む enum を無限サイズとして拒否する。所有する木は `Box` で書く。`Box<T>` は `T` に消して受理する。`Box::new(v)` は `v` になる。実行時の間接化は出さない。

`Arc<T>` と `Mutex<T>` も同じように `T` に消す。`Arc::new(v)` と `Mutex::new(v)` は `v` になる。`lock` や `clone` はメソッドとして受理しない。生成した型と式には、Rust での用途と、TS はシングルスレッドなのでその包みを無視する、というコメントを付ける。

```rust
pub enum Ast {
    Num(i32),
    Add(Box<Ast>, Box<Ast>),
}
```

生成 TS の型は `Ast` を自分で参照する type alias である。`Rc`、`Cell`、`RefCell` は拒否したままにする。シングルスレッドでも、共有先の書き換えや内部可変は値に潰すと意味が変わる。

### 2.3 `Option` の入れ子を書けない

`Option<T>` は `T | null` である。`Option<Option<T>>` は `null` が二段分潰れるので拒否する。`Option`・`()`・`!` を包む newtype も拒否する。`null & { readonly [Brand]: true }` は `never` になる。

「未設定」と「明示的に空」を分けるドメインは、入れ子の `Option` ではなく enum で書く。

```rust
pub enum Patch {
    Unset,
    Clear,
    Set(i32),
}
```

これは TS が表せない区別を、Rust 側の定義から外す制約である。enum で書けば、同じ区別は表せる。

### 2.4 モジュールは名前空間にならない

クレート内の `mod` は平坦になる。平坦化の後で型名か自由関数名が重なれば拒否する。`billing::State` と `shipping::State` は同時に書けない。公開名はクレート全体で一意にする。

`const` と `static` は変換しない。名前付き定数は関数にする。

### 2.5 構造体更新

`State { n: state.n + 1, ..state }` は `({ ...state, n: state.n + 1 })` になる。省略したフィールドは `state` から来る。この位置に置ける副作用は `?` だけで、書き下したフィールドの `?` は `..` の元より先に関数から抜ける。enum のバリアントと newtype の `..` は拒否する。Rust も enum の functional record update を拒否する。

## 3. 記法と注釈

能力は残る。新しいコードは次の形で書く。

| 書きたくなる形 | 代わりに書く形 |
| --- | --- |
| `xs.iter().map(\|x\| f(x)).collect()` | 渡された `Vec` は添字と再帰で読む。新しい列は再帰 enum で返す（§2.1） |
| `opt.map(\|x\| x + 1)`、`and_then` | `match` または `?` |
| `format!("{}", n)` | 文字列が要る遷移では、呼び出し側が整形する。ドメイン関数は数と ADT を返す |
| `state == other`（struct / enum） | `eq` メソッド。JS の構造比較は Rust と一致しないため、演算子は拒否する |
| `s < t`（`String`） | 今は拒否。コードポイント順の比較を入れるまで、順序が要るなら enum か整数にする |
| `for x in xs`、`while`、`loop`、`break` / `continue` | 整数範囲の `for i in a..b` と早期 `return`（§1）。それで書けない繰り返しは、名前のある関数の再帰 |
| ガード付き `match`、入れ子パターン、`let else` | 腕の中の `if`、一段ずつの `match` |
| ユーザ定義ジェネリクス、トレイト、`HashMap` | 具体型を並べる。キー探索が要る状態は v0 の対象外（`HashMap` は v1、design/00 §16） |
| 型の付かない整数リテラル、引数型のないクロージャ、戻り型のないクロージャ内の `?` | 接尾辞（`1i32`）、`\|v: T\|`、`\|v: T\| -> R { .. }` |

`usize` は 0 から 2^53−1 まで検査する `number` である（design/04 §1.5）。`char` の表現は決まっているが、実装はまだ拒否する。文字の列は `String` で書く。

## 4. 生成 TS を呼ぶ側の制約

変換が成功した関数について、呼び出し側が守ることを挙げる。

### 4.1 呼び方

- メソッドは値のメソッドではない。`state.bump()` ではなく `State.bump(state)`。レシーバは第一引数。`this` は出ない。
- 構造体・タプル・`Vec` は `Readonly`。更新は、関数が返す新しい値で行う。
- enum は `kind` で分岐する。バリアントは `Cmd.Move(a, b)` でも、`{ kind: "Move", content: [a, b] }` でも作れる。網羅しない `switch` は `assertNever` が受け持ける。
- モジュールパスは残らない。import はパッケージの `index.ts` から平坦な名前で行う。
- 公開関数の引数名が、同じクレートのアイテム名と重なると `inc$1` のように変わる。呼び出しは位置引数なので結果は変わらない。読んだ名前は Rust の引数名と違うことがある。
- 生成ファイルは編集しない。変えるときは Rust を変えて作り直す。

### 4.2 失敗の二種類

- 想定した失敗は値である。`Result` は `{ kind: "Ok", value } | { kind: "Err", error }`。`Option` の不在は `null`。`undefined` は `()` であり、不在ではない。
- 想定外の失敗は throw である。整数のオーバーフロー、ゼロ除算、範囲外の添字（§2.1 の後）、`assertNever`。ドメインの失敗を throw に載せない。
- 生成関数の中の整数除算は切り捨てる。呼び出し側が `I32` 同士を `/` で割ると、結果の型は `number` になり、`I32` の引数には戻せない。Rust と同じ計算は `Int.i32.div` か、生成された関数を呼ぶ。

### 4.3 数と文字列の形

| Rust | TS の値 | 呼び出し側が足すこと |
| --- | --- | --- |
| `i8`〜`i32`、`u8`〜`u32`、`usize` | ブランド付き `number`（`I32`, `Usize` など） | `Int.i32.of` で入れる。生の `number` の演算結果は戻せない |
| `f32`, `f64` | `F32`, `F64` | `Int.f32.of` / `Int.f64.of` で入れる。`F32` と `F64` は別の型 |
| `i64`、`u64` | ブランド付き `bigint` | `number` と混ぜない。`JSON.parse` は 2^53 を超える整数の精度を落とすので、serde_json の JSON は `parseJson` で読む |
| `String` | `string` | `===` は Rust の等価と一致する。`.length` と `[i]` は UTF-16 の単位で、Rust のバイト長・バイト添字ではない。今のサブセットは、その演算を生成しない |
| newtype | 中身の値にブランドを交差した型 | 実行時の値は中身そのもの。ブランドは JSON を通ると消える。中身が `pub` なら構築は `Meters.of`。非 `pub` なら閉じた型で、クレートの公開関数から得る（§4.7） |
| 非公開フィールドを持つ struct | ブランド付きの `Readonly` オブジェクト（§4.7） | `of` はない。`Email.parse` のように、Rust の公開関数を呼んで作る |
| `Vec<T>` | `ReadonlyArray<T>` | 添字と `len` は読める。追加、削除、`map` / `filter` は生成しない。実行時に freeze はしない |

### 4.4 所有は型の上だけで消える

Rust では `step` が `state` を値で受け取るので、呼び出し後に元の束縛は使えない。生成 TS は引数を変異しないので、呼び出し後も元のオブジェクトは呼び出し前の状態のまま残る。前の状態を保持できる。

`Readonly` を外してオブジェクトを書き換えると、同じオブジェクトを指す別名から書き換えが見える。生成物は `Object.freeze` しない。

### 4.5 ワイヤ形式はメモリ上の値と別である

生成物の enum は `kind` 内部タグである。serde の既定 JSON（外部タグ、unit バリアントは文字列）とは一致しない（[design/05](./05-type-sharing-scope.md) §2.2）。`JSON.parse` の結果を、そのまま関数の引数にはできない。`--schema` を付けたときだけ、公開した struct と enum のワイヤ用スキーマが `src/purecrate-wire.ts` に出る。読み取りだけであり、ドメイン値から JSON を書く側はまだない（[design/08](./08-limits-and-roadmap.md) §5.3）。

`#[serde(...)]` は拒否する。フィールド名は Rust の名前のまま出る。

### 4.6 パッケージの読み方

生成パッケージは npm のパッケージとして配る。ソースは `src/*.ts` で、`.ts` 拡張子の import を使う。`npm run build`（`npm pack` と `npm publish` の前に `prepack` で走る）が、TypeScript 6 か 7 で `dist` に JavaScript と宣言を出す。`.ts` の import は `rewriteRelativeImportExtensions` で `.js` に書き換わる。`exports` は `dist` を指すので、消費側は TypeScript のローダーなしで node から読め、tsc は `nodenext` でも `bundler` でも読める。`--schema` を付けたときは、ワイヤ用スキーマを `<package>/wire` から読む。`version` は crate の `Cargo.toml` の version である（2026-09-29）。

ランタイム `purecrate` と、スキーマのアダプタ `purecrate-zod` などは `peerDependencies` である。ブランド型（`I32` など）は `purecrate` の `unique symbol` で区別されるので、生成パッケージが二つあっても、ランタイムは一つでなければ値を受け渡せない。ランタイムとアダプタも同じ形で `dist` を持つ。このリポジトリの中では、ビルドせずに条件 `purecrate-source`（node の `--conditions`、tsc の `customConditions`）でソースを読む。

生成パッケージを pack して別のプロジェクトに入れ、node で実行し、TypeScript 6・7 の tsc で `nodenext` と `bundler` の両方の型検査を通すことを検査している（`crates/cli/tests/package.rs`）。`purecrate` はまだ npm に公開していない。

予約名は `Result`、`Int`、`Str`、数値ブランド（`I32` など）、`assertNever`、`Readonly`、`ReadonlyArray`、`globalThis`、ファイル幹 `index` / `result` / `assert-never` / `int` / `str`。判別子のフィールド名 `kind` と、コンパニオンの `of`。ドメインの型にこれらの名前は使えない。生成コードは `Math`・`Number`・`Error`・`BigInt` を `globalThis.Error` のように読むので、ドメインの `Error` 型は使える。フィールド名・バリアント名・メソッド名の `__proto__` は、オブジェクトリテラルでプロトタイプの設定になるので拒否する。バリアントのない enum は TS のユニオンにもワイヤ形式にもならないので拒否する。

### 4.7 閉じた型は公開関数から作る（2026-09-29）

<!-- constrained-by ./04-objective-means-demand.md#16-検証の共有と公開コンストラクタ -->

Rust で非 `pub` のフィールドを一つでも持つ struct は、閉じた型になる。閉じた型について、呼び出し側が守ることは次のとおりである。

- コンパニオンに `of` はない。値は、Rust の公開関数が返したものを使う。`pub fn parse(raw: String) -> Result<Email, EmailError>` は `Email.parse` になる。
- 型にはブランドが付く。オブジェクトリテラルや生の `string` は、そのままでは閉じた型にならない。`as Email` で型を付けた値は、同値性の約束の外である（design/04 §1.3.1）。
- フィールドは今までどおり読める。書き換えはできない。
- ワイヤから読んだ閉じた型の値は、Rust の `Deserialize` と同じく形だけを検査したものである。不変条件までは検査していない（design/05 §7.7）。

消費側から書けないことは、`@ts-expect-error` を付けた消費側のファイルを TypeScript 6 と 7 で検査して確かめている（`crates/cli/tests/closed_equivalence.rs`）。

Rust を書く側から見ると、フィールドを `pub` にするかどうかが、TS で `of` を許すかどうかを決める。不変条件を持つ型は、フィールドを非 `pub` にし、検査つきの公開関数を書く。

## 5. この評価の後にやること

能力の穴だけを塞ぐ。既存コードの受理率を上げる許可リストは作らない。足す順序は [design/08](./08-limits-and-roadmap.md) にある。

`usize` は `Vec` の添字と `len` のために入っている。`Box<T>`、`Arc<T>`、`Mutex<T>` は `T` に消して受理し、生成物に注意コメントを残す。

10 進小数型は入れない（2026-09-27）。`number` に `Decimal` という型を被せても、計算は 2 進浮動小数のままである。`rust_decimal` のような外部クレートも受理しない。金額は、Rust 側で最小単位の整数 newtype として書く。

```rust
pub struct Yen(i64);
```

生成 TS の実行時の値は `bigint` で、型の上ではその newtype のブランドが付く。端数の丸めは、この型のメソッドとして整数演算で書く。中身が非 `pub` なので閉じた型になる（§4.7）。TS から作れるようにするには、[examples/order](../examples/order/src/lib.rs) の `Yen::new` のような検査つきの公開関数を書く。
