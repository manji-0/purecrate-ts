//! Rewrites that only change how the output reads keep its meaning, and
//! print as intended: `c ? b : a` for `!c ? a : b`, `const { w, h } = s`,
//! the place for an arm that returns the variant it matched, and for a
//! variant built again from all its own fields in order.

use crate::support;

purecrate_canon::fixture!(mod readability = "fixtures/readability.rs");

const SOURCE: &str = readability::SOURCE;

#[test]
fn generated_readability_matches_rust() {
    use readability::{Dir, Shape};
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for c in [false, true] {
            cases.push(case!(readability::flipped(c, 7)));
        }
        let shapes = [Shape::Rect { w: 3, h: 4 }, Shape::Pair(5, 6), Shape::Empty, Shape::Pair(i32::MAX, 1)];
        for s in shapes {
            cases.push(case!(readability::area(s)));
            cases.push(case!(readability::same_again(s)));
            cases.push(case!(readability::swapped(s)));
        }
        for d in [Dir::Less, Dir::Equal, Dir::Greater] {
            for (a, b) in [(1, 2), (2, 2), (3, 2)] {
                cases.push(case!(readability::then_by(d, a, b)));
            }
        }
        cases
    });
    support::assert_equivalent("readability", SOURCE, &cases);
}

#[test]
fn readability_rewrites_print_as_intended() {
    let krate = purecrate_syntax::parse_source("readability", SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    let flipped = file("flipped");
    assert!(flipped.contains("c ? (0 as I32) : a"), "{flipped}");
    let area = file("area");
    assert!(area.contains("const { w, h } = s;"), "{area}");
    let then_by = file("then-by");
    assert!(then_by.contains("return d;") && !then_by.contains("kind: \"Less\"") && !then_by.contains("switch"), "{then_by}");
    let same_again = file("same-again");
    assert!(!same_again.contains("kind: \"Pair\"") && !same_again.contains("kind: \"Rect\""), "{same_again}");
    let swapped = file("swapped");
    assert!(swapped.contains("kind: \"Pair\""), "{swapped}");
}
