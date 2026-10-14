// Type parameters on structs, enums, impls, and functions: emitted as TS
// generics, their arguments inferred at each call and construction, `==`
// on a generic value through `Eq.deep`, `clone` of a `T` its value.

#[derive(Clone, Debug, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u32,
}

impl<T: Clone> Page<T> {
    pub fn new(items: Vec<T>, total: u32) -> Self {
        Page { items, total }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn first(&self) -> Option<T> {
        self.items.first().cloned()
    }

    pub fn with_total(&self, total: u32) -> Page<T> {
        Page { total, ..self.clone() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pair<A, B> {
    pub left: A,
    pub right: B,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Either<A, B> {
    Left(A),
    Right(B),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tree<T> {
    Leaf,
    Node(Box<Tree<T>>, T, Box<Tree<T>>),
}

/// A generic newtype: its inner value at runtime, `Tagged<T>` in TS.
#[derive(Clone, Debug, PartialEq)]
pub struct Tagged<T>(pub T);

impl<T: Clone> Tagged<T> {
    pub fn get(&self) -> T {
        self.0.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub counts: Page<u32>,
    pub names: Option<Page<String>>,
}

fn first_of<T: Clone>(xs: &[T]) -> Option<T> {
    xs.first().cloned()
}

fn same<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

fn swap<A: Clone, B: Clone>(p: &Pair<A, B>) -> Pair<B, A> {
    Pair { left: p.right.clone(), right: p.left.clone() }
}

fn size<T>(t: &Tree<T>) -> u32 {
    match t {
        Tree::Leaf => 0,
        Tree::Node(l, _, r) => size(l) + 1 + size(r),
    }
}

fn insert<T: Clone>(t: Tree<T>, x: T, left: bool) -> Tree<T> {
    match t {
        Tree::Leaf => Tree::Node(Box::new(Tree::Leaf), x, Box::new(Tree::Leaf)),
        Tree::Node(l, v, r) => {
            if left {
                Tree::Node(Box::new(insert(*l, x, !left)), v, r)
            } else {
                Tree::Node(l, v, Box::new(insert(*r, x, !left)))
            }
        }
    }
}

fn values<T: Clone>(t: &Tree<T>) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    if let Tree::Node(l, v, r) = t {
        for x in values(l) {
            out.push(x);
        }
        out.push(v.clone());
        for x in values(r) {
            out.push(x);
        }
    }
    out
}

fn tag<T>(x: T) -> Tagged<T> {
    Tagged(x)
}

/// A generic newtype built, read through `.0`, and returned at two types.
pub fn tags(n: u32, s: String) -> (Tagged<u32>, Tagged<String>, u32, usize, bool) {
    let a = tag(n);
    let b: Tagged<String> = Tagged(s);
    let c = Tagged(n / 2);
    (a.clone(), b.clone(), a.0, b.get().len(), a == c)
}

pub fn untag(t: Tagged<i32>, u: Tagged<String>) -> (i32, String, Tagged<i32>) {
    (t.get(), u.0, Tagged(t.0 / 3))
}

pub fn pages(xs: Vec<u32>, total: u32) -> (usize, Option<u32>, Page<u32>, bool) {
    let p = Page::new(xs.clone(), total);
    let q: Page<u32> = Page { items: xs, total: total + 1 };
    (p.len(), p.first(), p.with_total(7), same(&p, &q.with_total(total)))
}

/// One generic function at two instantiations in one body.
pub fn both(xs: Vec<i64>, ws: Vec<String>) -> (Option<i64>, Option<String>, bool) {
    (first_of(&xs), first_of(&ws), same(&ws, &ws.clone()))
}

pub fn pairs(a: u8, s: String) -> (Pair<String, u8>, Either<u8, String>, Either<u8, String>, bool) {
    let p = Pair { left: a, right: s.clone() };
    let l: Either<u8, String> = Either::Left(a);
    let r: Either<u8, String> = Either::Right(s);
    (swap(&p), l.clone(), r.clone(), l == r)
}

pub fn side(e: Either<i32, String>) -> i32 {
    match e {
        Either::Left(n) => n * 2,
        Either::Right(s) => s.len() as i32,
    }
}

pub fn grow(xs: Vec<i32>) -> (u32, Vec<i32>, Tree<i32>) {
    let mut t: Tree<i32> = Tree::Leaf;
    let mut left = true;
    for x in xs {
        t = insert(t, x, left);
        left = !left;
    }
    (size(&t), values(&t), t)
}

pub fn summarize(counts: Page<u32>, names: Option<Page<String>>) -> (usize, usize, Report) {
    let r = Report { counts: counts.clone(), names };
    let n = match &r.names {
        Some(p) => p.len(),
        None => 0,
    };
    (r.counts.len(), n, r)
}
