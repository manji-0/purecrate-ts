//! Small rewrites of printed TS that keep its meaning: the parentheses
//! around a whole condition, `return` value, or initializer, `!(a === b)` as
//! `a !== b`, parentheses by operator precedence, and long lines broken at
//! a comma-separated bracket, an `if (cond) return`, a long `&&` / `||` /
//! `?:`, or an arrow body. Each reads the text with strings, templates, and
//! comments skipped, so a bracket, comma, or operator inside one is never
//! taken for code.

mod arrows;
mod brackets;
mod heads;
mod operators;
mod scan;

use arrows::*;
use brackets::*;
use heads::*;
pub(crate) use operators::*;
pub(crate) use scan::*;

/// `src` with each line longer than `width` broken: a comma-separated
/// bracket, a semicolon-separated type literal, `if (cond) return …`, a
/// long `&&` / `||` / `?:`, or an arrow body. Comment lines stay as they are.
pub(crate) fn wrap(src: &str, width: usize) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        wrap_line(&comments_before_groups(line), width, &mut out);
    }
    out
}

/// `(/* '?' */ 63 as U8)` as `/* '?' */ (63 as U8)`: oxfmt moves a comment
/// that opens a parenthesized expression in front of the parenthesis. A
/// call's or a statement's parenthesis keeps it (`f(/* 'a' */ 97)`).
fn comments_before_groups(line: &str) -> std::borrow::Cow<'_, str> {
    if !line.contains("(/*") || is_comment(line) {
        return std::borrow::Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("(/*") {
        let before = &rest[..at];
        let group = !before.ends_with(|c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | ')' | ']' | '>'))
            && !["if ", "while ", "for "].iter().any(|k| before.trim_end().ends_with(k.trim_end()));
        let after = &rest[at + 1..];
        match (group, after.find("*/")) {
            (true, Some(end)) if !after[..end].contains('\n') => {
                let comment = &after[..end + 2];
                out.push_str(before);
                out.push_str(comment);
                out.push_str(" (");
                rest = after[end + 2..].trim_start();
            }
            _ => {
                out.push_str(&rest[..at + 1]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

fn emit_raw(line: &str, out: &mut String) {
    out.push_str(line);
    out.push('\n');
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')
}

/// `import { a as b } from "m";` with one name, which oxfmt keeps on its
/// line however long.
fn lone_import(line: &str) -> bool {
    line.starts_with("import ")
        && line
            .split_once('{')
            .and_then(|(_, rest)| rest.split_once('}'))
            .is_some_and(|(names, _)| !names.contains(','))
}

fn wrap_line(line: &str, width: usize, out: &mut String) {
    if !is_comment(line) && wrap_composition(line, width, out) {
        return;
    }
    let short = cols(line) <= width || is_comment(line) || lone_import(line);
    if short || !WRAPS.iter().any(|wrap| wrap(line, width, out)) {
        emit_raw(line, out);
    }
}

/// The ways to break a long line, tried in order. Each writes nothing
/// where it does not apply.
const WRAPS: &[fn(&str, usize, &mut String) -> bool] = &[
    wrap_if_return,
    wrap_if_open,
    wrap_for,
    wrap_condition,
    wrap_arrow_first,
    wrap_assign_ternary,
    wrap_assign_logical,
    wrap_return_logical,
    wrap_branch_operator,
    wrap_paren_group,
    // A top-level `?:` splits before a bracket in one of its branches
    // opens, and before the `&&` / `||` in its operands, which bind tighter.
    wrap_ternary,
    wrap_after_assign,
    wrap_last_arrow,
    wrap_sole_item,
    |line, width, out| wrap_bracket(line, width, true, out),
    |line, width, out| wrap_bracket(line, width, false, out),
    wrap_type_fields,
    wrap_fat_group,
    wrap_arrow,
    wrap_logical,
    wrap_outer_parens,
    wrap_top_commas,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_a_pair_around_everything() {
        assert_eq!(strip_outer("((a === b))"), "a === b");
        assert_eq!(strip_outer("(a) || (b)"), "(a) || (b)");
        assert_eq!(strip_outer("(\")\" + x)"), "\")\" + x");
        assert_eq!(strip_outer("f(x)"), "f(x)");
    }

    #[test]
    fn a_comment_opening_a_group_goes_before_it_as_oxfmt_moves_it() {
        assert_eq!(
            wrap("  x === (/* '?' */ 63 as U8 as number as U32) || y;", 100),
            "  x === /* '?' */ (63 as U8 as number as U32) || y;\n"
        );
        assert_eq!(wrap("  f(/* 'a' */ 97);", 100), "  f(/* 'a' */ 97);\n");
        assert_eq!(wrap("  if (/* 'a' */ 97 === x) {", 100), "  if (/* 'a' */ 97 === x) {\n");
    }

    #[test]
    fn an_array_of_numbers_fills_its_lines_as_oxfmt_lays_it_out() {
        let line = "  return [/* 'A' */ 65, /* 'E' */ 69, /* 'I' */ 73, -1, 2n, 300].includes(x);";
        assert_eq!(
            wrap(line, 40),
            "  return [\n    /* 'A' */ 65, /* 'E' */ 69,\n    /* 'I' */ 73, -1, 2n, 300,\n  ].includes(x);\n"
        );
        // A cast is no number literal: one item per line.
        assert!(wrap("  return [65 as U8, 69 as U8, 73 as U8, 77 as U8].includes(x);", 30).contains("    65 as U8,\n"));
    }

    #[test]
    fn a_long_logical_return_is_parenthesized_as_oxfmt_lays_it_out() {
        let line = "  return aaaaaaaaaaaa && bbbbbbbbbbbbbbbbbb && cccccccccc(s);";
        assert_eq!(wrap(line, 60), "  return (\n    aaaaaaaaaaaa && bbbbbbbbbbbbbbbbbb && cccccccccc(s)\n  );\n");
        let line = "  return aaaaaaaaaaaa(s) || (bbbbbbbbbbbb(s) && ccccccccc(s)) || dddddd(s);";
        assert_eq!(
            wrap(line, 40),
            "  return (\n    aaaaaaaaaaaa(s) ||\n    (bbbbbbbbbbbb(s) && ccccccccc(s)) ||\n    dddddd(s)\n  );\n"
        );
        let line = "  return aaaaaaaaaaaaaaaa(s) ?? bbbbbbbbbbbbbbbbbbbbbb(s) ?? ccc(s);";
        assert_eq!(
            wrap(line, 40),
            "  return (\n    aaaaaaaaaaaaaaaa(s) ??\n    bbbbbbbbbbbbbbbbbbbbbb(s) ??\n    ccc(s)\n  );\n"
        );
        // A `?:` or a call holding the operators is laid out as before.
        assert!(!wrap("  return aaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbb && c : dddddddddd;", 30).contains("return ("));
        assert!(!wrap("  return fffffffff(aaaaaaaaaaaa && bbbbbbbbbbbbbbbbbb);", 30).contains("return ("));
    }

    #[test]
    fn wraps_the_outermost_bracket_with_commas() {
        let line = "  return Result.ok({ kind: \"AwaitingConsent\", request, auth: f(a, b), note: \"x, y\" });";
        assert_eq!(
            wrap(line, 40),
            "  return Result.ok({\n    kind: \"AwaitingConsent\",\n    request,\n    auth: f(a, b),\n    note: \"x, y\",\n  });\n"
        );
        let sig = "export const f = (a: A, b: /* x, y */ B): R => {";
        assert_eq!(wrap(sig, 30), "export const f = (\n  a: A,\n  b: /* x, y */ B,\n): R => {\n");
        assert_eq!(wrap("short(a, b);", 40), "short(a, b);\n");
        let header = "for (let i = (0 as Usize), $e = (n as Usize); i < $e; i = (i + 1) as Usize) {";
        let wrapped = wrap(header, 40);
        assert!(wrapped.lines().all(|l| l.len() <= 40), "{wrapped}");
        assert!(wrapped.contains("for (\n"), "{wrapped}");
        let small = "if (!is_upper(Slice.at(b, i)) && !is_digit(Slice.at(b, i))) return Result.err(e);";
        assert_eq!(
            wrap(small, 60),
            "if (!is_upper(Slice.at(b, i)) && !is_digit(Slice.at(b, i)))\n  return Result.err(e);\n"
        );
        assert!(!wrap(small, 60).contains("Slice.at(\n"), "small calls stay closed");
        assert_eq!(
            wrap("f(r: Result<A, B>, n: Map<K, V<W>>, b: boolean): Result<A, B> => r;", 30),
            "f(\n  r: Result<A, B>,\n  n: Map<K, V<W>>,\n  b: boolean,\n): Result<A, B> => r;\n"
        );
        assert_eq!(wrap("// a, very, long, comment, line, here", 10), "// a, very, long, comment, line, here\n");
    }

    #[test]
    fn line_comment_is_not_code() {
        // An apostrophe or a lone `(` in a comment opens nothing.
        let d = depths("// don't (\nf(x)");
        assert_eq!(d[9], None);
        assert_eq!(d[10..], [Some(0), Some(0), Some(0), Some(1), Some(0)]);
    }

    #[test]
    fn wraps_if_return_and_logical_and_ternary() {
        let line = "          if (amountToCapture !== null && (amountToCapture < (1n as I64) || amountToCapture > capturable)) return Result.err({ kind: \"InvalidCaptureAmount\", capturable });";
        let out = wrap(line, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.contains("if ("), "{out}");
        assert!(out.contains("return Result.err("), "{out}");
        let ident = "export const isIdentChar = (c: U8): boolean => c >= (48 as U8) && c <= (57 as U8) || c >= (65 as U8) && c <= (90 as U8) || c === (45 as U8);";
        let out = wrap(ident, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.contains("=>\n"), "{out}");
        let tern = "  return reusable !== null ? Result.err({ kind: \"ConsentRequired\" }) : Result.ok({ kind: \"AwaitingPassword\", request, failures: (0 as U32), notice: { kind: \"Clear\" } });";
        let out = wrap(tern, 80);
        assert!(out.contains("?\n") || out.contains("? "), "{out}");
        assert!(out.lines().all(|l| l.len() <= 80), "{out}");
    }

    #[test]
    fn splits_a_ternary_before_the_and_in_its_else() {
        let line = "      const matches: boolean = pkce.method.kind === \"Plain\" ? verifier === pkce.challenge : verifierS256 !== null && verifierS256 === pkce.challenge;";
        assert_eq!(
            wrap(line, 100),
            "      const matches: boolean =\n        pkce.method.kind === \"Plain\"\n          ? verifier === pkce.challenge\n          : verifierS256 !== null && verifierS256 === pkce.challenge;\n"
        );
    }

    #[test]
    fn a_ternary_splits_before_its_branches_open() {
        let line = "          return !apart ? divide(Int.i64.mul(line.amount, 100n as I64), Int.i64.add(100n as I64, percent(rate)), conversion) : (0n as I64);";
        assert_eq!(
            wrap(line, 100),
            "          return !apart\n            ? divide(\n                Int.i64.mul(line.amount, 100n as I64),\n                Int.i64.add(100n as I64, percent(rate)),\n                conversion,\n              )\n            : (0n as I64);\n"
        );
        // A `?:` in the middle operand: its `:` is not the outer one.
        let nested = "  return reusable !== null ? request.prompt.no_interaction && needsConsent ? Result.err(e) : Result.ok(x) : Result.ok(y);";
        assert_eq!(
            wrap(nested, 80),
            "  return reusable !== null\n    ? request.prompt.no_interaction && needsConsent\n      ? Result.err(e)\n      : Result.ok(x)\n    : Result.ok(y);\n"
        );
    }

    #[test]
    fn breaks_a_condition_at_its_or_before_a_call() {
        let line = "        if (n === 0 || n > 63 || Slice.at(b, start) === 45 || Slice.at(b, Int.usize.sub(i, 1 as Usize)) === 45) return Result.err({ kind: \"BadDomain\" });";
        assert_eq!(
            wrap(line, 100),
            "        if (\n          n === 0 ||\n          n > 63 ||\n          Slice.at(b, start) === 45 ||\n          Slice.at(b, Int.usize.sub(i, 1 as Usize)) === 45\n        )\n          return Result.err({ kind: \"BadDomain\" });\n"
        );
    }

    #[test]
    fn an_arrow_keeps_its_parameters_and_opens_or_moves_its_body() {
        let ctor = "  RequiresCapture: (method: PaymentMethod, capturable: I64): Status => ({ kind: \"RequiresCapture\", method, capturable }),";
        assert_eq!(
            wrap(ctor, 100),
            "  RequiresCapture: (method: PaymentMethod, capturable: I64): Status => ({\n    kind: \"RequiresCapture\",\n    method,\n    capturable,\n  }),\n"
        );
        let call = "export const by = (n: U8, m: U8, c: Char, d: Char): Ordering => Ord.then(Ord.cmp(n, m), Ord.cmpStr(c, d));";
        assert_eq!(
            wrap(call, 100),
            "export const by = (n: U8, m: U8, c: Char, d: Char): Ordering =>\n  Ord.then(Ord.cmp(n, m), Ord.cmpStr(c, d));\n"
        );
        let generic = "  new: (code: string): Result<Sku, OrderError> => (code.length === 0) ? Result.err({ kind: \"EmptySku\" }) : Result.ok(x),";
        assert!(wrap(generic, 100).starts_with("  new: (code: string): Result<Sku, OrderError> =>\n"));
    }

    #[test]
    fn an_object_heading_a_longer_body_does_not_hug() {
        let line = "export const digitCount = (d: OtpDigits): Usize => ({ Six: 6, Seven: 7, Eight: 8 } satisfies Record<OtpDigits[\"kind\"], number>)[d.kind] as Usize;";
        assert!(wrap(line, 100).starts_with("export const digitCount = (d: OtpDigits): Usize =>\n  ({ Six"));
        let block =
            "export const validateRequestWithAVeryLongName = (params: AuthorizationParams, client: Client): R => {";
        assert!(!wrap(block, 100).contains("=>\n"), "{}", wrap(block, 100));
    }

    #[test]
    fn wraps_long_import_and_closed_ctor() {
        let imp =
            "import { type Char, type F64, type I32, type U64, type U8, type Usize } from \"./purecrate-runtime.ts\";";
        let out = wrap(imp, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.contains("import {\n"), "{out}");
        let ctor = "export const AuthorizationRequest$of = (fields: Readonly<{ client_id: string; redirect_uri: string; scope: string; state: string; nonce: string | null; pkce: Pkce | null; prompt: Prompt; max_age: I64 | null; wants_mfa: boolean }>): AuthorizationRequest => fields as AuthorizationRequest;";
        let out = wrap(ctor, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
    }

    #[test]
    fn a_group_broken_inside_takes_no_trailing_comma() {
        let line = "export const f = (s: string, t: string, xs: ReadonlyArray<I32>, a: I32): I32 => (Str.slice(t, 3 as Usize, 3 as Usize).includes(\"a\") ? Iter.sum(Iter.map(xs, (x: I32): I32 => Int.i32.mul(x, -1 as I32)), Int.i32.add, 0 as I32) : 2 < 5 ? (s.length === 0 ? a : (1 as I32)) : Iter.sum(xs, Int.i32.add, 0 as I32));";
        let out = wrap(line, 100);
        assert!(!out.contains("I32),\n  );"), "{out}");
    }

    #[test]
    fn a_group_after_a_keyword_takes_no_trailing_comma() {
        // `(x,)` is a syntax error; with two items, the comma operator.
        assert!(is_call("f") && is_call("Int.i32.add") && is_call("g(a)") && is_call("xs[0]"));
        assert!(is_call("notif") && is_call("format"));
        for before in ["return", "  return", "a !==", "if", "while", "typeof", "throw", "=", ""] {
            assert!(!is_call(before), "{before}");
        }
        let line = "  return (aVeryLongLocalNameForTheValue !== 2147483647) !== (somethingElseEntirely && anotherThingToTest);";
        let out = wrap(line, 60);
        assert!(!out.contains(",\n"), "{out}");
    }

    #[test]
    fn a_cast_states_its_type() {
        assert_eq!(cast_type("512 as Usize"), Some("Usize"));
        assert_eq!(cast_type("Int.u8.and(x, 15 as U8) as number as Usize"), Some("Usize"));
        assert_eq!(cast_type("(b.length as Usize)"), Some("Usize"));
        assert_eq!(cast_type("a + b as I32"), None);
        assert_eq!(cast_type("c ? (1 as I32) : (2 as I32)"), None);
        assert_eq!(cast_type("f(1 as I32)"), None);
    }
}
