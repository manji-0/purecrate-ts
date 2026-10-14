// Associated consts, read as `Self::N` in their `impl` and as `T::N`
// anywhere, and paths through modules: `self::m::f`, `super::f`,
// `super::super::m::f`, and a child's `m::f`, to functions, types,
// consts, and variants. No `crate::`: the differential test includes this
// file in a module of its own, where `crate` is the test's (`build.rs`
// tests it on a crate).

pub use shapes::{Grid, Page, Shape};

pub mod shapes {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Shape {
        Dot,
        Square(u32),
    }

    impl Shape {
        pub const SIDES: u32 = 4;
        /// The largest side whose area a `u32` holds.
        pub const MAX_SIDE: u32 = 65_535;
        const ORIGIN: u32 = Self::SIDES - 4;

        pub fn area(self) -> u32 {
            match self {
                Shape::Dot => Self::ORIGIN + 1,
                Shape::Square(n) => n * n,
            }
        }

        pub fn perimeter(self) -> u32 {
            match self {
                Self::Dot => Self::ORIGIN,
                Self::Square(n) => n * Self::SIDES,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Grid {
        pub cells: Vec<Shape>,
    }

    impl Grid {
        pub const LIMIT: usize = super::limits::CELLS;
        pub const LABEL: &'static str = "grid·";
        pub const EMPTY_OK: bool = true;
        pub const SCALE: f64 = -0.5;
        pub const MARK: char = 'é';
        const WIDE: u64 = 1 << 60;

        pub fn fits(&self) -> bool {
            self.cells.len() <= Self::LIMIT
        }

        pub fn weight(&self) -> u64 {
            let mut total: u64 = 0;
            for c in &self.cells {
                total += u64::from(c.area()) * Grid::WIDE;
            }
            total
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Page<T> {
        pub items: Vec<T>,
    }

    impl<T: Clone> Page<T> {
        pub const SIZE: usize = 2;

        pub fn full(&self) -> bool {
            self.items.len() >= Self::SIZE
        }
    }
}

mod limits {
    pub const CELLS: usize = 3;

    pub fn clamp(n: u32) -> u32 {
        if n > super::shapes::Shape::MAX_SIDE {
            super::shapes::Shape::MAX_SIDE
        } else {
            n
        }
    }

    pub mod deep {
        pub fn twice(n: u32) -> u32 {
            super::clamp(n) * 2 + super::super::shapes::Shape::SIDES
        }
    }
}

/// Every side of three squares: a crate const from an associated one.
pub const ALL_SIDES: u32 = Shape::SIDES * 3;

pub fn square(n: u32) -> shapes::Shape {
    self::shapes::Shape::Square(self::limits::clamp(n))
}

pub fn area_of(n: u32) -> u32 {
    shapes::Shape::Square(n).area()
}

pub fn perimeter_of(s: Shape) -> u32 {
    s.perimeter()
}

pub fn sides() -> u32 {
    Shape::SIDES + shapes::Shape::SIDES + ALL_SIDES
}

pub fn doubled(n: u32) -> u32 {
    limits::deep::twice(n)
}

pub fn bump(n: u32) -> u32 {
    const STEP: u32 = Shape::MAX_SIDE / 5;
    n + STEP
}

pub fn kind(s: Shape) -> u32 {
    match s {
        shapes::Shape::Dot => 0,
        self::shapes::Shape::Square(_) => 1,
    }
}

pub fn is_dot(s: Shape) -> bool {
    matches!(s, shapes::Shape::Dot)
}

pub fn fits(n: usize) -> bool {
    let mut cells: Vec<Shape> = Vec::new();
    for _ in 0..n {
        cells.push(Shape::Dot);
    }
    self::shapes::Grid { cells }.fits()
}

pub fn weight(g: Grid) -> u64 {
    g.weight()
}

pub fn label(s: &str) -> String {
    let mut t = String::new();
    t.push_str(Grid::LABEL);
    t.push_str(s);
    t.push(Grid::MARK);
    t
}

pub fn label_len() -> usize {
    shapes::Grid::LABEL.len()
}

pub fn empty_ok(g: Grid) -> bool {
    g.cells.is_empty() == Grid::EMPTY_OK
}

pub fn scaled(x: f64) -> f64 {
    x * Grid::SCALE
}

pub fn page_full(xs: Vec<u32>) -> bool {
    Page { items: xs }.full()
}
