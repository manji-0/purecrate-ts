//! One test binary for the check crate: every test file is a module, so the
//! suite links once and its tests run in parallel.

mod common;

mod assoc_consts;
mod bits;
mod box_erase;
mod chars;
mod closures;
mod collect;
mod complete;
mod consts;
mod early_exit;
mod exhaustive;
mod format;
mod generics;
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
mod strings_more;
mod tuple_match;
mod types;
mod widen;
