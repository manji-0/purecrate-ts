import * as v from "valibot";
import { Char, Int, Uuid, type UuidError } from "purecrate";

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
/** serde reads a `Uuid` from any string `Uuid::parse_str` accepts; the value is its canonical form. */
export const uuid = v.pipe(
  v.string(),
  v.check((s) => Uuid.parseStr(s).kind === "Ok", "a UUID"),
  v.transform((s): Uuid => {
    const r = Uuid.parseStr(s);
    return r.kind === "Ok" ? r.value : (undefined as never);
  }),
);
/** `uuid::Error` has no serde form. */
export const uuidError = v.pipe(v.never(), v.transform((x): UuidError => x));
export const bool = v.boolean();
export const unit = v.pipe(v.null(), v.transform(() => undefined));
export const nullable = <T extends v.GenericSchema>(inner: T) => v.union([inner, v.null()]);

/** serde's unit variant `V`: the string `"V"`, or `{"V": null}`. */
export const unitVariant = (name: string) => v.union([v.literal(name), v.strictObject({ [name]: v.null() })]);

/**
 * A fieldless enum: each of `names` read as `unitVariant` reads it, into
 * `{ kind: name }`. The arms are tried in order, as a union of the
 * variants one by one would be.
 */
export const unitEnum = <K extends string>(names: readonly [K, ...K[]]): v.GenericSchema<unknown, Readonly<{ kind: K }>> =>
  v.union(names.map((name) => v.pipe(unitVariant(name), v.transform((): Readonly<{ kind: K }> => ({ kind: name })))));
