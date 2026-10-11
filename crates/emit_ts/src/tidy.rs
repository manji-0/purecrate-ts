//! Reading printed TS where the printer still decides by its text: the
//! parentheses around a whole expression, a cast, operator precedence, a
//! top-level `&&` / `||` or `?:`, and the columns a line takes. Each reads
//! the text with strings, templates, and comments skipped, so a bracket,
//! comma, or operator inside one is never taken for code. The layout is
//! oxfmt's, run over the output by `purecrate-ts`.

mod operators;
mod scan;

pub(crate) use operators::*;
pub(crate) use scan::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_a_pair_around_everything() {
        assert_eq!(strip_outer("((a === b))"), "a === b");
        assert_eq!(strip_outer("(a) || (b)"), "(a) || (b)");
        assert_eq!(strip_outer("(\")\" + x)"), "\")\" + x");
        assert_eq!(strip_outer("f(x)"), "f(x)");
    }

    #[test]
    fn line_comment_is_not_code() {
        // An apostrophe or a lone `(` in a comment opens nothing.
        let d = depths("// don't (\nf(x)");
        assert_eq!(d[9], None);
        assert_eq!(d[10..], [Some(0), Some(0), Some(0), Some(1), Some(0)]);
    }

    #[test]
    fn a_cast_states_its_type() {
        assert_eq!(cast_type("512 as Usize"), Some("Usize"));
        assert_eq!(cast_type("Int.u8.and(x, 15 as U8) as number as Usize"), Some("Usize"));
        assert_eq!(cast_type("(b.length as Usize)"), Some("Usize"));
        assert_eq!(cast_type("a + b as I32"), None);
        assert_eq!(cast_type("c ? (1 as I32) : (2 as I32)"), None);
        assert_eq!(cast_type("f(1 as I32)"), None);
    }
}
