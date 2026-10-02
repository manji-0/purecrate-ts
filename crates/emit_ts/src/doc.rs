//! A layout document and its printer, the model oxfmt (and Prettier before
//! it) formats with: text, groups that print flat when they fit on the line
//! and break every line of theirs otherwise, indentation, a choice of
//! layouts tried in order, and an assignment that breaks after its `=` only
//! where its value cannot start on the line. `js` builds documents for the
//! wire module; this module knows nothing of TypeScript.

/// A layout. Build with the functions below; `group` marks itself broken
/// when it holds a hard line, so the groups around one break too.
#[derive(Clone, Debug)]
pub(crate) enum Doc {
    Text(String),
    Concat(Vec<Doc>),
    /// Flat when it fits, else broken; `broken` forces the latter.
    Group { doc: Box<Doc>, broken: bool },
    Indent(Box<Doc>),
    /// A space flat, a newline broken.
    Line,
    /// Nothing flat, a newline broken.
    SoftLine,
    /// Always a newline; breaks every enclosing group.
    HardLine,
    /// Nothing; breaks every enclosing group, as a hard line would.
    BreakParent,
    /// `broken` in a broken group, `flat` in a flat one.
    IfBreak { broken: Box<Doc>, flat: Box<Doc> },
    /// Layouts tried in order, each printed flat (its broken groups still
    /// break) where it fits up to its first newline; else the last, broken.
    /// A hard line inside does not break the groups around it.
    Conditional(Vec<Doc>),
    /// `lhs = rhs`, the `=` already in `lhs`: `rhs` on the line where it fits
    /// up to its first possible break, else on the next line, indented.
    Assign { lhs: Box<Doc>, rhs: Box<Doc> },
}

pub(crate) fn text(s: impl Into<String>) -> Doc {
    Doc::Text(s.into())
}

pub(crate) fn concat(docs: Vec<Doc>) -> Doc {
    Doc::Concat(docs)
}

pub(crate) fn group(doc: Doc) -> Doc {
    let broken = has_hard(&doc);
    Doc::Group { doc: Box::new(doc), broken }
}

pub(crate) fn broken_group(doc: Doc) -> Doc {
    Doc::Group { doc: Box::new(doc), broken: true }
}

pub(crate) fn indent(doc: Doc) -> Doc {
    Doc::Indent(Box::new(doc))
}

pub(crate) fn if_break(broken: Doc, flat: Doc) -> Doc {
    Doc::IfBreak { broken: Box::new(broken), flat: Box::new(flat) }
}

pub(crate) fn assign(lhs: Doc, rhs: Doc) -> Doc {
    Doc::Assign { lhs: Box::new(lhs), rhs: Box::new(rhs) }
}

pub(crate) fn join(sep: Doc, docs: Vec<Doc>) -> Doc {
    let mut out = Vec::with_capacity(docs.len() * 2);
    for (i, d) in docs.into_iter().enumerate() {
        if i > 0 {
            out.push(sep.clone());
        }
        out.push(d);
    }
    Doc::Concat(out)
}

/// Whether `doc` holds a hard line that breaks the groups around it: one
/// outside a choice of layouts, which decides for itself.
pub(crate) fn has_hard(doc: &Doc) -> bool {
    match doc {
        Doc::HardLine | Doc::BreakParent => true,
        Doc::Group { doc, broken } => *broken || has_hard(doc),
        Doc::Concat(ds) => ds.iter().any(has_hard),
        Doc::Indent(d) => has_hard(d),
        Doc::IfBreak { broken, flat } => has_hard(broken) || has_hard(flat),
        Doc::Assign { lhs, rhs } => has_hard(lhs) || has_hard(rhs),
        Doc::Text(_) | Doc::Line | Doc::SoftLine | Doc::Conditional(_) => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Flat,
    Break,
}

type Cmd<'d> = (usize, Mode, &'d Doc);

/// `doc` printed at `width` columns from column 0, at `level` indents of two
/// spaces. Lines carry no trailing spaces.
pub(crate) fn print(doc: &Doc, width: usize, level: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    run(width, &mut out, &mut col, vec![(level, Mode::Break, doc)]);
    out.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

fn newline(out: &mut String, col: &mut usize, level: usize) {
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('\n');
    let pad = "  ".repeat(level);
    out.push_str(&pad);
    *col = pad.len();
}

static LINE: Doc = Doc::Line;
static HARD: Doc = Doc::HardLine;

/// Prints the commands, a stack whose top is printed first. A hard line
/// printed inside flat content makes the next group measure itself again,
/// as Prettier's printer does, rather than stay flat with its parent.
fn run(width: usize, out: &mut String, col: &mut usize, cmds: Vec<Cmd>) {
    let mut stack = cmds;
    let mut remeasure = false;
    while let Some((level, mode, doc)) = stack.pop() {
        match doc {
            Doc::Text(s) => {
                out.push_str(s);
                *col += s.chars().count();
            }
            Doc::Concat(ds) => stack.extend(ds.iter().rev().map(|d| (level, mode, d))),
            Doc::Indent(d) => stack.push((level + 1, mode, d)),
            Doc::Line => match mode {
                Mode::Flat => {
                    out.push(' ');
                    *col += 1;
                }
                Mode::Break => newline(out, col, level),
            },
            Doc::SoftLine => {
                if mode == Mode::Break {
                    newline(out, col, level);
                }
            }
            Doc::HardLine => {
                if mode == Mode::Flat {
                    remeasure = true;
                }
                newline(out, col, level);
            }
            Doc::BreakParent => {}
            Doc::IfBreak { broken, flat } => stack.push((level, mode, if mode == Mode::Break { broken } else { flat })),
            Doc::Group { doc, broken } => {
                // A broken group breaks even inside a flat one.
                let measure = mode == Mode::Break || remeasure;
                if mode == Mode::Flat && remeasure {
                    remeasure = false;
                }
                let flat = !broken
                    && (!measure || fits(&[(level, Mode::Flat, &**doc)], &stack, width.saturating_sub(*col)));
                stack.push((level, if flat { Mode::Flat } else { Mode::Break }, doc));
            }
            Doc::Conditional(states) => {
                if mode == Mode::Flat && !remeasure {
                    stack.push((level, Mode::Flat, &states[0]));
                    continue;
                }
                remeasure = false;
                let room = width.saturating_sub(*col);
                let last = states.len() - 1;
                match (0..last).find(|&i| fits(&[(level, Mode::Flat, &states[i])], &stack, room)) {
                    Some(i) => stack.push((level, Mode::Flat, &states[i])),
                    None => stack.push((level, Mode::Break, &states[last])),
                }
            }
            Doc::Assign { lhs, rhs } => {
                // The value starts on the `=` line where it fits there up to
                // its first possible break; else on the next, one indent in.
                // Flat, it stays on the line.
                let head = [(level, mode, &**lhs), (level, Mode::Flat, &LINE), (level, Mode::Break, &**rhs)];
                if (mode == Mode::Flat && !remeasure) || fits(&head, &stack, width.saturating_sub(*col)) {
                    stack.push((level, mode, rhs));
                    stack.push((level, Mode::Flat, &LINE));
                } else {
                    stack.push((level + 1, mode, rhs));
                    stack.push((level + 1, Mode::Break, &HARD));
                }
                stack.push((level, mode, lhs));
            }
        }
    }
}

/// Whether the commands fit in `room` columns up to their first newline,
/// then the rest of the stack (`rest`, top last) in its own modes. `next`
/// is printed first to last.
fn fits(next: &[Cmd], rest: &[Cmd], room: usize) -> bool {
    let mut room = room as isize;
    let mut cmds: Vec<(Mode, &Doc)> = next.iter().rev().map(|(_, m, d)| (*m, *d)).collect();
    let mut rest_at = rest.len();
    loop {
        let (mode, doc) = match cmds.pop() {
            Some(c) => c,
            None => {
                if rest_at == 0 {
                    return true;
                }
                rest_at -= 1;
                (rest[rest_at].1, rest[rest_at].2)
            }
        };
        match doc {
            Doc::Text(s) => {
                room -= s.chars().count() as isize;
                if room < 0 {
                    return false;
                }
            }
            Doc::Concat(ds) => cmds.extend(ds.iter().rev().map(|d| (mode, d))),
            Doc::Indent(d) => cmds.push((mode, d)),
            Doc::Line => match mode {
                Mode::Flat => {
                    room -= 1;
                    if room < 0 {
                        return false;
                    }
                }
                Mode::Break => return true,
            },
            Doc::SoftLine => {
                if mode == Mode::Break {
                    return true;
                }
            }
            Doc::HardLine => return true,
            Doc::BreakParent => {}
            Doc::IfBreak { broken, flat } => cmds.push((mode, if mode == Mode::Break { broken } else { flat })),
            Doc::Group { doc, broken } => cmds.push((if *broken { Mode::Break } else { mode }, doc)),
            Doc::Conditional(states) => {
                cmds.push(if mode == Mode::Break { (Mode::Break, states.last().expect("a layout")) } else { (mode, &states[0]) })
            }
            // Broken, its value may move to the next line: measured to the
            // `=`, then a newline.
            Doc::Assign { lhs, rhs } => {
                match mode {
                    Mode::Break => cmds.push((Mode::Break, &HARD)),
                    Mode::Flat => {
                        cmds.push((mode, rhs));
                        cmds.push((mode, &LINE));
                    }
                }
                cmds.push((mode, lhs));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: Vec<Doc>) -> Doc {
        group(concat(vec![
            text(format!("{name}(")),
            indent(concat(vec![Doc::SoftLine, join(concat(vec![text(","), Doc::Line]), args)])),
            if_break(text(","), text("")),
            Doc::SoftLine,
            text(")"),
        ]))
    }

    #[test]
    fn a_group_breaks_only_when_it_does_not_fit() {
        let d = call("f", vec![text("aaaa"), text("bbbb")]);
        assert_eq!(print(&d, 20, 0), "f(aaaa, bbbb)");
        assert_eq!(print(&d, 10, 0), "f(\n  aaaa,\n  bbbb,\n)");
    }

    #[test]
    fn a_hard_line_breaks_its_groups() {
        let d = call("f", vec![concat(vec![text("{"), indent(concat(vec![Doc::HardLine, text("x")])), Doc::HardLine, text("}")])]);
        assert_eq!(print(&d, 80, 0), "f(\n  {\n    x\n  },\n)");
    }

    #[test]
    fn a_conditional_takes_the_first_layout_that_fits() {
        let d = Doc::Conditional(vec![text("a very long layout"), text("short")]);
        assert_eq!(print(&d, 10, 0), "short");
        assert_eq!(print(&d, 40, 0), "a very long layout");
    }

    #[test]
    fn an_assignment_breaks_after_its_operator_only_when_the_value_cannot_start() {
        let d = assign(text("const x ="), call("f", vec![text("aaaa"), text("bbbb")]));
        assert_eq!(print(&d, 40, 0), "const x = f(aaaa, bbbb)");
        assert_eq!(print(&d, 16, 0), "const x = f(\n  aaaa,\n  bbbb,\n)");
        assert_eq!(print(&assign(text("const x ="), text("longvalue")), 15, 0), "const x =\n  longvalue");
    }
}
