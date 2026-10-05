import { type, type ArkErrors, type Out, type Traversal, type Type } from "arktype";
import { Char, Int, JsonFloat, Uuid, fromSequence, structIssue, type UuidError } from "purecrate";

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

/**
 * A fieldless enum named `enumName`: each of `names` as serde writes a unit
 * variant, the string `"V"` or `{"V": null}`, into `{ kind: V }`. The
 * variants are tried in turn; an object keyed by one reports that one's
 * errors, anything else one error for the enum.
 */
export const unitEnum = <K extends string>(enumName: string, names: readonly [K, ...K[]]): Wire<Readonly<{ kind: K }>> => {
  // The key is `name` at run time; TS sees one fixed key, as a computed
  // key would give it an index signature that arktype reads differently.
  const arms = names.map((name) => [name, memo(() => type({ "+": "reject", [name]: "null" } as { "+": "reject"; V: "null" }))] as const);
  return type("unknown").pipe((v, ctx): Readonly<{ kind: K }> => {
    for (const [name, arm] of arms) {
      if (v === name) return { kind: name };
      const issue = keyed(v, name) ? structIssue(v) : undefined;
      if (issue !== undefined) return ctx.error(issue) as never;
      const parsed = arm()(v);
      if (!(parsed instanceof type.errors)) return { kind: name };
      if (keyed(v, name)) return fail(ctx, parsed);
    }
    return ctx.error(enumName) as never;
  });
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
 * A JSON number that is a safe integer, or a bigint (`parseJson` reads larger
 * literals as one). A number past 2^53 was already rounded by `JSON.parse`,
 * so it is rejected rather than read as a wrong value; so is a string, and a
 * float (`50.0`, which `parseJson` reads as a `JsonFloat`), as serde does.
 */
const big = <T>(min: bigint, max: bigint, of: (n: bigint) => T) =>
  type("bigint | number").pipe((value, ctx) => {
    if (typeof value === "number" && !Number.isSafeInteger(value)) return ctx.error("a safe integer");
    const n = BigInt(value);
    if (n < min || n > max) return ctx.error(`between ${min} and ${max}`);
    return of(n);
  });

export const i64 = big(-9223372036854775808n, 9223372036854775807n, Int.i64.of);
export const u64 = big(0n, 18446744073709551615n, Int.u64.of);
/** A JSON number, or one `parseJson` read as a `JsonFloat` (`2.0`). */
const float = type("number").or(type.instanceOf(JsonFloat).pipe((x) => x.value));
export const f32 = float.pipe(Int.f32.of);
export const f64 = float.pipe(Int.f64.of);
/**
 * serde's struct: after `sequence`, a JSON object, not an array. arktype's
 * object shapes also take an array and read every field as missing.
 */
export const record = (x: object, ctx: Traversal): boolean => !Array.isArray(x) || ctx.mustBe("an object");

/**
 * Before a struct's shape with `fields`: the sequence form, an array of the
 * fields in order, as the object serde reads it to; and a field the JSON
 * holds twice, or a key with a lone surrogate, refused as serde refuses it
 * (`structIssue`). Without `fields`, an enum variant's one-key wrapper, where
 * any key twice is refused.
 */
export const sequence =
  (fields?: readonly string[]) =>
  (x: unknown, ctx: Traversal): unknown => {
    const issue = structIssue(x, fields);
    if (issue !== undefined) return ctx.error(issue);
    return fields === undefined ? x : fromSequence(x, fields);
  };
/** serde_json refuses a lone surrogate in a string, which no Rust `String` holds. */
const LONE_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;
export const str = type("string").narrow((s, ctx) => !LONE_SURROGATE.test(s) || ctx.mustBe("a string without a lone surrogate"));
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
