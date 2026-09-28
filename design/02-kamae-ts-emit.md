# 生成 TS は kamae-ts スタイル

日付: 2026-09-27
参照: [iwasa-kosui/kamae-ts](https://github.com/iwasa-kosui/kamae-ts)

PureCrate の出力は、kamae-ts のドメイン層（Discriminated Union / 純粋遷移 / Companion / Result）に合わせる。Zod・Sensitive・ポート分割は生成範囲外。それらは変換後パッケージの利用側（境界）の仕事である。

## 1. 踏襲するもの

| kamae-ts | 生成規則 |
| --- | --- |
| 判別子は常に `kind` | `tag` / `type` / `status` は出さない |
| `type`。`interface` は使わない | declaration merging を避ける |
| `Readonly<{ ... }>` | フィールド再代入を型で止める |
| 型と関数を同名 Companion にまとめる | `export type T` + `export const T = { ... } as const` |
| 1概念1ファイル | `event.ts` / `state.ts` / `step.ts`。barrel は `index.ts` のみ |
| 関数プロパティ記法 | `apply: (s, e) => r`。`apply(s, e)` メソッド記法は出さない |
| 純粋遷移 | 入力型が始状態、戻り値が終状態。無効遷移は型で拒否できる形を優先 |
| 想定失敗は Result | `{ kind: "Ok"; value } \| { kind: "Err"; error }` |
| エラーも `kind` ユニオン | クレートの error enum をそのまま |
| `assertNever` | `switch` の default |
| class / メソッド記法を避ける | `impl` は Companion の関数プロパティへ |

## 1.1 状態と列（2026-09-28）

kamae の状態は、段階ごとの `Readonly` な値である。遷移は次の状態を返す。起きたことは状態に積まず、別の値として呼び出し側が扱う。永続化が状態とイベントを同時に書くのは、生成パッケージの外である。

要素が増減する列は、状態の中の可変配列にしない。再帰 enum で新しい列を返す。これは kamae の `[...lines, line]` や `filter` に当たる。

```rust
pub enum Lines {
    Empty,
    Cons(Line, Box<Lines>),
}

pub fn cons(line: Line, lines: Lines) -> Lines {
    Lines::Cons(line, Box::new(lines))
}
```

`Vec<T>` は、呼び出し側が長さを決めた列を、添字と `len` で読む型である。遷移の中で `Vec` を伸ばす、要素を抜く、要素を置き換える操作は v0 に入れない。

## 2. 生成しないもの

kamae-ts のうち、閉じた純粋クレートを越えるもの。

- Zod / Valibot / ArkType（外部入力の境界）
- `Sensitive<T>`（PII。クレートにその型が無い）
- neverthrow / fp-ts への依存（v0。入出力型をクレート内で閉じる）
- repository / use case / ポート
- 時刻や ID の生成。遷移が必要なら引数として受け取る（kamae-ts の `now: Date` と同じ）

v1 で Result ライブラリを選ぶなら、生成オプションで neverthrow に差し替えてよい。既定は自前の `result.ts`。

## 3. 組み込み Result

`src/result.ts`:

```ts
export type Result<T, E> =
  | Readonly<{ kind: "Ok"; value: T }>
  | Readonly<{ kind: "Err"; error: E }>;

export const Result = {
  ok: <T, E>(value: T): Result<T, E> => ({ kind: "Ok", value }),
  err: <T, E>(error: E): Result<T, E> => ({ kind: "Err", error }),
  isOk: <T, E>(r: Result<T, E>): r is Readonly<{ kind: "Ok"; value: T }> =>
    r.kind === "Ok",
  isErr: <T, E>(r: Result<T, E>): r is Readonly<{ kind: "Err"; error: E }> =>
    r.kind === "Err",
} as const;
```

`?` は次に写す。

```ts
if (r.kind === "Err") return r;
const value = r.value;
```

## 4. ファイル配置（カウンタ）

```
src/
  assert-never.ts
  event.ts
  state.ts
  step.ts
  index.ts
```

`event.ts`:

```ts
export type Event =
  | Readonly<{ kind: "Inc" }>
  | Readonly<{ kind: "Dec" }>
  | Readonly<{ kind: "Reset" }>;

export const Event = {
  Inc: (): Event => ({ kind: "Inc" }),
  Dec: (): Event => ({ kind: "Dec" }),
  Reset: (): Event => ({ kind: "Reset" }),
} as const;
```

`state.ts`:

```ts
export type State = Readonly<{
  n: number;
}>;

export const State = {
  of: (n: number): State => ({ n }),
} as const;
```

`step.ts`:

```ts
import { assertNever } from "./assert-never.ts";
import type { Event } from "./event.ts";
import type { State } from "./state.ts";

export const step = (state: State, event: Event): State => {
  switch (event.kind) {
    case "Inc":
      return { n: state.n + 1 };
    case "Dec":
      return { n: state.n - 1 };
    case "Reset":
      return { n: 0 };
    default:
      return assertNever(event);
  }
};
```

`assert-never.ts`:

```ts
export const assertNever = (x: never): never => {
  throw new Error("unexpected variant");
};
```

`index.ts` は再エクスポートだけ。

自由関数は `export const name = (...) =>` にする。`export function` は使わない（Companion / 関数プロパティと表記を揃える）。

## 5. 部分ユニオン

Rust の到達可能な始状態がバリアントの一部なら、生成側で部分ユニオンを出してよい。

```ts
export type Cancellable = Waiting | EnRoute | InTrip;
```

v0 は明示 `type` 別名があるときだけ出す。推論での自動部分ユニオンは v1。

## 6. impl の写し方

```rust
impl State {
    pub fn bump(self) -> State { State { n: self.n + 1 } }
}
```

```ts
export const State = {
  of: (n: number): State => ({ n }),
  bump: (state: State): State => ({ n: state.n + 1 }),
} as const;
```

レシーバは第一引数。`this` は出さない。共有参照（`&self`・`&T`・`&str`・`&[T]`）は値と同じに写す。生成 TS は値を変異させない。`Cell` と `RefCell` は拒否し、`Mutex<T>` は `T` に消すので、参照と値を区別する必要がない。`&mut` は拒否する。

`Self` は impl の型名に置き換えてから読む。`Type::method(x)` は Companion の関数プロパティ呼び出し `Type.method(x)` になる。レシーバ構文 `x.method(y)` は、型検査でレシーバの型 `T` を求め、`T.method(x, y)` にする。解決先はクレート自身の固有 impl である。std のメソッドは、`Vec` の添字と `len` を除いて拒否する。

## 6.1 newtype

1 要素のタプル構造体は、ブランド付きの中身の型にする。実行時の値は中身そのもので、serde の JSON 表現（newtype は中身として直列化される）とも一致する。

```rust
pub struct Meters(i32);
impl Meters {
    pub fn plus(&self, other: &Meters) -> Self { Self(self.0 + other.0) }
}
```

```ts
declare const MetersBrand: unique symbol;
export type Meters = number & { readonly [MetersBrand]: true };

export const Meters = {
  of: (value: number): Meters => value as Meters,
  plus: (self: Meters, other: Meters): Meters => Meters.of(Int.i32.add(self, other)),
} as const;
```

`.0` は値そのものになる。ブランドの鍵を `unique symbol` にするのは、newtype の newtype で鍵が衝突しないようにするため。中身が `Option`・`()`・`!`（別名経由を含む）の newtype は拒否する。`null & { ... }` は `never` になり、ブランドを付けられない。2 要素以上のタプル構造体は v0 の範囲外。

## 6.2 クロージャ

関数本体の中で `let` に束縛して呼ぶクロージャは、型付きのアロー関数にする。

```rust
pub fn scaled(x: i32) -> i32 {
    let k: i32 = 3;
    let scale = |v: i32| v * k;
    scale(x)
}
```

```ts
export const scaled = (x: number): number => {
  const k: number = 3;
  const scale: ((_0: number) => number) = ((v: number): number => Int.i32.mul(v, k));
  return scale(x);
};
```

- **捕捉は不変の束縛だけ。** Rust のクロージャは捕捉した値を持つが、JS のクロージャは変数そのものを見るので、後の再代入が見えてしまう。`let mut` を読む・書くクロージャは拒否する（`let` で今の値を束縛し直せば受理される）。
- **引数の型は注釈か期待型から決まる。** 決まらなければ `|v: T|` を求める。戻り値の型は本体から推論する。
- **`?` と `return` はクロージャから抜ける。** そのため戻り値の型の注釈（`|..| -> T { .. }`）を必須にする。`?` の持ち上げはクロージャ本体ごとに行い、外側の関数の文へ漏らさない。
- **ローカルはアイテムを隠す。** `let inc = |v| ..` の後の `inc(x)` はクロージャを呼ぶ。TS の `const` はブロック全体で同名の import を隠す（宣言より前の使用も含めて）ので、アイテム名と同じローカルは `inc$1` のように改名して出す。
- 関数の引数・戻り値・フィールドとしてのクロージャ（`impl Fn`・`fn` 型）は v0 の範囲外。std のメソッドに渡すクロージャは、そのメソッドの許可（TODO 32〜34）で受理される。

## 7. テストデータ

生成物のテストを書くなら kamae-ts どおり `as const satisfies Type` でリテラルを狭める。変換器本体の話ではないが、examples の期待値はこの形にする。

```ts
const ev = { kind: "Inc" } as const satisfies Event;
```
