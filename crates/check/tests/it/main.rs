//! One test binary for the check crate: every test file is a module, so the
//! suite links once and its tests run in parallel.

mod common;

mod bits;
mod box_erase;
mod chars;
mod collect;
mod closures;
mod complete;
mod consts;
mod early_exit;
mod exhaustive;
mod loops;
mod method_call;
mod names;
mod newtype;
mod option_result;
mod ordering;
mod reach;
mod resolve;
mod serde_attrs;
mod statements;
mod strings;
mod tuple_match;
mod types;
mod widen;
