//! `examples/calendar`, RFC 3339 timestamps, proleptic Gregorian dates
//! (Hinnant's algorithms), ISO 8601 week dates, and RFC 5545 recurrence
//! rules:
//!
//! - the published examples come out as printed: RFC 3339 §5.8, and the
//!   RRULE examples of RFC 5545 §3.8.5.3 that fall in this subset;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against) agree on every day count from -9999 to 9999 (a
//!   civil round trip, the weekday, and the ISO week), on timestamps built
//!   around every offset and month end, and on every rule below expanded
//!   from several starts;
//! - the generated package agrees with Rust on a sample of each.

use crate::support;

purecrate_canon::fixture!(mod calendar = "../../../examples/calendar/src/lib.rs", "fixtures/calendar_driver.rs");

/// The same model as one would write it without the subset's constraints:
/// `div_euclid`, iterators, a `sort` and `dedup` of the candidates, string
/// methods, `TryFrom`. Not converted; the reference only. Types carry the
/// names and fields of the constrained side, so `Debug` compares them.
#[allow(clippy::enum_variant_names)]
mod idiomatic {
    pub const MIN_DAYS: i64 = -4_371_587;
    pub const MAX_DAYS: i64 = 2_932_896;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CalendarError {
        YearOutOfRange,
        MonthOutOfRange,
        DayOutOfRange,
        DaysOutOfRange,
        HourOutOfRange,
        MinuteOutOfRange,
        SecondOutOfRange,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CivilDate {
        pub year: i64,
        pub month: i64,
        pub day: i64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DayOfWeek {
        Monday,
        Tuesday,
        Wednesday,
        Thursday,
        Friday,
        Saturday,
        Sunday,
    }

    const DAYS: [DayOfWeek; 7] = [
        DayOfWeek::Monday,
        DayOfWeek::Tuesday,
        DayOfWeek::Wednesday,
        DayOfWeek::Thursday,
        DayOfWeek::Friday,
        DayOfWeek::Saturday,
        DayOfWeek::Sunday,
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IsoWeekDate {
        pub year: i64,
        pub week: i64,
        pub day: DayOfWeek,
    }

    pub fn is_leap_year(y: i64) -> bool {
        y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
    }

    fn month_length(y: i64, m: i64) -> i64 {
        match m {
            2 if is_leap_year(y) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    fn to_days(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
        era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
    }

    fn to_civil(days: i64) -> CivilDate {
        let z = days + 719_468;
        let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let month = (mp + 2) % 12 + 1;
        CivilDate { year: yoe + era * 400 + i64::from(month <= 2), month, day: doy - (153 * mp + 2) / 5 + 1 }
    }

    fn checked(y: i64, m: i64, d: i64) -> Result<CivilDate, CalendarError> {
        if !(-9999..=9999).contains(&y) {
            Err(CalendarError::YearOutOfRange)
        } else if !(1..=12).contains(&m) {
            Err(CalendarError::MonthOutOfRange)
        } else if !(1..=month_length(y, m)).contains(&d) {
            Err(CalendarError::DayOutOfRange)
        } else {
            Ok(CivilDate { year: y, month: m, day: d })
        }
    }

    pub fn days_from_civil(y: i64, m: i64, d: i64) -> Result<i64, CalendarError> {
        checked(y, m, d).map(|c| to_days(c.year, c.month, c.day))
    }

    fn in_range(days: i64) -> Result<i64, CalendarError> {
        (MIN_DAYS..=MAX_DAYS).contains(&days).then_some(days).ok_or(CalendarError::DaysOutOfRange)
    }

    pub fn civil_from_days(days: i64) -> Result<CivilDate, CalendarError> {
        in_range(days).map(to_civil)
    }

    fn day_index(days: i64) -> i64 {
        (days + 3).rem_euclid(7)
    }

    pub fn weekday(days: i64) -> DayOfWeek {
        DAYS[day_index(days) as usize]
    }

    pub fn iso_week(days: i64) -> Result<IsoWeekDate, CalendarError> {
        let days = in_range(days)?;
        let thursday = days - day_index(days) + 3;
        let year = to_civil(thursday).year;
        Ok(IsoWeekDate { year, week: (thursday - to_days(year, 1, 1)) / 7 + 1, day: weekday(days) })
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TimestampError {
        Syntax(usize),
        Month,
        Day,
        Hour,
        Minute,
        Second,
        LeapSecond,
        OffsetHour,
        OffsetMinute,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Instant {
        pub seconds: i64,
        pub millis: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Offset {
        Utc,
        Numeric { negative: bool, hours: i64, minutes: i64 },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Timestamp {
        pub instant: Instant,
        pub offset: Offset,
        pub leap_second: bool,
    }

    fn offset_minutes(o: Offset) -> i64 {
        match o {
            Offset::Utc => 0,
            Offset::Numeric { negative, hours, minutes } => (hours * 60 + minutes) * if negative { -1 } else { 1 },
        }
    }

    fn num(b: &[u8], at: usize, n: usize) -> Result<i64, TimestampError> {
        match b.get(at..at + n) {
            Some(ds) if ds.iter().all(u8::is_ascii_digit) => Ok(ds.iter().fold(0, |v, d| v * 10 + i64::from(d - b'0'))),
            _ => Err(TimestampError::Syntax(at)),
        }
    }

    fn sep(b: &[u8], at: usize, ok: &[u8]) -> Result<(), TimestampError> {
        b.get(at).filter(|c| ok.contains(c)).map(|_| ()).ok_or(TimestampError::Syntax(at))
    }

    pub fn parse_rfc3339(s: &str) -> Result<Timestamp, TimestampError> {
        use TimestampError as E;
        let b = s.as_bytes();
        let year = num(b, 0, 4)?;
        sep(b, 4, b"-")?;
        let month = num(b, 5, 2)?;
        sep(b, 7, b"-")?;
        let day = num(b, 8, 2)?;
        sep(b, 10, b"Tt")?;
        let hour = num(b, 11, 2)?;
        sep(b, 13, b":")?;
        let minute = num(b, 14, 2)?;
        sep(b, 16, b":")?;
        let second = num(b, 17, 2)?;
        let mut pos = 19;
        let mut millis = 0;
        if b.get(pos) == Some(&b'.') {
            let digits = b[pos + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 {
                return Err(E::Syntax(pos + 1));
            }
            let kept: String = s[pos + 1..pos + 1 + digits.min(3)].chars().chain("00".chars()).take(3).collect();
            millis = kept.parse().expect("three digits");
            pos += 1 + digits;
        }
        let offset = match b.get(pos) {
            Some(b'Z' | b'z') if pos + 1 == b.len() => Offset::Utc,
            Some(b'Z' | b'z') => return Err(E::Syntax(pos + 1)),
            Some(&c @ (b'+' | b'-')) => {
                let hours = num(b, pos + 1, 2)?;
                sep(b, pos + 3, b":")?;
                let minutes = num(b, pos + 4, 2)?;
                if pos + 6 != b.len() {
                    return Err(E::Syntax(pos + 6));
                }
                if hours > 23 {
                    return Err(E::OffsetHour);
                }
                if minutes > 59 {
                    return Err(E::OffsetMinute);
                }
                Offset::Numeric { negative: c == b'-', hours, minutes }
            }
            _ => return Err(E::Syntax(pos)),
        };
        if !(1..=12).contains(&month) {
            return Err(E::Month);
        }
        if !(1..=month_length(year, month)).contains(&day) {
            return Err(E::Day);
        }
        for (v, max, e) in [(hour, 23, E::Hour), (minute, 59, E::Minute), (second, 60, E::Second)] {
            if v > max {
                return Err(e);
            }
        }
        let utc_minute = hour * 60 + minute - offset_minutes(offset);
        let utc_days = to_days(year, month, day) + utc_minute.div_euclid(1440);
        let minute_of_day = utc_minute.rem_euclid(1440);
        if second == 60 {
            let d = to_civil(utc_days);
            if minute_of_day != 1439 || !matches!((d.month, d.day), (6, 30) | (12, 31)) {
                return Err(E::LeapSecond);
            }
        }
        Ok(Timestamp {
            instant: Instant { seconds: utc_days * 86_400 + minute_of_day * 60 + second.min(59), millis },
            offset,
            leap_second: second == 60,
        })
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LocalDateTime {
        pub date: CivilDate,
        pub hour: i64,
        pub minute: i64,
        pub second: i64,
    }

    impl LocalDateTime {
        pub fn new(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> Result<Self, CalendarError> {
            let date = checked(y, mo, d)?;
            if !(0..24).contains(&h) {
                return Err(CalendarError::HourOutOfRange);
            }
            if !(0..60).contains(&mi) {
                return Err(CalendarError::MinuteOutOfRange);
            }
            if !(0..60).contains(&s) {
                return Err(CalendarError::SecondOutOfRange);
            }
            Ok(LocalDateTime { date, hour: h, minute: mi, second: s })
        }

        fn seconds_of_day(&self) -> i64 {
            self.hour * 3600 + self.minute * 60 + self.second
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Freq {
        Daily,
        Weekly,
        Monthly,
        Yearly,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ByDay {
        pub ordinal: i64,
        pub day: DayOfWeek,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Until {
        Date(CivilDate),
        Floating(LocalDateTime),
        Utc(LocalDateTime),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuleEnd {
        Forever,
        Count(u32),
        Until(Until),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RulePart {
        Freq,
        Interval,
        Count,
        Until,
        ByDay,
        ByMonthDay,
        ByMonth,
        Wkst,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuleError {
        MissingEquals,
        UnknownPart,
        UnsupportedPart,
        RepeatedPart(RulePart),
        InvalidValue(RulePart),
        UnsupportedFreq,
        MissingFreq,
        CountWithUntil,
        OrdinalNotAllowed,
        MonthDayWithWeekly,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Rule {
        pub freq: Freq,
        pub interval: i64,
        pub end: RuleEnd,
        pub by_day: Vec<ByDay>,
        pub by_month_day: Vec<i64>,
        pub by_month: Vec<i64>,
        pub wkst: DayOfWeek,
    }

    fn day_code(v: &str) -> Option<DayOfWeek> {
        ["MO", "TU", "WE", "TH", "FR", "SA", "SU"].iter().position(|c| v.eq_ignore_ascii_case(c)).map(|i| DAYS[i])
    }

    fn signed(s: &str, max: i64) -> Option<i64> {
        let (sign, digits) = match s.as_bytes().first() {
            Some(b'-') => (-1, &s[1..]),
            Some(b'+') => (1, &s[1..]),
            _ => (1, s),
        };
        let ok = (1..=2).contains(&digits.len()) && digits.bytes().all(|c| c.is_ascii_digit());
        let v: i64 = if ok { digits.parse().ok()? } else { None? };
        (1..=max).contains(&v).then_some(sign * v)
    }

    fn positive(s: &str) -> Option<u32> {
        let ok = !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
        if ok {
            s.parse().ok().filter(|v| *v > 0)
        } else {
            None
        }
    }

    fn by_day(v: &str) -> Option<ByDay> {
        if !v.is_ascii() || v.len() < 2 {
            return None;
        }
        let (ordinal, code) = v.split_at(v.len() - 2);
        let day = day_code(code)?;
        let ordinal = if ordinal.is_empty() { 0 } else { signed(ordinal, 53)? };
        Some(ByDay { ordinal, day })
    }

    fn until(v: &str) -> Option<Until> {
        let b = v.as_bytes();
        if ![8, 15, 16].contains(&b.len()) {
            return None;
        }
        let n = |at, len| num(b, at, len).ok();
        let date = checked(n(0, 4)?, n(4, 2)?, n(6, 2)?).ok()?;
        if b.len() == 8 {
            return Some(Until::Date(date));
        }
        if !b"Tt".contains(&b[8]) {
            return None;
        }
        let t = LocalDateTime::new(date.year, date.month, date.day, n(9, 2)?, n(11, 2)?, n(13, 2)?).ok()?;
        match b.get(15) {
            None => Some(Until::Floating(t)),
            Some(b'Z' | b'z') => Some(Until::Utc(t)),
            _ => None,
        }
    }

    fn list<T>(v: &str, f: impl Fn(&str) -> Option<T>, part: RulePart) -> Result<Vec<T>, RuleError> {
        v.split(',').map(|x| f(x).ok_or(RuleError::InvalidValue(part))).collect()
    }

    pub fn parse_rrule(s: &str) -> Result<Rule, RuleError> {
        use RuleError as E;
        use RulePart as P;
        let names = [
            ("FREQ", P::Freq),
            ("INTERVAL", P::Interval),
            ("COUNT", P::Count),
            ("UNTIL", P::Until),
            ("BYDAY", P::ByDay),
            ("BYMONTHDAY", P::ByMonthDay),
            ("BYMONTH", P::ByMonth),
            ("WKST", P::Wkst),
        ];
        let unsupported = ["BYSECOND", "BYMINUTE", "BYHOUR", "BYYEARDAY", "BYWEEKNO", "BYSETPOS"];
        let mut seen: Vec<RulePart> = Vec::new();
        let mut rule = Rule {
            freq: Freq::Daily,
            interval: 1,
            end: RuleEnd::Forever,
            by_day: vec![],
            by_month_day: vec![],
            by_month: vec![],
            wkst: DayOfWeek::Monday,
        };
        let (mut count, mut end_until) = (None, None);
        for part in s.split(';') {
            let (name, value) = part.split_once('=').ok_or(E::MissingEquals)?;
            let which = match names.iter().find(|(n, _)| name.eq_ignore_ascii_case(n)) {
                Some((_, p)) => *p,
                None if unsupported.iter().any(|n| name.eq_ignore_ascii_case(n)) => return Err(E::UnsupportedPart),
                None => return Err(E::UnknownPart),
            };
            if seen.contains(&which) {
                return Err(E::RepeatedPart(which));
            }
            seen.push(which);
            let bad = E::InvalidValue(which);
            match which {
                P::Freq => {
                    let freqs = [
                        ("DAILY", Freq::Daily),
                        ("WEEKLY", Freq::Weekly),
                        ("MONTHLY", Freq::Monthly),
                        ("YEARLY", Freq::Yearly),
                    ];
                    rule.freq = match freqs.iter().find(|(n, _)| value.eq_ignore_ascii_case(n)) {
                        Some((_, f)) => *f,
                        None if ["SECONDLY", "MINUTELY", "HOURLY"].iter().any(|n| value.eq_ignore_ascii_case(n)) => {
                            return Err(E::UnsupportedFreq)
                        }
                        None => return Err(bad),
                    }
                }
                P::Interval => rule.interval = i64::from(positive(value).ok_or(bad)?),
                P::Count => count = Some(positive(value).ok_or(bad)?),
                P::Until => end_until = Some(until(value).ok_or(bad)?),
                P::ByDay => rule.by_day = list(value, by_day, which)?,
                P::ByMonthDay => rule.by_month_day = list(value, |x| signed(x, 31), which)?,
                P::ByMonth => {
                    let month = |x: &str| {
                        let ok = (1..=2).contains(&x.len()) && x.bytes().all(|c| c.is_ascii_digit());
                        ok.then(|| x.parse::<i64>().ok()).flatten().filter(|m| (1..=12).contains(m))
                    };
                    rule.by_month = list(value, month, which)?;
                }
                P::Wkst => rule.wkst = day_code(value).ok_or(bad)?,
            }
        }
        if !seen.contains(&P::Freq) {
            return Err(E::MissingFreq);
        }
        rule.end = match (count, end_until) {
            (Some(_), Some(_)) => return Err(E::CountWithUntil),
            (Some(c), None) => RuleEnd::Count(c),
            (None, Some(u)) => RuleEnd::Until(u),
            (None, None) => RuleEnd::Forever,
        };
        if !matches!(rule.freq, Freq::Monthly | Freq::Yearly) && rule.by_day.iter().any(|d| d.ordinal != 0) {
            return Err(E::OrdinalNotAllowed);
        }
        if rule.freq == Freq::Weekly && !rule.by_month_day.is_empty() {
            return Err(E::MonthDayWithWeekly);
        }
        Ok(rule)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Stop {
        Ended,
        Limit,
        MaxPeriods,
        OutOfRange,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Expansion {
        pub occurrences: Vec<LocalDateTime>,
        pub stop: Stop,
    }

    impl Rule {
        fn month_ok(&self, m: i64) -> bool {
            self.by_month.is_empty() || self.by_month.contains(&m)
        }

        fn month_day_ok(&self, dom: i64, len: i64) -> bool {
            self.by_month_day.is_empty() || self.by_month_day.iter().any(|&x| x == dom || x == dom - len - 1)
        }

        /// BYDAY, with ordinals counted within `span`.
        fn day_ok(&self, day: i64, span: (i64, i64)) -> bool {
            self.by_day.is_empty()
                || self.by_day.iter().any(|e| {
                    weekday(day) == e.day
                        && match e.ordinal {
                            0 => true,
                            n if n > 0 => (day - span.0) / 7 + 1 == n,
                            n => (span.1 - day) / 7 + 1 == -n,
                        }
                })
        }

        fn month(&self, start: &LocalDateTime, y: i64, m: i64) -> Vec<i64> {
            if !self.month_ok(m) {
                return vec![];
            }
            let len = month_length(y, m);
            let first = to_days(y, m, 1);
            let span = (first, first + len - 1);
            (1..=len)
                .filter(|&dom| {
                    if self.by_month_day.is_empty() && self.by_day.is_empty() {
                        dom == start.date.day
                    } else {
                        self.month_day_ok(dom, len) && self.day_ok(first + dom - 1, span)
                    }
                })
                .map(|dom| first + dom - 1)
                .collect()
        }

        /// The candidate days of the period starting on `first`.
        fn period(&self, start: &LocalDateTime, start_day: i64, first: i64) -> Vec<i64> {
            let c = to_civil(first);
            let mut days = match self.freq {
                Freq::Daily => {
                    let ok = self.month_ok(c.month)
                        && self.month_day_ok(c.day, month_length(c.year, c.month))
                        && (self.by_day.is_empty() || self.by_day.iter().any(|e| e.day == weekday(first)));
                    if ok {
                        vec![first]
                    } else {
                        vec![]
                    }
                }
                Freq::Weekly => (first..first + 7)
                    .filter(|&d| self.month_ok(to_civil(d).month))
                    .filter(|&d| match self.by_day.is_empty() {
                        true => weekday(d) == weekday(start_day),
                        false => self.by_day.iter().any(|e| e.day == weekday(d)),
                    })
                    .collect(),
                Freq::Monthly => self.month(start, c.year, c.month),
                Freq::Yearly if !self.by_day.is_empty() && self.by_month.is_empty() && self.by_month_day.is_empty() => {
                    let span = (to_days(c.year, 1, 1), to_days(c.year, 12, 31));
                    (span.0..=span.1).filter(|&d| self.day_ok(d, span)).collect()
                }
                Freq::Yearly => (1..=12)
                    .filter(|&m| !self.by_month.is_empty() || !self.by_month_day.is_empty() || m == start.date.month)
                    .flat_map(|m| self.month(start, c.year, m))
                    .collect(),
            };
            days.sort_unstable();
            days.dedup();
            days
        }

        fn period_start(&self, start: &LocalDateTime, start_day: i64, k: i64) -> i64 {
            let step = k * self.interval;
            match self.freq {
                Freq::Daily => start_day + step,
                Freq::Weekly => {
                    let back = (weekday(start_day) as i64 - self.wkst as i64).rem_euclid(7);
                    start_day - back + 7 * step
                }
                Freq::Monthly => {
                    let index = start.date.year * 12 + start.date.month - 1 + step;
                    to_days(index.div_euclid(12), index.rem_euclid(12) + 1, 1)
                }
                Freq::Yearly => to_days(start.date.year + step, 1, 1),
            }
        }
    }

    pub fn expand(rule: &Rule, start: &LocalDateTime, limit: u32, max_periods: u32) -> Expansion {
        let done = |occurrences, stop| Expansion { occurrences, stop };
        let mut out = Vec::new();
        if limit == 0 {
            return done(out, Stop::Limit);
        }
        let start_day = to_days(start.date.year, start.date.month, start.date.day);
        let (last_day, last_time) = match rule.end {
            RuleEnd::Until(Until::Date(d)) => (to_days(d.year, d.month, d.day), 86_399),
            RuleEnd::Until(Until::Floating(t) | Until::Utc(t)) => {
                (to_days(t.date.year, t.date.month, t.date.day), t.seconds_of_day())
            }
            _ => (MAX_DAYS, 86_399),
        };
        let count = match rule.end {
            RuleEnd::Count(c) => c as usize,
            _ => usize::MAX,
        };
        for k in 0..max_periods {
            let first = rule.period_start(start, start_day, i64::from(k));
            if first > MAX_DAYS {
                return done(out, Stop::OutOfRange);
            }
            for day in rule.period(start, start_day, first).into_iter().filter(|&d| d >= start_day) {
                if day > MAX_DAYS {
                    return done(out, Stop::OutOfRange);
                }
                if (day, start.seconds_of_day()) > (last_day, last_time) {
                    return done(out, Stop::Ended);
                }
                out.push(LocalDateTime { date: to_civil(day), ..*start });
                if out.len() == count {
                    return done(out, Stop::Ended);
                }
                if out.len() == limit as usize {
                    return done(out, Stop::Limit);
                }
            }
        }
        done(out, Stop::MaxPeriods)
    }
}

use calendar::{CalendarError, CivilDate, DayOfWeek, Expanded, Stop};

fn dbg<T: std::fmt::Debug>(x: T) -> String {
    format!("{x:?}")
}

/// Unix seconds and milliseconds of a constrained parse, for the spec checks.
fn instant(s: &str) -> Result<(i64, u32), calendar::TimestampError> {
    calendar::parse_rfc3339(s).map(|t| (t.instant().seconds, t.instant().millis))
}

/// The dates of an expansion, as `YYYY-MM-DD`.
fn dates(rule: &str, start: (i64, i64, i64), limit: u32) -> (Vec<String>, Stop) {
    match calendar::expand_text(rule, start.0, start.1, start.2, 9, 0, 0, limit, 2000) {
        Expanded::Done(e) => (
            e.occurrences()
                .iter()
                .map(|t| format!("{:04}-{:02}-{:02}", t.date().year, t.date().month, t.date().day))
                .collect(),
            e.stop(),
        ),
        other => panic!("{rule}: {other:?}"),
    }
}

#[test]
fn the_published_examples_come_out_as_printed() {
    // RFC 3339 §5.8.
    assert_eq!(instant("1985-04-12T23:20:50.52Z"), Ok((482_196_050, 520)));
    assert_eq!(instant("1996-12-19T16:39:57-08:00"), Ok((851_042_397, 0)));
    assert_eq!(instant("1990-12-31T23:59:60Z"), Ok((662_687_999, 0)), "a leap second is 23:59:59's second");
    assert_eq!(instant("1990-12-31T15:59:60-08:00"), Ok((662_687_999, 0)), "the same leap second");
    assert_eq!(instant("1937-01-01T12:00:27.87+00:20"), Ok((-1_041_337_173, 870)));
    assert_eq!(instant("2016-12-31T23:59:60+01:00"), Err(calendar::TimestampError::LeapSecond));
    assert_eq!(instant("1985-04-12 23:20:50Z"), Err(calendar::TimestampError::Syntax(10)));

    // Hinnant: 1970-01-01 is day 0, a Thursday; the range ends.
    assert_eq!(calendar::days_from_civil(1970, 1, 1), Ok(0));
    assert_eq!(calendar::days_from_civil(-9999, 1, 1), Ok(calendar::MIN_DAYS));
    assert_eq!(calendar::civil_from_days(-1), Ok(CivilDate { year: 1969, month: 12, day: 31 }));
    assert_eq!(calendar::weekday(0), DayOfWeek::Thursday);
    assert_eq!(calendar::days_from_civil(1900, 2, 29), Err(CalendarError::DayOutOfRange));
    assert_eq!(calendar::days_from_civil(2000, 2, 29), Ok(11_016));
    // ISO 8601: 2021-01-03 is in 2020's week 53; 2024-12-30 in 2025's week 1.
    let week = |y, m, d| {
        let w = calendar::iso_week(calendar::days_from_civil(y, m, d).unwrap()).unwrap();
        (w.year, w.week, calendar::day_number(w.day))
    };
    assert_eq!(week(2021, 1, 3), (2020, 53, 7));
    assert_eq!(week(2024, 12, 30), (2025, 1, 1));
    assert_eq!(week(2008, 12, 29), (2009, 1, 1));

    // RFC 5545 §3.8.5.3, DTSTART 09:00 local.
    let d = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        dates("FREQ=DAILY;COUNT=10", (1997, 9, 2), 100).0,
        d(&[
            "1997-09-02",
            "1997-09-03",
            "1997-09-04",
            "1997-09-05",
            "1997-09-06",
            "1997-09-07",
            "1997-09-08",
            "1997-09-09",
            "1997-09-10",
            "1997-09-11"
        ])
    );
    assert_eq!(
        dates("FREQ=WEEKLY;INTERVAL=2;COUNT=8;WKST=SU;BYDAY=TU,TH", (1997, 9, 2), 100).0,
        d(&[
            "1997-09-02",
            "1997-09-04",
            "1997-09-16",
            "1997-09-18",
            "1997-09-30",
            "1997-10-02",
            "1997-10-14",
            "1997-10-16"
        ])
    );
    assert_eq!(
        dates("FREQ=MONTHLY;COUNT=10;BYDAY=1FR", (1997, 9, 5), 100).0,
        d(&[
            "1997-09-05",
            "1997-10-03",
            "1997-11-07",
            "1997-12-05",
            "1998-01-02",
            "1998-02-06",
            "1998-03-06",
            "1998-04-03",
            "1998-05-01",
            "1998-06-05"
        ])
    );
    // Friday the 13th; the RFC excludes DTSTART with EXDATE, here it is not
    // generated (a Tuesday).
    assert_eq!(
        dates("FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13", (1997, 9, 2), 5),
        (d(&["1998-02-13", "1998-03-13", "1998-11-13", "1999-08-13", "2000-10-13"]), Stop::Limit)
    );
    assert_eq!(
        dates("FREQ=MONTHLY;BYMONTHDAY=-3", (1997, 9, 28), 6).0,
        d(&["1997-09-28", "1997-10-29", "1997-11-28", "1997-12-29", "1998-01-29", "1998-02-26"])
    );
    // WKST changes which weeks INTERVAL=2 keeps.
    let wkst = |w| dates(&format!("FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST={w}"), (1997, 8, 5), 100).0;
    assert_eq!(wkst("MO"), d(&["1997-08-05", "1997-08-10", "1997-08-19", "1997-08-24"]));
    assert_eq!(wkst("SU"), d(&["1997-08-05", "1997-08-17", "1997-08-19", "1997-08-31"]));
    // "Every 20th Monday of the year".
    assert_eq!(dates("FREQ=YEARLY;BYDAY=20MO", (1997, 5, 19), 3).0, d(&["1997-05-19", "1998-05-18", "1999-05-17"]));
    // MONTHLY on the 31st skips the shorter months; an impossible rule ends.
    assert_eq!(
        dates("FREQ=MONTHLY;BYMONTHDAY=31;COUNT=4", (2024, 1, 31), 100).0,
        d(&["2024-01-31", "2024-03-31", "2024-05-31", "2024-07-31"])
    );
    assert_eq!(dates("FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=30", (2024, 1, 1), 5), (vec![], Stop::MaxPeriods));
}

fn rules() -> Vec<&'static str> {
    vec![
        "FREQ=DAILY",
        "FREQ=DAILY;INTERVAL=3;COUNT=7",
        "freq=daily;bymonth=1,2;byday=mo,fr",
        "FREQ=DAILY;BYMONTHDAY=1,-1;UNTIL=20250301",
        "FREQ=WEEKLY",
        "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=SU;COUNT=9",
        "FREQ=WEEKLY;BYDAY=MO,WE,FR;UNTIL=20240215T090000",
        "FREQ=WEEKLY;BYMONTH=12;BYDAY=SA",
        "FREQ=MONTHLY",
        "FREQ=MONTHLY;INTERVAL=5;BYMONTHDAY=29,30,31",
        "FREQ=MONTHLY;BYDAY=-1FR,2MO",
        "FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13",
        "FREQ=MONTHLY;BYMONTHDAY=-31,15;UNTIL=20240630T235959Z",
        "FREQ=YEARLY",
        "FREQ=YEARLY;BYDAY=20MO,-1SU",
        "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29",
        "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=30",
        "FREQ=YEARLY;BYMONTHDAY=1",
        "FREQ=YEARLY;INTERVAL=4;BYMONTH=11;BYDAY=1TU",
        // Refused.
        "",
        "FREQ=DAILY;",
        "INTERVAL=2",
        "FREQ=HOURLY",
        "FREQ=FORTNIGHTLY",
        "FREQ=DAILY;BYSETPOS=1",
        "FREQ=DAILY;X-NAME=1",
        "FREQ=DAILY;FREQ=WEEKLY",
        "FREQ=DAILY;COUNT=2;UNTIL=20240101",
        "FREQ=DAILY;COUNT=0",
        "FREQ=DAILY;INTERVAL=+2",
        "FREQ=WEEKLY;BYDAY=1MO",
        "FREQ=WEEKLY;BYMONTHDAY=1",
        "FREQ=MONTHLY;BYDAY=54MO",
        "FREQ=MONTHLY;BYDAY=MOO",
        "FREQ=MONTHLY;BYDAY=é1MO",
        "FREQ=MONTHLY;BYMONTHDAY=0",
        "FREQ=MONTHLY;BYMONTH=13",
        "FREQ=DAILY;UNTIL=20240230",
        "FREQ=DAILY;UNTIL=20240101T250000Z",
        "FREQ=DAILY;WKST=XX",
    ]
}

fn starts() -> Vec<(i64, i64, i64, i64, i64, i64)> {
    vec![
        (2024, 1, 31, 9, 0, 0),
        (2024, 2, 29, 23, 59, 59),
        (1997, 9, 2, 9, 0, 0),
        (-1, 12, 31, 0, 0, 0),
        (9999, 12, 1, 12, 0, 0),
        (2023, 2, 29, 0, 0, 0),
    ]
}

fn timestamps() -> Vec<String> {
    let mut out = Vec::new();
    for (y, m, d) in [(1990, 12, 31), (2016, 6, 30), (2024, 2, 29), (2023, 2, 29), (0, 1, 1), (9999, 12, 31)] {
        for time in ["00:00:00", "23:59:59", "23:59:60", "15:59:60", "08:00:60.5"] {
            for offset in ["Z", "z", "+00:00", "-00:00", "-08:00", "+09:00", "+23:59", "+24:00", "+05:60", ""] {
                out.push(format!("{y:04}-{m:02}-{d:02}T{time}{offset}"));
            }
        }
    }
    for s in [
        "1985-04-12t23:20:50.123456Z",
        "1985-04-12T23:20:50.Z",
        "1985-04-12T23:20:50.5",
        "1985-04-12T23:20:50Zx",
        "1985-13-12T23:20:50Z",
        "1985-04-12T24:20:50Z",
        "1985-04-12T23:60:50Z",
        "1985-04-12T23:20:61Z",
        "1985-04-12T23:20:50+0900",
        "85-04-12T23:20:50Z",
        "",
    ] {
        out.push(s.to_string());
    }
    out
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    // Every day count in range, and a step past each end.
    for days in calendar::MIN_DAYS - 2..=calendar::MAX_DAYS + 2 {
        let c = calendar::civil_from_days(days);
        assert_eq!(dbg(c), dbg(idiomatic::civil_from_days(days)), "civil {days}");
        if let Ok(c) = c {
            assert_eq!(calendar::days_from_civil(c.year, c.month, c.day), Ok(days));
        }
        assert_eq!(dbg(calendar::weekday(days)), dbg(idiomatic::weekday(days)));
        assert_eq!(dbg(calendar::iso_week(days)), dbg(idiomatic::iso_week(days)), "week {days}");
    }
    for y in [-10_000, -9999, -401, -400, -1, 0, 1, 1900, 2000, 9999, 10_000] {
        for m in 0..=13 {
            for d in [0, 1, 28, 29, 30, 31, 32] {
                assert_eq!(dbg(calendar::days_from_civil(y, m, d)), dbg(idiomatic::days_from_civil(y, m, d)));
            }
        }
    }
    for s in timestamps() {
        let c = calendar::parse_rfc3339(&s).map(|t| (t.instant(), t.offset(), t.is_leap_second()));
        let i = idiomatic::parse_rfc3339(&s).map(|t| (t.instant, t.offset, t.leap_second));
        assert_eq!(dbg(c), dbg(i), "{s}");
    }
    for rule in rules() {
        let c = calendar::parse_rrule(rule);
        let i = idiomatic::parse_rrule(rule);
        let rule_fields = |r: &calendar::Rule| {
            format!(
                "Rule {{ freq: {:?}, interval: {:?}, end: {:?}, by_day: {:?}, by_month_day: {:?}, by_month: {:?}, wkst: {:?} }}",
                r.freq(),
                r.interval(),
                r.end(),
                r.by_day(),
                r.by_month_day(),
                r.by_month(),
                r.week_start()
            )
        };
        assert_eq!(c.as_ref().map(rule_fields).map_err(dbg), i.as_ref().map(dbg).map_err(dbg), "{rule}");
        let (Ok(cr), Ok(ir)) = (c, i) else { continue };
        for (y, mo, d, h, mi, s) in starts() {
            let (Ok(cs), Ok(is)) =
                (calendar::LocalDateTime::new(y, mo, d, h, mi, s), idiomatic::LocalDateTime::new(y, mo, d, h, mi, s))
            else {
                continue;
            };
            for (limit, periods) in [(0, 100), (1, 100), (40, 500), (200, 30)] {
                let ce = calendar::expand(&cr, &cs, limit, periods);
                let ie = idiomatic::expand(&ir, &is, limit, periods);
                let occurrences = |o: &Vec<calendar::LocalDateTime>| {
                    o.iter()
                        .map(|t| {
                            format!(
                                "LocalDateTime {{ date: {:?}, hour: {}, minute: {}, second: {} }}",
                                t.date(),
                                t.hour(),
                                t.minute(),
                                t.second()
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let mine = format!(
                    "Expansion {{ occurrences: [{}], stop: {:?} }}",
                    occurrences(ce.occurrences()).join(", "),
                    ce.stop()
                );
                assert_eq!(mine, dbg(ie), "{rule} from {y}-{mo}-{d}");
            }
        }
    }
}

#[test]
fn calendar_matches_rust() {
    support::equivalence("calendar", calendar::SOURCE, |cases| {
        let edges = [calendar::MIN_DAYS - 1, calendar::MIN_DAYS, -719_469, -719_468, -146_097, -1, 0, 59, 11_016];
        let ends = [19_722, 19_723, 20_088, calendar::MAX_DAYS, calendar::MAX_DAYS + 1, i64::MIN / 2];
        grid!(cases, [calendar::civil_from_days, calendar::iso_week]; days in edges.iter().chain(&ends).copied());
        for days in (calendar::MIN_DAYS..=calendar::MAX_DAYS).step_by(9_973) {
            cases.push(case!(calendar::civil_from_days(days)));
            cases.push(case!(calendar::iso_week(days)));
        }
        grid!(cases, calendar::days_from_civil; y in [-10_000i64, -9999, -1, 0, 1900, 2000, 9999], m in [0i64, 1, 2, 12, 13], d in [0i64, 1, 29, 31]);
        for s in timestamps() {
            cases.push(case!(calendar::parse_rfc3339(s.as_str())));
        }
        for rule in rules() {
            cases.push(case!(calendar::parse_rrule(rule)));
            for (y, mo, d, h, mi, s) in starts() {
                cases.push(case!(calendar::expand_text(rule, y, mo, d, h, mi, s, 25u32, 300u32)));
            }
        }
    });
}
