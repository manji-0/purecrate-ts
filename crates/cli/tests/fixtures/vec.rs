pub fn at(xs: Vec<i32>, i: usize) -> i32 {
    xs[i]
}

pub fn len(xs: Vec<i32>) -> usize {
    xs.len()
}

pub fn sum_from(xs: &[i32], i: usize) -> i32 {
    if i == xs.len() {
        0
    } else {
        xs[i] + sum_from(xs, i + 1)
    }
}

pub fn second(xs: Vec<i32>) -> i32 {
    xs[1]
}
