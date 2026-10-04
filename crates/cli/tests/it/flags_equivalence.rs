//! `const` items and enum discriminants: permission flags in the shape of
//! Stoat's (a `#[repr(u64)]` enum of bits, sets built in consts with `as`),
//! implicit and negative discriminants, and consts of every accepted type.
//! Rust and the generated package agree on each function.

use crate::support;

purecrate_canon::fixture!(mod flags = "fixtures/flags.rs");

#[test]
fn generated_consts_and_discriminants_match_rust() {
    use flags::{Level, Override, Permission, Sign};
    let all = [
        Permission::ViewChannel,
        Permission::ReadMessageHistory,
        Permission::SendMessage,
        Permission::ManageMessages,
        Permission::ManageChannel,
        Permission::Connect,
        Permission::Speak,
        Permission::Masquerade,
        Permission::GrantAll,
    ];
    let sets = [0u64, 1, flags::DEFAULT_PERMISSIONS, flags::ALLOW_IN_TIMEOUT, 1 << 63, u64::MAX, 0x1234_5678_9abc_def0];
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for p in all {
            cases.push(case!(flags::bit(p)));
            for s in sets {
                cases.push(case!(flags::has(s, p)));
            }
        }
        for s in sets {
            cases.push(case!(flags::in_timeout(s)));
            cases.push(case!(flags::classify(s)));
            for allow in [0u64, 1 << 4, u64::MAX] {
                for deny in [0u64, 1, 1 << 63] {
                    cases.push(case!(flags::apply(s, Override { allow, deny })));
                }
            }
        }
        cases.push(case!(flags::defaults()));
        cases.push(case!(flags::everything()));
        cases.push(case!(flags::strict()));
        cases.push(case!(flags::wrapped()));
        for used in [0usize, 7, 15, 16, 17] {
            cases.push(case!(flags::room(used)));
        }
        for name in ["role:admin", "role", "", "rôle:x"] {
            cases.push(case!(flags::is_role(name)));
        }
        for c in [':', ';', '😀'] {
            cases.push(case!(flags::is_separator(c)));
        }
        for x in [0.0f64, 3.0, -1.25, f64::MAX] {
            cases.push(case!(flags::scaled(x)));
        }
        for x in [0i32, 13, i32::MIN, i32::MIN + 12, i32::MAX] {
            cases.push(case!(flags::shifted(x)));
        }
        for l in [Level::Low, Level::Mid, Level::High] {
            cases.push(case!(flags::level_value(l)));
            cases.push(case!(flags::level_byte(l)));
        }
        for s in [Sign::Negative, Sign::Zero, Sign::Positive] {
            cases.push(case!(flags::sign_value(s)));
        }
        cases
    });
    support::assert_equivalent("flags", flags::SOURCE, &cases);
}

#[test]
fn folded_values_are_rusts() {
    assert_eq!(flags::DEFAULT_PERMISSIONS, 0b11_0000_0000_0000_0000_0111);
    assert_eq!(flags::HALF, 7);
    assert_eq!(flags::OFFSET, -13);
    assert_eq!(flags::WRAPPED, 2);
    assert_eq!(flags::ALL_BITS, u64::MAX);
    let krate = purecrate_syntax::parse_source("flags", flags::SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let folded = |name: &str| {
        typed
            .items
            .iter()
            .find_map(|item| match item {
                purecrate_ir::Item::Const(c) if c.name.as_str() == name => Some(c.value.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no const {name}"))
    };
    let int = |value: i128, ty| {
        purecrate_ir::Expr::Lit(purecrate_ir::Lit::Int { value, ty: Some(ty), byte: false, hex: false })
    };
    use purecrate_ir::IntTy;
    assert_eq!(folded("DEFAULT_PERMISSIONS"), int(0b11_0000_0000_0000_0000_0111, IntTy::U64));
    assert_eq!(folded("ALL_BITS"), int(u64::MAX.into(), IntTy::U64));
    assert_eq!(folded("OFFSET"), int(-13, IntTy::I32));
    assert_eq!(folded("WRAPPED"), int(2, IntTy::U8));
}
