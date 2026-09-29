import { z } from "zod";
import { Char, Int, type F32, type F64 } from "purecrate";

type Out<T, In> = z.ZodType<T, z.ZodTypeDef, In>;

const small = <T>(min: number, max: number, of: (n: number) => T): Out<T, number> =>
  z.number().int().gte(min).lte(max).transform(of) as unknown as Out<T, number>;

/** JSON number checked as `i32`, branded as the shared `I32`. */
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
const big = <T>(pattern: RegExp, min: bigint, max: bigint, of: (n: bigint) => T): Out<T, bigint | number | string> =>
  z
    .union([
      z.bigint(),
      z.number().refine(Number.isSafeInteger, "a safe integer; read larger integers with parseJson"),
      z.string().regex(pattern),
    ])
    .transform((v) => BigInt(v))
    .refine((n) => n >= min && n <= max, `between ${min} and ${max}`)
    .transform(of) as unknown as Out<T, bigint | number | string>;

export const i64 = big(/^-?(?:0|[1-9]\d*)$/, -9223372036854775808n, 9223372036854775807n, Int.i64.of);
export const u64 = big(/^(?:0|[1-9]\d*)$/, 0n, 18446744073709551615n, Int.u64.of);

export const f32: Out<F32, number> = z.number().transform(Int.f32.of) as unknown as Out<F32, number>;
export const f64: Out<F64, number> = z.number().transform(Int.f64.of) as unknown as Out<F64, number>;

export const str = z.string();
/** serde reads a `char` from a string of exactly one scalar value. */
export const char: Out<Char, string> = z.string().refine(Char.is, "a single character") as unknown as Out<Char, string>;
export const bool = z.boolean();

/** serde writes `()` as JSON `null`. The domain value is `undefined`. */
export const unit = z.null().transform(() => undefined);

/** serde writes `None` as `null`. A missing field is not `None` unless the Rust type says so. */
export const nullable = <T extends z.ZodTypeAny>(inner: T) => z.union([inner, z.null()]);
