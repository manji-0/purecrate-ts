// `s.split(c).collect()` and `s.split(c).map(f).collect()`, and `str::split_once`
// with a `char` or a `&str`. The list's length is the input's; a `Vec` that
// grows with state stays a recursive enum.

fn own(s: &str) -> String {
    String::from(s)
}

/// Pieces, empty ones included, owned. A function name as `map`'s argument.
pub fn words(s: &str) -> Vec<String> {
    s.split(' ').map(own).collect()
}

/// A closure, and a separator that is not ASCII.
pub fn lengths(s: &str, c: char) -> Vec<usize> {
    s.split(c).map(|p| p.len()).collect()
}

/// The turbofish, not the return type, names the `Vec`.
pub fn dotted(s: &str) -> usize {
    let parts = s.split('.').collect::<Vec<&str>>();
    parts.len()
}

/// A typed `let` names it.
pub fn commas(s: String) -> Vec<String> {
    let parts: Vec<String> = s.split(',').map(|p| String::from(p)).collect();
    parts
}

fn token(s: &str) -> Result<u8, bool> {
    if s == "bad" {
        return Err(false);
    }
    if s == "boom" {
        let xs: Vec<u8> = vec![];
        return Ok(xs[0]);
    }
    Ok(1)
}

/// Stops at the first `Err`, so a later `"boom"` is not indexed.
pub fn tokens(s: &str) -> Result<Vec<u8>, bool> {
    s.split(',').map(token).collect()
}

fn halves(s: &str, p: &str) -> Option<(String, String)> {
    match s.split_once(p) {
        Some(pair) => {
            let (a, b) = pair;
            Some((String::from(a), String::from(b)))
        }
        None => None,
    }
}

pub fn once_str(s: &str, p: &str) -> Option<(String, String)> {
    halves(s, p)
}

pub fn once_char(s: &str, c: char) -> Option<(String, String)> {
    match s.split_once(c) {
        Some((a, b)) => Some((String::from(a), String::from(b))),
        None => None,
    }
}

/// The pair pattern with a guard: the names are in scope for the condition.
pub fn once_head(s: &str) -> bool {
    match s.split_once('-') {
        Some((a, b)) if a.is_empty() => b.is_empty(),
        _ => false,
    }
}

pub fn once_owned(s: String, p: String) -> Option<(String, String)> {
    halves(s.as_str(), p.as_str())
}

/// `_` in the target: filled from what `map`'s function returns.
pub fn tokens_inferred(s: &str) -> Result<usize, bool> {
    let v = s.split(',').map(token).collect::<Result<Vec<_>, _>>()?;
    Ok(v.len())
}

/// `Vec<_>` over a function returning a `Result` keeps each `Result`.
pub fn each_token(s: &str) -> Vec<Result<u8, bool>> {
    s.split(',').map(token).collect::<Vec<_>>()
}

pub fn pieces(s: &str) -> usize {
    let v = s.split('.').collect::<Vec<_>>();
    v.len()
}
