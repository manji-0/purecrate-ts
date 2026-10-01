//! Drops the imports a generated file does not use, so it passes a
//! consumer's `noUnusedLocals`. The import lists are gathered from the IR
//! ahead of printing and may name more than the printed code reads; this
//! pass decides from the printed code itself.

use std::collections::BTreeSet;

/// `src` with every `import { .. }` name that the rest of the file does not
/// mention removed, and imports left empty dropped. Other lines are kept.
pub fn prune_unused(src: &str) -> String {
    let used = code_idents(&src.lines().filter(|l| !l.starts_with("import ")).collect::<Vec<_>>().join("\n"));
    let mut out = String::with_capacity(src.len());
    for line in src.split_inclusive('\n') {
        match prune_line(line.trim_end_matches('\n'), &used) {
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
    out
}

/// `None` drops the line. Lines that are not a named import are returned
/// unchanged.
fn prune_line(line: &str, used: &BTreeSet<String>) -> Option<String> {
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
    let kept: Vec<&str> = rest[open + 1..close]
        .split(',')
        .map(str::trim)
        .filter(|spec| !spec.is_empty())
        .filter(|spec| used.contains(local_name(spec)))
        .collect();
    if kept.is_empty() {
        return None;
    }
    let from = &rest[close + 1..];
    let kw = if type_only { "import type" } else { "import" };
    Some(format!("{kw} {{ {} }}{from}", kept.join(", ")))
}

/// The binding an import specifier introduces: `type A as B` binds `B`.
fn local_name(spec: &str) -> &str {
    let spec = spec.strip_prefix("type ").unwrap_or(spec);
    spec.rsplit(" as ").next().unwrap_or(spec).trim()
}

/// Identifiers in TS source, outside comments and string literals, and not
/// after a `.` (a property name is not a reference to a binding). The
/// `${..}` of a template literal is code.
fn code_idents(src: &str) -> BTreeSet<String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = BTreeSet::new();
    // Open template literals, each with the brace depth of the code in its
    // current `${..}`.
    let mut templates: Vec<usize> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let mut after_dot = false;
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
            '{' => depth += 1,
            '}' if depth == 0 && !templates.is_empty() => {
                i = skip_template_text(&chars, i + 1, &mut templates, &mut depth);
                after_dot = false;
                continue;
            }
            '}' => depth = depth.saturating_sub(1),
            _ if c.is_ascii_alphabetic() || c == '_' || c == '$' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                    i += 1;
                }
                if !after_dot {
                    out.insert(chars[start..i].iter().collect());
                }
                after_dot = false;
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
        i += 1;
    }
    out
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
    result: bool,
    /// `Str` from `./str.ts`.
    str: bool,
    /// `Slice` from `./str.ts`, for indexing and slicing a `Vec`.
    slice: bool,
    /// `Ord` from `./str.ts`, for `cmp` and `Ordering::then`.
    ord: bool,
    /// `Iter` from `./str.ts`, for `all`, `any`, `position`, `count`, `sum`.
    iter: bool,
    /// The `Char` runtime, and the `Char` type, from `./str.ts`.
    char_value: bool,
    char_type: bool,
    /// The `Uuid` runtime, and the `Uuid` / `UuidError` types, from `./str.ts`.
    uuid_value: bool,
    uuid_type: bool,
    uuid_error: bool,
    /// Brand type names (`I32`, `F64`) this file mentions.
    nums: BTreeSet<String>,
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
            Ty::Named(n) => {
                self.types.insert(n.as_str().to_string());
            }
            Ty::Result { ok, err } => {
                self.result = true;
                self.ty(ok);
                self.ty(err);
            }
            Ty::Option(inner) | Ty::Vec(inner) | Ty::Ignored { inner, .. } => self.ty(inner),
            Ty::Tuple(elems) => elems.iter().for_each(|t| self.ty(t)),
            Ty::Fn { params, ret } => {
                params.iter().for_each(|t| self.ty(t));
                self.ty(ret);
            }
            Ty::Prim(purecrate_ir::Prim::Char) => self.char_type = true,
            Ty::Prim(purecrate_ir::Prim::Uuid) => self.uuid_type = true,
            Ty::Prim(purecrate_ir::Prim::UuidError) => self.uuid_error = true,
            Ty::Prim(p) => {
                if let Some(name) = p.int().map(|t| t.ts_name()).or_else(|| p.float().map(|t| t.ts_name())) {
                    self.nums.insert(name.to_string());
                }
            }
            Ty::Never => {}
        }
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

    /// What the printed code of `expr` will import; everything else is
    /// walked through.
    fn expr(&mut self, krate: &Crate, expr: &Expr) {
        match expr {
            Expr::Match { scrutinee, arms } => {
                self.never |= arms
                    .iter()
                    .any(|a| matches!(a.pattern, Pattern::Variant { .. }) || (matches!(a.pattern, Pattern::Or(_)) && !a.pattern.is_lit_case()));
                self.arm_literals(arms.iter().map(|a| &a.pattern));
                if !is_place(scrutinee) {
                    if let Some(ty) = scrutinee_ty(arms) {
                        self.types.insert(ty.as_str().to_string());
                    }
                }
            }
            Expr::Unreachable => self.never = true,
            Expr::Index { .. } => self.slice = true,
            Expr::Call { callee, .. } => {
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
                    Callee::Method { ty, .. } | Callee::Variant { ty, .. } | Callee::StructNew(ty) => {
                        self.values.insert(ty.as_str().to_string());
                    }
                    Callee::ResultOk | Callee::ResultErr => self.result = true,
                    Callee::OrdCmp { .. } | Callee::OrdThen => self.ord = true,
                    Callee::Consume { method, over } => {
                        self.iter = true;
                        match over {
                            purecrate_ir::Over::Chars => self.char_type = true,
                            purecrate_ir::Over::Bytes => self.str = true,
                            purecrate_ir::Over::Items => {}
                        }
                        if let purecrate_ir::Consume::Sum(int) = method {
                            self.int = true;
                            self.nums.insert(int.ts_name().to_string());
                        }
                        if matches!(method, purecrate_ir::Consume::Position | purecrate_ir::Consume::Count) {
                            self.nums.insert("Usize".into());
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
                    | Callee::Str(purecrate_ir::StrMethod::StripPrefix | purecrate_ir::StrMethod::StripSuffix) => {
                        self.str = true
                    }
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
                    Callee::CharCode(to) => {
                        self.char_value = true;
                        if to.is_big() {
                            self.nums.insert(to.ts_name().to_string());
                        }
                    }
                    Callee::CharFromU8 | Callee::CharFromU32 | Callee::Char(_) => self.char_value = true,
                    Callee::UuidParse | Callee::UuidNil => self.uuid_value = true,
                    Callee::Discriminant { to, .. } => {
                        self.nums.insert(to.ts_name().to_string());
                    }
                    _ => {}
                }
            }
            Expr::Construct { ty, variant, .. } => {
                if variant.is_none() && closed_in(krate, ty.as_str()) {
                    self.ctors.insert(ty.as_str().to_string());
                }
            }
            Expr::ForEach { over, .. } => match over {
                purecrate_ir::Over::Chars => self.char_type = true,
                purecrate_ir::Over::Bytes => self.str = true,
                purecrate_ir::Over::Items => {}
            },
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
            Expr::Cast { .. } => unreachable!("`check::accept` rewrites `as`"),
            _ => {}
        }
        expr.own_types().into_iter().for_each(|t| self.ty(t));
        expr.children().into_iter().for_each(|c| self.expr(krate, c));
    }

    fn fn_sig_and_body(&mut self, krate: &Crate, f: &Fn) {
        f.params.iter().for_each(|p| self.ty(&p.ty));
        self.ty(&f.ret);
        self.expr(krate, &f.body);
    }
}

pub(crate) fn closed_in(krate: &Crate, name: &str) -> bool {
    krate
        .items
        .iter()
        .any(|item| matches!(item, Item::Struct(st) if st.closed && st.name.as_str() == name))
}

pub(crate) fn private_in(krate: &Crate, ty: &str, name: &str) -> bool {
    krate.items.iter().any(|item| {
        matches!(item, Item::Fn(f) if f.vis == Vis::Internal
            && f.name.as_str() == name
            && f.owner.as_ref().is_some_and(|o| o.as_str() == ty))
    })
}

pub(crate) fn is_const(krate: &Crate, name: &str) -> bool {
    krate.items.iter().any(|item| matches!(item, Item::Const(c) if c.name.as_str() == name))
}

pub(crate) fn is_free_fn(krate: &Crate, name: &str) -> bool {
    krate
        .items
        .iter()
        .any(|item| matches!(item, Item::Fn(f) if f.owner.is_none() && f.name.as_str() == name))
}

pub(crate) fn imports_for(krate: &Crate, stem: &str, items: &[&Item]) -> String {
    let mut refs = Refs::default();
    for item in items {
        match item {
            Item::Struct(st) => {
                st.fields.iter().for_each(|f| refs.ty(&f.ty));
                methods_on(krate, st.name.as_str())
                    .into_iter()
                    .for_each(|m| refs.fn_sig_and_body(krate, m));
            }
            Item::Enum(en) => {
                for v in &en.variants {
                    match &v.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(tys) => tys.iter().for_each(|t| refs.ty(t)),
                        VariantFields::Struct(fs) => fs.iter().for_each(|f| refs.ty(&f.ty)),
                    }
                }
                methods_on(krate, en.name.as_str())
                    .into_iter()
                    .for_each(|m| refs.fn_sig_and_body(krate, m));
            }
            Item::Alias(al) => refs.ty(&al.ty),
            Item::Const(c) => {
                refs.ty(&c.ty);
                refs.expr(krate, &c.value);
            }
            Item::Fn(f) if f.owner.is_none() => refs.fn_sig_and_body(krate, f),
            Item::Fn(_) => {}
        }
    }

    // A const lives in `consts.ts`, not in a file named after it.
    let file_of = |name: &String| {
        if is_const(krate, name) {
            purecrate_ir::CONSTS_STEM.to_string()
        } else {
            Name::new(name.clone()).file_stem()
        }
    };
    let elsewhere = |name: &String| file_of(name) != stem;
    let mut out = String::new();
    if refs.never {
        out.push_str("import { assertNever } from \"./assert-never.ts\";\n");
    }
    if refs.int || !refs.nums.is_empty() {
        let types = refs.nums.iter().map(|n| format!("type {n}")).collect::<Vec<_>>().join(", ");
        if refs.int && types.is_empty() {
            out.push_str("import { Int } from \"./int.ts\";\n");
        } else if refs.int {
            out.push_str(&format!("import {{ Int, {types} }} from \"./int.ts\";\n"));
        } else {
            out.push_str(&format!("import {{ {types} }} from \"./int.ts\";\n"));
        }
    }
    if refs.result {
        out.push_str("import { Result } from \"./result.ts\";\n");
    }
    let str_names = [
        refs.char_value.then_some("Char"),
        (refs.char_type && !refs.char_value).then_some("type Char"),
        refs.uuid_value.then_some("Uuid"),
        (refs.uuid_type && !refs.uuid_value).then_some("type Uuid"),
        refs.uuid_error.then_some("type UuidError"),
        refs.str.then_some("Str"),
        refs.slice.then_some("Slice"),
        refs.ord.then_some("Ord"),
        refs.iter.then_some("Iter"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !str_names.is_empty() {
        out.push_str(&format!("import {{ {} }} from \"./str.ts\";\n", str_names.join(", ")));
    }
    for v in refs.values.iter().filter(|v| elsewhere(v)) {
        out.push_str(&format!("import {{ {v} }} from \"./{s}.ts\";\n", s = file_of(v)));
    }
    for (ty, name) in refs.privates.iter().filter(|(ty, _)| elsewhere(ty)) {
        out.push_str(&format!(
            "import {{ {m} }} from \"./{s}.ts\";\n",
            m = private_method(ty, name),
            s = Name::new(ty.clone()).file_stem()
        ));
    }
    for c in refs.ctors.iter().filter(|c| elsewhere(c)) {
        out.push_str(&format!(
            "import {{ {ctor} }} from \"./{s}.ts\";\n",
            ctor = closed_ctor(c),
            s = Name::new(c.clone()).file_stem()
        ));
    }
    for t in refs
        .types
        .iter()
        .filter(|t| elsewhere(t) && !refs.values.contains(*t))
    {
        out.push_str(&format!(
            "import type {{ {t} }} from \"./{s}.ts\";\n",
            s = Name::new(t.clone()).file_stem()
        ));
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
            import { Int } from \"./int.ts\";\n\
            import type { Order } from \"./order.ts\";\n\
            import { type A as A$ } from \"./a.ts\";\n\
            import * as v from \"valibot\";\n\
            \n\
            // F64 in a comment\n\
            export const f = (o: Order): A$ => Int.u32.add(o.F64, \"U32\" as never);\n"
        );
    }
}
