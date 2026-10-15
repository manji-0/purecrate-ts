//! Common shapes of domain code, one public function each, for
//! `purecrate-ts survey corpus/patterns.rs` (design/07 §2.5). Not a
//! metric to chase: each shape is judged by what it costs to write.
use std::collections::HashMap;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub enum Shape { Circle { r: i32 }, Square(i32), Empty }

#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub sku: String, pub qty: u32, pub price: i64 }

pub struct Page<T> { pub items: Vec<T> }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UserId(pub u64);

pub trait Priced { fn price(&self) -> i64; }

pub fn p01_if_let_variant(s: Shape) -> i32 { if let Shape::Square(n) = s { n } else { 0 } }
pub fn p02_let_else(o: Option<i32>) -> i32 { let Some(x) = o else { return 0 }; x }
pub fn p03_format_int(n: u32) -> String { format!("INV-{}", n) }
pub fn p04_to_string(n: u32) -> String { n.to_string() }
pub fn p05_hashmap(xs: Vec<Item>) -> HashMap<String, u32> { let mut m = HashMap::new(); for x in xs { m.insert(x.sku, x.qty); } m }
pub fn p06_btreeset(xs: Vec<u32>) -> BTreeSet<u32> { xs.into_iter().collect() }
pub fn p06b_count_by_key(xs: Vec<u32>) -> BTreeMap<u32, usize> { let mut m: BTreeMap<u32, usize> = BTreeMap::new(); for x in xs { let n = m.get(&x).copied().unwrap_or(0); m.insert(x, n + 1); } m }
pub fn p06c_newtype_key(m: &BTreeMap<UserId, i64>, id: UserId) -> Option<i64> { m.get(&id).copied() }
pub fn p07_generic<T: Clone>(x: T) -> T { x }
pub fn p08_array(b: [u8; 4]) -> u8 { b[0] }
pub fn p11_rev(xs: Vec<u32>) -> Vec<u32> { xs.into_iter().rev().collect() }
pub fn p12_zip(a: Vec<u32>, b: Vec<u32>) -> u32 { a.iter().zip(b.iter()).map(|(x, y)| x * y).sum() }
pub fn p15_find(xs: &[Item], sku: &str) -> Option<Item> { xs.iter().find(|i| i.sku == sku).cloned() }
pub fn p16_to_lowercase(s: &str) -> String { s.to_lowercase() }
pub fn p17_trim(s: &str) -> &str { s.trim() }
pub fn p18_and_then(o: Option<i32>) -> Option<i32> { o.and_then(|x| if x > 0 { Some(x) } else { None }) }
pub fn p19_unwrap_or_else(o: Option<i32>) -> i32 { o.unwrap_or_else(|| 3) }
pub fn p20_iter_max(xs: &[u32]) -> Option<u32> { xs.iter().copied().max() }
pub fn p21_fold(xs: &[i64]) -> i64 { xs.iter().fold(0, |a, x| a + x) }
pub fn p22_windows(xs: &[u32]) -> bool { xs.windows(2).all(|w| w[0] <= w[1]) }
pub fn p23_slice_pattern(xs: &[u32]) -> u32 { match xs { [] => 0, [x, ..] => *x } }
pub fn p23b_slice_rest(xs: &[u32]) -> usize { match xs { [_, rest @ ..] => rest.len(), [] => 0 } }
pub fn p23c_slice_ends(xs: &[u32]) -> u32 { match xs { [first, .., last] => first + last, [x] => *x, [] => 0 } }
pub fn p23d_slice_let_else(xs: Vec<u32>) -> u32 { let [a, b, ..] = xs.as_slice() else { return 0 }; a + b }
pub fn p24_trait_impl_call(i: &Item) -> i64 { i.price() }
impl Priced for Item { fn price(&self) -> i64 { self.price } }
pub trait Weighed { fn grams(&self) -> u32; fn kilos(&self) -> u32 { self.grams() / 1000 } }
impl Weighed for Item { fn grams(&self) -> u32 { self.qty } }
pub fn p24b_trait_default(i: &Item) -> u32 { i.kilos() }
pub fn p24c_trait_bound<T: Weighed>(xs: &[T]) -> u32 { xs.iter().map(|x| x.kilos()).sum() }
pub fn p24d_impl_trait_arg(x: impl Weighed) -> u32 { x.grams() }
pub fn p24e_trait_path(i: &Item) -> i64 { Priced::price(i) + Item::price(i) }
pub fn p25_mut_self(it: &mut Item) { it.qty += 1; }
pub fn p26_usize_bits(n: usize) -> usize { n & 1 }
pub fn p27_tuple_field(t: (u32, u32)) -> u32 { t.0 }
pub fn p28_struct_update(i: Item) -> Item { Item { qty: 0, ..i } }
pub fn p29_chars_rev(s: &str) -> String { s.chars().rev().collect() }
pub fn p31_contains_vec(xs: &[u32], x: u32) -> bool { xs.contains(&x) }
pub fn p32_len_vec(xs: &[u32]) -> usize { xs.len() }
pub fn p33_last(xs: &[u32]) -> Option<u32> { xs.last().copied() }
pub fn p34_get(xs: &[u32], i: usize) -> Option<u32> { xs.get(i).copied() }
pub fn p35_flat_map(xs: Vec<Vec<u32>>) -> Vec<u32> { xs.into_iter().flatten().collect() }
pub fn p36_filter_map(xs: &[i32]) -> Vec<i32> { xs.iter().filter_map(|x| if *x > 0 { Some(*x) } else { None }).collect() }
pub fn p37_option_tuple(a: Option<i32>, b: Option<i32>) -> i32 { match (a, b) { (Some(x), Some(y)) => x + y, _ => 0 } }
pub fn p38_f64_fmt(x: f64) -> String { format!("{:.2}", x) }
pub fn p39_matches(s: &Shape) -> bool { matches!(s, Shape::Circle { .. }) }
pub fn p42_is_sorted_iter_sum(xs: &[Item]) -> i64 { xs.iter().map(|i| i.price * i64::from(i.qty)).sum() }
pub fn p43_assoc_const() -> u32 { Item2::MAX }
pub struct Item2 {}
impl Item2 { pub const MAX: u32 = 3; }
pub fn p44_chars_count(s: &str) -> usize { s.chars().count() }
pub fn p45_char_upper(c: char) -> char { c.to_ascii_uppercase() }
pub fn p46_split_whitespace(s: &str) -> usize { s.split_whitespace().count() }
pub fn p47_lines(s: &str) -> usize { s.lines().count() }
pub fn p48_join(xs: Vec<String>) -> String { xs.join(",") }
pub fn p49_repeat(s: &str) -> String { s.repeat(3) }
pub fn p50_inclusive_range(n: u32) -> u32 { let mut s = 0; for i in 0..=n { s += i; } s }
pub fn p51_step_by(n: u32) -> u32 { let mut s = 0; for i in (0..n).step_by(2) { s += i; } s }
pub fn p52_iter_any_vec(xs: &[Item]) -> bool { xs.iter().any(|i| i.qty == 0) }
pub fn p53_unwrap(o: Option<i32>) -> i32 { o.unwrap() }
pub fn p54_expect(o: Option<i32>) -> i32 { o.expect("x") }
pub fn p55_panic() -> i32 { panic!("no") }
pub fn p56_assert(x: i32) -> i32 { assert!(x > 0); x }
pub fn p57_tuple_return(x: i32) -> (i32, i32) { (x, x) }
pub fn p58_nested_closure(xs: &[i32], k: i32) -> Vec<i32> { let f = |x: i32| x * k; xs.iter().map(|x| f(*x)).collect() }
pub fn p59_partition(xs: Vec<i32>) -> (Vec<i32>, Vec<i32>) { xs.into_iter().partition(|x| *x > 0) }
pub fn p60_min_by_key(xs: &[Item]) -> Option<Item> { xs.iter().min_by_key(|i| i.price).cloned() }
pub fn i01_flat_map_vec(xs: &[Item]) -> Vec<u32> { xs.iter().flat_map(|i| vec![i.qty, i.qty * 2]).collect() }
pub fn i02_flat_map_option(xs: &[&str]) -> Vec<u32> { xs.iter().flat_map(|s| s.parse::<u32>().ok()).collect() }
pub fn i03_flatten_option(xs: Vec<Option<i32>>) -> i32 { xs.into_iter().flatten().sum() }
pub fn i04_filter_map_chars(s: &str) -> Vec<u32> { s.chars().filter_map(|c| c.to_digit(10)).collect() }
pub fn i05_for_windows(xs: &[i64]) -> i64 { let mut m = 0; for w in xs.windows(2) { m = m.max(w[1] - w[0]); } m }
pub fn i06_chunks(xs: &[u8]) -> usize { xs.chunks(4).filter(|c| c.len() == 4).count() }
pub fn i07_dedup_by_key(xs: Vec<Item>) -> Vec<Item> { let mut v = xs; v.dedup_by_key(|i| i.qty); v }
pub fn i08_rev_step_by(n: u32) -> Vec<u32> { (0..=n).rev().step_by(5).collect() }
pub fn i09_step_by_rev(n: u8) -> Vec<u8> { (0..n).step_by(5).rev().collect() }
pub fn i10_range_sum(n: u64) -> u64 { (1..=n).map(|x| x * x).sum() }
pub fn i11_range_any(n: u32) -> bool { (2..n).any(|d| n % d == 0) }
pub fn i12_for_filter_map(xs: &[Item]) -> u32 { let mut n = 0; for q in xs.iter().filter_map(|i| i.qty.checked_sub(1)) { n += q; } n }

pub fn q09_loop(n: u32) -> u32 { let mut n = n; loop { if n > 10 { return n; } n += 1; } }
pub fn q10_while_let(s: &str) -> u32 { let mut rest = s; let mut n = 0u32; while let Some((_head, tail)) = rest.split_once(',') { n += 1; rest = tail; } n }
pub fn q13_sort_by_key(xs: Vec<Item>) -> Vec<Item> { let mut xs = xs; xs.sort_by_key(|i| i.price); xs }
pub fn q14_sort_by(xs: Vec<Item>) -> Vec<Item> { let mut xs = xs; xs.sort_by(|a, b| a.price.cmp(&b.price)); xs }
pub fn q30_split_str(s: &str) -> Vec<String> { s.split(", ").map(|t| String::from(t)).collect() }
pub fn s01_to_uppercase(s: &str) -> String { s.to_uppercase() }
pub fn s02_split_param(s: &str, sep: &str) -> usize { s.split(sep).count() }
pub fn s03_lines_for(s: &str) -> usize { let mut n = 0usize; for l in s.lines() { if !l.trim().is_empty() { n += 1; } } n }
pub fn s04_join_strs(xs: &[&str]) -> String { xs.join("-") }
pub fn s05_bool_to_string(b: bool) -> String { b.to_string() }
pub fn s06_words_upper(s: &str) -> Vec<String> { s.split_whitespace().map(|w| w.to_uppercase()).collect() }
pub fn q40_extend(a: Vec<u32>, b: Vec<u32>) -> Vec<u32> { let mut a = a; for x in b { a.push(x); } a }
pub fn q41_dedup(a: Vec<u32>) -> Vec<u32> { let mut a = a; a.dedup(); a }
pub fn q61_struct_method(i: Item) -> Item { i.with_qty(3) }
impl Item { pub fn with_qty(self, q: u32) -> Item { Item { qty: q, ..self } } }
pub fn q62_mod_path() -> u32 { inner::f() }
mod inner { pub fn f() -> u32 { 1 } }
pub fn q71_self_const() -> u32 { Item3::double() }
pub struct Item3 {}
impl Item3 { const BASE: u32 = 2; pub fn double() -> u32 { Self::BASE * 2 } }
pub fn q72_crate_path() -> u32 { crate::inner::f() + self::outer::g() }
pub mod outer { pub enum Kind { A, B } pub fn g() -> u32 { super::inner::f() } }
pub fn q73_variant_path(k: outer::Kind) -> u32 { match k { outer::Kind::A => 1, crate::outer::Kind::B => 2 } }
pub fn q74_generic_assoc_const(p: Page<u32>) -> bool { p.items.len() >= Page::<u32>::SIZE }
impl<T> Page<T> { pub const SIZE: usize = 10; }
pub fn q63_display_err(e: Item) -> String { e.sku.clone() }
pub fn q64_nested_enum(o: Option<Result<u32, String>>) -> u32 { match o { Some(Ok(n)) => n, _ => 0 } }
pub fn q65_vec_of_vec(xs: Vec<Vec<u32>>) -> u32 { let mut s = 0; for r in &xs { for x in r { s += x; } } s }
pub fn q66_opt_filter(o: Option<u32>) -> Option<u32> { o.filter(|x| *x > 3) }
pub fn q67_iter_sum_u64(xs: &[u64]) -> u64 { xs.iter().sum() }
pub fn q68_div_round(a: i64, b: i64) -> i64 { (a + b / 2) / b }
pub fn q69_i128(a: i128) -> i128 { a * 2 }
pub fn q70_str_cmp(a: &str, b: &str) -> bool { a < b }
#[derive(Debug, PartialEq)]
pub enum AppErr { Parse, Low(u8) }
impl From<std::num::ParseIntError> for AppErr { fn from(_e: std::num::ParseIntError) -> Self { AppErr::Parse } }
impl From<u8> for AppErr { fn from(n: u8) -> Self { AppErr::Low(n) } }
pub fn f01_try_converts(s: &str) -> Result<u32, AppErr> { let n = s.parse::<u32>()?; Ok(n) }
pub fn f02_into(n: u8) -> AppErr { n.into() }
pub fn f03_from_call(n: u8) -> AppErr { AppErr::from(n) }
#[derive(Default)]
pub struct Opts { pub n: u32, pub name: String, pub tags: Vec<String> }
pub fn d01_default_update(n: u32) -> Opts { Opts { n, ..Default::default() } }
pub fn d02_default_call() -> Opts { Opts::default() }
pub struct Money { pub cents: i64 }
impl std::fmt::Display for Money { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}.{:02}", self.cents / 100, self.cents % 100) } }
impl std::fmt::Display for Shape { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Shape::Circle { r } => write!(f, "circle {r}"), Shape::Square(n) => { write!(f, "square ")?; write!(f, "{n}")?; Ok(()) } Shape::Empty => f.write_str("empty") } } }
pub fn d01_display_format(m: Money) -> String { format!("total: {m}") }
pub fn d02_display_to_string(s: Shape) -> String { s.to_string() }
pub fn d03_display_nested(s: Shape, m: Money) -> String { format!("{} costs {}", s, m) }
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ver { pub major: u32, pub minor: u32, pub pre: Option<String> }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prio { pub n: u8 }
impl Ord for Prio { fn cmp(&self, other: &Self) -> std::cmp::Ordering { other.n.cmp(&self.n) } }
impl PartialOrd for Prio { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) } }
pub fn o01_derived_lt(a: Ver, b: Ver) -> bool { a < b }
pub fn o02_derived_sort(xs: Vec<Ver>) -> Vec<Ver> { let mut xs = xs; xs.sort(); xs }
pub fn o03_derived_iter_max(xs: Vec<Ver>) -> Option<Ver> { xs.into_iter().max() }
pub fn o04_derived_clamp(v: Ver, lo: Ver, hi: Ver) -> Ver { v.clamp(lo, hi) }
pub fn o05_tuple_cmp(a: (u32, String), b: (u32, String)) -> std::cmp::Ordering { a.cmp(&b) }
pub fn o06_impl_ord(a: Prio, b: Prio) -> bool { a > b }
pub fn o07_sort_by_derived_key(xs: Vec<Item>) -> Vec<Item> { let mut xs = xs; xs.sort_by_key(|i| (i.qty, i.sku.clone())); xs }
pub fn b01_largest<T: Ord + Clone>(xs: &[T]) -> Option<T> { xs.iter().max().cloned() }
pub fn b02_largest_ver(xs: Vec<Ver>) -> Option<Ver> { b01_largest(&xs) }
pub fn b03_show<T: std::fmt::Display>(x: T) -> String { format!("[{x}]") }
pub fn b04_show_money(m: Money, n: u32) -> (String, String) { (b03_show(m), b03_show(n)) }
pub fn b05_or_default<T: Default>(o: Option<T>) -> T { match o { Some(x) => x, None => T::default() } }
pub fn b06_less<T: PartialOrd>(a: &T, b: &T) -> bool { a < b }
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Ranked<T> { pub rank: u8, pub value: T }
pub fn b07_generic_derive(a: Ranked<String>, b: Ranked<String>) -> bool { let d: Ranked<u8> = Ranked::default(); a < b && d.rank == 0 }
