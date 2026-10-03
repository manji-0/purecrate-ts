//! Consecutive reads of same-named fields of one place, `const a = x.a;`
//! `const b = x.b;`, as one destructuring `const { a, b } = x;`. The fields
//! are read in the same order, and nothing runs between them. Runs after
//! `plain::plain_names`, so a name that reads differently from its field
//! (`const secondFactor = event.second_factor;`, `request2`) stays apart.

/// `src` with each run of two or more such lines at one indent joined.
pub(crate) fn destructure(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < lines.len() {
        let Some((pad, place, name)) = field_read(lines[i]) else {
            out.push_str(lines[i]);
            out.push('\n');
            i += 1;
            continue;
        };
        let mut names = vec![name];
        let mut j = i + 1;
        while let Some((p, q, n)) = lines.get(j).and_then(|l| field_read(l)) {
            if p != pad || q != place {
                break;
            }
            names.push(n);
            j += 1;
        }
        if names.len() > 1 {
            out.push_str(&format!("{pad}const {{ {} }} = {place};\n", names.join(", ")));
        } else {
            out.push_str(lines[i]);
            out.push('\n');
        }
        i = j;
    }
    out
}

/// `(indent, place, name)` for `const name = place.name;`, where `place` is
/// a name or a chain of fields (`flow`, `intent.status`) whose root is not
/// `name`.
fn field_read(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.trim_start();
    let pad = &line[..line.len() - rest.len()];
    let rest = rest.strip_prefix("const ")?.strip_suffix(';')?;
    let (name, value) = rest.split_once(" = ")?;
    let (place, field) = value.rsplit_once('.')?;
    let ident = |s: &str| {
        !s.is_empty()
            && !s.starts_with(|c: char| c.is_ascii_digit())
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    };
    let root = place.split('.').next()?;
    (ident(name) && field == name && place.split('.').all(ident) && root != name).then_some((pad, place, name))
}

#[cfg(test)]
mod tests {
    use super::destructure;

    #[test]
    fn same_named_fields_of_one_place_are_one_const() {
        let src = "  const request = flow.request;
  const failures = flow.failures;
  const secondFactor = event.second_factor;
  const now = event.now;
  const auth = event.auth;
  const x = s.value;
  const y = a.y;
";
        assert_eq!(
            destructure(src),
            "  const { request, failures } = flow;
  const secondFactor = event.second_factor;
  const { now, auth } = event;
  const x = s.value;
  const y = a.y;
"
        );
    }

    #[test]
    fn other_lines_stay() {
        let src = "  const a = f(x).a;
  const b = f(x).b;
  const c = c.c;
  const d = c.d;
  let e = x.e;
  const f = x.f;
";
        assert_eq!(destructure(src), src);
    }
}
