import { type, type ArkErrors, type Out, type Traversal, type Type } from "arktype";
import { Char, Int, JsonFloat, Uuid, adjacentTag, fromSequence, internalTag, structIssue, type UuidError } from "purecrate";

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
/** serde_json reads `-0` into an `i128` (only) as 0. */
export const i128 = type("unknown").pipe((value, ctx) => {
  const v = value instanceof JsonFloat && value.minusZero ? 0 : value;
  if (typeof v !== "bigint" && typeof v !== "number") return ctx.error("an integer");
  if (typeof v === "number" && !Number.isSafeInteger(v)) return ctx.error("a safe integer");
  const n = BigInt(v);
  const [min, max] = [-170141183460469231731687303715884105728n, 170141183460469231731687303715884105727n];
  if (n < min || n > max) return ctx.error(`between ${min} and ${max}`);
  return Int.i128.of(n);
});
export const u128 = big(0n, 340282366920938463463374607431768211455n, Int.u128.of);
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

/**
 * serde's `BTreeMap<String, V>`: a JSON object read into a `Map`. Every own
 * key is a key, `__proto__` and `constructor` too (the libraries' records
 * drop some), and a key with a lone surrogate is refused as serde refuses
 * it; `JSON.parse` has kept a repeated key's last value, as serde does.
 */
export const btreeMap = <T extends Type<any>>(inner: T): Wire<ReadonlyMap<string, T["infer"]>> =>
  type("unknown").pipe((x, ctx): ReadonlyMap<string, T["infer"]> => {
    if (typeof x !== "object" || x === null || Array.isArray(x)) return ctx.error("a JSON object") as never;
    const m = new Map<string, T["infer"]>();
    for (const k of Object.keys(x)) {
      if (LONE_SURROGATE.test(k)) return ctx.error(`key ${JSON.stringify(k)} without a lone surrogate`) as never;
      const parsed = inner((x as Readonly<Record<string, unknown>>)[k]);
      if (parsed instanceof type.errors) return fail(ctx, parsed);
      m.set(k, parsed);
    }
    return m;
  });
/** serde's `BTreeSet<String>`: a JSON array of strings, read into a `Set` (a repeated one kept once). */
export const btreeSet: Wire<ReadonlySet<string>> = type("unknown").pipe((x, ctx): ReadonlySet<string> => {
  const parsed = str.array()(x);
  return parsed instanceof type.errors ? fail(ctx, parsed) : new Set(parsed);
});

/**
 * The value an integer map key's text stands for, as serde_json 1.0.145
 * reads it: JSON's integer grammar only (no `+`, leading zero, space,
 * fraction, or exponent), a `number` where it is safe and a `bigint` past
 * that, and `-0` as the float the `i128` schema alone reads (as 0).
 * `undefined` for any other text.
 */
const INTEGER_KEY = /^-?(?:0|[1-9]\d*)$/;
const integerKey = (k: string): number | bigint | JsonFloat | undefined => {
  if (!INTEGER_KEY.test(k)) return undefined;
  if (k === "-0") return new JsonFloat(-0, true);
  const n = BigInt(k);
  return Number.isSafeInteger(Number(n)) ? Number(n) : n;
};

/** A map key of an integer type (or a newtype of one): its text read as `integerKey` reads it, then by `inner`, which checks the range. */
export const intKey = <T extends Type<any>>(inner: T) =>
  type("string").pipe((k, ctx): T["infer"] => {
    const parsed = inner(integerKey(k));
    return parsed instanceof type.errors ? (ctx.error("an integer key in range") as never) : parsed;
  });

/** A map key of `bool` (or a newtype of one): exactly `true` or `false`, as serde_json reads one. */
export const boolKey = <T extends Type<any>>(inner: T) =>
  type("string").pipe((k, ctx): T["infer"] => {
    const parsed = inner(k === "true" ? true : k === "false" ? false : undefined);
    return parsed instanceof type.errors ? (ctx.error("`true` or `false`") as never) : parsed;
  });

/**
 * serde's `BTreeMap<K, V>` for a key other than `String` (design/04 §3.3):
 * a JSON object, each own key read from its text by `key` (`intKey`,
 * `boolKey`, or the schema of a `char`, a `Uuid`, or a newtype of a
 * `String`). Two texts of one key (a `Uuid` in two cases) keep the later
 * value, as serde's map does.
 */
export const btreeMapOf = <K extends Type<any>, T extends Type<any>>(
  key: K,
  inner: T,
): Wire<ReadonlyMap<K["infer"], T["infer"]>> =>
  type("unknown").pipe((x, ctx): ReadonlyMap<K["infer"], T["infer"]> => {
    if (typeof x !== "object" || x === null || Array.isArray(x)) return ctx.error("a JSON object") as never;
    const m = new Map<K["infer"], T["infer"]>();
    for (const k of Object.keys(x)) {
      const parsedKey = key(k);
      if (parsedKey instanceof type.errors) return fail(ctx, parsedKey);
      const parsed = inner((x as Readonly<Record<string, unknown>>)[k]);
      if (parsed instanceof type.errors) return fail(ctx, parsed);
      m.set(parsedKey, parsed);
    }
    return m;
  });
/** serde's `BTreeSet<K>` for a key other than `String`: a JSON array of `key`'s values, read into a `Set` (a repeated one kept once). */
export const btreeSetOf = <K extends Type<any>>(key: K): Wire<ReadonlySet<K["infer"]>> =>
  type("unknown").pipe((x, ctx): ReadonlySet<K["infer"]> => {
    const parsed = key.array()(x);
    return parsed instanceof type.errors ? fail(ctx, parsed) : new Set(parsed as ReadonlyArray<K["infer"]>);
  });
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

const own = <T>(table: Readonly<Record<string, T>>, name: string): T | undefined => (Object.hasOwn(table, name) ? table[name] : undefined);

/** A variant's schema, built on first use like every generated one (`memo`). */
type Arm = () => (v: unknown) => unknown;

/**
 * An internally tagged enum (`#[serde(tag)]`) named `enumName`: `units`
 * maps the JSON name of each unit variant to its `kind`, `arms` each other
 * variant's to the schema that reads it from the object (or the rest of the
 * sequence form).
 */
export const internallyTagged = <T>(enumName: string, tag: string, units: Readonly<Record<string, string>>, arms: Readonly<Record<string, Arm>>): Wire<T> =>
  // arktype cannot follow a type parameter through its morph's output.
  type("unknown").pipe((v, ctx): unknown => {
    const found = internalTag(v, tag);
    if (typeof found === "string") return ctx.error(`${enumName}: ${found}`) as never;
    const unit = own(units, found.name);
    if (unit !== undefined) {
      return Array.isArray(found.rest) && found.rest.length > 0 ? (ctx.error("a unit variant") as never) : { kind: unit };
    }
    const arm = own(arms, found.name);
    if (arm === undefined) return ctx.error(`unknown variant \`${found.name}\``) as never;
    const parsed = arm()(found.rest);
    return parsed instanceof type.errors ? fail(ctx, parsed) : parsed;
  }) as unknown as Wire<T>;

/**
 * An adjacently tagged enum (`#[serde(tag, content)]`) named `enumName`:
 * `units` as for `internallyTagged`, `arms` each other variant's schema of
 * its content. The content of a struct variant (`maps`) is an object, never
 * the sequence form; one left out reads as `null` for a variant in
 * `optional` (it holds an `Option`), and is refused for the others.
 */
export const adjacentlyTagged = <T>(
  enumName: string,
  tag: string,
  content: string,
  units: Readonly<Record<string, string>>,
  arms: Readonly<Record<string, Arm>>,
  maps: readonly string[] = [],
  optional: readonly string[] = [],
): Wire<T> =>
  type("unknown").pipe((v, ctx): unknown => {
    const found = adjacentTag(v, tag, content);
    if (typeof found === "string") return ctx.error(`${enumName}: ${found}`) as never;
    const unit = own(units, found.name);
    if (unit !== undefined) {
      return found.content === undefined || found.content === null ? { kind: unit } : (ctx.error("a unit variant") as never);
    }
    const arm = own(arms, found.name);
    if (arm === undefined) return ctx.error(`unknown variant \`${found.name}\``) as never;
    if (!found.present && !optional.includes(found.name)) return ctx.error(`missing field \`${content}\``) as never;
    if (maps.includes(found.name) && Array.isArray(found.content)) return ctx.error("an object") as never;
    const parsed = arm()(found.present ? found.content : null);
    return parsed instanceof type.errors ? fail(ctx, parsed) : parsed;
  }) as unknown as Wire<T>;
