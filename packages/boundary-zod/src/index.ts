import { z } from "zod";
import { Int, type I64, type F32, type F64 } from "purecrate";

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

/** Decimal text or bigint. A JSON number is not accepted: it loses precision past 2^53. */
export const i64: Out<I64, bigint | string> = z
  .union([z.bigint(), z.string().regex(/^-?(?:0|[1-9]\d*)$/)])
  .transform((v) => Int.i64.of(typeof v === "bigint" ? v : BigInt(v))) as unknown as Out<I64, bigint | string>;

export const u64 = z
  .union([z.bigint(), z.string().regex(/^(?:0|[1-9]\d*)$/)])
  .transform((v) => Int.u64.of(typeof v === "bigint" ? v : BigInt(v)));

export const f32: Out<F32, number> = z.number().transform(Int.f32.of) as unknown as Out<F32, number>;
export const f64: Out<F64, number> = z.number().transform(Int.f64.of) as unknown as Out<F64, number>;

export const str = z.string();
export const bool = z.boolean();

/** serde writes `()` as JSON `null`. The domain value is `undefined`. */
export const unit = z.null().transform(() => undefined);

/** serde writes `None` as `null`. A missing field is not `None` unless the Rust type says so. */
export const nullable = <T extends z.ZodTypeAny>(inner: T) => z.union([inner, z.null()]);
