declare const I8Brand: unique symbol;
export type I8 = number & { readonly [I8Brand]: true };
declare const I16Brand: unique symbol;
export type I16 = number & { readonly [I16Brand]: true };
declare const I32Brand: unique symbol;
export type I32 = number & { readonly [I32Brand]: true };
declare const I64Brand: unique symbol;
export type I64 = bigint & { readonly [I64Brand]: true };
declare const U8Brand: unique symbol;
export type U8 = number & { readonly [U8Brand]: true };
declare const U16Brand: unique symbol;
export type U16 = number & { readonly [U16Brand]: true };
declare const U32Brand: unique symbol;
export type U32 = number & { readonly [U32Brand]: true };
declare const U64Brand: unique symbol;
export type U64 = bigint & { readonly [U64Brand]: true };
declare const UsizeBrand: unique symbol;
export type Usize = number & { readonly [UsizeBrand]: true };
declare const F32Brand: unique symbol;
export type F32 = number & { readonly [F32Brand]: true };
declare const F64Brand: unique symbol;
export type F64 = number & { readonly [F64Brand]: true };

const panic = (what: string): never => {
  throw new Error(`attempt to ${what}`);
};

const small = <T extends number>(min: number, max: number) => {
  const fit = (n: number, what: string): T =>
    (n < min || n > max ? panic(`${what} with overflow`) : n + 0) as T;
  const of = (value: number): T => {
    if (!Number.isInteger(value)) panic("convert a non-integer");
    return fit(value, "convert");
  };
  return {
    of,
    add: (a: T, b: T): T => fit(a + b, "add"),
    sub: (a: T, b: T): T => fit(a - b, "subtract"),
    mul: (a: T, b: T): T => fit(a * b, "multiply"),
    div: (a: T, b: T): T =>
      b === 0 ? panic("divide by zero") : fit(Math.trunc(a / b), "divide"),
    rem: (a: T, b: T): T =>
      b === 0
        ? panic("calculate the remainder with a divisor of zero")
        : ((fit(Math.trunc(a / b), "calculate the remainder"), (a % b) + 0) as T),
    neg: (a: T): T => fit(-a, "negate"),
  } as const;
};

const big = <T extends bigint>(min: bigint, max: bigint) => {
  const fit = (n: bigint, what: string): T =>
    (n < min || n > max ? panic(`${what} with overflow`) : n) as T;
  const n = (x: T): bigint => x as bigint;
  return {
    of: (value: bigint): T => fit(value, "convert"),
    add: (a: T, b: T): T => fit(n(a) + n(b), "add"),
    sub: (a: T, b: T): T => fit(n(a) - n(b), "subtract"),
    mul: (a: T, b: T): T => fit(n(a) * n(b), "multiply"),
    div: (a: T, b: T): T =>
      n(b) === 0n ? panic("divide by zero") : fit(n(a) / n(b), "divide"),
    rem: (a: T, b: T): T =>
      n(b) === 0n
        ? panic("calculate the remainder with a divisor of zero")
        : ((fit(n(a) / n(b), "calculate the remainder"), n(a) % n(b)) as unknown as T),
    neg: (a: T): T => fit(-n(a), "negate"),
  } as const;
};

const INTEGER_LITERAL = /^-?(?:0|[1-9]\d*)$/;

/**
 * JSON text read as `JSON.parse` reads it, except that an integer literal
 * outside ±(2^53−1) becomes a `bigint` with its exact value. serde_json writes
 * `i64` and `u64` as JSON numbers; `JSON.parse` would round them.
 *
 * Needs a runtime that passes the literal's source text to the reviver
 * (Node 21+). Elsewhere the number stays rounded, and the `i64`/`u64`
 * schemas reject it instead of reading a wrong value.
 */
export const parseJson = (text: string): unknown =>
  JSON.parse(text, (_key: string, value: unknown, context?: { source?: string }) =>
    typeof value === "number" &&
    !Number.isSafeInteger(value) &&
    context?.source !== undefined &&
    INTEGER_LITERAL.test(context.source)
      ? BigInt(context.source)
      : value,
  );

/**
 * `str` operations whose result depends on the encoding (design/04 §1.5).
 * Rust counts and indexes a string in UTF-8 bytes; JS in UTF-16 units. The
 * string must be well-formed: a lone surrogate is not a Rust `String`, and
 * its bytes here are not specified.
 */
export const Str = {
  /** `str::as_bytes`: the UTF-8 bytes. */
  bytes: (s: string): ReadonlyArray<U8> => {
    const out: number[] = [];
    for (const c of s) {
      const p = c.codePointAt(0) as number;
      if (p < 0x80) out.push(p);
      else if (p < 0x800) out.push(0xc0 | (p >> 6), 0x80 | (p & 0x3f));
      else if (p < 0x10000) out.push(0xe0 | (p >> 12), 0x80 | ((p >> 6) & 0x3f), 0x80 | (p & 0x3f));
      else out.push(0xf0 | (p >> 18), 0x80 | ((p >> 12) & 0x3f), 0x80 | ((p >> 6) & 0x3f), 0x80 | (p & 0x3f));
    }
    return out as unknown as ReadonlyArray<U8>;
  },
} as const;

/** Integer and float widths. Domain packages and schema adapters share these brands. */
export const Int = {
  i8: small<I8>(-128, 127),
  i16: small<I16>(-32768, 32767),
  i32: small<I32>(-2147483648, 2147483647),
  u8: small<U8>(0, 255),
  u16: small<U16>(0, 65535),
  u32: small<U32>(0, 4294967295),
  usize: small<Usize>(0, 9007199254740991),
  i64: big<I64>(-9223372036854775808n, 9223372036854775807n),
  u64: big<U64>(0n, 18446744073709551615n),
  f32: {
    of: (value: number): F32 => Math.fround(value) as F32,
  },
  f64: {
    of: (value: number): F64 => value as F64,
  },
} as const;
