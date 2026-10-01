//! Owned IR for a PureCrate.
//!
//! No I/O. Names are already flattened. Companion grouping is a function of
//! `Item::file_stem` and `Fn::owner`.

mod expr;
mod item;
mod krate;
mod name;
mod reason;
mod ty;

pub use expr::{Arm, BinOp, Callee, CharMethod, ClosureParam, Expr, Fields, IntMethod, IntOp, Lit, Over, Pattern, Pos, SliceOf, StrMethod, TryOn, UnOp, VariantBind};
pub use item::{Alias, Const, CONSTS_STEM, Enum, Field, Fn, Item, Param, Serde, Struct, Variant, VariantFields, Vis, NEWTYPE_FIELD, ORDERING, tuple_field};
pub use krate::Crate;
pub use name::{to_kebab, Name};
pub use reason::Reason;
pub use ty::{FloatTy, IntTy, Prim, Ty, Wrapper};

/// Globals the emitted code reads without importing them: a crate name that
/// equals one would shadow it. Runtime globals (`Math`, `Number`, `Error`,
/// `BigInt`) are read through `globalThis`, so a crate may define `Error`.
pub const TS_GLOBALS: &[&str] = &["globalThis", "Readonly", "ReadonlyArray"];

/// A property key an object literal treats as the prototype, not a field.
pub const PROTO_KEY: &str = "__proto__";

/// Minimal accepting example: `step(State, Event) -> State`.
pub fn counter_example() -> Crate {
    use item::{Enum as EnumItem, Field as FieldItem, Fn as FnItem, Struct as StructItem};

    let event = Item::Enum(EnumItem {
        vis: Vis::Pub,
        name: Name::new("Event"),
        variants: vec![
            Variant {
                name: Name::new("Inc"),
                fields: VariantFields::Unit,
                discriminant: None,
            },
            Variant {
                name: Name::new("Dec"),
                fields: VariantFields::Unit,
                discriminant: None,
            },
            Variant {
                name: Name::new("Reset"),
                fields: VariantFields::Unit,
                discriminant: None,
            },
        ],
        repr: None,
        std: false,
        serde: item::Serde::default(),
    });

    let state = Item::Struct(StructItem {
        vis: Vis::Pub,
        name: Name::new("State"),
        fields: vec![FieldItem {
            name: Name::new("n"),
            ty: Ty::i32(),
        }],
        closed: false,
        wire_from: None,
        serde: item::Serde::default(),
    });

    let arm = |variant: &str, body: Expr| Arm {
        guard: None,
        pattern: Pattern::Variant {
            ty: Name::new("Event"),
            variant: Name::new(variant),
            bind: VariantBind::Unit,
        },
        body,
    };

    let construct_state = |n: Expr| Expr::Construct {
        ty: Name::new("State"),
        variant: None,
        fields: Fields::Named(vec![(Name::new("n"), n)]),
        base: None,
    };

    let n_field = Expr::Field {
        base: Box::new(Expr::var("state")),
        name: Name::new("n"),
    };

    let step = Item::Fn(FnItem {
        vis: Vis::Pub,
        name: Name::new("step"),
        owner: None,
        params: vec![
            Param {
                name: Name::new("state"),
                ty: Ty::named("State"),
            },
            Param {
                name: Name::new("event"),
                ty: Ty::named("Event"),
            },
        ],
        ret: Ty::named("State"),
        body: Expr::Match {
            scrutinee: Box::new(Expr::var("event")),
            arms: vec![
                arm(
                    "Inc",
                    construct_state(Expr::Binary {
                        op: BinOp::Add,
                        left: Box::new(n_field.clone()),
                        right: Box::new(Expr::int(1)),
                    }),
                ),
                arm(
                    "Dec",
                    construct_state(Expr::Binary {
                        op: BinOp::Sub,
                        left: Box::new(n_field),
                        right: Box::new(Expr::int(1)),
                    }),
                ),
                arm("Reset", construct_state(Expr::int(0))),
            ],
        },
    });

    Crate::new("counter", vec![event, state, step])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_is_closed_and_flat() {
        let krate = counter_example();
        assert_eq!(krate.name.as_str(), "counter");
        assert_eq!(krate.exported().count(), 3);
        assert_eq!(krate.file_stems(), vec!["event", "state", "step"]);
        let names: Vec<&str> = krate.items.iter().map(|i| i.name().as_str()).collect();
        assert_eq!(names, ["Event", "State", "step"]);
    }
}
