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

pub use expr::{Arm, BinOp, Callee, Expr, Fields, IntOp, Lit, Pattern, TryOn, UnOp, VariantBind};
pub use item::{Alias, Enum, Field, Fn, Item, Param, Struct, Variant, VariantFields, Vis};
pub use krate::Crate;
pub use name::{to_kebab, Name};
pub use reason::Reason;
pub use ty::{FloatTy, IntTy, Prim, Ty};

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
            },
            Variant {
                name: Name::new("Dec"),
                fields: VariantFields::Unit,
            },
            Variant {
                name: Name::new("Reset"),
                fields: VariantFields::Unit,
            },
        ],
    });

    let state = Item::Struct(StructItem {
        vis: Vis::Pub,
        name: Name::new("State"),
        fields: vec![FieldItem {
            name: Name::new("n"),
            ty: Ty::i32(),
        }],
    });

    let arm = |variant: &str, body: Expr| Arm {
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
