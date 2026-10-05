/// How an expansion from text ends: the rule or DTSTART refused, or the
/// expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expanded {
    BadRule(RuleError),
    BadStart(CalendarError),
    Done(Expansion),
}

/// `parse_rrule`, `LocalDateTime::new`, then `expand`.
#[allow(clippy::too_many_arguments)]
pub fn expand_text(
    rule: &str,
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    limit: u32,
    max_periods: u32,
) -> Expanded {
    let parsed = match parse_rrule(rule) {
        Ok(r) => r,
        Err(e) => return Expanded::BadRule(e),
    };
    match LocalDateTime::new(year, month, day, hour, minute, second) {
        Ok(start) => Expanded::Done(expand(&parsed, &start, limit, max_periods)),
        Err(e) => Expanded::BadStart(e),
    }
}
