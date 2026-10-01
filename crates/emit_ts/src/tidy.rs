//! Small rewrites of printed TS that keep its meaning: the parentheses
//! around a whole condition, `return` value, or initializer, `!(a === b)` as
//! `a !== b`, and long lines broken inside their brackets. Each reads the
//! text with strings, templates, and `/* */` comments skipped, so a bracket,
//! comma, or operator inside one is never taken for code.

/// The byte ranges of `s` outside string and template literals, with the
/// parenthesis depth at each byte (`None` inside a literal).
fn depths(s: &str) -> Vec<Option<usize>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                out.push(None);
                if b == b'\\' && i + 1 < bytes.len() {
                    out.push(None);
                    i += 2;
                    continue;
                }
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' | b'`' => {
                    quote = Some(b);
                    out.push(None);
                }
                // A `/* .. */` comment reads as a literal.
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    let end = s[i + 2..].find("*/").map_or(bytes.len(), |e| i + 2 + e + 2);
                    out.extend(std::iter::repeat_n(None, end - i));
                    i = end;
                    continue;
                }
                b'(' | b'[' | b'{' => {
                    out.push(Some(depth));
                    depth += 1;
                }
                b')' | b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    out.push(Some(depth));
                }
                // Type arguments, `Result<A, B>`: the printer writes a
                // comparison with spaces around `<` and `>`, and `=>` is not
                // a bracket.
                b'<' if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || matches!(bytes[i - 1], b'_' | b'$')) => {
                    out.push(Some(depth));
                    depth += 1;
                }
                b'>' if i > 0 && !matches!(bytes[i - 1], b'=' | b' ') && depth > 0 => {
                    depth -= 1;
                    out.push(Some(depth));
                }
                _ => out.push(Some(depth)),
            },
        }
        i += 1;
    }
    out
}

/// `s` without the parentheses around all of it, as often as they wrap it.
/// A template literal's `${..}` counts as inside the literal, so it is left
/// alone; that only keeps a pair of parentheses that could have gone.
pub(crate) fn strip_outer(mut s: &str) -> &str {
    loop {
        let t = s.trim();
        if !(t.starts_with('(') && t.ends_with(')')) {
            return t;
        }
        let d = depths(t);
        // The first `(` closes at the last byte exactly when no byte in
        // between is back at depth 0.
        let closes_at_end = d[1..d.len() - 1].iter().all(|x| x.is_none_or(|x| x >= 1));
        if !closes_at_end {
            return t;
        }
        s = &t[1..t.len() - 1];
    }
}

/// `!(l === r)` as `l !== r` and the reverse, when `s` is one comparison
/// at its top level: `===` binds looser than every operator but `&&`, `||`,
/// `?:`, and assignment, so swapping it negates the whole.
pub(crate) fn negate(s: &str) -> Option<String> {
    let s = strip_outer(s);
    let d = depths(s);
    let bytes = s.as_bytes();
    let top = |i: usize| d[i] == Some(0);
    let mut found: Option<(usize, &str)> = None;
    let mut i = 0;
    while i < bytes.len() {
        if top(i) {
            let rest = &s[i..];
            for op in ["&&", "||", "?", "=>"] {
                if rest.starts_with(op) {
                    return None;
                }
            }
            for (op, flip) in [(" === ", " !== "), (" !== ", " === ")] {
                if rest.starts_with(op) {
                    if found.is_some() {
                        return None;
                    }
                    found = Some((i, flip));
                    i += op.len();
                    continue;
                }
            }
        }
        i += 1;
    }
    let (at, flip) = found?;
    Some(format!("{}{flip}{}", &s[..at], &s[at + 5..]))
}

/// `src` with each line longer than `width` broken inside a bracket, the
/// way prettier breaks it: the outermost `(..)`, `[..]`, or `{..}` closed on
/// the line whose items a top-level comma separates is opened, one item per
/// line two spaces in, with a trailing comma; the lines that gives are
/// broken in turn. A line breaks only inside brackets, where a newline never
/// ends a statement. Comment lines are left as they are.
pub(crate) fn wrap(src: &str, width: usize) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        wrap_line(line, width, &mut out);
    }
    out
}

fn wrap_line(line: &str, width: usize, out: &mut String) {
    let trimmed = line.trim_start();
    let comment = trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*');
    let Some((open, close, items)) = (line.len() > width && !comment).then(|| split_point(line, line.len() - width)).flatten() else {
        out.push_str(line);
        out.push('\n');
        return;
    };
    let pad = &line[..line.len() - trimmed.len()];
    wrap_line(&line[..=open], width, out);
    for item in items {
        wrap_line(&format!("{pad}  {item},"), width, out);
    }
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
}

/// The brackets to open on `line` and the items between them: the first,
/// outermost pair closed on the line with a comma at its top level, and no
/// `;` there (a `for` header or a type literal), that holds more than the
/// `excess` past the width, so opening it shortens the line.
fn split_point(line: &str, excess: usize) -> Option<(usize, usize, Vec<String>)> {
    let d = depths(line);
    let bytes = line.as_bytes();
    let mut best: Option<(usize, usize, usize)> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if !matches!(b, b'(' | b'[' | b'{') || d[i].is_none() {
            continue;
        }
        let level = d[i].expect("checked");
        let Some(close) = (i + 1..bytes.len()).find(|&j| d[j] == Some(level) && matches!(bytes[j], b')' | b']' | b'}')) else {
            continue;
        };
        let top = |c: u8| (i + 1..close).any(|j| bytes[j] == c && d[j] == Some(level + 1));
        if top(b',') && !top(b';') && close - i > excess && best.is_none_or(|(_, _, l)| level < l) {
            best = Some((i, close, level));
        }
    }
    let (open, close, level) = best?;
    let mut items = Vec::new();
    let mut start = open + 1;
    for j in open + 1..close {
        if bytes[j] == b',' && d[j] == Some(level + 1) {
            items.push(line[start..j].trim().to_string());
            start = j + 1;
        }
    }
    let last = line[start..close].trim();
    if !last.is_empty() {
        items.push(last.to_string());
    }
    Some((open, close, items))
}

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
        assert_eq!(wrap(header, 40), format!("{header}\n"));
        let small = "if (!is_upper(Slice.at(b, i)) && !is_digit(Slice.at(b, i))) return Result.err(e);";
        assert_eq!(wrap(small, 60), format!("{small}\n"));
        assert_eq!(
            wrap("f(r: Result<A, B>, n: Map<K, V<W>>, b: boolean): Result<A, B> => r;", 30),
            "f(\n  r: Result<A, B>,\n  n: Map<K, V<W>>,\n  b: boolean,\n): Result<A, B> => r;\n"
        );
        assert_eq!(wrap("// a, very, long, comment, line, here", 10), "// a, very, long, comment, line, here\n");
    }

    #[test]
    fn negates_one_top_level_comparison() {
        assert_eq!(negate("(k.kind === \"C\")").as_deref(), Some("k.kind !== \"C\""));
        assert_eq!(negate("x !== null").as_deref(), Some("x === null"));
        assert_eq!(negate("a === b || c"), None);
        assert_eq!(negate("f(a === b)"), None);
        assert_eq!(negate("s === \" === \""), Some("s !== \" === \"".into()));
        assert_eq!(negate("a === b === c"), None);
    }
}
