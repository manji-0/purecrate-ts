// Sorting a local's own `Vec` (stable, as std's `sort` is) and picking from
// a sequence: `find`, `max` / `min` (the last of the greatest, the first of
// the least), by key and by an `Ordering`.

#[derive(Clone)]
pub struct Ranked {
    pub name: String,
    pub q: u32,
    pub specificity: u32,
}

pub fn sorted(xs: Vec<i32>) -> Vec<i32> {
    let mut v = xs;
    v.sort();
    v
}

pub fn sorted_text(xs: Vec<String>) -> Vec<String> {
    let mut v = xs;
    v.sort();
    v
}

/// Best first: by q, then specificity, both descending; ties keep the
/// order they came in.
pub fn best_first(xs: Vec<Ranked>) -> Vec<String> {
    let mut v = xs;
    v.sort_by(|a, b| b.q.cmp(&a.q).then(b.specificity.cmp(&a.specificity)));
    let mut out: Vec<String> = Vec::new();
    for r in &v {
        out.push(r.name.clone());
    }
    out
}

pub fn by_length(xs: Vec<String>) -> Vec<String> {
    let mut v = xs;
    v.sort_by_key(|s| s.len());
    v
}

pub fn picked(xs: Vec<Ranked>) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let best = xs.iter().max_by_key(|r| r.q).map(|r| r.name.clone());
    let worst = xs.iter().min_by_key(|r| r.q).map(|r| r.name.clone());
    let first_zero = xs.iter().find(|r| r.q == 0).map(|r| r.name.clone());
    let most = xs.iter().max_by(|a, b| a.q.cmp(&b.q).then(a.specificity.cmp(&b.specificity))).map(|r| r.name.clone());
    (best, worst, first_zero, most)
}

pub fn extremes(xs: Vec<u8>) -> (Option<u8>, Option<u8>) {
    (xs.iter().copied().max(), xs.iter().copied().min())
}

pub fn text_extremes(s: &str) -> (Option<char>, Option<char>) {
    (s.chars().max(), s.chars().min())
}

pub fn least_by(xs: Vec<i64>) -> Option<i64> {
    xs.iter().copied().min_by(|a, b| a.cmp(b))
}
