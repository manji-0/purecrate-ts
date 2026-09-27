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
    pub fn describe(&mut self) -> i32 {
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

impl Shape {
    pub fn sides(&self) -> i32 {
        match self {
            Shape::Square(_) => 4,
            Shape::Rect { w, h } => 4,
        }
    }

    pub fn diagonal(&self) -> usize {
        0
    }
}

pub fn sides_of(s: Shape) -> i32 {
    s.sides()
}

pub fn diagonal_of(s: Shape) -> i32 {
    s.diagonal();
    0
}
