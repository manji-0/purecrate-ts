pub enum Ast {
    Num(i32),
    Add(Box<Ast>, Box<Ast>),
    Neg(Box<Ast>),
}

pub fn num(n: i32) -> Ast {
    Ast::Num(n)
}

pub fn add(a: Ast, b: Ast) -> Ast {
    Ast::Add(Box::new(a), Box::new(b))
}

pub fn neg(a: Ast) -> Ast {
    Ast::Neg(Box::new(a))
}

pub fn calc(e: Ast) -> i32 {
    match e {
        Ast::Num(n) => n,
        Ast::Add(a, b) => calc(*a) + calc(*b),
        Ast::Neg(a) => -calc(*a),
    }
}

pub fn calc_num(n: i32) -> i32 {
    calc(num(n))
}

pub fn calc_add(x: i32, y: i32) -> i32 {
    calc(add(num(x), num(y)))
}

pub fn calc_nested(x: i32, y: i32, z: i32) -> i32 {
    calc(add(add(num(x), num(y)), neg(num(z))))
}

pub fn through(n: i32) -> i32 {
    let b = Box::new(n);
    *b + 1
}
