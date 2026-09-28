import * as v from "valibot";
import { Int } from "purecrate";

const small = <T>(min: number, max: number, of: (n: number) => T) =>
  v.pipe(v.number(), v.integer(), v.minValue(min), v.maxValue(max), v.transform(of));

export const i32 = small(-2147483648, 2147483647, Int.i32.of);
export const i8 = small(-128, 127, Int.i8.of);
export const i16 = small(-32768, 32767, Int.i16.of);
export const u8 = small(0, 255, Int.u8.of);
export const u16 = small(0, 65535, Int.u16.of);
export const u32 = small(0, 4294967295, Int.u32.of);
export const usize = small(0, 9007199254740991, Int.usize.of);

const intText = <T>(pattern: RegExp, of: (n: bigint) => T) =>
  v.pipe(
    v.union([v.bigint(), v.pipe(v.string(), v.regex(pattern))]),
    v.transform((value) => of(typeof value === "bigint" ? value : BigInt(value))),
  );

export const i64 = intText(/^-?(?:0|[1-9]\d*)$/, Int.i64.of);
export const u64 = intText(/^(?:0|[1-9]\d*)$/, Int.u64.of);

export const f32 = v.pipe(v.number(), v.transform(Int.f32.of));
export const f64 = v.pipe(v.number(), v.transform(Int.f64.of));
export const str = v.string();
export const bool = v.boolean();
export const unit = v.pipe(v.null(), v.transform(() => undefined));
export const nullable = <T extends v.GenericSchema>(inner: T) => v.union([inner, v.null()]);
