// Iterator consumers that yield a scalar: `all`, `any`, `position`,
// `count`, and `sum` on `chars()`, `bytes()`, and `iter()`, and `for` over
// `.enumerate()`.

pub fn all_digits(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_digit())
}

pub fn any_upper(s: String) -> bool {
    s.bytes().any(|b| matches!(b, b'A'..=b'Z'))
}

pub fn has_zero(xs: &[u32]) -> bool {
    xs.iter().any(|&x| x == 0)
}

fn small(x: &u32) -> bool {
    *x < 10
}

pub fn all_small(xs: Vec<u32>) -> bool {
    xs.iter().all(small)
}

// The predicate runs only until the answer is known: a later overflow
// does not happen.
pub fn stops_early(xs: &[u8]) -> bool {
    xs.iter().any(|x| x + 200 > 250)
}

pub fn at_sign(s: &str) -> Option<usize> {
    s.bytes().position(|b| b == b'@')
}

// A char index, not a byte index.
pub fn char_at(s: &str, c: char) -> Option<usize> {
    s.chars().position(|d| d == c)
}

pub fn chars(s: &str) -> usize {
    s.chars().count()
}

pub fn items(xs: Vec<i64>) -> usize {
    xs.into_iter().count()
}

pub fn total(xs: &[u8]) -> u8 {
    xs.iter().sum()
}

pub fn total_wide(xs: Vec<i64>) -> i64 {
    xs.iter().sum::<i64>()
}

pub fn byte_sum(s: &str) -> u32 {
    let mut n: u32 = 0;
    for (i, b) in s.bytes().enumerate() {
        if b == b' ' {
            continue;
        }
        if i % 2 == 0 {
            n += u32::from(b);
        }
    }
    n
}

pub fn last_index(xs: &[u32], target: u32) -> Option<usize> {
    let mut found: Option<usize> = None;
    for (i, &x) in xs.iter().enumerate() {
        if x == target {
            found = Some(i);
        }
    }
    found
}

// After `split(c)`: the pieces, empty ones included.
pub fn has_token(list: &str, word: &str) -> bool {
    list.split(' ').any(|token| token == word)
}

pub fn pieces(list: &str) -> usize {
    list.split(',').count()
}

pub fn token_at(list: &str, word: &str) -> Option<usize> {
    list.split(' ').position(|token| token == word)
}
