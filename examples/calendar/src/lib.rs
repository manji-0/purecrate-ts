// A calendar domain model: timestamps as the Internet exchanges them, civil
// dates on the proleptic Gregorian calendar, and recurrence rules as
// iCalendar writes them.
//
// Specifications:
// - RFC 3339 §5.6 date-time grammar (`full-date "T" partial-time
//   time-offset`), its note that "T" and "Z" may be lower case, §5.7 the
//   leap second (second 60 "at the end of months in which a leap second
//   occurs", at the same instant in every offset), and §4.3 "-00:00".
// - Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms"
//   (http://howardhinnant.github.io/date_algorithms.html): days_from_civil,
//   civil_from_days, weekday_from_days.
// - ISO 8601 week dates: weeks start on Monday, week 1 is the week with the
//   year's first Thursday.
// - RFC 5545 §3.3.10 RECUR (with erratum 3779: in a YEARLY rule a BYDAY
//   ordinal counts within the year unless BYMONTH is present) and §3.8.5.3
//   RRULE (a subset, below); §3.1 names are case-insensitive; §3.3.5 and
//   §3.3.12 allow second 60 for a positive leap second.
//
// Everything impure is outside: the caller reads the clock, chooses a time
// zone for a floating time, formats dates for display, and stores the
// occurrences.
//
// Policy choices, not the specifications':
// - Years are -9999..=9999 for civil dates (RFC 3339 itself only has
//   0000..=9999). Every date field is an `i64`.
// - A leap second is accepted at 23:59:60 UTC on the last day of any month.
//   §5.7 names June and December only as the months used "to date", and
//   which months will have one is announced weeks ahead, so the model does
//   not keep a table; second 60 at any other UTC time is `LeapSecond`. A
//   negative leap second (no 23:59:59) is not checked.
// - A leap second maps to the same Unix second as 23:59:59 of that UTC day;
//   the fraction is kept, and `Timestamp::is_leap_second` tells the two apart.
//   So 23:59:60.500Z and 23:59:59.500Z have the same `Instant`.
// - Fractional seconds keep the first three digits (milliseconds) and drop
//   the rest without rounding.
// - "-00:00" (unknown local offset, §4.3) is accepted; it is UTC for the
//   instant, and `Offset::Numeric { negative: true, .. }` keeps it apart from
//   "+00:00" and "Z".
// - DTSTART: §3.8.5.3 says the DTSTART value "always counts as the first
//   occurrence", and also that a recurrence set whose DTSTART is not
//   synchronized with the rule is undefined. We read this as: a synchronized
//   DTSTART is the first instance the rule generates anyway, and an
//   unsynchronized one is not emitted (nor counted by COUNT). The output is
//   only what the rule generates at or after DTSTART.
// - DTSTART is a floating local date-time, so a UNTIL with "Z" is compared
//   as if it were floating too (RFC 5545 asks for a local UNTIL in that case;
//   we accept both). A date-only UNTIL includes every occurrence on that
//   date.
// - DTSTART (a `LocalDateTime`) has no leap second (second 0..=59).
// - UNTIL may have second 60. With "Z" it must be 23:59:60 on the last day
//   of a month, as for RFC 3339 above; floating, any minute may end in a
//   leap second (the offset is unknown). It is read as second 59 of the same
//   minute, as §3.3.5 asks of implementations without leap seconds: `Until`
//   holds :59 (the leap flag is not kept, unlike `Timestamp`), so
//   "235960Z" and "235959Z" make equal rules, and an occurrence at :59 of
//   that minute is included.
// - A BYDAY ordinal counts within the month for MONTHLY, and for YEARLY
//   with BYMONTH; within the year for YEARLY without BYMONTH, whether
//   BYDAY expands or (with BYMONTHDAY) limits.
// - YEARLY with BYMONTHDAY and no BYMONTH applies the month days to every
//   month.
// - INTERVAL and COUNT must be 1..=4294967295 (u32); a larger value is
//   `InvalidValue`, not a wrap. Ordinals are 1..=53, month days 1..=31
//   (signed).
//
// Integer bounds: no input overflows. Every day count and year passes a
// range check before arithmetic, except `weekday`, which is reduced mod 7
// first and so is defined for every i64. In `expand`, `k * interval` stays
// small because the loop stops (`OutOfRange`) at the first period past
// 9999-12-31, so k * interval days never exceeds the range plus one
// INTERVAL. The one bound left to the caller: `offset_minutes` of an
// `Offset::Numeric` built by hand needs `hours * 60 + minutes` to fit in an
// i64 (it panics otherwise); `parse_rfc3339` only makes hours 0..=23 and
// minutes 0..=59.
//
// Left out on purpose:
// - FREQ=SECONDLY/MINUTELY/HOURLY, BYSECOND, BYMINUTE, BYHOUR, BYYEARDAY,
//   BYWEEKNO, BYSETPOS (rejected as unsupported), RSCALE/SKIP (RFC 7529),
//   time zones (DTSTART with TZID), RDATE and EXDATE.
// - Formatting back to text.

// ---------------------------------------------------------------------------
// Civil dates (Hinnant)
// ---------------------------------------------------------------------------

pub const MIN_YEAR: i64 = -9999;
pub const MAX_YEAR: i64 = 9999;
/// Days from 1970-01-01 to -9999-01-01.
pub const MIN_DAYS: i64 = -4371587;
/// Days from 1970-01-01 to 9999-12-31.
pub const MAX_DAYS: i64 = 2932896;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarError {
    YearOutOfRange,
    MonthOutOfRange,
    DayOutOfRange,
    /// A day count outside `MIN_DAYS..=MAX_DAYS`.
    DaysOutOfRange,
    HourOutOfRange,
    MinuteOutOfRange,
    SecondOutOfRange,
}

/// A date on the proleptic Gregorian calendar. Year 0 is 1 BC.
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

/// An ISO 8601 week date. `year` is the ISO week-numbering year, which can
/// differ from the civil year near January 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoWeekDate {
    pub year: i64,
    pub week: i64,
    pub day: DayOfWeek,
}

/// Division rounding toward negative infinity, for a positive divisor
/// (Rust's `/` truncates toward zero).
fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b < 0 {
        q - 1
    } else {
        q
    }
}

/// The remainder of `floor_div`, in `0..b` for a positive divisor.
fn floor_mod(a: i64, b: i64) -> i64 {
    let r = a % b;
    if r < 0 {
        r + b
    } else {
        r
    }
}

pub fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Days in a month; the month must be 1..=12.
fn month_length(year: i64, month: i64) -> i64 {
    match month {
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Hinnant's days_from_civil, without range checks. Correct for any year
/// whose day count fits in an i64.
fn civil_to_days(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Hinnant's civil_from_days, without range checks.
fn days_to_civil(days: i64) -> CivilDate {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let carry: i64 = if month <= 2 { 1 } else { 0 };
    CivilDate {
        year: yoe + era * 400 + carry,
        month,
        day,
    }
}

fn checked_civil(year: i64, month: i64, day: i64) -> Result<CivilDate, CalendarError> {
    if year < MIN_YEAR || year > MAX_YEAR {
        return Err(CalendarError::YearOutOfRange);
    }
    if month < 1 || month > 12 {
        return Err(CalendarError::MonthOutOfRange);
    }
    if day < 1 || day > month_length(year, month) {
        return Err(CalendarError::DayOutOfRange);
    }
    Ok(CivilDate { year, month, day })
}

/// Days since 1970-01-01 (negative before it).
pub fn days_from_civil(year: i64, month: i64, day: i64) -> Result<i64, CalendarError> {
    let d = checked_civil(year, month, day)?;
    Ok(civil_to_days(d.year, d.month, d.day))
}

pub fn civil_from_days(days: i64) -> Result<CivilDate, CalendarError> {
    if days < MIN_DAYS || days > MAX_DAYS {
        return Err(CalendarError::DaysOutOfRange);
    }
    Ok(days_to_civil(days))
}

/// ISO day number, Monday = 1 .. Sunday = 7. 1970-01-01 was a Thursday.
/// Reduced before the shift, so no day count overflows.
fn iso_day_number(days: i64) -> i64 {
    floor_mod(floor_mod(days, 7) + 3, 7) + 1
}

/// The ISO number of a weekday, Monday = 1 .. Sunday = 7.
pub fn day_number(d: DayOfWeek) -> i64 {
    match d {
        DayOfWeek::Monday => 1,
        DayOfWeek::Tuesday => 2,
        DayOfWeek::Wednesday => 3,
        DayOfWeek::Thursday => 4,
        DayOfWeek::Friday => 5,
        DayOfWeek::Saturday => 6,
        DayOfWeek::Sunday => 7,
    }
}

/// The weekday for an ISO number; the number must be 1..=7.
fn day_from_number(n: i64) -> DayOfWeek {
    match n {
        1 => DayOfWeek::Monday,
        2 => DayOfWeek::Tuesday,
        3 => DayOfWeek::Wednesday,
        4 => DayOfWeek::Thursday,
        5 => DayOfWeek::Friday,
        6 => DayOfWeek::Saturday,
        _ => DayOfWeek::Sunday,
    }
}

/// The weekday of a day count. Defined for every day count, in range or not.
pub fn weekday(days: i64) -> DayOfWeek {
    day_from_number(iso_day_number(days))
}

/// The ISO 8601 week date of a day count: the week belongs to the year of
/// its Thursday.
pub fn iso_week(days: i64) -> Result<IsoWeekDate, CalendarError> {
    if days < MIN_DAYS || days > MAX_DAYS {
        return Err(CalendarError::DaysOutOfRange);
    }
    let wd = iso_day_number(days);
    let thursday = days - wd + 4;
    let year = days_to_civil(thursday).year;
    let week = (thursday - civil_to_days(year, 1, 1)) / 7 + 1;
    Ok(IsoWeekDate {
        year,
        week,
        day: day_from_number(wd),
    })
}

// ---------------------------------------------------------------------------
// RFC 3339 timestamps
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampError {
    /// The grammar fails at this byte position.
    Syntax(usize),
    Month,
    Day,
    Hour,
    Minute,
    Second,
    /// Second 60 where the UTC time is not 23:59:60 on a month's last day.
    LeapSecond,
    OffsetHour,
    OffsetMinute,
}

/// A point in time: seconds since 1970-01-01T00:00:00Z (leap seconds not
/// counted, as POSIX) and milliseconds into that second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instant {
    pub seconds: i64,
    pub millis: u32,
}

/// The time-offset as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offset {
    /// "Z" or "z".
    Utc,
    /// "+hh:mm" or "-hh:mm"; "-00:00" is the unknown local offset (§4.3).
    Numeric {
        negative: bool,
        hours: i64,
        minutes: i64,
    },
}

/// A valid RFC 3339 date-time. Only [`parse_rfc3339`] makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    instant: Instant,
    offset: Offset,
    leap_second: bool,
}

impl Timestamp {
    pub fn instant(&self) -> Instant {
        self.instant
    }

    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub fn is_leap_second(&self) -> bool {
        self.leap_second
    }
}

/// Minutes east of UTC. For a hand-built offset, `hours * 60 + minutes`
/// must fit in an i64 (see the header).
pub fn offset_minutes(o: Offset) -> i64 {
    match o {
        Offset::Utc => 0,
        Offset::Numeric {
            negative: true,
            hours,
            minutes,
        } => -(hours * 60 + minutes),
        Offset::Numeric {
            negative: false,
            hours,
            minutes,
        } => hours * 60 + minutes,
    }
}

/// `n` ASCII digits at `start`, or `None` when they are not there.
fn digits_at(b: &[u8], start: usize, n: usize) -> Option<i64> {
    if start + n > b.len() {
        return None;
    }
    let mut v: i64 = 0;
    for i in start..start + n {
        if !matches!(b[i], b'0'..=b'9') {
            return None;
        }
        v = v * 10 + i64::from(b[i] - b'0');
    }
    Some(v)
}

fn field(b: &[u8], start: usize, n: usize) -> Result<i64, TimestampError> {
    digits_at(b, start, n).ok_or(TimestampError::Syntax(start))
}

fn separator(b: &[u8], i: usize, x: u8, y: u8) -> Result<(), TimestampError> {
    if i < b.len() && (b[i] == x || b[i] == y) {
        Ok(())
    } else {
        Err(TimestampError::Syntax(i))
    }
}

fn parse_offset(b: &[u8], pos: usize) -> Result<Offset, TimestampError> {
    if pos >= b.len() {
        return Err(TimestampError::Syntax(pos));
    }
    let c = b[pos];
    if c == b'Z' || c == b'z' {
        if pos + 1 != b.len() {
            return Err(TimestampError::Syntax(pos + 1));
        }
        return Ok(Offset::Utc);
    }
    if c != b'+' && c != b'-' {
        return Err(TimestampError::Syntax(pos));
    }
    let hours = field(b, pos + 1, 2)?;
    separator(b, pos + 3, b':', b':')?;
    let minutes = field(b, pos + 4, 2)?;
    if pos + 6 != b.len() {
        return Err(TimestampError::Syntax(pos + 6));
    }
    if hours > 23 {
        return Err(TimestampError::OffsetHour);
    }
    if minutes > 59 {
        return Err(TimestampError::OffsetMinute);
    }
    Ok(Offset::Numeric {
        negative: c == b'-',
        hours,
        minutes,
    })
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.fraction](Z|+hh:mm|-hh:mm)` (RFC 3339 §5.6).
pub fn parse_rfc3339(s: &str) -> Result<Timestamp, TimestampError> {
    let b = s.as_bytes();
    let year = field(b, 0, 4)?;
    separator(b, 4, b'-', b'-')?;
    let month = field(b, 5, 2)?;
    separator(b, 7, b'-', b'-')?;
    let day = field(b, 8, 2)?;
    separator(b, 10, b'T', b't')?;
    let hour = field(b, 11, 2)?;
    separator(b, 13, b':', b':')?;
    let minute = field(b, 14, 2)?;
    separator(b, 16, b':', b':')?;
    let second = field(b, 17, 2)?;
    let mut pos: usize = 19;
    let mut millis: u32 = 0;
    if pos < b.len() && b[pos] == b'.' {
        pos += 1;
        let first = pos;
        while pos < b.len() && matches!(b[pos], b'0'..=b'9') {
            if pos - first < 3 {
                millis = millis * 10 + u32::from(b[pos] - b'0');
            }
            pos += 1;
        }
        if pos == first {
            return Err(TimestampError::Syntax(pos));
        }
        let mut taken = pos - first;
        while taken < 3 {
            millis *= 10;
            taken += 1;
        }
    }
    let offset = parse_offset(b, pos)?;
    if month < 1 || month > 12 {
        return Err(TimestampError::Month);
    }
    if day < 1 || day > month_length(year, month) {
        return Err(TimestampError::Day);
    }
    if hour > 23 {
        return Err(TimestampError::Hour);
    }
    if minute > 59 {
        return Err(TimestampError::Minute);
    }
    if second > 60 {
        return Err(TimestampError::Second);
    }
    // Minutes from the start of the local day to the UTC time; the UTC day
    // may be the day before or after.
    let utc_minute = hour * 60 + minute - offset_minutes(offset);
    let utc_days = civil_to_days(year, month, day) + floor_div(utc_minute, 1440);
    let minute_of_day = floor_mod(utc_minute, 1440);
    if second == 60 {
        let d = days_to_civil(utc_days);
        if minute_of_day != 1439 || d.day != month_length(d.year, d.month) {
            return Err(TimestampError::LeapSecond);
        }
    }
    Ok(Timestamp {
        instant: Instant {
            seconds: utc_days * 86400 + minute_of_day * 60 + second.min(59),
            millis,
        },
        offset,
        leap_second: second == 60,
    })
}

// ---------------------------------------------------------------------------
// RFC 5545 recurrence rules
// ---------------------------------------------------------------------------

/// A floating local date-time (no time zone), as DTSTART is here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    date: CivilDate,
    hour: i64,
    minute: i64,
    second: i64,
}

impl LocalDateTime {
    pub fn new(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
    ) -> Result<LocalDateTime, CalendarError> {
        let date = checked_civil(year, month, day)?;
        if hour < 0 || hour > 23 {
            return Err(CalendarError::HourOutOfRange);
        }
        if minute < 0 || minute > 59 {
            return Err(CalendarError::MinuteOutOfRange);
        }
        if second < 0 || second > 59 {
            return Err(CalendarError::SecondOutOfRange);
        }
        Ok(LocalDateTime {
            date,
            hour,
            minute,
            second,
        })
    }

    pub fn date(&self) -> CivilDate {
        self.date
    }

    pub fn hour(&self) -> i64 {
        self.hour
    }

    pub fn minute(&self) -> i64 {
        self.minute
    }

    pub fn second(&self) -> i64 {
        self.second
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// One BYDAY entry: `ordinal` 0 is every such weekday, otherwise the n-th
/// (negative: from the end) in the month or year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByDay {
    pub ordinal: i64,
    pub day: DayOfWeek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    Date(CivilDate),
    Floating(LocalDateTime),
    /// Written with "Z"; compared as floating (see the header).
    Utc(LocalDateTime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleEnd {
    Forever,
    Count(u32),
    Until(Until),
}

/// The rule parts this crate reads, named in errors.
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
    /// A part without "=" (this includes an empty part, as in a trailing ";").
    MissingEquals,
    UnknownPart,
    /// A part RFC 5545 defines that this crate does not implement.
    UnsupportedPart,
    RepeatedPart(RulePart),
    InvalidValue(RulePart),
    /// SECONDLY, MINUTELY, HOURLY.
    UnsupportedFreq,
    MissingFreq,
    CountWithUntil,
    /// A BYDAY ordinal with FREQ other than MONTHLY or YEARLY.
    OrdinalNotAllowed,
    /// BYMONTHDAY with FREQ=WEEKLY.
    MonthDayWithWeekly,
}

/// A valid recurrence rule. Only [`parse_rrule`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    freq: Freq,
    interval: i64,
    end: RuleEnd,
    by_day: Vec<ByDay>,
    by_month_day: Vec<i64>,
    by_month: Vec<i64>,
    wkst: DayOfWeek,
}

impl Rule {
    pub fn freq(&self) -> Freq {
        self.freq
    }

    pub fn interval(&self) -> i64 {
        self.interval
    }

    pub fn end(&self) -> RuleEnd {
        self.end
    }

    pub fn by_day(&self) -> &Vec<ByDay> {
        &self.by_day
    }

    pub fn by_month_day(&self) -> &Vec<i64> {
        &self.by_month_day
    }

    pub fn by_month(&self) -> &Vec<i64> {
        &self.by_month
    }

    pub fn week_start(&self) -> DayOfWeek {
        self.wkst
    }
}

fn part_name(name: &str) -> Result<RulePart, RuleError> {
    let names = ["FREQ", "INTERVAL", "COUNT", "UNTIL", "BYDAY", "BYMONTHDAY", "BYMONTH", "WKST"];
    let parts = vec![
        RulePart::Freq,
        RulePart::Interval,
        RulePart::Count,
        RulePart::Until,
        RulePart::ByDay,
        RulePart::ByMonthDay,
        RulePart::ByMonth,
        RulePart::Wkst,
    ];
    if let Some(i) = names.iter().position(|n| name.eq_ignore_ascii_case(n)) {
        return Ok(parts[i]);
    }
    let unsupported = ["BYSECOND", "BYMINUTE", "BYHOUR", "BYYEARDAY", "BYWEEKNO", "BYSETPOS"];
    if unsupported.iter().any(|u| name.eq_ignore_ascii_case(u)) {
        return Err(RuleError::UnsupportedPart);
    }
    Err(RuleError::UnknownPart)
}

fn parse_freq(v: &str) -> Result<Freq, RuleError> {
    let names = ["DAILY", "WEEKLY", "MONTHLY", "YEARLY"];
    let freqs = vec![Freq::Daily, Freq::Weekly, Freq::Monthly, Freq::Yearly];
    if let Some(i) = names.iter().position(|n| v.eq_ignore_ascii_case(n)) {
        return Ok(freqs[i]);
    }
    if ["SECONDLY", "MINUTELY", "HOURLY"].iter().any(|n| v.eq_ignore_ascii_case(n)) {
        return Err(RuleError::UnsupportedFreq);
    }
    Err(RuleError::InvalidValue(RulePart::Freq))
}

fn day_code(v: &str) -> Option<DayOfWeek> {
    let i = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"].iter().position(|c| v.eq_ignore_ascii_case(c))?;
    let days = vec![
        DayOfWeek::Monday,
        DayOfWeek::Tuesday,
        DayOfWeek::Wednesday,
        DayOfWeek::Thursday,
        DayOfWeek::Friday,
        DayOfWeek::Saturday,
        DayOfWeek::Sunday,
    ];
    Some(days[i])
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| matches!(b, b'0'..=b'9'))
}

/// A positive integer written as 1*DIGIT (no sign).
fn parse_positive(s: &str) -> Option<u32> {
    if !all_digits(s) {
        return None;
    }
    let v = s.parse::<u32>().ok()?;
    if v == 0 {
        return None;
    }
    Some(v)
}

/// `[+/-] 1*2DIGIT` with a value in `1..=max`.
fn parse_signed(s: &str, max: i64) -> Option<i64> {
    let (negative, rest) = match s.strip_prefix("-") {
        Some(r) => (true, r),
        None => (false, s.strip_prefix("+").unwrap_or(s)),
    };
    let b = rest.as_bytes();
    if b.is_empty() || b.len() > 2 {
        return None;
    }
    let v = digits_at(b, 0, b.len())?;
    if v < 1 || v > max {
        return None;
    }
    Some(if negative { -v } else { v })
}

fn parse_by_day(v: &str) -> Result<ByDay, RuleError> {
    let bad = RuleError::InvalidValue(RulePart::ByDay);
    // Only ASCII, so the slices below fall on character boundaries.
    if !v.bytes().all(|c| c < 128) {
        return Err(bad);
    }
    let n = v.len();
    if n < 2 {
        return Err(bad);
    }
    let day = day_code(&v[n - 2..n]).ok_or(bad)?;
    if n == 2 {
        return Ok(ByDay { ordinal: 0, day });
    }
    let ordinal = parse_signed(&v[0..n - 2], 53).ok_or(bad)?;
    Ok(ByDay { ordinal, day })
}

fn parse_month_day(v: &str) -> Result<i64, RuleError> {
    parse_signed(v, 31).ok_or(RuleError::InvalidValue(RulePart::ByMonthDay))
}

fn parse_month(v: &str) -> Result<i64, RuleError> {
    let bad = RuleError::InvalidValue(RulePart::ByMonth);
    let b = v.as_bytes();
    if b.is_empty() || b.len() > 2 {
        return Err(bad);
    }
    let m = digits_at(b, 0, b.len()).ok_or(bad)?;
    if m < 1 || m > 12 {
        return Err(bad);
    }
    Ok(m)
}

/// `YYYYMMDD`, `YYYYMMDDTHHMMSS`, or `YYYYMMDDTHHMMSSZ`.
fn parse_until(v: &str) -> Result<Until, RuleError> {
    let bad = RuleError::InvalidValue(RulePart::Until);
    let b = v.as_bytes();
    if b.len() != 8 && b.len() != 15 && b.len() != 16 {
        return Err(bad);
    }
    let year = digits_at(b, 0, 4).ok_or(bad)?;
    let month = digits_at(b, 4, 2).ok_or(bad)?;
    let day = digits_at(b, 6, 2).ok_or(bad)?;
    let date = checked_civil(year, month, day).map_err(|_| bad)?;
    if b.len() == 8 {
        return Ok(Until::Date(date));
    }
    if b[8] != b'T' && b[8] != b't' {
        return Err(bad);
    }
    let hour = digits_at(b, 9, 2).ok_or(bad)?;
    let minute = digits_at(b, 11, 2).ok_or(bad)?;
    let second = digits_at(b, 13, 2).ok_or(bad)?;
    let utc = b.len() == 16;
    if utc && b[15] != b'Z' && b[15] != b'z' {
        return Err(bad);
    }
    if second > 60 {
        return Err(bad);
    }
    if second == 60 && utc {
        if hour != 23 || minute != 59 || day != month_length(year, month) {
            return Err(bad);
        }
    }
    // A leap second is read as second 59 (RFC 5545 §3.3.5).
    let t = LocalDateTime::new(year, month, day, hour, minute, second.min(59)).map_err(|_| bad)?;
    if utc {
        Ok(Until::Utc(t))
    } else {
        Ok(Until::Floating(t))
    }
}

/// Parses an RFC 5545 RECUR value such as
/// `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=6`.
pub fn parse_rrule(s: &str) -> Result<Rule, RuleError> {
    let mut freq: Option<Freq> = None;
    let mut interval: Option<u32> = None;
    let mut count: Option<u32> = None;
    let mut until: Option<Until> = None;
    let mut by_day: Option<Vec<ByDay>> = None;
    let mut by_month_day: Option<Vec<i64>> = None;
    let mut by_month: Option<Vec<i64>> = None;
    let mut wkst: Option<DayOfWeek> = None;
    for part in s.split(';') {
        match part.split_once('=') {
            None => return Err(RuleError::MissingEquals),
            Some((name, value)) => {
                let which = part_name(name)?;
                let repeated = match which {
                    RulePart::Freq => freq.is_some(),
                    RulePart::Interval => interval.is_some(),
                    RulePart::Count => count.is_some(),
                    RulePart::Until => until.is_some(),
                    RulePart::ByDay => by_day.is_some(),
                    RulePart::ByMonthDay => by_month_day.is_some(),
                    RulePart::ByMonth => by_month.is_some(),
                    RulePart::Wkst => wkst.is_some(),
                };
                if repeated {
                    return Err(RuleError::RepeatedPart(which));
                }
                match which {
                    RulePart::Freq => {
                        let f = parse_freq(value)?;
                        freq = Some(f);
                    }
                    RulePart::Interval => {
                        let n = parse_positive(value).ok_or(RuleError::InvalidValue(RulePart::Interval))?;
                        interval = Some(n);
                    }
                    RulePart::Count => {
                        let n = parse_positive(value).ok_or(RuleError::InvalidValue(RulePart::Count))?;
                        count = Some(n);
                    }
                    RulePart::Until => {
                        let u = parse_until(value)?;
                        until = Some(u);
                    }
                    RulePart::ByDay => {
                        let days = value.split(',').map(parse_by_day).collect::<Result<Vec<_>, _>>()?;
                        by_day = Some(days);
                    }
                    RulePart::ByMonthDay => {
                        let days = value.split(',').map(parse_month_day).collect::<Result<Vec<_>, _>>()?;
                        by_month_day = Some(days);
                    }
                    RulePart::ByMonth => {
                        let months = value.split(',').map(parse_month).collect::<Result<Vec<_>, _>>()?;
                        by_month = Some(months);
                    }
                    RulePart::Wkst => {
                        let d = day_code(value).ok_or(RuleError::InvalidValue(RulePart::Wkst))?;
                        wkst = Some(d);
                    }
                }
            }
        }
    }
    let freq = match freq {
        Some(f) => f,
        None => return Err(RuleError::MissingFreq),
    };
    let end = match (count, until) {
        (Some(_), Some(_)) => return Err(RuleError::CountWithUntil),
        (Some(c), None) => RuleEnd::Count(c),
        (None, Some(u)) => RuleEnd::Until(u),
        (None, None) => RuleEnd::Forever,
    };
    let by_day = by_day.unwrap_or(Vec::new());
    let by_month_day = by_month_day.unwrap_or(Vec::new());
    let ordinal_allowed = matches!(freq, Freq::Monthly | Freq::Yearly);
    if !ordinal_allowed && by_day.iter().any(|d| d.ordinal != 0) {
        return Err(RuleError::OrdinalNotAllowed);
    }
    if matches!(freq, Freq::Weekly) && !by_month_day.is_empty() {
        return Err(RuleError::MonthDayWithWeekly);
    }
    Ok(Rule {
        freq,
        interval: i64::from(interval.unwrap_or(1)),
        end,
        by_day,
        by_month_day,
        by_month: by_month.unwrap_or(Vec::new()),
        wkst: wkst.unwrap_or(DayOfWeek::Monday),
    })
}

// ---------------------------------------------------------------------------
// Expansion
// ---------------------------------------------------------------------------

/// Why an expansion stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// COUNT reached or UNTIL passed: the set is complete.
    Ended,
    /// The caller's `limit` of occurrences was reached.
    Limit,
    /// The caller's `max_periods` were examined.
    MaxPeriods,
    /// The next occurrence would fall after 9999-12-31.
    OutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    occurrences: Vec<LocalDateTime>,
    stop: Stop,
}

impl Expansion {
    pub fn occurrences(&self) -> &Vec<LocalDateTime> {
        &self.occurrences
    }

    pub fn stop(&self) -> Stop {
        self.stop
    }
}

fn listed(xs: &Vec<i64>, v: i64) -> bool {
    xs.iter().any(|x| *x == v)
}

/// BYMONTHDAY: `dom` of a month of `len` days is in the list (negative
/// entries count from the end: -1 is the last day).
fn month_day_listed(xs: &Vec<i64>, dom: i64, len: i64) -> bool {
    xs.iter().any(|x| *x == dom || *x == dom - len - 1)
}

/// BYDAY without ordinals (DAILY, WEEKLY).
fn weekday_listed(xs: &Vec<ByDay>, day: i64) -> bool {
    let w = iso_day_number(day);
    xs.iter().any(|e| day_number(e.day) == w)
}

fn ordinal_matches(ordinal: i64, day: i64, first: i64, last: i64) -> bool {
    if ordinal > 0 {
        (day - first) / 7 + 1 == ordinal
    } else if ordinal < 0 {
        (last - day) / 7 + 1 == -ordinal
    } else {
        true
    }
}

/// BYDAY with ordinals counted within `first..=last` (a month or a year).
fn by_day_matches(xs: &Vec<ByDay>, day: i64, first: i64, last: i64) -> bool {
    let w = iso_day_number(day);
    xs.iter().any(|e| day_number(e.day) == w && ordinal_matches(e.ordinal, day, first, last))
}

/// The first day of period `k` (0 is DTSTART's period).
fn period_first_day(rule: &Rule, start: &LocalDateTime, start_day: i64, k: i64) -> i64 {
    let step = k * rule.interval;
    match rule.freq {
        Freq::Daily => start_day + step,
        Freq::Weekly => {
            let back = floor_mod(iso_day_number(start_day) - day_number(rule.wkst), 7);
            start_day - back + 7 * step
        }
        Freq::Monthly => {
            let index = start.date.year * 12 + start.date.month - 1 + step;
            civil_to_days(floor_div(index, 12), floor_mod(index, 12) + 1, 1)
        }
        Freq::Yearly => civil_to_days(start.date.year + step, 1, 1),
    }
}

/// The days of one month the rule selects, in order (MONTHLY, and YEARLY
/// per month).
fn month_days(rule: &Rule, start: &LocalDateTime, year: i64, month: i64) -> Vec<i64> {
    let mut v: Vec<i64> = Vec::new();
    if !rule.by_month.is_empty() && !listed(&rule.by_month, month) {
        return v;
    }
    let len = month_length(year, month);
    let first = civil_to_days(year, month, 1);
    let last = first + len - 1;
    let plain = rule.by_month_day.is_empty() && rule.by_day.is_empty();
    for dom in 1..len + 1 {
        let day = first + dom - 1;
        let selected = if plain {
            dom == start.date.day
        } else {
            (rule.by_month_day.is_empty() || month_day_listed(&rule.by_month_day, dom, len))
                && (rule.by_day.is_empty() || by_day_matches(&rule.by_day, day, first, last))
        };
        if selected {
            v.push(day);
        }
    }
    v
}

fn year_days(rule: &Rule, start: &LocalDateTime, year: i64) -> Vec<i64> {
    let mut v: Vec<i64> = Vec::new();
    if !rule.by_day.is_empty() && rule.by_month.is_empty() {
        // Without BYMONTH, BYDAY ordinals count within the year (erratum
        // 3779), whether BYDAY expands or BYMONTHDAY makes it a limit.
        let first = civil_to_days(year, 1, 1);
        let last = civil_to_days(year, 12, 31);
        for month in 1..13i64 {
            let len = month_length(year, month);
            let month_first = civil_to_days(year, month, 1);
            for dom in 1..len + 1 {
                let day = month_first + dom - 1;
                let selected = (rule.by_month_day.is_empty() || month_day_listed(&rule.by_month_day, dom, len))
                    && by_day_matches(&rule.by_day, day, first, last);
                if selected {
                    v.push(day);
                }
            }
        }
        return v;
    }
    for month in 1..13i64 {
        let wanted = !rule.by_month.is_empty() || !rule.by_month_day.is_empty() || month == start.date.month;
        if wanted {
            let days = month_days(rule, start, year, month);
            for d in &days {
                v.push(*d);
            }
        }
    }
    v
}

/// The candidate days of the period starting on `first`, in order.
fn period_days(rule: &Rule, start: &LocalDateTime, start_day: i64, first: i64) -> Vec<i64> {
    let mut v: Vec<i64> = Vec::new();
    match rule.freq {
        Freq::Daily => {
            let c = days_to_civil(first);
            let selected = (rule.by_month.is_empty() || listed(&rule.by_month, c.month))
                && (rule.by_month_day.is_empty()
                    || month_day_listed(&rule.by_month_day, c.day, month_length(c.year, c.month)))
                && (rule.by_day.is_empty() || weekday_listed(&rule.by_day, first));
            if selected {
                v.push(first);
            }
            v
        }
        Freq::Weekly => {
            for day in first..first + 7 {
                let c = days_to_civil(day);
                let selected = (rule.by_month.is_empty() || listed(&rule.by_month, c.month))
                    && (if rule.by_day.is_empty() {
                        iso_day_number(day) == iso_day_number(start_day)
                    } else {
                        weekday_listed(&rule.by_day, day)
                    });
                if selected {
                    v.push(day);
                }
            }
            v
        }
        Freq::Monthly => {
            let c = days_to_civil(first);
            month_days(rule, start, c.year, c.month)
        }
        Freq::Yearly => year_days(rule, start, days_to_civil(first).year),
    }
}

/// The last day an occurrence may fall on, and the last second of the day
/// on that day.
fn until_day(end: RuleEnd) -> i64 {
    match end {
        RuleEnd::Until(Until::Date(d)) => civil_to_days(d.year, d.month, d.day),
        RuleEnd::Until(Until::Floating(t)) => civil_to_days(t.date.year, t.date.month, t.date.day),
        RuleEnd::Until(Until::Utc(t)) => civil_to_days(t.date.year, t.date.month, t.date.day),
        _ => MAX_DAYS,
    }
}

fn until_time(end: RuleEnd) -> i64 {
    match end {
        RuleEnd::Until(Until::Floating(t)) => t.hour * 3600 + t.minute * 60 + t.second,
        RuleEnd::Until(Until::Utc(t)) => t.hour * 3600 + t.minute * 60 + t.second,
        _ => 86399,
    }
}

/// Occurrences of `rule` from `dtstart`, in chronological order, until the
/// rule ends, `limit` occurrences are produced, `max_periods` periods
/// (days, weeks, months, or years of the rule's FREQ, INTERVAL apart) are
/// examined, or the year 9999 is passed.
pub fn expand(rule: &Rule, dtstart: &LocalDateTime, limit: u32, max_periods: u32) -> Expansion {
    let mut out: Vec<LocalDateTime> = Vec::new();
    if limit == 0 {
        return Expansion {
            occurrences: out,
            stop: Stop::Limit,
        };
    }
    let start_day = civil_to_days(dtstart.date.year, dtstart.date.month, dtstart.date.day);
    let time = dtstart.hour * 3600 + dtstart.minute * 60 + dtstart.second;
    let last_day = until_day(rule.end);
    let last_time = until_time(rule.end);
    let count: u32 = match rule.end {
        RuleEnd::Count(c) => c,
        _ => 0,
    };
    let mut emitted: u32 = 0;
    let mut periods: u32 = 0;
    while periods < max_periods {
        let first = period_first_day(rule, dtstart, start_day, i64::from(periods));
        if first > MAX_DAYS {
            return Expansion {
                occurrences: out,
                stop: Stop::OutOfRange,
            };
        }
        let days = period_days(rule, dtstart, start_day, first);
        for d in &days {
            let day = *d;
            if day < start_day {
                continue;
            }
            if day > MAX_DAYS {
                return Expansion {
                    occurrences: out,
                    stop: Stop::OutOfRange,
                };
            }
            if day > last_day || (day == last_day && time > last_time) {
                return Expansion {
                    occurrences: out,
                    stop: Stop::Ended,
                };
            }
            out.push(LocalDateTime {
                date: days_to_civil(day),
                hour: dtstart.hour,
                minute: dtstart.minute,
                second: dtstart.second,
            });
            emitted += 1;
            if emitted == count {
                return Expansion {
                    occurrences: out,
                    stop: Stop::Ended,
                };
            }
            if emitted == limit {
                return Expansion {
                    occurrences: out,
                    stop: Stop::Limit,
                };
            }
        }
        periods += 1;
    }
    Expansion {
        occurrences: out,
        stop: Stop::MaxPeriods,
    }
}
