//! Drops the imports a generated file does not use, so it passes a
//! consumer's `noUnusedLocals`. The import lists are gathered from the IR
//! ahead of printing and may name more than the printed code reads; this
//! pass decides from the printed code itself.

use std::collections::BTreeSet;

/// `src` with every `import { .. }` name that the rest of the file does not
/// mention removed, and imports left empty dropped. Other lines are kept.
pub fn prune_unused(src: &str) -> String {
    let code = src.lines().filter(|l| !l.starts_with("import ")).collect::<Vec<_>>().join("\n");
    let used = code_idents(&code);
    // A name that is a value and a type (the runtime's `Result`, `Char`,
    // `Uuid`, and `Ord`, a crate type and its companion) may be read only
    // as a type where the printed code no longer reads the value (a fold
    // took `Result.ok`, or the code after a decided `return`, away); lint
    // wants `type X` then.
    let values = value_reads(&code);
    let mut out = String::with_capacity(src.len());
    for line in src.split_inclusive('\n') {
        match prune_line(line.trim_end_matches('\n'), &used, &values) {
            Some(kept) if kept == line.trim_end_matches('\n') => out.push_str(line),
            Some(kept) => {
                out.push_str(&kept);
                out.push('\n');
            }
            None => {}
        }
    }
    // Every import of a block dropped leaves its blank line after the header.
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    organize(&out)
}

/// The imports as TS's organize-imports leaves them: one declaration per
/// module, `import type` when it brings only types (an inline `type` there
/// would keep a bare `import "./m.ts"` under `verbatimModuleSyntax`), values
/// then types, each by name. Packages and the runtime come first as they
/// are; the crate's own files follow by path.
fn organize(src: &str) -> String {
    let mut modules: Vec<(String, Vec<String>)> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    let mut at = None;
    let mut rest = String::with_capacity(src.len());
    for line in src.split_inclusive('\n') {
        let Some(import) = line.trim_end_matches('\n').strip_prefix("import ") else {
            rest.push_str(line);
            continue;
        };
        at.get_or_insert(rest.len());
        let (type_only, import) = match import.strip_prefix("type ") {
            Some(r) => (true, r),
            None => (false, import),
        };
        let parsed = import.strip_prefix('{').and_then(|r| r.split_once("} from ")).filter(|(_, m)| m.ends_with(';'));
        let Some((specs, module)) = parsed else {
            others.push(line.trim_end_matches('\n').to_string());
            continue;
        };
        let specs = specs.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| {
            if type_only && !s.starts_with("type ") {
                format!("type {s}")
            } else {
                s.to_string()
            }
        });
        let module = module.trim_end_matches(';').to_string();
        match modules.iter_mut().find(|(m, _)| *m == module) {
            Some((_, list)) => list.extend(specs),
            None => modules.push((module, specs.collect())),
        }
    }
    let Some(at) = at else { return src.to_string() };
    let local = |m: &str| m.starts_with("\"./") && !m.starts_with("\"./purecrate");
    let (mut own, packages): (Vec<_>, Vec<_>) = modules.into_iter().partition(|(m, _)| local(m));
    own.sort_by(|a, b| a.0.cmp(&b.0));
    let key = |s: &String| s.trim_start_matches("type ").to_lowercase();
    let declaration = |(module, mut specs): (String, Vec<String>)| {
        specs.sort_by_key(|s| (s.starts_with("type "), key(s)));
        specs.dedup();
        if specs.iter().all(|s| s.starts_with("type ")) {
            let names: Vec<&str> = specs.iter().map(|s| s.trim_start_matches("type ")).collect();
            format!("import type {{ {} }} from {module};\n", names.join(", "))
        } else {
            format!("import {{ {} }} from {module};\n", specs.join(", "))
        }
    };
    let mut head: String = packages.into_iter().map(declaration).collect();
    for line in &others {
        head.push_str(line);
        head.push('\n');
    }
    head.extend(own.into_iter().map(declaration));
    let mut out = rest;
    out.insert_str(at, &head);
    out
}

/// `None` drops the line. Lines that are not a named import are returned
/// unchanged.
fn prune_line(line: &str, used: &BTreeSet<String>, values: &BTreeSet<String>) -> Option<String> {
    let Some(rest) = line.strip_prefix("import ") else {
        return Some(line.to_string());
    };
    let (type_only, rest) = match rest.strip_prefix("type ") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let (Some(open), Some(close)) = (rest.find('{'), rest.find('}')) else {
        return Some(line.to_string());
    };
    if !rest[..open].trim().is_empty() {
        return Some(line.to_string());
    }
    let from = &rest[close + 1..];
    // A value import that may also be a type: a name that is not a const's
    // (`consts.ts`), a function's (lowercase), or an internal one (`$`).
    let dual = |name: &str| {
        name.starts_with(|c: char| c.is_ascii_uppercase())
            && !name.contains('$')
            && !from.contains(&format!("\"./{}.ts\"", purecrate_ir::CONSTS_STEM))
    };
    let kept: Vec<String> = rest[open + 1..close]
        .split(',')
        .map(str::trim)
        .filter(|spec| !spec.is_empty())
        .filter(|spec| used.contains(local_name(spec)))
        .map(|spec| {
            let name = local_name(spec);
            if !type_only && !spec.starts_with("type ") && dual(name) && !values.contains(name) {
                format!("type {spec}")
            } else {
                spec.to_string()
            }
        })
        .collect();
    if kept.is_empty() {
        return None;
    }
    let kw = if type_only { "import type" } else { "import" };
    Some(format!("{kw} {{ {} }}{from}", kept.join(", ")))
}

/// The binding an import specifier introduces: `type A as B` binds `B`.
fn local_name(spec: &str) -> &str {
    let spec = spec.strip_prefix("type ").unwrap_or(spec);
    spec.rsplit(" as ").next().unwrap_or(spec).trim()
}

/// The names `code` reads as values: each identifier not after a `.`
/// whose place is not one only a type takes. A place that may be either is
/// a value's, so an import is made `type` only where every read of it is a
/// type's.
fn value_reads(code: &str) -> BTreeSet<String> {
    let (idents, _, contexts) = scan(code);
    idents
        .into_iter()
        .zip(contexts)
        .filter(|((start, end, after_dot), context)| !after_dot && !type_place(code, *start, *end, *context))
        .map(|((start, end, _), _)| code[start..end].to_string())
        .collect()
}

/// The identifier at `start..end` stands where only a type goes, as the
/// printer writes one: after `as`, `satisfies`, `readonly`, `extends`,
/// `keyof`, or `is`; in a union or an intersection; a generic's argument
/// (`Iterable<Char>`, `Map<Char, U8>`, `Result<..>`) or a tuple type's
/// element (`readonly [Char, Char]`); after an annotation's `:` (no space
/// before it, as a ternary's has; not an object literal's value, `{ k: v }`).
/// Read by its member (`Char.code`), called, passed (`pair(i, Item)`), or
/// anywhere else, it is a value.
fn type_place(code: &str, start: usize, end: usize, context: Option<char>) -> bool {
    let next = code[end..].trim_start();
    if next.starts_with('.') {
        return false;
    }
    if code[end..].starts_with('<') {
        return true;
    }
    let before = code[..start].trim_end();
    let word_at = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$').len();
    match &before[word_at..] {
        "as" | "satisfies" | "readonly" | "extends" | "keyof" | "is" => return true,
        "" => {}
        _ => return false,
    }
    match before.chars().next_back() {
        Some('|') => !before.ends_with("||"),
        Some('&') => !before.ends_with("&&"),
        Some('<' | ',') => matches!(context, Some('<' | 'T')),
        Some('[') => context == Some('T'),
        Some(':') => {
            let spaced = before[..before.len() - 1].ends_with(char::is_whitespace);
            let field_value = context == Some('{') && (next.starts_with(',') || next.starts_with('}'));
            !spaced && !field_value
        }
        _ => false,
    }
}

/// Identifiers in TS source, outside comments and string literals, and not
/// after a `.` (a property name is not a reference to a binding). The
/// `${..}` of a template literal is code.
fn code_idents(src: &str) -> BTreeSet<String> {
    code_spans(src).into_iter().map(|(start, end)| src[start..end].to_string()).collect()
}

/// The byte range of each identifier in the code of `src` that is not
/// after a `.`: a binding's, not a property's.
pub(crate) fn code_spans(src: &str) -> Vec<(usize, usize)> {
    ident_spans(src).into_iter().filter(|(_, _, after_dot)| !after_dot).map(|(start, end, _)| (start, end)).collect()
}

/// Each identifier in the code of `src`, outside comments and string
/// literals, as its byte range and whether it follows a `.`.
pub(crate) fn ident_spans(src: &str) -> Vec<(usize, usize, bool)> {
    scan(src).0
}

/// The byte ranges of the `{ .. }` pairs in the code of `src`, braces
/// included, and of each `for (..) { .. }` from its `(`: the head's
/// bindings are the loop's own. A template's `${ .. }` is not one.
pub(crate) fn brace_spans(src: &str) -> Vec<(usize, usize)> {
    scan(src).1
}

/// Identifier spans (`ident_spans`), brace spans (`brace_spans`), and
/// each identifier's innermost open bracket: `(`, `[`, `{`, `<` for a
/// generic's arguments, and `T` for a tuple type's (`readonly [`).
type Spans = (Vec<(usize, usize, bool)>, Vec<(usize, usize)>, Vec<Option<char>>);

fn scan(src: &str) -> Spans {
    let chars: Vec<char> = src.chars().collect();
    let offsets: Vec<usize> = src.char_indices().map(|(at, _)| at).chain([src.len()]).collect();
    let mut out = Vec::new();
    // Each open `{`, with the `(` of the `for` head it is the body of.
    let mut open: Vec<(usize, Option<usize>)> = Vec::new();
    let mut blocks = Vec::new();
    // Each open `(`, and whether it is a `for` head; a closed head waits
    // for its body.
    let mut parens: Vec<(usize, bool)> = Vec::new();
    let mut after_for = false;
    let mut head: Option<usize> = None;
    // Open template literals, each with the brace depth of the code in its
    // current `${..}`.
    let mut templates: Vec<usize> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let mut after_dot = false;
    // The open brackets, for `contexts`.
    let mut brackets: Vec<char> = Vec::new();
    let mut contexts = Vec::new();
    let word = |c: Option<&char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$');
    // Closes the innermost `open`, and any generic `<` misread inside it.
    let close = |brackets: &mut Vec<char>, open: char| {
        while brackets.last() == Some(&'<') {
            brackets.pop();
        }
        if brackets.last() == Some(&open) || (open == '[' && brackets.last() == Some(&'T')) {
            brackets.pop();
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i += 2;
                continue;
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '"' | '\'' => {
                i += 1;
                while i < chars.len() && chars[i] != c {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                after_dot = false;
                continue;
            }
            '`' => {
                templates.push(depth);
                depth = 0;
                i = skip_template_text(&chars, i + 1, &mut templates, &mut depth);
                after_dot = false;
                continue;
            }
            '{' => {
                depth += 1;
                open.push((offsets[i], head.take()));
                brackets.push('{');
            }
            '(' => {
                brackets.push('(');
                parens.push((offsets[i], after_for));
                after_for = false;
                head = None;
                i += 1;
                continue;
            }
            ')' => {
                close(&mut brackets, '(');
                if let Some((start, true)) = parens.pop() {
                    head = Some(start);
                }
                i += 1;
                continue;
            }
            '}' if depth == 0 && !templates.is_empty() => {
                i = skip_template_text(&chars, i + 1, &mut templates, &mut depth);
                after_dot = false;
                continue;
            }
            '}' => {
                close(&mut brackets, '{');
                depth = depth.saturating_sub(1);
                if let Some((start, for_head)) = open.pop() {
                    blocks.push((start, offsets[i + 1]));
                    if let Some(h) = for_head {
                        blocks.push((h, offsets[i + 1]));
                    }
                }
            }
            _ if c.is_ascii_alphabetic() || c == '_' || c == '$' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                    i += 1;
                }
                out.push((offsets[start], offsets[i.min(chars.len())], after_dot));
                contexts.push(brackets.last().copied());
                after_for = !after_dot && chars[start..i].iter().collect::<String>() == "for";
                head = None;
                after_dot = false;
                continue;
            }
            '[' => {
                let mut b = i;
                while b > 0 && chars[b - 1].is_whitespace() {
                    b -= 1;
                }
                let tuple = chars[..b].ends_with(&['r', 'e', 'a', 'd', 'o', 'n', 'l', 'y']);
                brackets.push(if tuple { 'T' } else { '[' });
            }
            ']' => close(&mut brackets, '['),
            // A generic's `<` follows its name with no space; a comparison's
            // has spaces.
            '<' if i > 0 && word(chars.get(i - 1)) && chars.get(i + 1).is_some_and(|c| !c.is_whitespace()) => {
                brackets.push('<');
            }
            '>' if brackets.last() == Some(&'<') && i > 0 && chars[i - 1] != '=' => {
                brackets.pop();
            }
            // A spread (`...x`) reads a binding.
            '.' if chars.get(i + 1) == Some(&'.') && chars.get(i + 2) == Some(&'.') => {
                after_dot = false;
                i += 3;
                continue;
            }
            '.' => {
                after_dot = true;
                i += 1;
                continue;
            }
            _ if c.is_whitespace() => {
                i += 1;
                continue;
            }
            _ => {}
        }
        after_dot = false;
        after_for = false;
        i += 1;
    }
    (out, blocks, contexts)
}

/// From inside a template literal's text, to just after the `${` that opens
/// code (brace depth 0) or after the closing backquote (the enclosing
/// template, or plain code, resumes at the depth it had).
fn skip_template_text(chars: &[char], mut i: usize, templates: &mut Vec<usize>, depth: &mut usize) -> usize {
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '`' => {
                *depth = templates.pop().unwrap_or(0);
                return i + 1;
            }
            '$' if chars.get(i + 1) == Some(&'{') => {
                *depth = 0;
                return i + 2;
            }
            _ => i += 1,
        }
    }
    i
}

use super::*;

/// A `char` range, which prints through `Char.code`.
pub(crate) fn has_char_range(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Range { lo: Lit::Char(_), .. } => true,
        Pattern::Or(alts) => alts.iter().any(has_char_range),
        _ => false,
    }
}

/// The integer types of the literals in an integer arm.
pub(crate) fn int_case_lits(pattern: &Pattern, f: &mut impl FnMut(purecrate_ir::IntTy)) {
    match pattern {
        Pattern::Lit(Lit::Int { ty: Some(t), .. }) => f(*t),
        Pattern::Range { lo, hi, .. } => {
            for l in [lo, hi] {
                if let Lit::Int { ty: Some(t), .. } = l {
                    f(*t);
                }
            }
        }
        Pattern::Or(alts) => alts.iter().for_each(|a| int_case_lits(a, f)),
        _ => {}
    }
}

/// Names one file refers to. A value import also brings the same-named type.
#[derive(Default)]
pub(crate) struct Refs {
    never: bool,
    int: bool,
    /// `Result.ok` / `Result.err`, and the `Result` type.
    result_value: bool,
    result_type: bool,
    /// `Str` from `./str.ts`.
    str: bool,
    /// `Slice` from `./str.ts`, for indexing and slicing a `Vec`.
    slice: bool,
    eq: bool,
    /// `Ord` from `./str.ts`, for `cmp` and `Ordering::then`.
    ord: bool,
    /// `Iter` from `./str.ts`, for `all`, `any`, `position`, `count`, `sum`.
    iter: bool,
    /// `BTree`, for a `BTreeMap`'s `get`, `insert`, `remove`, and order.
    btree: bool,
    /// The `Char` runtime, and the `Char` type, from `./str.ts`.
    char_value: bool,
    char_type: bool,
    /// The `Uuid` runtime, and the `Uuid` / `UuidError` types, from `./str.ts`.
    uuid_value: bool,
    uuid_type: bool,
    uuid_error: bool,
    /// The `ParseIntError` type, from `str::parse`.
    parse_int_error: bool,
    /// Brand type names (`I32`, `F64`) this file mentions.
    nums: BTreeSet<String>,
    /// The std traits' records (`Ord<T>`, `Display<T>`) a dictionary
    /// parameter's type names, which the runtime exports.
    std_traits: BTreeSet<String>,
    types: BTreeSet<String>,
    values: BTreeSet<String>,
    /// Closed structs this file builds through their `$of`.
    ctors: BTreeSet<String>,
    /// Methods that are not `pub` this file calls, as `Ty$name`.
    privates: BTreeSet<(String, String)>,
}

impl Refs {
    fn ty(&mut self, ty: &Ty) {
        match ty {
            Ty::App { name: n, .. } if crate::context::is_std_trait(n.as_str()) => {
                self.std_traits.insert(n.as_str().to_string());
            }
            Ty::Named(n) | Ty::App { name: n, .. } => {
                self.types.insert(n.as_str().to_string());
            }
            Ty::Param(_) => {}
            Ty::Result { .. } => self.result_type = true,
            Ty::Prim(purecrate_ir::Prim::Char) => self.char_type = true,
            Ty::Prim(purecrate_ir::Prim::Uuid) => self.uuid_type = true,
            Ty::Prim(purecrate_ir::Prim::UuidError) => self.uuid_error = true,
            Ty::Prim(purecrate_ir::Prim::ParseIntError) => self.parse_int_error = true,
            Ty::Prim(p) => {
                if let Some(name) = p.int().map(|t| t.ts_name()).or_else(|| p.float().map(|t| t.ts_name())) {
                    self.nums.insert(name.to_string());
                }
            }
            Ty::Option(_)
            | Ty::Vec(_)
            | Ty::Array { .. }
            | Ty::Map { .. }
            | Ty::Set(_)
            | Ty::Ignored { .. }
            | Ty::Tuple(_)
            | Ty::Fn { .. }
            | Ty::Never => {}
        }
        ty.children().into_iter().for_each(|t| self.ty(t));
    }

    /// The integer brands and `Char` that literal and range patterns print.
    fn arm_literals<'p>(&mut self, patterns: impl Iterator<Item = &'p Pattern>) {
        for p in patterns {
            int_case_lits(p, &mut |t| {
                self.nums.insert(t.ts_name().to_string());
            });
            self.char_value |= has_char_range(p);
            self.char_type |= p.is_char_case();
        }
    }

    /// What a walk over a string reads: `Char` for its chars, `Str` for its
    /// bytes; nothing for a list's items.
    fn over(&mut self, over: purecrate_ir::Over) {
        match over {
            purecrate_ir::Over::Chars => self.char_type = true,
            purecrate_ir::Over::Bytes => self.str = true,
            purecrate_ir::Over::Items => {}
        }
    }

    /// What the printed code of `expr` will import; everything else is
    /// walked through.
    fn expr(&mut self, krate: &Crate, expr: &Expr) {
        match expr {
            Expr::Match { scrutinee, arms } => {
                self.never |= arms.iter().any(|a| {
                    matches!(a.pattern, Pattern::Variant { .. })
                        || (matches!(a.pattern, Pattern::Or(_)) && !a.pattern.is_lit_case())
                });
                self.arm_literals(arms.iter().map(|a| &a.pattern));
                if !is_place(scrutinee) {
                    if let Some(ty) = scrutinee_ty(arms) {
                        self.types.insert(ty.as_str().to_string());
                    }
                }
            }
            Expr::Unreachable => self.never = true,
            Expr::Index { .. } => self.slice = true,
            Expr::Call { callee, args } => {
                match callee {
                    Callee::Fn(n) if is_free_fn(krate, n.as_str()) => {
                        self.values.insert(n.as_str().to_string());
                    }
                    Callee::StructNew(ty) if closed_in(krate, ty.as_str()) => {
                        self.ctors.insert(ty.as_str().to_string());
                    }
                    Callee::Method { ty, name } if private_in(krate, ty.as_str(), name.as_str()) => {
                        self.privates.insert((ty.as_str().to_string(), name.as_str().to_string()));
                    }
                    Callee::Method { ty, .. } | Callee::StructNew(ty) => {
                        self.values.insert(ty.as_str().to_string());
                    }
                    Callee::ResultOk | Callee::ResultErr => self.result_value = true,
                    Callee::OrdCmp { .. } | Callee::OrdCmpList { .. } | Callee::OrdThen => self.ord = true,
                    // `Ord.` where a comparator is the runtime's, and each
                    // crate type whose `cmp` it calls.
                    Callee::OrdCmpBy(by) | Callee::OrdBy(by) | Callee::OrdPick { by, .. } => {
                        use purecrate_ir::Cmp;
                        self.ord |= !matches!(by, Cmp::Type(_) | Cmp::Dict(_) | Cmp::Fn(_))
                            || matches!(callee, Callee::OrdPick { .. });
                        self.values.extend(by.types().into_iter().map(|t| t.as_str().to_string()));
                    }
                    Callee::Collect { over, .. }
                    | Callee::IterMap { over }
                    | Callee::IterFilter { over }
                    | Callee::IterFilterMap { over }
                    | Callee::IterFlatten { over }
                    | Callee::IterCut { over, .. } => {
                        // `Iter.map`, `Iter.filter`, and `Iter.tryCollect`, or
                        // the array's own method.
                        // A `collect` over a closure's `split` maps a generator
                        // with `Iter.map`; an unread import is pruned.
                        self.iter = true;
                        self.over(*over);
                    }
                    Callee::Consume { method, over } => {
                        self.iter = true;
                        self.over(*over);
                        if let purecrate_ir::Consume::Sum(int) = method {
                            self.int = true;
                            self.nums.insert(int.ts_name().to_string());
                        }
                        if matches!(method, purecrate_ir::Consume::Position | purecrate_ir::Consume::Count) {
                            self.nums.insert("Usize".into());
                        }
                        // `Ord.cmp` where no comparator follows the key.
                        if matches!(method, purecrate_ir::Consume::Max { .. } | purecrate_ir::Consume::Min { .. })
                            || matches!(
                                method,
                                purecrate_ir::Consume::MaxByKey { .. } | purecrate_ir::Consume::MinByKey { .. }
                            ) && args.len() < 3
                        {
                            self.ord = true;
                        }
                    }
                    Callee::Int { ty, .. } => {
                        self.int = true;
                        self.nums.insert(ty.ts_name().to_string());
                    }
                    Callee::Fround => {
                        self.nums.insert("F32".into());
                        self.nums.insert("F64".into());
                    }
                    Callee::AsFloat(ft) => {
                        self.nums.insert(ft.ts_name().to_string());
                    }
                    Callee::VecLen => {
                        self.nums.insert("Usize".into());
                    }
                    Callee::StrBytes
                    | Callee::StrCmp
                    | Callee::Str(
                        purecrate_ir::StrMethod::StripPrefix
                        | purecrate_ir::StrMethod::StripSuffix
                        | purecrate_ir::StrMethod::SplitOnce
                        | purecrate_ir::StrMethod::EqIgnoreAsciiCase
                        | purecrate_ir::StrMethod::ToAsciiLowercase
                        | purecrate_ir::StrMethod::ToAsciiUppercase
                        | purecrate_ir::StrMethod::Trim
                        | purecrate_ir::StrMethod::TrimStart
                        | purecrate_ir::StrMethod::TrimEnd
                        | purecrate_ir::StrMethod::TrimMatches
                        | purecrate_ir::StrMethod::TrimStartMatches
                        | purecrate_ir::StrMethod::TrimEndMatches
                        | purecrate_ir::StrMethod::Find
                        | purecrate_ir::StrMethod::SplitWhitespace
                        | purecrate_ir::StrMethod::Lines,
                    )
                    | Callee::StrWellFormed
                    | Callee::StrSplitStr
                    // `Str.splitBy` for a closure; an unused import is pruned.
                    | Callee::StrSplit => self.str = true,
                    Callee::SliceElem { .. } | Callee::SliceRest { .. } => self.slice = true,
                    Callee::ArrayRepeat { .. } | Callee::SliceLengths { .. } | Callee::VecInsert | Callee::VecRemove | Callee::VecSet | Callee::VecGet | Callee::VecLast | Callee::VecContains | Callee::VecZip | Callee::VecRev => self.slice = true,
                    Callee::Tree(op) => {
                        use purecrate_ir::TreeOp as T;
                        match op {
                            T::Get | T::Insert | T::Add | T::RemoveKey | T::Entries | T::Keys | T::Values => {
                                self.btree = true
                            }
                            T::Lookup | T::Put | T::Include | T::Take | T::EntriesBy | T::KeysBy | T::ValuesBy => {
                                self.btree = true
                            }
                            T::Len => {
                                self.nums.insert("Usize".into());
                            }
                            _ => {}
                        }
                    }
                    Callee::Format(pieces) => {
                        self.str |= pieces
                            .iter()
                            .any(|p| matches!(p, purecrate_ir::FmtPiece::Arg { spec, .. } if spec.width.is_some()));
                    }
                    Callee::VecSort(sort) => {
                        self.slice = true;
                        if !matches!(sort, purecrate_ir::Sort::By) {
                            self.ord = true;
                        }
                    }
                    Callee::DeepEq => self.eq = true,
                    Callee::VecWindows { .. } | Callee::VecDedup => self.slice = true,
                    Callee::Range { .. } | Callee::RangeCut { .. } => self.iter = true,
                    Callee::Slice { of, start, .. } => {
                        let of_str = *of == Some(purecrate_ir::SliceOf::Str);
                        self.str |= of_str;
                        self.slice |= !of_str;
                        // An open start of a string prints as `(0 as Usize)`.
                        if !start && of_str {
                            self.nums.insert("Usize".into());
                        }
                    }
                    Callee::Str(purecrate_ir::StrMethod::Len) => {
                        self.str = true;
                        self.nums.insert("Usize".into());
                    }
                    Callee::IntFrom { to, .. } => {
                        self.nums.insert(to.ts_name().to_string());
                    }
                    Callee::IntCast { .. } => self.int = true,
                    Callee::Float { ty, m } => {
                        if matches!(m, purecrate_ir::FloatMethod::Round | purecrate_ir::FloatMethod::IsInfinite) {
                            self.int = true;
                        }
                        if !m.is_test() {
                            self.nums.insert(ty.ts_name().to_string());
                        }
                    }
                    Callee::FloatConst { ty, .. } => {
                        self.nums.insert(ty.ts_name().to_string());
                    }
                    Callee::FloatToInt { .. } => self.int = true,
                    Callee::IntToFloat { from, to } => {
                        if from.is_big() && *to == purecrate_ir::FloatTy::F32 {
                            self.int = true;
                        } else {
                            self.nums.insert(to.ts_name().to_string());
                        }
                    }
                    Callee::FloatToFloat { to } => {
                        self.nums.insert(to.ts_name().to_string());
                    }
                    Callee::CharCode(to) => {
                        self.char_value = true;
                        if to.is_big() {
                            self.nums.insert(to.ts_name().to_string());
                        }
                    }
                    Callee::CharFromU8 | Callee::CharFromU32 | Callee::Char(_) => self.char_value = true,
                    Callee::UuidParse | Callee::UuidNil => self.uuid_value = true,
                    Callee::StrParse(_) => self.int = true,
                    Callee::Discriminant { to, of, .. } => {
                        self.nums.insert(to.ts_name().to_string());
                        self.ty(&Ty::Named(of.clone()));
                    }
                    _ => {}
                }
            }
            Expr::Construct { ty, variant, .. } => {
                if variant.is_none() && closed_in(krate, ty.as_str()) {
                    self.ctors.insert(ty.as_str().to_string());
                }
            }
            Expr::ForEach { over, .. } => self.over(*over),
            Expr::Lit(lit) => match lit {
                purecrate_ir::Lit::Int { ty: Some(t), .. } => {
                    self.nums.insert(t.ts_name().to_string());
                }
                purecrate_ir::Lit::Float { ty: Some(t), .. } => {
                    self.nums.insert(t.ts_name().to_string());
                }
                purecrate_ir::Lit::Char(_) => self.char_type = true,
                _ => {}
            },
            Expr::Var(n) if is_const(krate, n.as_str()) => {
                self.values.insert(n.as_str().to_string());
            }
            // A type's companion as the dictionary a call passes.
            Expr::Var(n) if is_companion(krate, n.as_str()) => {
                self.values.insert(n.as_str().to_string());
            }
            // An associated const, from its type's file.
            Expr::Var(n) => match purecrate_ir::split_assoc_path(n.as_str()) {
                Some((ty, name)) if private_in(krate, ty, name) => {
                    self.privates.insert((ty.to_string(), name.to_string()));
                }
                Some((ty, _)) => {
                    self.values.insert(ty.to_string());
                }
                None => {}
            },
            Expr::Cast { .. } => unreachable!("`check::accept` rewrites `as`"),
            _ => {}
        }
        expr.own_types().into_iter().for_each(|t| self.ty(t));
        expr.children().into_iter().for_each(|c| self.expr(krate, c));
    }

    fn fn_sig_and_body(&mut self, krate: &Crate, f: &Fn) {
        f.params.iter().for_each(|p| self.ty(&p.ty));
        self.ty(&f.ret);
        // As printed: with the entry checks of a `pub` function's strings.
        self.expr(krate, &crate::expr::checked_strings(f).body);
    }
}

pub(crate) fn closed_in(krate: &Crate, name: &str) -> bool {
    krate.items.iter().any(|item| matches!(item, Item::Struct(st) if st.closed && st.name.as_str() == name))
}

/// `ty` has a method or an associated const `name` that is not `pub`.
pub(crate) fn private_in(krate: &Crate, ty: &str, name: &str) -> bool {
    krate.items.iter().any(|item| {
        item.vis() == Vis::Internal
            && matches!(item, Item::Fn(_) | Item::Const(_))
            && item.name().as_str() == name
            && item.owner().is_some_and(|o| o.as_str() == ty)
    })
}

/// A crate-level const (not an associated one) is named `name`.
pub(crate) fn is_const(krate: &Crate, name: &str) -> bool {
    krate.items.iter().any(|item| matches!(item, Item::Const(c) if c.owner.is_none() && c.name.as_str() == name))
}

/// A struct or enum of the crate is named `name`: its companion.
pub(crate) fn is_companion(krate: &Crate, name: &str) -> bool {
    krate.items.iter().any(|item| matches!(item, Item::Struct(_) | Item::Enum(_)) && item.name().as_str() == name)
}

pub(crate) fn is_free_fn(krate: &Crate, name: &str) -> bool {
    krate.items.iter().any(|item| matches!(item, Item::Fn(f) if f.owner.is_none() && f.name.as_str() == name))
}

fn refs_of(krate: &Crate, items: &[&Item]) -> Refs {
    let mut refs = Refs::default();
    for item in items {
        match item {
            Item::Struct(st) => {
                st.fields.iter().for_each(|f| refs.ty(&f.ty));
                methods_on(krate, st.name.as_str()).into_iter().for_each(|m| refs.fn_sig_and_body(krate, m));
            }
            Item::Enum(en) => {
                for v in &en.variants {
                    match &v.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(tys) => tys.iter().for_each(|t| refs.ty(t)),
                        VariantFields::Struct(fs) => fs.iter().for_each(|f| refs.ty(&f.ty)),
                    }
                }
                methods_on(krate, en.name.as_str()).into_iter().for_each(|m| refs.fn_sig_and_body(krate, m));
            }
            Item::Alias(al) => refs.ty(&al.ty),
            Item::Trait(t) => {
                t.supertraits.iter().for_each(|s| refs.ty(&Ty::Named(s.clone())));
                for m in &t.methods {
                    m.params.iter().for_each(|p| refs.ty(&p.ty));
                    refs.ty(&m.ret);
                }
                t.consts.iter().for_each(|c| refs.ty(&c.ty));
                methods_on(krate, t.name.as_str()).into_iter().for_each(|m| refs.fn_sig_and_body(krate, m));
            }
            Item::Const(c) => {
                refs.ty(&c.ty);
                refs.expr(krate, &c.value);
            }
            Item::Fn(f) if f.owner.is_none() => refs.fn_sig_and_body(krate, f),
            Item::Fn(_) => {}
        }
    }
    refs
}

pub(crate) fn imports_for(krate: &Crate, stem: &str, items: &[&Item]) -> String {
    let refs = refs_of(krate, items);
    // A const lives in `consts.ts`, not in a file named after it; a helper
    // with one user file lives in that file (`Crate::homes`).
    let file_of = |name: &String| {
        if is_const(krate, name) {
            purecrate_ir::CONSTS_STEM.to_string()
        } else if let Some(host) = crate::hosted_in(name) {
            host
        } else {
            Name::new(name.clone()).file_stem()
        }
    };
    let elsewhere = |name: &String| file_of(name) != stem;
    let mut out = String::new();
    // Everything from the runtime in one import, values then types; `pack`
    // points `"purecrate"` at the package's copy.
    let values = [
        // Folding may print a `switch` the IR did not hold; unread, it is
        // pruned (`prune_unused`).
        Some("assertNever"),
        Some("Panic"),
        refs.int.then_some("Int"),
        refs.result_value.then_some("Result"),
        refs.char_value.then_some("Char"),
        refs.uuid_value.then_some("Uuid"),
        refs.str.then_some("Str"),
        refs.slice.then_some("Slice"),
        refs.eq.then_some("Eq"),
        refs.ord.then_some("Ord"),
        refs.iter.then_some("Iter"),
        refs.btree.then_some("BTree"),
    ];
    let types = [
        (refs.result_type && !refs.result_value).then_some("Result"),
        (refs.char_type && !refs.char_value).then_some("Char"),
        (refs.uuid_type && !refs.uuid_value).then_some("Uuid"),
        refs.uuid_error.then_some("UuidError"),
        refs.parse_int_error.then_some("ParseIntError"),
    ];
    // `Ord` the value is `Ord` the type too.
    let std_traits = refs.std_traits.iter().map(String::as_str).filter(|t| *t != "Ord" || !refs.ord);
    let runtime: Vec<String> = values
        .into_iter()
        .flatten()
        .map(str::to_string)
        .chain(
            types
                .into_iter()
                .flatten()
                .chain(std_traits)
                .chain(refs.nums.iter().map(String::as_str))
                .map(|t| format!("type {t}")),
        )
        .collect();
    if !runtime.is_empty() {
        out.push_str(&format!("import {{ {} }} from \"purecrate\";\n", runtime.join(", ")));
    }
    // Each name from another file of the crate: the keyword, the name, and
    // the file's stem.
    let stem_of = |name: &String| Name::new(name.clone()).file_stem();
    let own = (refs.values.iter().filter(|v| elsewhere(v)).map(|v| ("import", v.clone(), file_of(v))))
        .chain(
            (refs.privates.iter().filter(|(ty, _)| elsewhere(ty)))
                .map(|(ty, name)| ("import", private_method(ty, name), stem_of(ty))),
        )
        .chain(refs.ctors.iter().filter(|c| elsewhere(c)).map(|c| ("import", closed_ctor(c), stem_of(c))))
        .chain(
            (refs.types.iter().filter(|t| elsewhere(t) && !refs.values.contains(*t)))
                .map(|t| ("import type", t.clone(), stem_of(t))),
        );
    for (keyword, name, stem) in own {
        out.push_str(&format!("{keyword} {{ {name} }} from \"./{stem}.ts\";\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_substitutions_are_code() {
        let used = code_idents("const s = `a ${Json.str(`b ${Inner} {`)} c { ${ { k: Last } } }` + After; `${Unused`;");
        for name in ["Json", "Inner", "Last", "After", "s", "k"] {
            assert!(used.contains(name), "{name}: {used:?}");
        }
        let used = code_idents("const t = `Text only ${1} Word`;");
        assert!(!used.contains("Text") && !used.contains("Word"), "{used:?}");
    }

    #[test]
    fn one_declaration_per_module_types_last() {
        let src = "import { Result, type I64, Int } from \"purecrate\";\n\
            import { unsafeMakeYen } from \"./yen.ts\";\n\
            import type { Lines } from \"./lines.ts\";\n\
            import type { Yen } from \"./yen.ts\";\n\
            import { type U8 } from \"./u8.ts\";\n\
            \n\
            const f = (l: Lines, y: Yen, b: U8): I64 => Int.i64.add(unsafeMakeYen(y), Result);\n";
        assert_eq!(
            prune_unused(src),
            "import { Int, Result, type I64 } from \"purecrate\";\n\
            import type { Lines } from \"./lines.ts\";\n\
            import type { U8 } from \"./u8.ts\";\n\
            import { unsafeMakeYen, type Yen } from \"./yen.ts\";\n\
            \n\
            const f = (l: Lines, y: Yen, b: U8): I64 => Int.i64.add(unsafeMakeYen(y), Result);\n"
        );
    }

    #[test]
    fn result_read_only_as_a_type_is_a_type_import() {
        // A fold took every `Result.ok` away; `xResult.kind` is a local.
        let src = "import { Result, type I32 } from \"purecrate\";\n\nconst f = (r: Result<I32, I32>): I32 => { const xResult = r; return xResult.kind === \"Ok\" ? xResult.value : 0; };\n";
        assert!(prune_unused(src).starts_with("import type { I32, Result }"), "{}", prune_unused(src));
    }

    #[test]
    fn a_name_read_only_as_a_type_is_a_type_import() {
        let types_only = [
            "const n = Iter.count(s as Iterable<Char>);",
            "const f = (c: Char): Char => c;",
            "let m: ReadonlyMap<U8, Char> = new Map();",
            "const p = (x: readonly [Char, Char]): boolean => x[0] === x[1];",
            "const o = (x: null | Char): boolean => x === null;",
            "const k = \"a\" as Char;",
            "const r = (r: Result<I32, I32>): I32 => (r.kind === \"Ok\" ? r.value : 0);",
            "const s = (x?: Char): I32 => 0;",
        ];
        for code in types_only {
            let src = format!("import {{ Char, Result, Int }} from \"purecrate\";\n\n{code} Int.x;\n");
            let out = prune_unused(&src);
            assert!(!out.contains("{ Char") && !out.contains(" Char,") && !out.contains(", Result,"), "{code}\n{out}");
        }
        let values = [
            "const c = Char.code(x);",
            "const p = pair(i, Char, Coin);",
            "const p = Named.label(Char, self);",
            "const v = c ? x : Char;",
            "const v = { k: Char, j: 1 };",
            "const v = x instanceof Char;",
            "const v = (): unknown => Char;",
            "const v = [x, Char];",
            "return Char;",
        ];
        for code in values {
            let src = format!("import {{ Char }} from \"purecrate\";\n\nconst t = (x: Char): I32 => 0; {code}\n");
            assert!(prune_unused(&src).starts_with("import { Char }"), "{code}\n{}", prune_unused(&src));
        }
        // An alias, a crate type, and a const, which is never a type.
        let src = "import { Yen as DomainYen } from \"./yen.ts\";\n\
            import { P } from \"./p.ts\";\n\
            import { K } from \"./consts.ts\";\n\
            \n\
            const f = (y: DomainYen, p: P): boolean => p < K;\n";
        let out = prune_unused(src);
        assert!(out.contains("import type { Yen as DomainYen }"), "{out}");
        assert!(out.contains("import type { P }") && out.contains("import { K }"), "{out}");
    }

    #[test]
    fn keeps_what_the_code_reads() {
        let src = "/* generated */\n\
            import { Int, type U32, type F64 } from \"./int.ts\";\n\
            import type { Order } from \"./order.ts\";\n\
            import { A as A$value, type A as A$ } from \"./a.ts\";\n\
            import * as v from \"valibot\";\n\
            \n\
            // F64 in a comment\n\
            export const f = (o: Order): A$ => Int.u32.add(o.F64, \"U32\" as never);\n";
        assert_eq!(
            prune_unused(src),
            "/* generated */\n\
            import * as v from \"valibot\";\n\
            import type { A as A$ } from \"./a.ts\";\n\
            import { Int } from \"./int.ts\";\n\
            import type { Order } from \"./order.ts\";\n\
            \n\
            // F64 in a comment\n\
            export const f = (o: Order): A$ => Int.u32.add(o.F64, \"U32\" as never);\n"
        );
    }
}
