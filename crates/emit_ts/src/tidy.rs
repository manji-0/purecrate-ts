//! Small rewrites of printed TS that keep its meaning: the parentheses
//! around a whole condition, `return` value, or initializer, and `!(a === b)`
//! as `a !== b`. Both read the text with strings and templates skipped, so
//! a parenthesis or operator inside a literal is never taken for code.

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
                b'(' | b'[' | b'{' => {
                    out.push(Some(depth));
                    depth += 1;
                }
                b')' | b']' | b'}' => {
                    depth = depth.saturating_sub(1);
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
    fn negates_one_top_level_comparison() {
        assert_eq!(negate("(k.kind === \"C\")").as_deref(), Some("k.kind !== \"C\""));
        assert_eq!(negate("x !== null").as_deref(), Some("x === null"));
        assert_eq!(negate("a === b || c"), None);
        assert_eq!(negate("f(a === b)"), None);
        assert_eq!(negate("s === \" === \""), Some("s !== \" === \"".into()));
        assert_eq!(negate("a === b === c"), None);
    }
}
