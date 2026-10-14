//! Working days: Monday to Friday without public holidays. The holidays are those of Hamburg
//! (decision 2026-10-14); other states or shift work can follow as further calendars.

use jiff::civil::{Date, Weekday};

/// A working-day calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calendar {
    /// Whose public holidays count.
    region: Region,
}

/// Whose public holidays count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Hamburg,
}

/// Easter Sunday of a year (Gregorian; Meeus/Jones/Butcher).
pub fn easter(year: i16) -> Option<Date> {
    let y = i32::from(year);
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    Date::new(year, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()
}

/// The day `days` days after `date` (before, if negative); `date` itself at the calendar's end.
fn plus(date: Date, days: i32) -> Date {
    date.checked_add(jiff::Span::new().days(days))
        .unwrap_or(date)
}

impl Calendar {
    /// Working days in Hamburg: Monday to Friday without the public holidays there (New Year,
    /// Good Friday, Easter Monday, 1 May, Ascension, Whit Monday, 3 October, Reformation Day
    /// since 2017, Christmas Day and Boxing Day).
    pub const fn hamburg() -> Self {
        Self {
            region: Region::Hamburg,
        }
    }

    /// Whether `date` is a public holiday.
    pub fn is_holiday(&self, date: Date) -> bool {
        match self.region {
            Region::Hamburg => {
                let fixed = matches!(
                    (date.month(), date.day()),
                    (1, 1) | (5, 1) | (10, 3) | (12, 25) | (12, 26)
                ) || (date.month() == 10 && date.day() == 31 && date.year() >= 2017);
                let moving = easter(date.year()).is_some_and(|easter| {
                    [-2, 1, 39, 50]
                        .into_iter()
                        .any(|offset| plus(easter, offset) == date)
                });
                fixed || moving
            }
        }
    }

    /// Whether people work on `date`.
    pub fn is_working(&self, date: Date) -> bool {
        !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday) && !self.is_holiday(date)
    }

    /// `date` if it is a working day, otherwise the next one.
    pub fn next_working(&self, date: Date) -> Date {
        let mut day = date;
        // At most a few days off in a row; the bound only guards the end of the calendar.
        for _ in 0..30 {
            if self.is_working(day) {
                return day;
            }
            let next = plus(day, 1);
            if next == day {
                break;
            }
            day = next;
        }
        day
    }

    /// `date` if it is a working day, otherwise the one before.
    pub fn previous_working(&self, date: Date) -> Date {
        let mut day = date;
        for _ in 0..30 {
            if self.is_working(day) {
                return day;
            }
            let before = plus(day, -1);
            if before == day {
                break;
            }
            day = before;
        }
        day
    }

    /// `days` working days after `date` (before, if negative), starting from the working day
    /// at or after `date`. `add(d, 0)` is the working day at or after `d`.
    pub fn add(&self, date: Date, days: i32) -> Date {
        let mut day = if days >= 0 {
            self.next_working(date)
        } else {
            self.previous_working(date)
        };
        let step = if days >= 0 { 1 } else { -1 };
        for _ in 0..days.unsigned_abs() {
            let next = plus(day, step);
            if next == day {
                break;
            }
            day = if step > 0 {
                self.next_working(next)
            } else {
                self.previous_working(next)
            };
        }
        day
    }

    /// How many working days lie in `start..=end` (0 if `end` is before `start`).
    pub fn count(&self, start: Date, end: Date) -> i32 {
        let mut n: i32 = 0;
        let mut day = start;
        // Plans span months, not centuries; the bound keeps hostile dates cheap.
        for _ in 0..20_000 {
            if day > end {
                break;
            }
            if self.is_working(day) {
                n = n.saturating_add(1);
            }
            let next = plus(day, 1);
            if next == day {
                break;
            }
            day = next;
        }
        n
    }

    /// Working days from `from` to `to` (both taken as working days): negative if `to` is
    /// earlier. `add(from, between(from, to)) == to` for working days.
    pub fn between(&self, from: Date, to: Date) -> i32 {
        let from = self.next_working(from);
        let to = self.next_working(to);
        if to >= from {
            self.count(from, to).saturating_sub(1)
        } else {
            -self.count(to, from).saturating_sub(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn easter_is_right() {
        assert_eq!(easter(2024), Some(d("2024-03-31")));
        assert_eq!(easter(2025), Some(d("2025-04-20")));
        assert_eq!(easter(2026), Some(d("2026-04-05")));
        assert_eq!(easter(2027), Some(d("2027-03-28")));
        assert_eq!(easter(2038), Some(d("2038-04-25")));
    }

    #[test]
    fn hamburg_holidays_2026() {
        let hh = Calendar::hamburg();
        for holiday in [
            "2026-01-01",
            "2026-04-03",
            "2026-04-06",
            "2026-05-01",
            "2026-05-14",
            "2026-05-25",
            "2026-10-03",
            "2026-10-31",
            "2026-12-25",
            "2026-12-26",
        ] {
            assert!(hh.is_holiday(d(holiday)), "{holiday}");
        }
        for ordinary in ["2026-04-02", "2026-06-04", "2026-11-01", "2026-12-24"] {
            assert!(!hh.is_holiday(d(ordinary)), "{ordinary}");
        }
        assert!(
            !hh.is_holiday(d("2016-10-31")),
            "Reformation Day only since 2017"
        );
    }

    #[test]
    fn working_days_skip_weekends_and_holidays() {
        let hh = Calendar::hamburg();
        // Friday 2026-10-16 + 1 → Monday 19th.
        assert_eq!(hh.add(d("2026-10-16"), 1), d("2026-10-19"));
        assert_eq!(
            hh.add(d("2026-10-17"), 0),
            d("2026-10-19"),
            "Saturday → Monday"
        );
        assert_eq!(hh.add(d("2026-10-19"), -1), d("2026-10-16"));
        // Before Easter: Thursday 2026-04-02 + 1 → Tuesday 7th (Good Friday, Easter Monday).
        assert_eq!(hh.add(d("2026-04-02"), 1), d("2026-04-07"));
        assert_eq!(hh.count(d("2026-10-12"), d("2026-10-18")), 5);
        assert_eq!(hh.count(d("2026-10-18"), d("2026-10-12")), 0);
        assert_eq!(hh.between(d("2026-10-16"), d("2026-10-19")), 1);
        assert_eq!(hh.between(d("2026-10-19"), d("2026-10-16")), -1);
        assert_eq!(hh.between(d("2026-10-19"), d("2026-10-19")), 0);
    }

    #[test]
    fn the_ends_of_the_calendar_do_not_loop() {
        let hh = Calendar::hamburg();
        let last = Date::MAX;
        let _ = hh.add(last, 10);
        let _ = hh.add(Date::MIN, -10);
        let _ = hh.count(Date::MIN, Date::MAX);
    }
}
