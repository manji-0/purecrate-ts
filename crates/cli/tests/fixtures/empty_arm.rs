// An arm that does nothing, last but one, where the value is returned:
// it still returns, and does not run into the next `case`.

pub enum M {
    A,
    B,
    C,
}

pub fn unit_cases(m: M, s: &str, n: u8) {
    match m {
        M::A => match s {
            "" => {}
            _ => {
                let _x = n + 1;
            }
        },
        M::B => {
            let _y = n * 200;
        }
        M::C => {}
    }
}

pub fn unit_if(c: bool, n: u8) {
    if c {
    } else {
        let _y = n * 200;
    }
}

// Statements that never fall through are not followed by a `return`,
// which `allowUnreachableCode: false` rejects.

pub fn all_return(x: Option<u32>) -> u32 {
    match x {
        Some(v) => return v,
        None => return 0,
    };
}

#[allow(while_true)]
pub fn spin(k: u32) {
    let mut i = 0u32;
    while true {
        if i > k {
            return;
        }
        i += 1;
    }
}
