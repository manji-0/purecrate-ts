pub fn bytes(s: &str) -> &[u8] {
    s.as_bytes()
}

pub fn byte_len(s: String) -> usize {
    s.as_bytes().len()
}

pub fn nth_byte(s: &str, i: usize) -> u8 {
    let b = s.as_bytes();
    b[i]
}

fn count(b: &[u8], i: usize, n: usize) -> usize {
    if i == b.len() {
        n
    } else if b[i] >= 128u8 && b[i] < 192u8 {
        count(b, i + 1usize, n)
    } else {
        count(b, i + 1usize, n + 1usize)
    }
}

/// Code points, counted from the UTF-8 bytes: a continuation byte is 0x80..=0xBF.
pub fn code_points(s: &str) -> usize {
    count(s.as_bytes(), 0usize, 0usize)
}
