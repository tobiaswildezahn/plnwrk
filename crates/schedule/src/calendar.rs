//! Working days. plnwrk counts **every day** (decision 2026-10-15: in public safety work goes on
//! at weekends and on holidays; a week is seven days) – [`Calendar::standard`]. The calendar of
//! Hamburg offices (Monday to Friday without public holidays) stays for showing days off and
//! for a later choice per project.

use jiff::civil::{Date, Weekday};

/// A working-day calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calendar {
    /// Whose public holidays count.
    region: Region,
}

/// Which days are worked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    /// Monday to Friday without the public holidays of Hamburg.
    Hamburg,
    /// Every day, weekends and holidays too.
    EveryDay,
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

/// The day `days` days after `date` (before, if negative); the first or last day of the
/// calendar beyond its ends.
fn plus(date: Date, days: i32) -> Date {
    jiff::Span::new()
        .try_days(days)
        .ok()
        .and_then(|span| date.checked_add(span).ok())
        .unwrap_or(if days >= 0 { Date::MAX } else { Date::MIN })
}

impl Calendar {
    /// The calendar plnwrk counts with: every day (see the module documentation).
    pub const fn standard() -> Self {
        Self::every_day()
    }

    /// Every day is a working day, weekends and public holidays too (around the clock).
    pub const fn every_day() -> Self {
        Self {
            region: Region::EveryDay,
        }
    }

    /// Working days in Hamburg: Monday to Friday without the public holidays there (New Year,
    /// Good Friday, Easter Monday, 1 May, Ascension, Whit Monday, 3 October, Reformation Day
    /// since 2017, Christmas Day and Boxing Day).
    pub const fn hamburg() -> Self {
        Self {
            region: Region::Hamburg,
        }
    }

    /// The public holidays of `year`, each once, in order.
    pub fn holidays(&self, year: i16) -> Vec<Date> {
        let (days, n) = self.holiday_days(year);
        days.get(..n).map_or_else(Vec::new, <[Date]>::to_vec)
    }

    /// The public holidays of `year`, each once, in order: the first `n` of the array. Without
    /// allocating, as counting working days asks for every year in a range.
    fn holiday_days(&self, year: i16) -> ([Date; 10], usize) {
        let mut days = [Date::MIN; 10];
        let mut n = 0;
        let mut add = |day: Date| {
            if let Some(slot) = days.get_mut(n) {
                *slot = day;
                n += 1;
            }
        };
        match self.region {
            Region::EveryDay => {}
            Region::Hamburg => {
                [(1, 1), (5, 1), (10, 3), (12, 25), (12, 26)]
                    .into_iter()
                    .chain((year >= 2017).then_some((10, 31)))
                    .filter_map(|(month, day)| Date::new(year, month, day).ok())
                    .for_each(&mut add);
                if let Some(easter) = easter(year) {
                    [-2, 1, 39, 50]
                        .into_iter()
                        .map(|offset| plus(easter, offset))
                        .for_each(&mut add);
                }
            }
        }
        let found = days.get_mut(..n).unwrap_or_default();
        found.sort_unstable();
        // Twice the same day (Ascension on 1 May): kept once.
        let mut unique = 0;
        for i in 0..found.len() {
            if unique == 0 || found.get(unique - 1) != found.get(i) {
                found.swap(unique, i);
                unique += 1;
            }
        }
        (days, unique)
    }

    /// Whether `date` is a public holiday.
    pub fn is_holiday(&self, date: Date) -> bool {
        let (days, n) = self.holiday_days(date.year());
        days.get(..n).is_some_and(|days| days.contains(&date))
    }

    /// Whether people work on `date`.
    pub fn is_working(&self, date: Date) -> bool {
        if self.region == Region::EveryDay {
            return true;
        }
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
    /// at or after `date` (before it, if negative). `add(d, 0)` is the working day at or
    /// after `d`.
    pub fn add(&self, date: Date, days: i32) -> Date {
        let anchor = if days >= 0 {
            self.next_working(date)
        } else {
            self.previous_working(date)
        };
        self.step(anchor, days)
    }

    /// Counted from an end: `days` working days after the last working day at or before
    /// `end` (a task ending on a Sunday ended on Friday). `after_end(sunday, 1)` is Monday.
    pub fn after_end(&self, end: Date, days: i32) -> Date {
        self.step(self.previous_working(end), days)
    }

    /// Counted from a start: `days` working days after the first working day at or after
    /// `start` (a task starting on a Saturday starts on Monday).
    pub fn from_start(&self, start: Date, days: i32) -> Date {
        self.step(self.next_working(start), days)
    }

    /// `days` working days on from `from` (back, if negative), skipping days off. Found by
    /// halving with [`Calendar::count`], so far dates cost no more than near ones; at an end of
    /// the calendar, the last working day there.
    fn step(&self, from: Date, days: i32) -> Date {
        if days == 0 {
            return from;
        }
        if self.region == Region::EveryDay {
            return plus(from, days);
        }
        // Working days in the range, both ends included, that the result must reach.
        let need = days.unsigned_abs().saturating_add(1);
        // Never fewer than 4 working days a week: twice the days plus a fortnight is enough.
        let reach = i32::try_from(need)
            .unwrap_or(i32::MAX)
            .saturating_mul(2)
            .saturating_add(14);
        let forward = days > 0;
        let counted = |far: Date| -> u32 {
            let n = if forward {
                self.count(from, far)
            } else {
                self.count(far, from)
            };
            n.unsigned_abs()
        };
        let (mut near, mut far) = if forward {
            (from, plus(from, reach))
        } else {
            (plus(from, -reach), from)
        };
        // Forward: the earliest day with `need` working days; back: the latest.
        while near < far {
            let gap = near.until(far).map_or(0, |span| span.get_days());
            if forward {
                let mid = plus(near, gap / 2);
                if counted(mid) >= need {
                    far = mid;
                } else {
                    near = plus(mid, 1);
                }
            } else {
                let mid = plus(far, -(gap / 2));
                if counted(mid) >= need {
                    near = mid;
                } else {
                    far = plus(mid, -1);
                }
            }
        }
        if forward {
            self.previous_working(near)
        } else {
            self.next_working(near)
        }
    }

    /// How many working days lie in `start..=end` (0 if `end` is before `start`). Counted
    /// by whole weeks and the holidays of each year, so far dates cost nothing more.
    pub fn count(&self, start: Date, end: Date) -> i32 {
        if end < start {
            return 0;
        }
        let total = start
            .until(end)
            .map_or(0, |span| span.get_days())
            .saturating_add(1);
        if self.region == Region::EveryDay {
            return total;
        }
        let weekday = |d: Date| !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday);
        let weeks = total / 7;
        let mut n = weeks.saturating_mul(5);
        let mut day = plus(start, weeks.saturating_mul(7));
        for _ in 0..7 {
            if day > end {
                break;
            }
            if weekday(day) {
                n = n.saturating_add(1);
            }
            let next = plus(day, 1);
            if next == day {
                break;
            }
            day = next;
        }
        for year in start.year()..=end.year() {
            let (days, found) = self.holiday_days(year);
            for &holiday in days.get(..found).unwrap_or_default() {
                if holiday >= start && holiday <= end && weekday(holiday) {
                    n = n.saturating_sub(1);
                }
            }
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
        // 2008: Ascension on 1 May, counted once (no Reformation Day yet): 8 days.
        assert_eq!(hh.holidays(2008).len(), 8);
        assert_eq!(hh.count(d("2008-05-01"), d("2008-05-01")), 0);
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

    /// Day by day, as a reference.
    fn walk(cal: &Calendar, from: Date, days: i32) -> Date {
        let mut day = from;
        let step = if days >= 0 { 1 } else { -1 };
        for _ in 0..days.unsigned_abs() {
            day = plus(day, step);
            while !cal.is_working(day) {
                day = plus(day, step);
            }
        }
        day
    }

    proptest::proptest! {
        #[test]
        fn stepping_matches_walking_day_by_day(offset in 0i32..4000, days in -600i32..600) {
            let hh = Calendar::hamburg();
            let from = hh.next_working(plus(d("2020-01-01"), offset));
            proptest::prop_assert_eq!(hh.step(from, days), walk(&hh, from, days));
        }
    }

    #[test]
    fn far_dates_are_quick() {
        let hh = Calendar::hamburg();
        let start = std::time::Instant::now();
        let far = hh.add(d("2026-10-14"), 1_500_000);
        assert!(far.year() > 7000, "{far}");
        assert_eq!(hh.between(d("2026-10-14"), far), 1_500_000);
        assert_eq!(hh.add(far, -1_500_000), d("2026-10-14"));
        assert!(
            // Day by day this takes minutes; the bound leaves room for slow, instrumented runs.
            start.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn every_day_counts_weekends_and_holidays_too() {
        let all = Calendar::every_day();
        assert_eq!(Calendar::standard(), all, "decision 2026-10-15");
        // Friday 2026-10-16 + 1 → Saturday; Christmas is a day like any other.
        assert_eq!(all.add(d("2026-10-16"), 1), d("2026-10-17"));
        assert_eq!(all.add(d("2026-12-24"), 1), d("2026-12-25"));
        assert_eq!(all.add(d("2026-10-19"), -2), d("2026-10-17"));
        assert_eq!(all.after_end(d("2026-10-18"), 1), d("2026-10-19"));
        assert_eq!(
            all.count(d("2026-10-12"), d("2026-10-18")),
            7,
            "a week is seven days"
        );
        assert_eq!(all.between(d("2026-10-16"), d("2026-10-23")), 7);
        assert!(all.holidays(2026).is_empty());
        assert!(all.is_working(d("2026-10-03")));
        let _ = all.add(Date::MAX, 10);
        let _ = all.add(Date::MIN, -10);
    }

    #[test]
    fn the_last_days_of_the_calendar_count_once() {
        let hh = Calendar::hamburg();
        // 9999-12-31 is a Friday.
        assert_eq!(hh.count(d("9999-12-29"), Date::MAX), 3);
        assert_eq!(hh.add(Date::MAX, -1), d("9999-12-30"));
        assert_eq!(hh.between(d("9999-12-27"), Date::MAX), 4);
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
