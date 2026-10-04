//! Blank lines between the statements of a block, so a body reads in
//! paragraphs. Statements of one line run on together; a statement over
//! several lines (one holding a block, or what one Rust statement became,
//! `let x: T;` and the `if` that assigns it) stands apart, with a blank
//! line before and after it, but keeps the one-line `const` its first line
//! reads (`const m = next(cmd);` then `switch (m.kind) {`, what a `match`
//! on a call became); a `//` comment heads a paragraph, with a blank
//! line before it. Never at a block's start or end, nor among `switch`
//! cases. Runs before `tidy::wrap`, so a line it breaks counts as one.

/// `src` with a blank line between the statements of each code block where
/// the rule above asks for one.
pub(crate) fn paragraphs(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut blank_before = vec![false; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        if opens_block(line) {
            mark(&lines, i, &mut blank_before);
        }
    }
    let mut out = String::with_capacity(src.len() + lines.len());
    for (line, blank) in lines.iter().zip(blank_before) {
        if blank {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A line opening a block of statements: a function body, an `if`, `else`,
/// loop, or `case` body. An object literal (`= {`, `({`, `key: {`) is not.
fn opens_block(line: &str) -> bool {
    let t = line.trim();
    t.ends_with("=> {")
        || t.ends_with(") {")
        || t.ends_with("else {")
        || t == "{"
        || (t.starts_with("case ") || t.starts_with("default:")) && t.ends_with(": {")
}

/// Marks the statements of the block opened on line `open`.
fn mark(lines: &[&str], open: usize, blank_before: &mut [bool]) {
    if lines[open].trim_start().starts_with("switch ") {
        return;
    }
    let inner = indent(lines[open]) + 2;
    // Each statement: its first line, its first line of code, the line
    // after the `let`s it opens with, its end, and whether a comment heads it.
    let mut stmts: Vec<Stmt> = Vec::new();
    let mut i = open + 1;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if indent(line) < inner {
            break;
        }
        let start = i;
        let mut commented = false;
        while i < lines.len() && indent(lines[i]) == inner && lines[i].trim_start().starts_with("//") {
            commented = true;
            i += 1;
        }
        let code = i;
        // A `let` without a value belongs with what assigns it.
        while i < lines.len() && indent(lines[i]) == inner && declares_only(lines[i]) {
            i += 1;
        }
        let main = i;
        i += 1;
        // Deeper lines, and the `}` lines that close what this one opened.
        while i < lines.len() && (indent(lines[i]) > inner || indent(lines[i]) == inner && closes(lines[i])) {
            i += 1;
        }
        stmts.push(Stmt { start, code, main, end: i, commented });
    }
    for pair in stmts.windows(2) {
        let (prev, here) = (&pair[0], &pair[1]);
        let apart = prev.lines() > 1 || here.lines() > 1 || here.commented;
        // The binding this statement's first line reads stays with it.
        let reads_prev = !here.commented
            && prev.end - prev.main == 1
            && bound(lines[prev.main]).into_iter().any(|name| reads(lines[here.code], name));
        if apart && !reads_prev && !jumps(lines[here.code]) {
            blank_before[here.start] = true;
        }
    }
}

struct Stmt {
    start: usize,
    code: usize,
    main: usize,
    end: usize,
    commented: bool,
}

impl Stmt {
    /// Lines of code, the comment above left out.
    fn lines(&self) -> usize {
        self.end - self.code
    }
}

/// `break;` or `continue;`, which ends what is above it.
fn jumps(line: &str) -> bool {
    matches!(line.trim(), "break;" | "continue;")
}

/// The names a one-line `const x = ..;`, `let x = ..;`, or a destructuring
/// `const { a, b } = ..;` / `const [a, b] = ..;` binds.
fn bound(line: &str) -> Vec<&str> {
    let t = line.trim();
    let Some(rest) = t.strip_prefix("const ").or_else(|| t.strip_prefix("let ")).filter(|_| t.ends_with(';')) else {
        return Vec::new();
    };
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    // `const { a, b } = ..` and `const [a, b] = ..` bind each name.
    let names = match rest.strip_prefix(['{', '[']) {
        Some(inner) => inner.split_once(['}', ']']).map_or("", |(names, _)| names),
        None => rest.split(|c: char| !word(c)).next().unwrap_or(""),
    };
    names.split(',').map(str::trim).filter(|n| !n.is_empty() && n.chars().all(word)).collect()
}

/// Whether `line` reads `name` as a whole word, not as a property.
fn reads(line: &str, name: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    line.match_indices(name).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + name.len()..].chars().next();
        !before.is_some_and(|c| word(c) || c == '.') && !after.is_some_and(word)
    })
}

/// `let x: T;`, a binding the next statement assigns.
fn declares_only(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("let ") && t.ends_with(';') && !t.contains(" = ")
}

/// A line that ends or continues the statement above it at its indent:
/// `}`, `} else {`, `});`, `]`, `)`.
fn closes(line: &str) -> bool {
    matches!(line.trim_start().as_bytes().first(), Some(b'}' | b']' | b')'))
}

#[cfg(test)]
mod tests {
    use super::paragraphs;

    #[test]
    fn a_statement_over_lines_stands_apart() {
        let src = "const f = (a: I32): I32 => {
  if (a === 0) return 1;
  const b = a;
  let c: I32;
  if (b > 1) {
    c = 2;
  } else {
    c = 3;
  }
  const d = c;
  // Why e.
  const e = d;
  const m = next(e);
  switch (m.kind) {
    case \"A\":
      return 1;
  }
};
";
        assert_eq!(
            paragraphs(src),
            "const f = (a: I32): I32 => {
  if (a === 0) return 1;
  const b = a;

  let c: I32;
  if (b > 1) {
    c = 2;
  } else {
    c = 3;
  }

  const d = c;

  // Why e.
  const e = d;
  const m = next(e);
  switch (m.kind) {
    case \"A\":
      return 1;
  }
};
"
        );
    }

    #[test]
    fn objects_and_switches_run_on() {
        let src = "const g = (s: S): R => {
  switch (s.kind) {
    case \"A\": {
      return 1;
    }
    case \"B\": {
      return 2;
    }
  }
};
export const T = {
  a: (x: I32): I32 => {
    return x;
  },
  b: 1,
} as const;
";
        assert_eq!(paragraphs(src), src);
    }
}
