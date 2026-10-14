//! The node driver: each case's TS call, printed as `Show` prints the Rust
//! value.

use purecrate_ir::{Crate, Item};

use super::printers::ts_printer;
use super::Case;

pub(super) fn driver(krate: &Crate, cases: &[Case]) -> String {
    // The TS spelling of each function (`check::rename`).
    let mut names: Vec<String> = cases.iter().map(|c| purecrate_ir::to_camel(c.name)).collect();
    names.sort();
    names.dedup();
    let runs = cases.iter().map(|c| {
        let ret = krate
            .items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.owner.is_none() && f.name.as_str() == c.name => Some(&f.ret),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no free function `{}`", c.name));
        let print = ts_printer(krate, ret).unwrap_or_else(|| panic!("a case cannot return {ret:?}"));
        format!("run(() => {}, {print})", c.call)
    });
    super::printers::driver(krate, &format!("const {{ {} }} = pkg;\n", names.join(", ")), runs, "out.join(\"\\n\")")
}
