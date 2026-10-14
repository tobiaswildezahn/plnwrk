//! Time comes from outside: the core never reads the system clock (docs/PLAN.md §2).

use jiff::Timestamp;
use jiff::civil::Date;

/// Where the current time comes from. Apps read the system clock in the user's time zone;
/// tests use [`FixedClock`].
pub trait Clock {
    /// The current instant (UTC).
    fn now(&self) -> Timestamp;
    /// The current calendar day where the user is.
    fn today(&self) -> Date;
}

/// A clock that always shows the same moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedClock {
    /// What [`Clock::now`] returns.
    pub now: Timestamp,
    /// What [`Clock::today`] returns.
    pub today: Date,
}

impl FixedClock {
    /// Midnight UTC of the given day (`YYYY-MM-DD`).
    pub fn at(day: &str) -> Result<Self, jiff::Error> {
        let today: Date = day.parse()?;
        let now = today.to_zoned(jiff::tz::TimeZone::UTC)?.timestamp();
        Ok(Self { now, today })
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.now
    }

    fn today(&self) -> Date {
        self.today
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_reports_its_day_at_midnight_utc() {
        let clock = FixedClock::at("2026-10-14").unwrap();
        assert_eq!(clock.today().to_string(), "2026-10-14");
        assert_eq!(clock.now().to_string(), "2026-10-14T00:00:00Z");
    }

    #[test]
    fn fixed_clock_rejects_invalid_dates() {
        assert!(FixedClock::at("2026-02-30").is_err());
        assert!(FixedClock::at("gestern").is_err());
    }
}
