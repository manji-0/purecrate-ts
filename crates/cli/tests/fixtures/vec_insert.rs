// `Vec::insert` on a local's own array: at the front, the middle, the end,
// and past it (a panic), with `push` beside it.

/// `xs` with `x` put at each index of `at` in turn.
pub fn inserted(xs: Vec<u32>, at: Vec<usize>, x: u32) -> Vec<u32> {
    let mut v = xs;
    for i in at {
        v.insert(i, x);
    }
    v
}

/// The argument is not changed: the caller's `xs` and the grown copy.
pub fn untouched(xs: Vec<u32>) -> (Vec<u32>, Vec<u32>) {
    let mut v = xs.clone();
    v.insert(0, 9);
    (xs, v)
}

/// Insertion sort, each element put before the first larger one.
pub fn sorted(xs: Vec<i32>) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::new();
    for x in xs {
        let mut i: usize = 0;
        while i < out.len() && out[i] <= x {
            i += 1;
        }
        out.insert(i, x);
    }
    out
}

/// Each word at the front and at the back.
pub fn mirrored(ws: Vec<String>) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for w in ws {
        v.insert(0, w.clone());
        v.push(w);
    }
    v
}

/// Each code point put at the last of `at` (0 when empty) taken mod the
/// length so far, an index held in a `u32` as RFC 3492 §6.2 keeps it.
pub fn placed(cps: Vec<u32>, at: Vec<u32>) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut len: u32 = 0;
    for cp in cps {
        let mut i: u32 = 0;
        for a in at.clone() {
            i = a;
        }
        out.insert((i % (len + 1)) as usize, cp);
        len += 1;
    }
    out
}

/// Each element again at the end: the loop reads a copy, so it stops at
/// the elements there were before it, and the copy stays in the output.
pub fn doubled(xs: Vec<u32>) -> Vec<u32> {
    let mut v = xs;
    for x in v.clone() {
        v.push(x);
    }
    v
}
