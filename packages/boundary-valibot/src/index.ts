import * as v from "valibot";
import { Char, Int } from "purecrate";

const small = <T>(min: number, max: number, of: (n: number) => T) =>
  v.pipe(v.number(), v.integer(), v.minValue(min), v.maxValue(max), v.transform(of));

export const i32 = small(-2147483648, 2147483647, Int.i32.of);
export const i8 = small(-128, 127, Int.i8.of);
export const i16 = small(-32768, 32767, Int.i16.of);
export const u8 = small(0, 255, Int.u8.of);
export const u16 = small(0, 65535, Int.u16.of);
export const u32 = small(0, 4294967295, Int.u32.of);
export const usize = small(0, 9007199254740991, Int.usize.of);

/**
 * A JSON number that is a safe integer, a bigint (`parseJson` reads larger
 * literals as one), or decimal text. A number past 2^53 was already rounded
 * by `JSON.parse`, so it is rejected rather than read as a wrong value.
 */
const big = <T>(pattern: RegExp, min: bigint, max: bigint, of: (n: bigint) => T) =>
  v.pipe(
    v.union([v.bigint(), v.pipe(v.number(), v.safeInteger()), v.pipe(v.string(), v.regex(pattern))]),
    v.transform((value) => BigInt(value)),
    v.minValue(min),
    v.maxValue(max),
    v.transform(of),
  );

export const i64 = big(/^-?(?:0|[1-9]\d*)$/, -9223372036854775808n, 9223372036854775807n, Int.i64.of);
export const u64 = big(/^(?:0|[1-9]\d*)$/, 0n, 18446744073709551615n, Int.u64.of);

export const f32 = v.pipe(v.number(), v.transform(Int.f32.of));
export const f64 = v.pipe(v.number(), v.transform(Int.f64.of));
export const str = v.string();
/** serde reads a `char` from a string of exactly one scalar value. */
export const char = v.pipe(v.string(), v.check(Char.is, "a single character"), v.transform((s) => s as Char));
export const bool = v.boolean();
export const unit = v.pipe(v.null(), v.transform(() => undefined));
export const nullable = <T extends v.GenericSchema>(inner: T) => v.union([inner, v.null()]);
