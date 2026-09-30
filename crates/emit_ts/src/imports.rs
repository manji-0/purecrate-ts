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
