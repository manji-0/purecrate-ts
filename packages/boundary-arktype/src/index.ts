import { type } from "arktype";
import { Int } from "purecrate";

const small = <T>(min: number, max: number, of: (n: number) => T) =>
  type("number.integer")
    .narrow((n, ctx) => (n >= min && n <= max ? true : ctx.mustBe(`between ${min} and ${max}`)))
    .pipe(of);

export const i32 = small(-2147483648, 2147483647, Int.i32.of);
export const i8 = small(-128, 127, Int.i8.of);
export const i16 = small(-32768, 32767, Int.i16.of);
export const u8 = small(0, 255, Int.u8.of);
export const u16 = small(0, 65535, Int.u16.of);
export const u32 = small(0, 4294967295, Int.u32.of);
export const usize = small(0, 9007199254740991, Int.usize.of);

const intText = (pattern: string, of: (n: bigint) => unknown) =>
  type(`bigint | string`).pipe((value, ctx) => {
    if (typeof value === "bigint") return of(value);
    if (!new RegExp(pattern).test(value)) return ctx.error("an integer string");
    return of(BigInt(value));
  });

export const i64 = intText("^-?(?:0|[1-9]\\d*)$", Int.i64.of);
export const u64 = intText("^(?:0|[1-9]\\d*)$", Int.u64.of);
export const f32 = type("number").pipe(Int.f32.of);
export const f64 = type("number").pipe(Int.f64.of);
export const str = type("string");
export const bool = type("boolean");
export const unit = type("null").pipe(() => undefined);
/** JSON `null` is `None`. A missing struct field is handled by the generated schema. */
export const nullable = <T>(inner: { or(other: "null"): T }): T => inner.or("null");
