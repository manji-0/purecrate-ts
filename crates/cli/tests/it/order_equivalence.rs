//! `examples/order`, written inside the constraints of design/02, agrees
//! between Rust and the generated package on every four-command run, as the
//! summary, as the lines, and as the whole final `Order`. Its invariants
//! hold by construction: `Line` and `Order` are closed, so a value comes
//! only from `Line::new`, `Order::draft`, and `step`, in Rust and in TS. And
//! `step` reads the order, so a refused command leaves the caller's order
//! as it was.

use crate::support;

use std::collections::BTreeSet;
use std::fs;

use purecrate_check::accept;
use purecrate_pack::assemble;
use purecrate_syntax::parse_source;

purecrate_canon::fixture!(mod order = "../../../examples/order/src/lib.rs", "fixtures/order_driver.rs");

#[test]
fn generated_order_lifecycle_matches_rust() {
    let codes = 0u8..10;
    let cases = support::cases(|cases| {
        grid!(
            cases, [order::run4, order::trace4, order::lines4, order::survive];
            a in codes.clone(), b in codes.clone(), c in codes.clone(), d in codes.clone()
        );
        grid!(cases, order::open_with; code in 0u8..4);
        grid!(cases, order::edge; code in 0u8..4);
    });
    for reached in [
        "Ok(1000450)",
        "Ok(2000200)",
        "Ok(4000200)",
        "Ok(5000000)",
        "Err(OrderError::QtyZero)",
        "Err(OrderError::UnknownSku)",
        "Err(OrderError::Empty)",
        "Err(OrderError::AmountMismatch { expected: Yen(",
        "Err(OrderError::EmptyTracking)",
        "Err(OrderError::InvalidTransition)",
        "Err(OrderError::EmptySku)",
        "Err(OrderError::NegativeAmount)",
        "Ok(1000300)",
        "Err(OrderError::PriceMismatch)",
        "Err(OrderError::Overflow)",
    ] {
        assert!(cases.iter().any(|c| c.rust.starts_with(reached)), "no run reaches {reached}");
    }
    support::assert_equivalent("order", order::SOURCE, &cases);
}

/// `Line::new` refuses a zero quantity, and every order reached has one
/// line per SKU, none of quantity zero, and a total that is the sum of its
/// lines.
#[test]
fn every_reachable_order_keeps_its_invariants() {
    assert!(matches!(order::open_with(2), Err(order::OrderError::QtyZero)));
    assert!(matches!(order::open_with(3), Ok(1000300)));
    let mut placed = 0;
    for a in 0u8..10 {
        for b in 0u8..10 {
            for c in 0u8..10 {
                for d in 0u8..10 {
                    if let Ok(lines) = order::lines4(a, b, c, d) {
                        let skus: BTreeSet<&str> = lines.iter().map(|(sku, _, _)| sku.as_str()).collect();
                        assert_eq!(skus.len(), lines.len(), "a repeated SKU after {a} {b} {c} {d}: {lines:?}");
                        assert!(lines.iter().all(|&(_, _, qty)| qty > 0), "a zero quantity after {a} {b} {c} {d}");
                    }
                    let Ok(o) = order::trace4(a, b, c, d) else { continue };
                    let (lines, total) = match o.status() {
                        order::Status::Placed { lines, total }
                        | order::Status::Paid { lines, total }
                        | order::Status::Shipped { lines, total, .. } => (lines, total),
                        _ => continue,
                    };
                    let sum: i64 = lines.iter().map(|l| l.unit_price().value() * i64::from(l.qty())).sum();
                    assert_eq!(total.value(), sum, "a total off its lines after {a} {b} {c} {d}");
                    placed += 1;
                }
            }
        }
    }
    assert!(placed > 0, "no run places an order");
}

/// A refused command gives no new order, and the caller still holds the one
/// it passed, which the next command reads.
#[test]
fn a_refused_command_leaves_the_order() {
    let show = |r: Result<(order::Order, Vec<order::OrderError>), order::OrderError>| support::Show::show(&r);
    // Paying 200 against 450 is refused; paying 450 and shipping then go on
    // from the placed order the refusal left.
    let run = show(order::survive(4, 5, 6, 8));
    assert!(run.starts_with("Ok((Order(Status::Shipped {"), "{run}");
    assert!(run.contains("total: Yen(450), tracking: \"T1\" })"), "{run}");
    assert!(run.ends_with(", [OrderError::AmountMismatch { expected: Yen(450), got: Yen(200) }]))"), "{run}");
    // A refusal in a draft keeps its lines: removing an unknown SKU, then
    // placing.
    let run = show(order::survive(3, 3, 4, 9));
    assert!(run.starts_with("Ok((Order(Status::Cancelled"), "{run}");
    assert!(run.ends_with(", [OrderError::UnknownSku]))"), "{run}");
    // Every refusal of a run leaves the order the accepted commands make.
    let draft = order::Order::draft();
    let refused = order::step(&draft, order::Command::Place);
    assert!(matches!(refused, Err(order::OrderError::Empty)));
    assert!(matches!(draft.status(), order::Status::Draft { lines } if lines.is_empty()));
    let next = order::step(&draft, order::decode(0).unwrap_or_else(|_| unreachable!()));
    assert!(matches!(next.as_ref().map(|o| o.status()), Ok(order::Status::Draft { lines }) if lines.len() == 1));
}

/// What a consumer of `examples/order` can and cannot write. Each
/// `@ts-expect-error` must be an error, or `tsc` fails on the directive.
const CONSUMER: &str = r#"import { Command, Line, Order, Sku, Status, Yen, step, type U32 } from "./index.ts";
import * as pkg from "./index.ts";

declare const qty: U32;
declare const sku: Sku;
declare const price: Yen;
const line = Line.new(sku, price, qty);
if (line.kind === "Ok") {
  const draft: Order = Order.draft();
  const next = step(draft, Command.AddLine(line.value));
  if (next.kind === "Ok") {
    const status: Status = Order.status(next.value);
    void status;
  }
  const n: U32 = Line.qty(line.value);
  void n;

  // @ts-expect-error: a closed struct has no `of`
  Line.of(sku, price, qty);
  // @ts-expect-error: an object literal is not a `Line`
  const forged: Line = { sku, unit_price: price, qty };
  // @ts-expect-error: a closed newtype has no `of`
  Order.of(Status.Draft([line.value, line.value]));
  // @ts-expect-error: a `Status` built outside is not an `Order`
  const built: Order = Status.Draft([line.value, line.value]);
  // @ts-expect-error: the package-internal constructors are not exported
  pkg.unsafeMakeLine;
  // @ts-expect-error: nor is `Order`'s
  pkg.unsafeMakeOrder;
  void forged;
  void built;
}
"#;

#[test]
fn a_consumer_cannot_build_a_line_or_an_order() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let source = fs::read_to_string(support::repo().join("examples/order/src/lib.rs")).expect("read order");
    let krate = parse_source("order", &source).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("order rejected: {d:#?}"));
    let dir = support::scratch("order-consumer");
    support::write_package(&dir, &assemble(&typed));
    fs::write(dir.join("src/consumer.ts"), CONSUMER).expect("write consumer");
    support::typecheck(&dir);
    fs::remove_dir_all(&dir).ok();
}
