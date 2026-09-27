pub enum Shape {
    Square(i32),
    Rect { w: i32, h: i32 },
}

pub fn area(s: Shape) -> i32 {
    match s {
        Shape::Square(n) => n * n,
        Shape::Rect { w, h } => w * h,
    }
}

impl Shape {
    pub fn describe(&self) -> i32 {
        0
    }

    pub fn unit() -> Shape {
        Shape::Square(1)
    }
}

impl Default for Shape {
    fn default() -> Self {
        Shape::Square(0)
    }
}
