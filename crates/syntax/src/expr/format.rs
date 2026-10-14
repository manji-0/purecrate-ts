//! `format!`: the format string read into `FmtPiece`s, the arguments in
//! Rust's evaluation order (design/01 §6.8).

use purecrate_ir::{Callee, Expr, FmtAlign, FmtPiece, FmtSpec, Lit, Name, Reason};
use syn::Expr as SynExpr;

use super::lower_expr;
use crate::item::{Cx, ParseError};

/// Where a `{..}` takes its value from.
#[derive(Debug, PartialEq)]
enum Ref {
    Next,
    Index(usize),
    Named(String),
}

fn refuse(message: impl Into<String>) -> ParseError {
    ParseError::new(Reason::Macro, message).detail("format")
}

/// A format string and its arguments, each named if written `name = e`.
pub(super) type FormatArgs = (syn::LitStr, Vec<(Option<String>, SynExpr)>);

/// `"..", a, name = b` up to the end of `input`, a trailing comma allowed.
pub(super) fn parse_format_args(input: syn::parse::ParseStream) -> syn::Result<FormatArgs> {
    let template: syn::LitStr = input.parse()?;
    let mut args: Vec<(Option<String>, SynExpr)> = Vec::new();
    while !input.is_empty() {
        input.parse::<syn::Token![,]>()?;
        if input.is_empty() {
            break;
        }
        if input.peek(syn::Ident) && input.peek2(syn::Token![=]) && !input.peek2(syn::Token![==]) {
            let name: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            args.push((Some(name.to_string()), input.parse()?));
        } else {
            args.push((None, input.parse()?));
        }
    }
    Ok((template, args))
}

pub(super) fn lower_format(cx: &Cx, mac: &syn::Macro) -> Result<Expr, ParseError> {
    let (template, explicit) = mac
        .parse_body_with(parse_format_args)
        .map_err(|e| refuse(format!("`format!` expects a string literal and its arguments: {e}")))?;
    lower_template(cx, &template, explicit, "")
}

/// The `String` a format string and its arguments make, after `prefix`.
pub(super) fn lower_template(
    cx: &Cx,
    template: &syn::LitStr,
    explicit: Vec<(Option<String>, SynExpr)>,
    prefix: &str,
) -> Result<Expr, ParseError> {
    let parsed = parse_template(&template.value())?;

    // Explicit arguments first, as written; then each name captured inline,
    // in the order it first appears. Rust evaluates them in this order.
    let mut args: Vec<Expr> = Vec::new();
    let mut atoms: Vec<bool> = Vec::new();
    for (_, e) in &explicit {
        args.push(lower_expr(cx, e)?);
        atoms.push(is_atom(e));
    }
    let named = |n: &str| explicit.iter().position(|(name, _)| name.as_deref() == Some(n));
    let mut captured: Vec<String> = Vec::new();
    let mut next = 0;
    let mut pieces = vec![FmtPiece::Text(prefix.to_string())];
    for piece in parsed {
        match piece {
            Parsed::Text(t) => pieces.push(FmtPiece::Text(t)),
            Parsed::Arg(r, spec) => {
                let index = match r {
                    Ref::Next => {
                        next += 1;
                        next - 1
                    }
                    Ref::Index(i) => i,
                    Ref::Named(n) => match named(&n) {
                        Some(i) => i,
                        None => {
                            let at = match captured.iter().position(|c| *c == n) {
                                Some(i) => i,
                                None => {
                                    captured.push(n.clone());
                                    captured.len() - 1
                                }
                            };
                            explicit.len() + at
                        }
                    },
                };
                pieces.push(FmtPiece::Arg { index, spec });
            }
        }
    }
    for n in captured {
        args.push(Expr::Var(Name::new(n)));
        atoms.push(true);
    }
    if let Some(i) = pieces.iter().find_map(|p| match p {
        FmtPiece::Arg { index, .. } if *index >= args.len() => Some(*index),
        _ => None,
    }) {
        return Err(refuse(format!("`format!` has no argument {i}")));
    }

    // A template literal runs its `${..}` in the order the pieces place
    // them. A local, a literal, or a field read gives the same value in any
    // order; every other argument must come once, in the order written.
    let placed: Vec<usize> = pieces
        .iter()
        .filter_map(|p| match p {
            FmtPiece::Arg { index, .. } if !atoms[*index] => Some(*index),
            _ => None,
        })
        .collect();
    let in_order = placed.windows(2).all(|w| w[0] < w[1]);
    let each_once = (0..args.len()).filter(|i| !atoms[*i]).all(|i| placed.iter().filter(|p| **p == i).count() == 1);
    if !in_order || !each_once {
        return Err(refuse(
            "`format!` uses its arguments out of order or more than once: bind each with `let` first and name it in the string (`{x}`)",
        ));
    }
    if args.iter().zip(&atoms).any(|(a, atom)| !atom && writes(a)) {
        return Err(refuse(
            "a `format!` argument with a statement or an assignment is not in v0: run it before the `format!`",
        ));
    }

    if args.is_empty() {
        let text: String = pieces
            .into_iter()
            .map(|p| match p {
                FmtPiece::Text(t) => t,
                FmtPiece::Arg { .. } => unreachable!("an argument without arguments was refused"),
            })
            .collect();
        return Ok(Expr::Call { callee: Callee::StringFrom, args: vec![Expr::Lit(Lit::Str(text))] });
    }
    Ok(Expr::Call { callee: Callee::Format(merge_text(pieces)), args })
}

/// A read that nothing in the subset can change while the other arguments
/// run: a name, a literal, a field of one, or a reference to one.
fn is_atom(e: &SynExpr) -> bool {
    match e {
        SynExpr::Path(p) => p.qself.is_none() && p.path.get_ident().is_some(),
        SynExpr::Lit(_) => true,
        SynExpr::Field(f) => is_atom(&f.base),
        SynExpr::Reference(r) => r.mutability.is_none() && is_atom(&r.expr),
        SynExpr::Paren(p) => is_atom(&p.expr),
        _ => false,
    }
}

/// Whether an argument may write a local: an assignment, `v[i] = x`, or a
/// statement in a block, where `push` and the other writes stand before
/// `check` rewrites them. A captured name placed before it would read the
/// local before the write in TS, after it in Rust.
fn writes(e: &Expr) -> bool {
    e.any(|e| matches!(e, Expr::Assign { .. } | Expr::Seq { .. } | Expr::Call { callee: Callee::VecSet, .. }))
}

fn merge_text(pieces: Vec<FmtPiece>) -> Vec<FmtPiece> {
    let mut out: Vec<FmtPiece> = Vec::new();
    for p in pieces {
        match (out.last_mut(), p) {
            (_, FmtPiece::Text(t)) if t.is_empty() => {}
            (Some(FmtPiece::Text(a)), FmtPiece::Text(b)) => a.push_str(&b),
            (_, p) => out.push(p),
        }
    }
    out
}

#[derive(Debug, PartialEq)]
enum Parsed {
    Text(String),
    Arg(Ref, FmtSpec),
}

fn parse_template(s: &str) -> Result<Vec<Parsed>, ParseError> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '}' => return Err(refuse("`format!` has an unmatched `}`: write `}}`")),
            '{' => {
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => inner.push(c),
                        None => return Err(refuse("`format!` has an unmatched `{`: write `{{`")),
                    }
                }
                if !text.is_empty() {
                    out.push(Parsed::Text(std::mem::take(&mut text)));
                }
                let (arg, spec) = match inner.split_once(':') {
                    Some((a, s)) => (a, parse_spec(s)?),
                    None => (inner.as_str(), FmtSpec::default()),
                };
                let arg = arg.trim();
                let r = if arg.is_empty() {
                    Ref::Next
                } else if let Ok(i) = arg.parse::<usize>() {
                    Ref::Index(i)
                } else if arg.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    Ref::Named(arg.to_string())
                } else {
                    return Err(refuse(format!("`{{{inner}}}` is not in v0: name an argument or a local")));
                };
                out.push(Parsed::Arg(r, spec));
            }
            c => text.push(c),
        }
    }
    if !text.is_empty() {
        out.push(Parsed::Text(text));
    }
    Ok(out)
}

const SPECS: &str = "`{}`, a width (`{:5}`), an alignment with an optional fill (`{:<5}`, `{:*^5}`), or zero padding on an integer (`{:05}`)";

/// `[[fill]align]['0'][width]`. The sign flags, `#`, a precision, a width
/// from an argument (`$`), and every type (`?`, `x`, `e`, ..) are refused.
fn parse_spec(spec: &str) -> Result<FmtSpec, ParseError> {
    let cs: Vec<char> = spec.chars().collect();
    let align_of = |c: char| match c {
        '<' => Some(FmtAlign::Left),
        '^' => Some(FmtAlign::Center),
        '>' => Some(FmtAlign::Right),
        _ => None,
    };
    let mut out = FmtSpec::default();
    let mut i = 0;
    if cs.len() >= 2 && align_of(cs[1]).is_some() {
        out.fill = Some(cs[0]);
        out.align = align_of(cs[1]);
        i = 2;
    } else if let Some(a) = cs.first().and_then(|c| align_of(*c)) {
        out.align = Some(a);
        i = 1;
    }
    if cs.get(i) == Some(&'0') {
        out.zero = true;
        i += 1;
    }
    let digits: String = cs[i..].iter().take_while(|c| c.is_ascii_digit()).collect();
    i += digits.chars().count();
    if !digits.is_empty() {
        out.width = Some(digits.parse().map_err(|_| refuse(format!("the width `{digits}` is too large")))?);
    }
    if i < cs.len() {
        return Err(refuse(format!("the format spec `{{:{spec}}}` is not in v0; v0 has {SPECS}")));
    }
    if out.zero && out.width.is_none() {
        return Err(refuse("zero padding needs a width in v0: `{:05}`"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(fill: Option<char>, align: Option<FmtAlign>, zero: bool, width: Option<u32>) -> FmtSpec {
        FmtSpec { fill, align, zero, width }
    }

    #[test]
    fn templates_read_as_rust_reads_them() {
        let p = parse_template("a{{b}}{}{0}{x:>5}{:*^3}{:05}").unwrap();
        assert_eq!(
            p,
            vec![
                Parsed::Text("a{b}".into()),
                Parsed::Arg(Ref::Next, FmtSpec::default()),
                Parsed::Arg(Ref::Index(0), FmtSpec::default()),
                Parsed::Arg(Ref::Named("x".into()), spec(None, Some(FmtAlign::Right), false, Some(5))),
                Parsed::Arg(Ref::Next, spec(Some('*'), Some(FmtAlign::Center), false, Some(3))),
                Parsed::Arg(Ref::Next, spec(None, None, true, Some(5))),
            ]
        );
    }

    #[test]
    fn specs_outside_v0_are_refused() {
        for s in ["?", "x", "#?", "+", ".2", "5$", "1$", "e", "05x", "0"] {
            assert!(parse_spec(s).is_err(), "{s}");
        }
        assert!(parse_template("a}b").is_err());
        assert!(parse_template("a{b").is_err());
    }
}
