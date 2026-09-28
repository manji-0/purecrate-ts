# 公開面・平坦化・版計画

日付: 2026-09-27
状態: 確定

## 1. 公開面

入力クレートの `pub` がそのままパッケージの export になる。

含めるもの:

- `pub struct` / `pub enum` / `pub type`
- `pub fn`（自由関数）
- `impl T { pub fn ... }`（レシーバ付きは第一引数に正規化）
- 上記から到達する非公開の型と関数（実装詳細として emit するが、TS の `export` は付けない）

含めないもの:

- `pub(crate)` / `pub(super)` / private で、公開面から到達しないもの
- `const` / `static`（v0。値の畳み込みが必要になるため後回し）
- トレイト定義そのもの

フィールド可視性: 非 `pub` フィールドも TS には出す。Rust のモジュール境界は消えるので、生成側で書き換え不能にする意味で `readonly` だけ付ける。モジュール非公開による情報隠蔽は v0 では再現しない。

<!-- constrained-by ./04-objective-means-demand.md#16-検証の共有と公開コンストラクタ -->

2026-09-29 改訂: 非公開フィールドを持つ struct の構築は、Rust と同じく閉じる。フィールドは今までどおり `readonly` で読める。ただし、型にブランドを付け、コンパニオンに `of` を出さない。クレートの外から値を得る手段は、公開関数だけになる。フィールドの読み取りまでは隠さない。

## 2. 平坦化

モジュールパスは生成名に使わない。

```
crate
  src/lib.rs        pub struct State
  src/event.rs      pub enum Event
  src/step.rs       pub fn step
```

はいずれも `State` / `Event` / `step` という平坦な名前になり、規則 6 に従って `state.ts` / `event.ts` / `step.ts` へ出る。

規則:

1. 定義されている名前（または `pub use` 先の名前）が公開名
2. 同じ公開名が二つあれば拒否。診断に両方のモジュールパスを書く
3. Rust のキーワードでも TS の予約語でもない名前だけ許可。衝突するなら拒否（リネームしない）
4. メソッド `impl State { pub fn apply }` は Companion `State.apply`（関数プロパティ）。自由関数 `apply` とは衝突しない
5. 名前としては、自由関数同士、型同士の衝突だけを見る
6. 各公開概念は kebab-case の単独ファイルへ出す（`State` → `state.ts`）。寄せ集めファイルは作らない。ファイル名が重なれば、型と関数でも拒否する（型 `Command` と関数 `command` はどちらも `command.ts`）

到達した非公開アイテムも平坦化する。非公開 `fn helper` が二つあれば、公開名衝突と同じく拒否する。自動プレフィックスは付けない。名前は入力クレート側で一意にすること。

## 3. ジェネリクス（決定）

v0 で許可する型コンストラクタは組み込み三つだけ。

- `Option<T>`
- `Result<T, E>`
- `Vec<T>`

ユーザーが書いた `struct Foo<T>` / `fn id<T>(x: T) -> T` は拒否する。

理由:

- 遷移関数の中核は閉じた ADT の `match` であり、まずそこを正しく写す
- ユーザー汎用型は生成側で単相化するか TS ジェネリクスにするかの分岐が要る
- v0 の検査（型閉包・網羅）を単純に保つ

v1 で入れるもの:

- 境界なしの型パラメータ（`T` だけ）
- TS 側もジェネリクスのまま出す（単相化しない）
- 境界・`where`・関連型は v1 でも拒否

## 4. Map（決定）

v0 では `HashMap` / `BTreeMap` を拒否する。

理由: Rust のキー等価は値、JS の `Map` のオブジェクトキーは同一性。`Record<string, V>` に落とせるのはキーが文字列のときに限る。中途半端に入れると遷移の意味が変わる。

v1: キーが `String` のときだけ `ReadonlyMap<string, V>`。挿入順は Rust `HashMap` と一致させない（順序依存の公開関数は拒否候補）。

## 5. WASM（決定）

v0 の成果物は TS ソースパッケージのみ。

IR は印刷先を知らないデータとする。後から `emit_wasm` を足せる余地は残す。v0 の完了条件に WASM を含めない。同じ IR を WASM に写すのは、TS 印刷がカウンタ例で安定したあとの任意作業。

## 6. Result は例外にしない

`?` は早期 `return { kind: "Err", error }` に写す。ドメイン関数は `throw` しない。遷移の失敗も値である。`assertNever` のみ、網羅が破れた予期しない故障として throw する（kamae-ts の「想定外は例外」）。

## 7. カウンタ例（受け入れ入力）

属性なし。モジュール分割しても平坦化後は同じ。

```rust
pub enum Event {
    Inc,
    Dec,
    Reset,
}

pub struct State {
    pub n: i32,
}

pub fn step(state: State, event: Event) -> State {
    match event {
        Event::Inc => State { n: state.n + 1 },
        Event::Dec => State { n: state.n - 1 },
        Event::Reset => State { n: 0 },
    }
}
```

期待する公開 TS は kamae-ts 形。詳細とファイル分割は `02-kamae-ts-emit.md`。
