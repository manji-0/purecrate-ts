//! A small TypeScript syntax tree for the wire module, laid out as oxfmt
//! lays it out (`doc`): member chains, call arguments that hug their last
//! object, array, or arrow, function composition broken out, arrays of
//! arrays one element per line, and the quote that needs fewer escapes.
//! Only what `schema` prints is here.

use crate::doc::{assign, broken_group, concat, group, has_hard, if_break, indent, join, text, Doc};

/// An expression.
#[derive(Clone, Debug)]
pub(crate) enum Js {
    /// Printed as is: a name, a number, a template literal, `type.errors`.
    Raw(String),
    /// A string literal of this text.
    Str(String),
    Call(Box<Js>, Vec<Js>),
    /// `object.name`.
    Member(Box<Js>, String),
    Array(Vec<Js>),
    /// Keys printed as they are; a value that is the key's name is shorthand.
    Object(Vec<(String, Js)>),
    Arrow(Arrow),
    Ternary(Box<Js>, Box<Js>, Box<Js>),
    /// `e as T`.
    As(Box<Js>, String),
    /// `(e)`, around an arrow called at once.
    Paren(Box<Js>),
}

#[derive(Clone, Debug)]
pub(crate) struct Arrow {
    /// `(x, ctx)`, as printed.
    pub params: String,
    pub ret: Option<String>,
    pub body: Body,
}

#[derive(Clone, Debug)]
pub(crate) enum Body {
    Expr(Box<Js>),
    Block(Vec<Stmt>),
}

/// A statement of a function body.
#[derive(Clone, Debug)]
pub(crate) enum Stmt {
    Const(String, Js),
    Return(Js),
    /// `if (c) s` without braces.
    If(Js, Box<Stmt>),
    IfBlock(Js, Vec<Stmt>),
    Block(Vec<Stmt>),
    Expr(Js),
    Switch(Js, Vec<(Js, Vec<Stmt>)>),
}

pub(crate) fn raw(s: impl Into<String>) -> Js {
    Js::Raw(s.into())
}

pub(crate) fn str_lit(s: impl Into<String>) -> Js {
    Js::Str(s.into())
}

pub(crate) fn call(callee: Js, args: Vec<Js>) -> Js {
    Js::Call(Box::new(callee), args)
}

/// `a.b.c(args)` from a dotted path.
pub(crate) fn call_path(path: &str, args: Vec<Js>) -> Js {
    call(path_expr(path), args)
}

/// `a.b.c` as members, so a chain reads it as one.
pub(crate) fn path_expr(path: &str) -> Js {
    let mut parts = path.split('.');
    let mut e = raw(parts.next().unwrap_or(""));
    for p in parts {
        e = member(e, p);
    }
    e
}

pub(crate) fn member(object: Js, name: &str) -> Js {
    Js::Member(Box::new(object), name.to_string())
}

pub(crate) fn arrow(params: &str, ret: Option<&str>, body: Body) -> Js {
    Js::Arrow(Arrow { params: params.to_string(), ret: ret.map(str::to_string), body })
}

pub(crate) fn expr_body(e: Js) -> Body {
    Body::Expr(Box::new(e))
}

pub(crate) fn ternary(test: Js, then: Js, else_: Js) -> Js {
    Js::Ternary(Box::new(test), Box::new(then), Box::new(else_))
}

fn comma_line() -> Doc {
    concat(vec![text(","), Doc::Line])
}

impl Js {
    pub(crate) fn doc(&self) -> Doc {
        match self {
            Js::Raw(s) => text(s.clone()),
            Js::Str(s) => text(crate::expr::js_string(s)),
            Js::Call(..) | Js::Member(..) => chain_doc(self),
            Js::Array(elems) => array_doc(elems),
            Js::Object(props) => object_doc(props),
            Js::Arrow(a) => a.doc(false),
            Js::Ternary(t, a, b) => group(concat(vec![
                t.doc(),
                indent(concat(vec![Doc::Line, text("? "), a.doc(), Doc::Line, text(": "), b.doc()])),
            ])),
            Js::As(e, ty) => concat(vec![e.doc(), text(format!(" as {ty}"))]),
            Js::Paren(e) => concat(vec![text("("), e.doc(), text(")")]),
        }
    }

    /// Each name in the expression's code passed through `f`: names, types,
    /// parameters, template substitutions; not strings, keys, or `.name`s.
    pub(crate) fn rename(&mut self, f: &dyn Fn(&str) -> String) {
        match self {
            Js::Raw(s) => *s = f(s),
            Js::Str(_) => {}
            Js::Call(c, args) => {
                c.rename(f);
                args.iter_mut().for_each(|a| a.rename(f));
            }
            Js::Member(o, _) => o.rename(f),
            Js::Array(xs) => xs.iter_mut().for_each(|x| x.rename(f)),
            Js::Object(ps) => ps.iter_mut().for_each(|(_, v)| v.rename(f)),
            Js::Arrow(a) => a.rename(f),
            Js::Ternary(t, a, b) => {
                t.rename(f);
                a.rename(f);
                b.rename(f);
            }
            Js::As(e, ty) => {
                e.rename(f);
                *ty = f(ty);
            }
            Js::Paren(e) => e.rename(f),
        }
    }

    /// An object literal, array literal, or arrow whose body is one, which
    /// call arguments may hug.
    fn could_expand(&self) -> bool {
        match self {
            Js::Object(p) => !p.is_empty(),
            Js::Array(e) => !e.is_empty(),
            Js::Arrow(a) => match &a.body {
                Body::Block(_) => true,
                Body::Expr(e) => {
                    matches!(**e, Js::Object(_) | Js::Array(_) | Js::Call(..) | Js::Ternary(..) | Js::Arrow(_))
                }
            },
            _ => false,
        }
    }

    fn kind(&self) -> u8 {
        match self {
            Js::Object(_) => 1,
            Js::Array(_) => 2,
            Js::Arrow(_) => 3,
            _ => 0,
        }
    }
}

impl Arrow {
    fn rename(&mut self, f: &dyn Fn(&str) -> String) {
        self.params = f(&self.params);
        if let Some(r) = &mut self.ret {
            *r = f(r);
        }
        match &mut self.body {
            Body::Expr(e) => e.rename(f),
            Body::Block(ss) => ss.iter_mut().for_each(|s| s.rename(f)),
        }
    }

    /// `expand_last`: the arrow is the hugged last argument, so a body that
    /// moves to its own line ends with the call's trailing comma.
    fn doc(&self, expand_last: bool) -> Doc {
        let ret = self.ret.as_ref().map(|r| format!(": {r}")).unwrap_or_default();
        let head = text(format!("{}{ret} =>", self.params));
        match &self.body {
            Body::Block(stmts) => concat(vec![head, text(" "), block_doc(stmts)]),
            Body::Expr(e) => match &**e {
                Js::Object(_) => concat(vec![head, text(" ("), e.doc(), text(")")]),
                Js::Array(_) => concat(vec![head, text(" "), e.doc()]),
                // The body's line and the hugged call's trailing comma break
                // with the arrow, as Prettier's `printArrowFunctionBody` has it.
                Js::Ternary(..) => group(concat(vec![
                    head,
                    indent(concat(vec![
                        Doc::Line,
                        if_break(text(""), text("(")),
                        e.doc(),
                        if_break(text(""), text(")")),
                    ])),
                    tail(expand_last),
                ])),
                _ => group(concat(vec![head, indent(concat(vec![Doc::Line, e.doc()])), tail(expand_last)])),
            },
        }
    }
}

fn tail(expand_last: bool) -> Doc {
    if expand_last {
        concat(vec![if_break(text(","), text("")), Doc::SoftLine])
    } else {
        text("")
    }
}

fn array_doc(elems: &[Js]) -> Doc {
    if elems.is_empty() {
        return text("[]");
    }
    // A list of two or more arrays of two or more, or of objects: a matrix,
    // one element per line however short.
    let matrix = elems.len() > 1
        && (elems.iter().all(|e| matches!(e, Js::Array(x) if x.len() > 1))
            || elems.iter().all(|e| matches!(e, Js::Object(p) if !p.is_empty())));
    let body = concat(vec![
        text("["),
        indent(concat(vec![Doc::SoftLine, join(comma_line(), elems.iter().map(Js::doc).collect())])),
        if_break(text(","), text("")),
        Doc::SoftLine,
        text("]"),
    ]);
    if matrix {
        broken_group(body)
    } else {
        group(body)
    }
}

fn object_doc(props: &[(String, Js)]) -> Doc {
    if props.is_empty() {
        return text("{}");
    }
    let items = props
        .iter()
        .map(|(k, v)| match v {
            Js::Raw(r) if r == k => text(k.clone()),
            v => assign(text(format!("{k}:")), v.doc()),
        })
        .collect();
    group(concat(vec![
        text("{"),
        indent(concat(vec![Doc::Line, join(comma_line(), items)])),
        if_break(text(","), text("")),
        Doc::Line,
        text("}"),
    ]))
}

pub(crate) fn block_doc(stmts: &[Stmt]) -> Doc {
    if stmts.is_empty() {
        return text("{}");
    }
    concat(vec![
        text("{"),
        indent(concat(vec![Doc::HardLine, join(Doc::HardLine, stmts.iter().map(Stmt::doc).collect())])),
        Doc::HardLine,
        text("}"),
    ])
}

impl Stmt {
    fn rename(&mut self, f: &dyn Fn(&str) -> String) {
        match self {
            Stmt::Const(name, v) => {
                *name = f(name);
                v.rename(f);
            }
            Stmt::Return(e) | Stmt::Expr(e) => e.rename(f),
            Stmt::If(c, s) => {
                c.rename(f);
                s.rename(f);
            }
            Stmt::IfBlock(c, ss) => {
                c.rename(f);
                ss.iter_mut().for_each(|s| s.rename(f));
            }
            Stmt::Block(ss) => ss.iter_mut().for_each(|s| s.rename(f)),
            Stmt::Switch(subject, cases) => {
                subject.rename(f);
                for (test, body) in cases {
                    test.rename(f);
                    body.iter_mut().for_each(|s| s.rename(f));
                }
            }
        }
    }

    pub(crate) fn doc(&self) -> Doc {
        match self {
            Stmt::Const(name, v) => concat(vec![assign(text(format!("const {name} =")), v.doc()), text(";")]),
            Stmt::Return(e) => concat(vec![text("return "), e.doc(), text(";")]),
            Stmt::If(c, s) => {
                group(concat(vec![text("if ("), c.doc(), text(")"), indent(concat(vec![Doc::Line, s.doc()]))]))
            }
            Stmt::IfBlock(c, ss) => concat(vec![text("if ("), c.doc(), text(") "), block_doc(ss)]),
            Stmt::Block(ss) => block_doc(ss),
            Stmt::Expr(e) => concat(vec![e.doc(), text(";")]),
            Stmt::Switch(subject, cases) => {
                let cases = cases
                    .iter()
                    .map(|(test, body)| {
                        // `default` is the test that is no `case`.
                        let head = match test {
                            Js::Raw(r) if r == "default" => text("default:"),
                            t => concat(vec![text("case "), t.doc(), text(":")]),
                        };
                        concat(vec![
                            head,
                            indent(concat(vec![
                                Doc::HardLine,
                                join(Doc::HardLine, body.iter().map(Stmt::doc).collect()),
                            ])),
                        ])
                    })
                    .collect();
                concat(vec![
                    text("switch ("),
                    subject.doc(),
                    text(") {"),
                    indent(concat(vec![Doc::HardLine, join(Doc::HardLine, cases)])),
                    Doc::HardLine,
                    text("}"),
                ])
            }
        }
    }
}

/// `const name: ty = value;`, exported or not.
pub(crate) fn decl(export: bool, name: &str, ty: Option<&str>, value: &Js) -> Doc {
    let export = if export { "export " } else { "" };
    let head = format!("{export}const {name}");
    // A generic type's arguments break one per line, with no trailing comma
    // (`v.GenericSchema<` .. `> =`), where the line does not fit.
    let lhs = match ty.and_then(|t| t.strip_suffix('>')).and_then(|t| t.split_once('<')) {
        Some((generic, args)) if args.contains(',') && !args.contains(['<', '{']) => group(concat(vec![
            text(format!("{head}: {generic}<")),
            indent(concat(vec![
                Doc::SoftLine,
                join(concat(vec![text(","), Doc::Line]), args.split(',').map(|a| text(a.trim())).collect()),
            ])),
            Doc::SoftLine,
            text("> ="),
        ])),
        _ => text(format!("{head}{} =", ty.map(|t| format!(": {t}")).unwrap_or_default())),
    };
    concat(vec![assign(lhs, value.doc()), text(";")])
}

/// A link of a member chain: `.name` or a call's arguments.
enum Link<'a> {
    Member(&'a str),
    Call(&'a [Js]),
}

/// A call or member expression, as a chain: the head with the calls right
/// after it, then each `.name` with its calls. Two groups or fewer (three
/// where the head is a capitalized name, merged with the first) print as
/// one line; more break one `.name` per line where the line does not fit,
/// or always where more than two calls take something but plain values.
fn chain_doc(e: &Js) -> Doc {
    let mut links = Vec::new();
    let mut head = e;
    loop {
        match head {
            Js::Call(callee, args) => {
                links.push(Link::Call(args));
                head = callee;
            }
            Js::Member(object, name) => {
                links.push(Link::Member(name));
                head = object;
            }
            _ => break,
        }
    }
    links.reverse();
    let mut groups: Vec<Vec<&Link>> = vec![Vec::new()];
    for link in &links {
        match link {
            Link::Call(_) if groups.len() == 1 => groups[0].push(link),
            Link::Member(_) => groups.push(vec![link]),
            Link::Call(_) => groups.last_mut().expect("a group").push(link),
        }
    }
    let print_links = |ls: &[&Link]| concat(ls.iter().map(|l| link_doc(l)).collect());
    let first = concat(vec![head.doc(), print_links(&groups[0])]);
    let rest: Vec<Doc> = groups[1..].iter().map(|g| print_links(g)).collect();
    let factory = groups[0].is_empty()
        && matches!(head, Js::Raw(n) if n.starts_with(|c: char| c.is_ascii_uppercase()) || n.chars().all(|c| c == '_' || c == '$'));
    let merge = factory && !rest.is_empty();
    let cutoff = if merge { 3 } else { 2 };
    let mut one_line = vec![first.clone()];
    one_line.extend(rest.iter().cloned());
    let one_line = concat(one_line);
    if groups.len() <= cutoff {
        return group(one_line);
    }
    let (merged, indented) = if merge { (Some(rest[0].clone()), &rest[1..]) } else { (None, &rest[..]) };
    let mut expanded = vec![first];
    if let Some(m) = merged {
        expanded.push(m);
    }
    expanded.push(indent(concat(indented.iter().flat_map(|d| [Doc::HardLine, d.clone()]).collect())));
    let expanded = concat(expanded);
    let calls: Vec<&[Js]> = links.iter().filter_map(|l| if let Link::Call(a) = l { Some(*a) } else { None }).collect();
    let complex = calls.len() > 2 && calls.iter().any(|args| !args.iter().all(simple_argument));
    if complex || rest[..rest.len() - 1].iter().any(has_hard) {
        return group(expanded);
    }
    let breaker = if has_hard(&one_line) { Doc::BreakParent } else { text("") };
    concat(vec![breaker, Doc::Conditional(vec![one_line, expanded])])
}

/// Prettier's `isSimpleCallArgument`: names, literals, and objects, arrays,
/// and calls of them; an arrow only with a block body.
fn simple_argument(e: &Js) -> bool {
    match e {
        Js::Raw(_) | Js::Str(_) => true,
        Js::Object(p) => p.iter().all(|(_, v)| simple_argument(v)),
        Js::Array(xs) => xs.iter().all(simple_argument),
        Js::Call(callee, args) => simple_argument(callee) && args.iter().all(simple_argument),
        Js::Member(o, _) => simple_argument(o),
        Js::Arrow(a) => matches!(a.body, Body::Block(_)),
        Js::Ternary(..) | Js::As(..) | Js::Paren(_) => false,
    }
}

fn link_doc(link: &Link) -> Doc {
    match link {
        Link::Member(name) => text(format!(".{name}")),
        Link::Call(args) => arguments_doc(args),
    }
}

/// A call's arguments: flat where they fit; the last hugged where it is an
/// object, array, or arrow (`f(a, {` .. `})`); else one per line. Function
/// composition (`pipe(a, transform(() => ..))`) is always one per line.
fn arguments_doc(args: &[Js]) -> Doc {
    if args.is_empty() {
        return text("()");
    }
    let docs: Vec<Doc> = args.iter().map(Js::doc).collect();
    let broken_out = |docs: Vec<Doc>| {
        broken_group(concat(vec![
            text("("),
            indent(concat(vec![Doc::SoftLine, join(comma_line(), docs)])),
            text(","),
            Doc::SoftLine,
            text(")"),
        ]))
    };
    if composition(args) {
        return broken_out(docs);
    }
    let n = args.len();
    let last = &args[n - 1];
    let group_last = last.could_expand()
        && (n < 2 || args[n - 2].kind() != last.kind() || last.kind() == 0)
        && !(n == 2 && matches!(args[0], Js::Arrow(_)) && matches!(last, Js::Array(_)));
    if !group_last {
        return group(concat(vec![
            text("("),
            indent(concat(vec![Doc::SoftLine, join(comma_line(), docs.clone())])),
            if_break(text(","), text("")),
            Doc::SoftLine,
            text(")"),
        ]));
    }
    if docs[..n - 1].iter().any(has_hard) {
        return broken_out(docs);
    }
    let last_expanded = match last {
        Js::Arrow(a) => a.doc(true),
        other => other.doc(),
    };
    let mut flat: Vec<Doc> = docs[..n - 1].to_vec();
    flat.push(last_expanded.clone());
    let mut hugged: Vec<Doc> = docs[..n - 1].to_vec();
    hugged.push(broken_group(last_expanded));
    let paren = |ds: Vec<Doc>| concat(vec![text("("), join(text(", "), ds), text(")")]);
    let breaker = if docs.iter().any(has_hard) { Doc::BreakParent } else { text("") };
    concat(vec![breaker, Doc::Conditional(vec![paren(flat), paren(hugged), broken_out(docs)])])
}

/// Prettier's `isFunctionCompositionArgs`: two arrows, or an arrow inside a
/// call among two arguments or more.
fn composition(args: &[Js]) -> bool {
    if args.len() < 2 {
        return false;
    }
    let mut arrows = 0;
    for a in args {
        match a {
            Js::Arrow(_) => {
                arrows += 1;
                if arrows > 1 {
                    return true;
                }
            }
            Js::Call(_, inner) if inner.iter().any(|x| matches!(x, Js::Arrow(_))) => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(js: &Js) -> String {
        crate::doc::print(&js.doc(), 100, 0)
    }

    #[test]
    fn quotes_take_fewer_escapes() {
        assert_eq!(p(&str_lit("a")), "\"a\"");
        assert_eq!(p(&str_lit("\"x\"")), "'\"x\"'");
    }

    #[test]
    fn arrays_of_pairs_break_however_short() {
        let pair = |k: &str| Js::Array(vec![str_lit(k), raw("v")]);
        let j = call_path("Json.object", vec![Js::Array(vec![pair("a"), pair("b")])]);
        assert_eq!(p(&j), "Json.object([\n  [\"a\", v],\n  [\"b\", v],\n])");
        let one = call_path("Json.object", vec![Js::Array(vec![pair("a")])]);
        assert_eq!(p(&one), "Json.object([[\"a\", v]])");
    }
}
