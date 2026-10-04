//! `Box<T>` is erased to `T`, so an owned recursive enum evaluates like Rust.

use crate::support;

purecrate_canon::fixture!(mod ast = "fixtures/ast.rs");

#[test]
fn generated_recursive_enum_matches_rust() {
    support::equivalence("ast", ast::SOURCE, |cases| {
        let ns = [-4i32, -1, 0, 2, 9];
        for n in ns {
            cases.push(case!(ast::calc_num(n)));
            cases.push(case!(ast::through(n)));
            cases.push(case!(ast::boxed_opt(Some(n))));
            for m in ns {
                cases.push(case!(ast::calc_add(n, m)));
                cases.push(case!(ast::checked_add(n, m)));
                for k in ns {
                    cases.push(case!(ast::calc_nested(n, m, k)));
                }
            }
        }
        cases.push(case!(ast::boxed_opt(None::<i32>)));
    });
}
