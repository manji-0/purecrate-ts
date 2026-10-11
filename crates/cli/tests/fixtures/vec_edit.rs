// `Vec::remove` and `v[i] = x` on a local's own array: at the front, the
// middle, the end, and past it (a panic), with `v[i] op= x` and the order
// Rust evaluates an assignment in.

/// `xs` with the element at each index of `at` taken out in turn, and what
/// was taken.
pub fn removed(xs: Vec<u32>, at: Vec<usize>) -> (Vec<u32>, Vec<u32>) {
    let mut v = xs;
    let mut taken: Vec<u32> = Vec::new();
    for i in at {
        taken.push(v.remove(i));
    }
    (v, taken)
}

/// `xs` with `x` written at each index of `at` in turn.
pub fn written(xs: Vec<u32>, at: Vec<usize>, x: u32) -> Vec<u32> {
    let mut v = xs;
    for i in at {
        v[i] = x;
    }
    v
}

/// The argument is not changed: the caller's `xs` and the edited copy.
pub fn untouched(xs: Vec<u32>) -> (Vec<u32>, Vec<u32>) {
    let mut v = xs.clone();
    v[0] = 9;
    v.remove(0);
    (xs, v)
}

/// Each count stepped by `by` at each index of `at`, overflow included.
pub fn counted(n: usize, at: Vec<usize>, by: u8) -> Vec<u8> {
    let mut counts: Vec<u8> = Vec::new();
    for _ in 0..n {
        counts.push(0);
    }
    for i in at {
        counts[i] += by;
    }
    counts
}

/// `v[at] = from[j]`: Rust reads `from[j]` before the place, so with both
/// past the end the panic is `from`'s.
pub fn ordered(at: usize, j: usize) -> Vec<u32> {
    let from: Vec<u32> = vec![7, 8];
    let mut v: Vec<u32> = vec![1, 2, 3];
    v[at] = from[j];
    v
}

/// Each element that is not `x`, by removing in place, and each word
/// marked where it repeats the one before.
pub fn without(xs: Vec<i32>, x: i32) -> Vec<i32> {
    let mut v = xs;
    let mut i: usize = 0;
    while i < v.len() {
        if v[i] == x {
            v.remove(i);
        } else {
            i += 1;
        }
    }
    v
}

/// Each word that repeats the one before it replaced by `"-"`.
pub fn marked(ws: Vec<String>) -> Vec<String> {
    let mut v = ws.clone();
    for i in 1..ws.len() {
        if ws[i] == ws[i - 1] {
            v[i] = String::from("-");
        }
    }
    v
}

/// Each element removed from the front while the loop reads a copy, and
/// what is left.
pub fn drained(xs: Vec<u32>) -> (Vec<u32>, usize) {
    let mut v = xs;
    let mut out: Vec<u32> = Vec::new();
    for _ in v.clone() {
        out.push(v.remove(0));
    }
    (out, v.len())
}

/// `v[i] op= x` where `x` and the place may both panic: Rust evaluates `x`
/// first.
pub fn stepped(i: usize, j: usize) -> Vec<u8> {
    let by: Vec<u8> = vec![1, 255];
    let mut v: Vec<u8> = vec![0, 250];
    v[i] += by[j];
    v[i + 0] = by[j] - 1;
    v
}

/// `v[i] += x` where `x` needs statements (a `match` lifted out): still
/// evaluated before the place.
pub fn stepped_by_max(xs: Vec<i32>, i: usize) -> Vec<i32> {
    let mut v: Vec<i32> = vec![0];
    v[i] += xs.iter().copied().max_by_key(|x| *x * 2).unwrap_or(0);
    v
}

/// `v[n - 1] = x` where the index may overflow and `x` may panic first.
pub fn set_after_value(xs: Vec<i32>, n: usize) -> Vec<Vec<i32>> {
    let mut v: Vec<Vec<i32>> = vec![vec![]];
    v[n - 1] = xs.iter().map(|x| -*x).collect::<Vec<i32>>();
    v
}

/// The index computed by a consumer, the value read past the end.
pub fn set_at_position(xs: Vec<i32>, k: i32) -> Vec<i32> {
    let mut v = xs.clone();
    v[xs.iter().position(|x| *x + k < 0).unwrap_or(0)] = xs[5];
    v
}
