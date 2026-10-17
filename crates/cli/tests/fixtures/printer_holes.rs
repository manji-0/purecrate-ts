// Shapes the printer got wrong while 0.25.0 was made, each now as Rust
// computes it: a typed `let` in a block that an argument needs statements
// for, and an integer widened from an `unwrap_or`.

#[derive(Clone, Copy)]
pub enum Mark {
    At(i32),
    Gone,
}

fn count(xs: &[Option<Mark>]) -> usize {
    xs.len()
}

fn first_at(xs: &[Option<Mark>]) -> i32 {
    if xs.is_empty() {
        return 0;
    }
    match xs[0] {
        Some(Mark::At(k)) => k,
        Some(Mark::Gone) => -1,
        None => 0,
    }
}

pub fn typed_in_block(o: Option<i32>, k: i32) -> i32 {
    let mut n = 0;
    if k > 0 {
        n = first_at(&{
            let e: Vec<Option<Mark>> = vec![Some(Mark::At(match o {
                Some(_) => k,
                None => 1,
            }))];
            e
        }[..]);
    }
    n + count(&{
        let e: Vec<Option<Mark>> = vec![match o {
            Some(v) if v > 0 => Some(Mark::At(v)),
            Some(_) => Some(Mark::Gone),
            None => None,
        }];
        e
    }[..]) as i32
}

pub struct Scaled {
    pub f: f64,
}

pub fn widened_default(o: Option<i32>, w: Option<i64>, c: bool) -> f64 {
    let s = Scaled { f: f64::from(o.unwrap_or(100)) };
    let chosen = f64::from(o.unwrap_or(if c { 1 } else { -2 }));
    let wide = i128::from(w.unwrap_or(7));
    s.f + chosen + f64::from(o.unwrap_or(3) / 2) + wide as f64
}
