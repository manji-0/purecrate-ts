mod deeper;

pub fn clamp01(n: i32) -> i32 {
    if n < 0 {
        0
    } else if n > 1 {
        1
    } else {
        n
    }
}
