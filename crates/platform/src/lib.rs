//! The real clock and real randomness for plnwrk apps (layer 2). They live outside the pure
//! core (docs/PLAN.md §2 "Deterministischer Kern") and are handed to a session from outside.

use jiff::Timestamp;
use jiff::civil::Date;
use plnwrk_core::{Clock, IdSource};
use ulid::Ulid;

/// The system clock; "today" in the computer's time zone.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }

    fn today(&self) -> Date {
        jiff::Zoned::now().date()
    }

    fn now_local(&self) -> jiff::civil::DateTime {
        jiff::Zoned::now().datetime()
    }
}

/// ULIDs from the current time and 80 random bits of the operating system, strictly
/// increasing: within the same millisecond (or if randomness fails) the next id is the previous
/// one plus one, so two ids are never equal.
#[derive(Default)]
pub struct RandomIds {
    last: u128,
}

impl IdSource for RandomIds {
    fn next_ulid(&mut self) -> Ulid {
        let millis = u64::try_from(Timestamp::now().as_millisecond()).unwrap_or(0);
        let mut bytes = [0u8; 16];
        let random = match getrandom::fill(&mut bytes) {
            Ok(()) => u128::from_le_bytes(bytes),
            Err(_) => 0,
        };
        let candidate = Ulid::from_parts(millis, random).0;
        let next = if candidate > self.last {
            candidate
        } else {
            self.last.saturating_add(1)
        };
        self.last = next;
        Ulid(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_strictly_increase() {
        let mut ids = RandomIds::default();
        let mut previous = ids.next_ulid();
        for _ in 0..10_000 {
            let next = ids.next_ulid();
            assert!(next > previous);
            previous = next;
        }
    }

    #[test]
    fn clock_is_close_to_now() {
        let now = SystemClock.now();
        assert!(now > Timestamp::from_second(1_790_000_000).unwrap());
    }
}
