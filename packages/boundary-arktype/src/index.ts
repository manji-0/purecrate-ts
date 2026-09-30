import { type, type ArkErrors, type Out, type Traversal, type Type } from "arktype";
import { Char, Int, Uuid, type UuidError } from "purecrate";

/**
 * A schema that reads unknown JSON into the domain value `T`. Generated
 * schemas are annotated with it so recursive and mutually recursive types do
 * not make TypeScript infer `any`.
 */
export type Wire<T> = Type<(In: unknown) => Out<T>>;

/**
 * Hands the errors of a nested read to the traversal a morph runs in: they
 * keep their location, prefixed with the morph's path, and the morph's own
 * result is dropped. A fresh `ctx.error` instead would replace a field's
 * error with one at the parent.
 */
export const fail = (ctx: Traversal, errors: ArkErrors): never => {
  ctx.errors.merge(errors);
  return undefined as never;
};

/** Whether `v` is an object with its own key `name`: serde's wrapper of variant `name`. */
export const keyed = (v: unknown, name: string): boolean =>
  typeof v === "object" && v !== null && !Array.isArray(v) && Object.hasOwn(v, name);

/** Builds a definition on first use, so it may refer to schemas declared later. */
export const memo = <T>(make: () => T): (() => T) => {
  let made: { value: T } | undefined;
  return () => (made ??= { value: make() }).value;
};

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

/**
 * A JSON number that is a safe integer, a bigint (`parseJson` reads larger
 * literals as one), or decimal text. A number past 2^53 was already rounded
 * by `JSON.parse`, so it is rejected rather than read as a wrong value.
 */
const big = <T>(pattern: RegExp, min: bigint, max: bigint, of: (n: bigint) => T) =>
  type("bigint | number | string").pipe((value, ctx) => {
    if (typeof value === "number" && !Number.isSafeInteger(value)) return ctx.error("a safe integer");
    if (typeof value === "string" && !pattern.test(value)) return ctx.error("an integer string");
    const n = BigInt(value);
    if (n < min || n > max) return ctx.error(`between ${min} and ${max}`);
    return of(n);
  });

export const i64 = big(/^-?(?:0|[1-9]\d*)$/, -9223372036854775808n, 9223372036854775807n, Int.i64.of);
export const u64 = big(/^(?:0|[1-9]\d*)$/, 0n, 18446744073709551615n, Int.u64.of);
export const f32 = type("number").pipe(Int.f32.of);
export const f64 = type("number").pipe(Int.f64.of);
export const str = type("string");
/** serde reads a `char` from a string of exactly one scalar value. */
export const char = type("string").pipe((s, ctx): Char => (Char.is(s) ? s : (ctx.error("a single character") as never)));
/** serde reads a `Uuid` from any string `Uuid::parse_str` accepts; the value is its canonical form. */
export const uuid = type("string").pipe((s, ctx): Uuid => {
  const r = Uuid.parseStr(s);
  return r.kind === "Ok" ? r.value : (ctx.error("a UUID") as never);
});
/** `uuid::Error` has no serde form. */
export const uuidError = type("never").pipe((x): UuidError => x);
export const bool = type("boolean");
export const unit = type("null").pipe(() => undefined);
/** JSON `null` is `None`. A missing struct field is handled by the generated schema. */
export const nullable = <T extends Type<any>>(inner: T): Wire<T["infer"] | null> =>
  type("unknown").pipe((v, ctx): T["infer"] | null => {
    if (v === null) return null;
    const parsed = inner(v);
    return parsed instanceof type.errors ? fail(ctx, parsed) : parsed;
  }) as unknown as Wire<T["infer"] | null>;
