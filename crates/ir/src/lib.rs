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

pub use expr::{
    Arm, BinOp, Callee, CharMethod, ClosureParam, Cmp, Consume, Expr, Fields, FloatConst, FloatMethod, FmtAlign,
    FmtPiece, FmtSpec, IntMethod, IntOp, Lit, Over, Pattern, Pick, Pos, SliceOf, Sort, StrMethod, TreeOp, TryOn, UnOp,
    VariantBind,
};
pub use item::{
    assoc_path, dict_param, dict_ty, from_member, split_assoc_path, tuple_field, Alias, Const, Enum, Field, Fn, Item,
    Order, Param, Serde, Struct, Tagging, Trait, TraitConst, TraitImpl, TraitMethod, Variant, VariantFields, Vis,
    CONSTS_STEM, DICT_SELF, NEWTYPE_FIELD, ORDERING, SELF, STD_DICT_TRAITS,
};
pub use krate::Crate;
pub use name::{lower_first, to_camel, to_kebab, upper_first, Name};
pub use reason::Reason;
pub use ty::{FloatTy, IntTy, Prim, Ty, Wrapper};

/// Globals the emitted code reads without importing them: a crate name that
/// equals one would shadow it. Runtime globals (`Math`, `Number`, `Error`,
/// `BigInt`) are read through `globalThis`, so a crate may define `Error`;
/// `Array` is not (`Array.from`, `Array<T>`), nor are the utility types.
pub const TS_GLOBALS: &[&str] =
    &["globalThis", "Array", "Readonly", "ReadonlyArray", "Record", "Map", "Set", "ReadonlyMap", "ReadonlySet"];

/// A property key an object literal treats as the prototype, not a field.
pub const PROTO_KEY: &str = "__proto__";

/// Minimal accepting example: `step(State, Event) -> State`.
pub fn counter_example() -> Crate {
    use item::{Enum as EnumItem, Field as FieldItem, Fn as FnItem, Struct as StructItem};

    let event = Item::Enum(EnumItem {
        generics: Vec::new(),
        vis: Vis::Pub,
        name: Name::new("Event"),
        variants: vec![
            Variant { name: Name::new("Inc"), fields: VariantFields::Unit, discriminant: None, doc: None, wire: None },
            Variant { name: Name::new("Dec"), fields: VariantFields::Unit, discriminant: None, doc: None, wire: None },
            Variant {
                name: Name::new("Reset"),
                fields: VariantFields::Unit,
                discriminant: None,
                doc: None,
                wire: None,
            },
        ],
        repr: None,
        std: false,
        serde: item::Serde::default(),
        order: item::Order::None,
        tagging: Tagging::External,
        doc: None,
    });

    let state = Item::Struct(StructItem {
        generics: Vec::new(),
        vis: Vis::Pub,
        name: Name::new("State"),
        fields: vec![FieldItem { name: Name::new("n"), ty: Ty::i32(), doc: None, wire: None }],
        closed: false,
        wire_from: None,
        serde: item::Serde::default(),
        order: item::Order::None,
        doc: None,
    });

    let arm = |variant: &str, body: Expr| Arm {
        guard: None,
        pattern: Pattern::Variant { ty: Name::new("Event"), variant: Name::new(variant), bind: VariantBind::Unit },
        body,
    };

    let construct_state = |n: Expr| Expr::Construct {
        ty: Name::new("State"),
        variant: None,
        fields: Fields::Named(vec![(Name::new("n"), n)]),
        base: None,
    };

    let n_field = Expr::Field { base: Box::new(Expr::var("state")), name: Name::new("n") };

    let step = Item::Fn(FnItem {
        generics: Vec::new(),
        vis: Vis::Pub,
        name: Name::new("step"),
        owner: None,
        params: vec![
            Param { name: Name::new("state"), ty: Ty::named("State") },
            Param { name: Name::new("event"), ty: Ty::named("Event") },
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
        doc: None,
        display: false,
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
