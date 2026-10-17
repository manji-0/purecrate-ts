// Shapes the printer got wrong while 0.25.0 was made, each now as Rust
// computes it: a typed `let` in a block that an argument needs statements
// for.

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
