//! Rewrites that only change how the output reads keep its meaning, and
//! print as intended: `c ? b : a` for `!c ? a : b`, `const { w, h } = s`,
//! the place for an arm that returns the variant it matched, and for a
//! variant built again from all its own fields in order. A helper called
//! from one file is printed in it.

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
        for v in [0, 20] {
            cases.push(case!(readability::bumped_twice(v)));
        }
        for s in ["00x", "01", "02", "99", "9"] {
            if s.len() >= 2 {
                for open in [false, true] {
                    cases.push(case!(readability::reserved_pair(s, open)));
                }
            }
        }
        for n in [3, 8, i32::MAX - 1] {
            cases.push(case!(readability::added_or_zero(n)));
            cases.push(case!(readability::named_temporaries(n)));
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
    // The file a function is printed in: its own, or its one caller's.
    let file = |stem: &str| {
        let name = purecrate_ir::to_camel(&stem.replace('-', "_"));
        let decl = format!("const {name} = ");
        pkg.files.iter().find(|f| f.source.contains(&decl)).expect(stem).source.clone()
    };
    let flipped = file("flipped");
    assert!(flipped.contains("c ? (0 as I32) : a"), "{flipped}");
    let area = file("area");
    assert!(area.contains("const { w, h } = s;"), "{area}");
    // `tie`, its helper, shares the file; only `thenBy` itself is checked.
    let then_by = file("then-by");
    let then_by = then_by[then_by.find("export const thenBy").expect("thenBy")..].to_string();
    assert!(then_by.contains("return d;") && !then_by.contains("kind: \"Less\"") && !then_by.contains("switch"), "{then_by}");
    let same_again = file("same-again");
    assert!(!same_again.contains("kind: \"Pair\"") && !same_again.contains("kind: \"Rect\""), "{same_again}");
    // A helper with one caller is printed in its file, its names apart
    // from what that file imports.
    assert!(!pkg.files.iter().any(|f| f.stem == "doubled"));
    let twice = file("bumped-twice");
    assert!(twice.contains("import { bump } from") && twice.contains("const doubled = (bump2: I32)"), "{twice}");
    let pair = file("reserved-pair");
    assert!(pair.contains("open || [\"00\", \"01\", \"99\"].includes(") && !pair.contains("=> {"), "{pair}");
    let named = file("named-temporaries");
    assert!(named.contains("const halfResult = ") && named.contains("const sumOpt = "), "{named}");
    let added = file("added-or-zero");
    assert!(added.contains("const s: I32 = ") && !added.contains("let s"), "{added}");
    let swapped = file("swapped");
    assert!(swapped.contains("kind: \"Pair\""), "{swapped}");
}
