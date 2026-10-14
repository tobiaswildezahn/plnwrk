//! Sort keys that never need renumbering ("fractional indexing"). Moving a card between two
//! others gives it a key strictly between theirs, so a move changes exactly one task – which
//! keeps undo, history and later synchronisation simple.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Longest rank in characters. Reached only after thousands of inserts at the same spot.
pub const RANK_MAX: usize = 256;

const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BASE: u8 = 36;

/// Why a rank was refused or could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RankError {
    /// Empty, a character outside `0-9a-z`, or ending in `0`.
    #[error("not a valid rank")]
    Invalid,
    /// The lower bound is not below the upper bound.
    #[error("the first rank must be lower than the second")]
    Order,
    /// The result would exceed [`RANK_MAX`] characters.
    #[error("rank too long")]
    TooLong,
}

/// A sort key: compares like text, always has room before, after and between.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(String);

fn digit_value(byte: u8) -> Option<u8> {
    DIGITS
        .iter()
        .position(|d| *d == byte)
        .and_then(|i| u8::try_from(i).ok())
}

fn digit_char(value: u8) -> Option<char> {
    DIGITS.get(usize::from(value)).map(|b| char::from(*b))
}

/// Digits strictly between `a` and `b` (`None` = after everything). Requires `a < b`, both
/// without trailing zeros.
fn midpoint(a: &[u8], b: Option<&[u8]>) -> Result<Vec<u8>, RankError> {
    if let Some(b) = b {
        let common = b
            .iter()
            .enumerate()
            .take_while(|(i, digit)| a.get(*i).copied().unwrap_or(0) == **digit)
            .count();
        if common > 0 {
            let rest_a = a.get(common..).unwrap_or_default();
            let rest_b = b.get(common..).unwrap_or_default();
            if rest_b.is_empty() {
                return Err(RankError::Order);
            }
            let mut out = b.get(..common).unwrap_or_default().to_vec();
            out.extend(midpoint(rest_a, Some(rest_b))?);
            return Ok(out);
        }
    }
    let low = a.first().copied().unwrap_or(0);
    let high = match b {
        Some(b) => b.first().copied().ok_or(RankError::Order)?,
        None => BASE,
    };
    if high <= low {
        return Err(RankError::Order);
    }
    if high - low > 1 {
        return Ok(vec![low + (high - low) / 2]);
    }
    match b {
        Some(b) if b.len() > 1 => Ok(vec![high]),
        _ => {
            let mut out = vec![low];
            out.extend(midpoint(a.get(1..).unwrap_or_default(), None)?);
            Ok(out)
        }
    }
}

impl Rank {
    /// Checks a stored rank.
    pub fn parse(text: &str) -> Result<Self, RankError> {
        let valid = !text.is_empty()
            && text.len() <= RANK_MAX
            && text.bytes().all(|b| digit_value(b).is_some())
            && !text.ends_with('0');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(RankError::Invalid)
        }
    }

    /// The text of the rank.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A rank strictly between `low` and `high`; `None` means "no neighbour on that side".
    pub fn between(low: Option<&Rank>, high: Option<&Rank>) -> Result<Rank, RankError> {
        if let (Some(low), Some(high)) = (low, high)
            && low >= high
        {
            return Err(RankError::Order);
        }
        let to_digits =
            |rank: &Rank| -> Vec<u8> { rank.0.bytes().filter_map(digit_value).collect() };
        let a = low.map(to_digits).unwrap_or_default();
        let b = high.map(to_digits);
        let digits = midpoint(&a, b.as_deref())?;
        if digits.len() > RANK_MAX {
            return Err(RankError::TooLong);
        }
        let text: String = digits
            .into_iter()
            .map(digit_char)
            .collect::<Option<String>>()
            .ok_or(RankError::Invalid)?;
        Rank::parse(&text)
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Rank {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Rank {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rank::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn r(text: &str) -> Rank {
        Rank::parse(text).unwrap()
    }

    #[test]
    fn first_rank_is_the_middle() {
        assert_eq!(Rank::between(None, None).unwrap().as_str(), "i");
    }

    #[test]
    fn before_the_smallest_digit_goes_one_level_deeper() {
        assert_eq!(Rank::between(None, Some(&r("1"))).unwrap().as_str(), "0i");
    }

    #[test]
    fn between_two_neighbours() {
        let mid = Rank::between(Some(&r("a")), Some(&r("b"))).unwrap();
        assert!(r("a") < mid && mid < r("b"));
        let after = Rank::between(Some(&r("z")), None).unwrap();
        assert!(after > r("z"));
    }

    #[test]
    fn wrong_order_is_an_error() {
        assert_eq!(
            Rank::between(Some(&r("b")), Some(&r("a"))),
            Err(RankError::Order)
        );
        assert_eq!(
            Rank::between(Some(&r("b")), Some(&r("b"))),
            Err(RankError::Order)
        );
    }

    #[test]
    fn invalid_texts_are_rejected() {
        for bad in ["", "a0", "A", "a-b", "ä"] {
            assert!(Rank::parse(bad).is_err(), "{bad}");
        }
        assert!(Rank::parse(&"z".repeat(RANK_MAX + 1)).is_err());
    }

    #[test]
    fn serde_checks_the_text() {
        assert_eq!(serde_json::to_string(&r("i")).unwrap(), "\"i\"");
        assert!(serde_json::from_str::<Rank>("\"a0\"").is_err());
    }

    proptest! {
        #[test]
        fn repeated_inserts_keep_strict_order(positions in proptest::collection::vec(0usize..1000, 1..200)) {
            let mut ranks: Vec<Rank> = Vec::new();
            for position in positions {
                let at = position % (ranks.len() + 1);
                let before = at.checked_sub(1).and_then(|i| ranks.get(i));
                let after = ranks.get(at);
                let new = Rank::between(before, after).unwrap();
                prop_assert!(Rank::parse(new.as_str()).is_ok());
                ranks.insert(at, new);
            }
            for pair in ranks.windows(2) {
                prop_assert!(pair[0] < pair[1]);
            }
        }
    }
}
