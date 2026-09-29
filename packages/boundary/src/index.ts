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
declare const CharBrand: unique symbol;
/** A Rust `char`: a string of exactly one Unicode scalar value (no lone surrogate). */
export type Char = string & { readonly [CharBrand]: true };

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
 * `str` operations whose result depends on the encoding (design/01 §6).
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
  /** `str::len`: the number of UTF-8 bytes. */
  len: (s: string): Usize => {
    let n = 0;
    for (const c of s) {
      const p = c.codePointAt(0) as number;
      n += p < 0x80 ? 1 : p < 0x800 ? 2 : p < 0x10000 ? 3 : 4;
    }
    return n as Usize;
  },
} as const;

const code = (c: Char): number => c.codePointAt(0) as number;
const within = (c: Char, lo: number, hi: number): boolean => code(c) >= lo && code(c) <= hi;
const upper = (c: Char): boolean => within(c, 0x41, 0x5a);
const lower = (c: Char): boolean => within(c, 0x61, 0x7a);
const digit = (c: Char): boolean => within(c, 0x30, 0x39);
const radix = (r: U32): number =>
  r < 2 || r > 36 ? panicWith("to_digit: invalid radix -- radix must be in the range 2 to 36 inclusive") : r;
const panicWith = (message: string): never => {
  throw new Error(message);
};
const digitValue = (c: Char, r: U32): number | null => {
  const base = radix(r);
  const p = code(c);
  const d = p >= 0x30 && p <= 0x39 ? p - 0x30 : (p | 0x20) >= 0x61 && (p | 0x20) <= 0x7a ? (p | 0x20) - 0x61 + 10 : 99;
  return d < base ? d : null;
};

/**
 * `char` operations (design/01 §6). Ordering and ranges go through `code`:
 * JS orders strings by UTF-16 unit, which puts U+E000..=U+FFFF above the
 * supplementary planes. Only ASCII and code-point methods are here; the
 * Unicode-table ones (`is_alphabetic`, ...) are not.
 */
export const Char = {
  /** Checks `s` is one Unicode scalar value, as serde reads a `char`. */
  is: (s: string): s is Char => {
    const p = s.codePointAt(0);
    return p !== undefined && s.length === (p > 0xffff ? 2 : 1) && (p < 0xd800 || p > 0xdfff);
  },
  /** `u32::from(c)`: the code point. */
  code: (c: Char): U32 => code(c) as U32,
  /** `char::from(b)`: U+0000..=U+00FF. */
  fromU8: (b: U8): Char => String.fromCharCode(b) as Char,
  /** `char::from_u32(n)`: `None` for a surrogate or past U+10FFFF. */
  fromU32: (n: U32): Char | null =>
    (n >= 0xd800 && n <= 0xdfff) || n > 0x10ffff ? null : (String.fromCodePoint(n) as Char),
  isAscii: (c: Char): boolean => code(c) < 0x80,
  isAsciiAlphabetic: (c: Char): boolean => upper(c) || lower(c),
  isAsciiAlphanumeric: (c: Char): boolean => upper(c) || lower(c) || digit(c),
  isAsciiControl: (c: Char): boolean => code(c) < 0x20 || code(c) === 0x7f,
  isAsciiDigit: digit,
  isAsciiGraphic: (c: Char): boolean => within(c, 0x21, 0x7e),
  isAsciiHexdigit: (c: Char): boolean => digit(c) || within(c, 0x41, 0x46) || within(c, 0x61, 0x66),
  isAsciiLowercase: lower,
  isAsciiPunctuation: (c: Char): boolean =>
    within(c, 0x21, 0x2f) || within(c, 0x3a, 0x40) || within(c, 0x5b, 0x60) || within(c, 0x7b, 0x7e),
  isAsciiUppercase: upper,
  /** Space, tab, LF, FF, CR. Not VT (U+000B), unlike JS `\s`. */
  isAsciiWhitespace: (c: Char): boolean => [0x20, 0x09, 0x0a, 0x0c, 0x0d].includes(code(c)),
  toAsciiLowercase: (c: Char): Char => (upper(c) ? (String.fromCharCode(code(c) + 32) as Char) : c),
  toAsciiUppercase: (c: Char): Char => (lower(c) ? (String.fromCharCode(code(c) - 32) as Char) : c),
  eqIgnoreAsciiCase: (a: Char, b: Char): boolean => Char.toAsciiLowercase(a) === Char.toAsciiLowercase(b),
  /** `char::len_utf8`: 1 to 4. */
  lenUtf8: (c: Char): Usize => (code(c) < 0x80 ? 1 : code(c) < 0x800 ? 2 : code(c) < 0x10000 ? 3 : 4) as Usize,
  /** ASCII digits and letters only, as Rust; panics on a radix outside 2..=36. */
  isDigit: (c: Char, r: U32): boolean => digitValue(c, r) !== null,
  toDigit: (c: Char, r: U32): U32 | null => digitValue(c, r) as U32 | null,
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

/** Shortest digits and decimal exponent: `digits` × 10^(`point` − length). */
const decimal = (x: number): { digits: string; point: number } => {
  const [mantissa, exponent] = x.toExponential().split("e");
  return { digits: mantissa.replace("-", "").replace(".", ""), point: Number(exponent) + 1 };
};

/**
 * A finite float laid out as ryu writes it for serde_json: `1.0`, `0.001`,
 * `1e16`, `1.5e-7`. Plain notation holds for 10^(`low`) ≤ |x| < 10^`high`.
 */
const ryu = (x: number, digits: string, point: number, high: number, low: number): string => {
  const sign = x < 0 || Object.is(x, -0) ? "-" : "";
  if (x === 0) return `${sign}0.0`;
  const length = digits.length;
  if (point >= length && point <= high) return `${sign}${digits}${"0".repeat(point - length)}.0`;
  if (point > 0 && point <= high) return `${sign}${digits.slice(0, point)}.${digits.slice(point)}`;
  if (point > low && point <= 0) return `${sign}0.${"0".repeat(-point)}${digits}`;
  const rest = length === 1 ? "" : `.${digits.slice(1)}`;
  return `${sign}${digits[0]}${rest}e${point - 1}`;
};

/**
 * Writes domain values as serde_json writes the Rust value (design/04 §6):
 * integers exactly (`bigint` included), floats as ryu lays them out, and
 * non-finite floats as `null`, which serde_json then cannot read back.
 * Generated `toJson` encoders compose these.
 */
export const Json = {
  int: (n: number | bigint): string => String(n),
  bool: (b: boolean): string => (b ? "true" : "false"),
  /** JSON.stringify escapes as serde_json does for well-formed strings. */
  str: (s: string): string => JSON.stringify(s),
  f64: (x: number): string => {
    if (!Number.isFinite(x)) return "null";
    const { digits, point } = decimal(x);
    return ryu(x, digits, point, 16, -5);
  },
  /**
   * The shortest digits that read back as the same `f32`. When two are
   * equally near, ryu takes the even one; `toPrecision` rounds half up.
   */
  f32: (x: number): string => {
    if (!Number.isFinite(x)) return "null";
    // At most 9 digits are needed; 100 give the exact value of any `f32`
    // whose expansion could end in a tie.
    const [exact, exponent] = Math.abs(x).toExponential(99).split("e");
    const all = exact.replace(".", "");
    for (let p = 1; p <= 9; p++) {
      const shorter = Number(x.toPrecision(p));
      if (Math.fround(shorter) !== x) continue;
      let digits = decimal(shorter).digits;
      let point = decimal(shorter).point;
      const tie = all[p] === "5" && /^0*$/.test(all.slice(p + 1));
      if (tie && Number(all[p - 1]) % 2 === 0) {
        const down = Number(`${x < 0 ? "-" : ""}${all[0]}.${all.slice(1, p)}e${exponent}`);
        if (Math.fround(down) === x) ({ digits, point } = decimal(down));
      }
      return ryu(x, digits, point, 13, -6);
    }
    return "null";
  },
  array: <T>(xs: ReadonlyArray<T>, write: (x: T) => string): string => `[${xs.map((x) => write(x)).join(",")}]`,
} as const;
