import { z } from "zod";
import { Char, Int, JsonFloat, Uuid, fromSequence, structIssue, type F32, type F64, type I128, type UuidError } from "purecrate";

type Out<T, In> = z.ZodType<T, In>;

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
 * A JSON number that is a safe integer, or a bigint (`parseJson` reads larger
 * literals as one). A number past 2^53 was already rounded by `JSON.parse`,
 * so it is rejected rather than read as a wrong value; so is a string, and a
 * float (`50.0`, which `parseJson` reads as a `JsonFloat`), as serde does.
 */
const big = <T>(min: bigint, max: bigint, of: (n: bigint) => T): Out<T, bigint | number> =>
  z
    .union([z.bigint(), z.number().refine(Number.isSafeInteger, "a safe integer; read larger integers with parseJson")])
    .transform((v) => BigInt(v))
    .refine((n) => n >= min && n <= max, `between ${min} and ${max}`)
    .transform(of) as unknown as Out<T, bigint | number>;

export const i64 = big(-9223372036854775808n, 9223372036854775807n, Int.i64.of);
export const u64 = big(0n, 18446744073709551615n, Int.u64.of);
/** serde_json reads `-0` into an `i128` (only) as 0. */
export const i128 = z.preprocess(
  (v) => (v instanceof JsonFloat && v.minusZero ? 0 : v),
  big(-170141183460469231731687303715884105728n, 170141183460469231731687303715884105727n, Int.i128.of),
) as unknown as Out<I128, bigint | number>;
export const u128 = big(0n, 340282366920938463463374607431768211455n, Int.u128.of);

/** A JSON number, or one `parseJson` read as a `JsonFloat` (`2.0`). */
const float = z.union([z.number(), z.instanceof(JsonFloat).transform((x) => x.value)]);
export const f32: Out<F32, number> = float.transform(Int.f32.of) as unknown as Out<F32, number>;
export const f64: Out<F64, number> = float.transform(Int.f64.of) as unknown as Out<F64, number>;

/** serde_json refuses a lone surrogate in a string, which no Rust `String` holds. */
const LONE_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;
export const str = z.string().refine((s) => !LONE_SURROGATE.test(s), "a string without a lone surrogate");
/** serde reads a `char` from a string of exactly one scalar value. */
export const char: Out<Char, string> = z.string().refine(Char.is, "a single character") as unknown as Out<Char, string>;
/** serde reads a `Uuid` from any string `Uuid::parse_str` accepts; the value is its canonical form. */
export const uuid: Out<Uuid, string> = z.string().transform((s, ctx) => {
  const r = Uuid.parseStr(s);
  if (r.kind === "Ok") return r.value;
  ctx.addIssue({ code: "custom", message: "a UUID", input: s });
  return z.NEVER;
}) as unknown as Out<Uuid, string>;
/** `uuid::Error` has no serde form. */
export const uuidError: Out<UuidError, never> = z.never() as unknown as Out<UuidError, never>;
export const bool = z.boolean();

/** serde writes `()` as JSON `null`. The domain value is `undefined`. */
export const unit = z.null().transform(() => undefined);

/** serde writes `None` as `null`. A missing field is not `None` unless the Rust type says so. */
export const nullable = <T extends z.ZodType>(inner: T) => z.union([inner, z.null()]);

/**
 * serde's struct with `shape`: a JSON object, or the sequence form, an array
 * of the fields in order. A field the JSON holds twice, or a key with a lone
 * surrogate, is refused as serde refuses it (`structIssue`); an unknown key
 * is dropped.
 */
export const record = <S extends z.ZodRawShape>(shape: S) => {
  const fields = Object.keys(shape);
  return z.preprocess((x, ctx) => {
    const issue = structIssue(x, fields);
    if (issue !== undefined) ctx.addIssue({ code: "custom", message: issue, input: x });
    return fromSequence(x, fields);
  }, z.object(shape));
};

/** serde's one-key wrapper of an enum variant: any key twice is refused. */
export const variant = <S extends z.ZodRawShape>(shape: S) =>
  z.preprocess((x, ctx) => {
    const issue = structIssue(x);
    if (issue !== undefined) ctx.addIssue({ code: "custom", message: issue, input: x });
    return x;
  }, z.object(shape).strict());

/** serde's unit variant `V`: the string `"V"`, or `{"V": null}`. */
export const unitVariant = (name: string) => z.union([z.literal(name), variant({ [name]: z.null() })]);

/**
 * A fieldless enum: each of `names` read as `unitVariant` reads it, into
 * `{ kind: name }`. The arms are tried in order, as a union of the
 * variants one by one would be.
 */
export const unitEnum = <K extends string>(names: readonly [K, ...K[]]): z.ZodType<Readonly<{ kind: K }>, unknown> => {
  const arms = names.map((name) => unitVariant(name).transform((): Readonly<{ kind: K }> => ({ kind: name })));
  return (arms.length === 1 ? arms[0] : z.union(arms as unknown as [z.ZodType, z.ZodType])) as unknown as z.ZodType<Readonly<{ kind: K }>, unknown>;
};

/** A struct field of type `Option<T>`: missing or `null` is `None`, as serde reads it. */
export const optionalField = <T extends z.ZodType>(inner: T) =>
  nullable(inner)
    .optional()
    .transform((v) => v ?? null);
