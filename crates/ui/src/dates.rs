//! Dates as people type and read them: `31.12.2026` in German, `2026-12-31` (ISO, the same in
//! the UK and the US) in English. Both forms are read in every language; commands always
//! receive ISO.

use jiff::civil::{Date, Time};

use crate::text::Language;

/// Shows a date as `31.12.2026` (German) or `2026-12-31` (English).
pub fn format(date: Date) -> String {
    match crate::text::language() {
        Language::German => format!("{:02}.{:02}.{:04}", date.day(), date.month(), date.year()),
        Language::English => format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day()),
    }
}

/// Shows a date without the year, for short labels: `16.10.` or `10-16`.
pub fn format_short(date: Date) -> String {
    match crate::text::language() {
        Language::German => format!("{:02}.{:02}.", date.day(), date.month()),
        Language::English => format!("{:02}-{:02}", date.month(), date.day()),
    }
}

/// Shows a moment in local time: `31.12.2026 14:00` or `2026-12-31 14:00`.
pub fn format_moment(moment: jiff::Timestamp) -> String {
    let local = moment.to_zoned(jiff::tz::TimeZone::system());
    format!("{} {}", format(local.date()), format_time(local.time()))
}

/// Shows an optional date; empty without one.
pub fn format_opt(date: Option<Date>) -> String {
    date.map(format).unwrap_or_default()
}

/// Reads a typed date. Empty text means "no date". Accepts `31.12.2026`, `1.2.2026`,
/// `31.12.26` (20xx) and `2026-12-31`.
pub fn parse(text: &str) -> Result<Option<Date>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let invalid = || crate::text::not_a_date(text);
    let numbers = |parts: Vec<&str>| -> Option<Vec<i32>> {
        parts.iter().map(|p| p.trim().parse::<i32>().ok()).collect()
    };
    let (year, month, day) = if text.contains('-') {
        match numbers(text.split('-').collect()).as_deref() {
            Some([y, m, d]) => (*y, *m, *d),
            _ => return Err(invalid()),
        }
    } else {
        let parts: Vec<&str> = text.split('.').collect();
        let short_year = parts.last().is_some_and(|y| y.trim().len() == 2);
        match numbers(parts).as_deref() {
            Some([d, m, y]) if short_year => (2000 + *y, *m, *d),
            Some([d, m, y]) => (*y, *m, *d),
            _ => return Err(invalid()),
        }
    };
    let (Ok(year), Ok(month), Ok(day)) =
        (i16::try_from(year), i8::try_from(month), i8::try_from(day))
    else {
        return Err(invalid());
    };
    if !(1900..=2999).contains(&year) {
        return Err(invalid());
    }
    Date::new(year, month, day).map(Some).map_err(|_| invalid())
}

/// Shows a time of day as `14:00`.
pub fn format_time(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

/// Shows an optional time; empty without one.
pub fn format_time_opt(time: Option<Time>) -> String {
    time.map(format_time).unwrap_or_default()
}

/// Reads a typed time of day. Empty means "none". Accepts `14:00`, `9:30`, `14.30`, `14`.
pub fn parse_time(text: &str) -> Result<Option<Time>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let invalid = || crate::text::not_a_time(text);
    let (hour, minute) = match text.split_once([':', '.']) {
        Some((h, m)) if m.len() == 2 => (h, m),
        Some(_) => return Err(invalid()),
        None => (text, "00"),
    };
    let (Ok(hour), Ok(minute)) = (hour.parse::<i8>(), minute.parse::<i8>()) else {
        return Err(invalid());
    };
    Time::new(hour, minute, 0, 0)
        .map(Some)
        .map_err(|_| invalid())
}

/// A deadline from a typed day and time. A time needs a day.
pub fn parse_deadline(day: &str, time: &str) -> Result<Option<(Date, Option<Time>)>, String> {
    match (parse(day)?, parse_time(time)?) {
        (Some(day), time) => Ok(Some((day, time))),
        (None, None) => Ok(None),
        (None, Some(_)) => Err(crate::text::t().time_needs_date.to_owned()),
    }
}

/// The form a command expects for a deadline (`2026-12-24` or `2026-12-24T14:00`), or JSON
/// `null` to clear it.
pub fn deadline_param(deadline: Option<(Date, Option<Time>)>) -> serde_json::Value {
    match deadline {
        None => serde_json::Value::Null,
        Some((day, None)) => serde_json::Value::String(day.to_string()),
        Some((day, Some(time))) => {
            serde_json::Value::String(format!("{day}T{}", format_time(time)))
        }
    }
}

/// A deadline for people: `24.12.2026` or `24.12.2026, 14:00 Uhr` (`2026-12-24, 14:00`).
pub fn format_deadline(day: Date, time: Option<Time>) -> String {
    match time {
        Some(time) => format!(
            "{}, {}{}",
            format(day),
            format_time(time),
            crate::text::t().o_clock
        ),
        None => format(day),
    }
}

/// The ISO form a command expects, or JSON `null` to clear the field.
pub fn param(date: Option<Date>) -> serde_json::Value {
    date.map_or(serde_json::Value::Null, |d| {
        serde_json::Value::String(d.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i16, m: i8, day: i8) -> Date {
        Date::new(y, m, day).unwrap()
    }

    #[test]
    fn reads_german_and_iso_dates() {
        assert_eq!(parse("31.12.2026"), Ok(Some(d(2026, 12, 31))));
        assert_eq!(parse(" 1.2.2026 "), Ok(Some(d(2026, 2, 1))));
        assert_eq!(parse("31.12.26"), Ok(Some(d(2026, 12, 31))));
        assert_eq!(parse("2026-12-31"), Ok(Some(d(2026, 12, 31))));
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("   "), Ok(None));
    }

    #[test]
    fn refuses_what_is_not_a_date() {
        for text in [
            "31.02.2026",
            "32.1.2026",
            "morgen",
            "1.2",
            "1.2.3.4",
            "2026-13-01",
            "1.1.99999",
            "1.1.0026",
            "-1.1.2026",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn reads_times_and_deadlines() {
        let t = |h, m| Time::new(h, m, 0, 0).unwrap();
        assert_eq!(parse_time("14:00"), Ok(Some(t(14, 0))));
        assert_eq!(parse_time(" 9:30 "), Ok(Some(t(9, 30))));
        assert_eq!(parse_time("14.30"), Ok(Some(t(14, 30))));
        assert_eq!(parse_time("8"), Ok(Some(t(8, 0))));
        assert_eq!(parse_time(""), Ok(None));
        for bad in ["24:00", "14:60", "14:5", "mittags", "-1:00", "14:00:00"] {
            assert!(parse_time(bad).is_err(), "{bad}");
        }
        assert_eq!(format_time(t(9, 5)), "09:05");
        let day = d(2026, 12, 24);
        assert_eq!(
            parse_deadline("24.12.2026", "14:00"),
            Ok(Some((day, Some(t(14, 0)))))
        );
        assert_eq!(parse_deadline("", ""), Ok(None));
        assert!(parse_deadline("", "14:00").is_err());
        assert_eq!(
            deadline_param(Some((day, Some(t(14, 0))))),
            serde_json::json!("2026-12-24T14:00")
        );
        assert_eq!(
            deadline_param(Some((day, None))),
            serde_json::json!("2026-12-24")
        );
        assert_eq!(deadline_param(None), serde_json::Value::Null);
        assert_eq!(
            format_deadline(day, Some(t(14, 0))),
            "24.12.2026, 14:00 Uhr"
        );
    }

    #[test]
    fn shows_dates_the_german_way() {
        assert_eq!(format(d(2026, 2, 1)), "01.02.2026");
        assert_eq!(format_opt(None), "");
        assert_eq!(parse(&format(d(2026, 2, 1))), Ok(Some(d(2026, 2, 1))));
        assert_eq!(param(Some(d(2026, 2, 1))), serde_json::json!("2026-02-01"));
        assert_eq!(param(None), serde_json::Value::Null);
    }
}
