//! The node driver: each case's TS call, printed as `Show` prints the Rust
//! value.

use purecrate_ir::{Crate, Item};

use super::printers::{ts_printer, ts_printers, PRELUDE};
use super::Case;

pub(super) fn driver(krate: &Crate, cases: &[Case]) -> String {
    // The TS spelling of each function (`check::rename`).
    let mut names: Vec<String> = cases.iter().map(|c| purecrate_ir::to_camel(c.name)).collect();
    names.sort();
    names.dedup();
    let mut out = String::from(PRELUDE);
    out.push_str(&ts_printers(krate));
    out.push_str(&format!("const {{ {} }} = pkg;\n", names.join(", ")));
    out.push_str("const out = [\n");
    for c in cases {
        let ret = krate
            .items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.owner.is_none() && f.name.as_str() == c.name => Some(&f.ret),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no free function `{}`", c.name));
        let print = ts_printer(krate, ret).unwrap_or_else(|| panic!("a case cannot return {ret:?}"));
        out.push_str(&format!("  run(() => {}, {print}),\n", c.call));
    }
    out.push_str("];\nconsole.log(out.join(\"\\n\"));\n");
    out
}
