//! Sort keys that never need renumbering ("fractional indexing"). Moving a card between two
//! others gives it a key strictly between theirs, so a move changes exactly one task – which
//! keeps undo, history and later synchronisation simple.
//!
//! A key is an integer part of variable length followed by an optional fraction, in base 62
//! (`0-9A-Za-z`, which sorts like the characters). The first character tells the length of the
//! integer part: `a`–`z` are 2–27 characters (counting up), `Z`–`A` the same downwards. Adding
//! at the start or end counts the integer up or down, so 10 000 appends need only four
//! characters; only repeated inserts at exactly the same gap grow the fraction.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Longest rank in characters.
pub const RANK_MAX: usize = 256;

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = 62;
const ZERO: u8 = b'0';
const LAST: u8 = b'z';

/// Why a rank was refused or could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RankError {
    /// Not a well-formed rank.
    #[error("not a valid rank")]
    Invalid,
    /// The lower bound is not below the upper bound.
    #[error("the first rank must be lower than the second")]
    Order,
    /// The result would exceed [`RANK_MAX`] characters (ranks of the project need spreading out).
    #[error("rank too long; the order needs to be spread out")]
    TooLong,
}

/// A sort key: compares like text, always has room before, after and between.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank(String);

fn value(byte: u8) -> Result<usize, RankError> {
    DIGITS
        .iter()
        .position(|d| *d == byte)
        .ok_or(RankError::Invalid)
}

fn digit(value: usize) -> Result<u8, RankError> {
    DIGITS.get(value).copied().ok_or(RankError::Invalid)
}

fn integer_length(head: u8) -> Result<usize, RankError> {
    match head {
        b'a'..=b'z' => Ok(usize::from(head - b'a') + 2),
        b'A'..=b'Z' => Ok(usize::from(b'Z' - head) + 2),
        _ => Err(RankError::Invalid),
    }
}

/// The smallest integer part; nothing can come before it without a fraction.
fn smallest() -> Vec<u8> {
    let mut key = vec![b'A'];
    key.extend(std::iter::repeat_n(ZERO, 26));
    key
}

/// Splits a key into integer part and fraction.
fn split(key: &[u8]) -> Result<(&[u8], &[u8]), RankError> {
    let head = *key.first().ok_or(RankError::Invalid)?;
    let length = integer_length(head)?;
    let integer = key.get(..length).ok_or(RankError::Invalid)?;
    let fraction = key.get(length..).unwrap_or_default();
    Ok((integer, fraction))
}

fn validate(key: &[u8]) -> Result<(), RankError> {
    if key.is_empty() || key.len() > RANK_MAX {
        return Err(RankError::Invalid);
    }
    for byte in key {
        value(*byte)?;
    }
    let (_, fraction) = split(key)?;
    if fraction.last() == Some(&ZERO) || key == smallest().as_slice() {
        return Err(RankError::Invalid);
    }
    Ok(())
}

/// Digits strictly between fractions `a` and `b` (`None` = after everything).
fn midpoint(a: &[u8], b: Option<&[u8]>) -> Result<Vec<u8>, RankError> {
    if let Some(b) = b {
        let common = b
            .iter()
            .enumerate()
            .take_while(|(i, d)| a.get(*i).copied().unwrap_or(ZERO) == **d)
            .count();
        if common > 0 {
            let rest_b = b.get(common..).unwrap_or_default();
            if rest_b.is_empty() {
                return Err(RankError::Order);
            }
            let mut out = b.get(..common).unwrap_or_default().to_vec();
            out.extend(midpoint(a.get(common..).unwrap_or_default(), Some(rest_b))?);
            return Ok(out);
        }
    }
    let low = match a.first() {
        Some(&d) => value(d)?,
        None => 0,
    };
    let high = match b {
        Some(b) => value(*b.first().ok_or(RankError::Order)?)?,
        None => BASE,
    };
    if high <= low {
        return Err(RankError::Order);
    }
    if high - low > 1 {
        return Ok(vec![digit((low + high).div_ceil(2))?]);
    }
    match b {
        Some(b) if b.len() > 1 => Ok(vec![*b.first().ok_or(RankError::Order)?]),
        _ => {
            let mut out = vec![digit(low)?];
            out.extend(midpoint(a.get(1..).unwrap_or_default(), None)?);
            Ok(out)
        }
    }
}

/// The next integer, or `None` if `z…z` is reached.
fn increment(integer: &[u8]) -> Result<Option<Vec<u8>>, RankError> {
    let (&head, rest) = integer.split_first().ok_or(RankError::Invalid)?;
    let mut digits = rest.to_vec();
    let mut carry = true;
    for d in digits.iter_mut().rev() {
        let next = value(*d)? + 1;
        if next == BASE {
            *d = ZERO;
        } else {
            *d = digit(next)?;
            carry = false;
            break;
        }
    }
    if !carry {
        let mut out = vec![head];
        out.extend(digits);
        return Ok(Some(out));
    }
    match head {
        b'Z' => Ok(Some(vec![b'a', ZERO])),
        b'z' => Ok(None),
        _ => {
            let new_head = head + 1;
            if new_head > b'a' {
                digits.push(ZERO);
            } else {
                digits.pop();
            }
            let mut out = vec![new_head];
            out.extend(digits);
            Ok(Some(out))
        }
    }
}

/// The previous integer, or `None` if `A0…0` is reached.
fn decrement(integer: &[u8]) -> Result<Option<Vec<u8>>, RankError> {
    let (&head, rest) = integer.split_first().ok_or(RankError::Invalid)?;
    let mut digits = rest.to_vec();
    let mut borrow = true;
    for d in digits.iter_mut().rev() {
        let current = value(*d)?;
        if current == 0 {
            *d = LAST;
        } else {
            *d = digit(current - 1)?;
            borrow = false;
            break;
        }
    }
    if !borrow {
        let mut out = vec![head];
        out.extend(digits);
        return Ok(Some(out));
    }
    match head {
        b'a' => Ok(Some(vec![b'Z', LAST])),
        b'A' => Ok(None),
        _ => {
            let new_head = head - 1;
            if new_head < b'Z' {
                digits.push(LAST);
            } else {
                digits.pop();
            }
            let mut out = vec![new_head];
            out.extend(digits);
            Ok(Some(out))
        }
    }
}

fn key_between(a: Option<&[u8]>, b: Option<&[u8]>) -> Result<Vec<u8>, RankError> {
    match (a, b) {
        (None, None) => Ok(vec![b'a', ZERO]),
        (None, Some(b)) => {
            let (integer, fraction) = split(b)?;
            if integer == smallest().as_slice() {
                let mut out = integer.to_vec();
                out.extend(midpoint(&[], Some(fraction))?);
                Ok(out)
            } else if !fraction.is_empty() {
                Ok(integer.to_vec())
            } else {
                decrement(integer)?.ok_or(RankError::TooLong)
            }
        }
        (Some(a), None) => {
            let (integer, fraction) = split(a)?;
            match increment(integer)? {
                Some(next) => Ok(next),
                None => {
                    let mut out = integer.to_vec();
                    out.extend(midpoint(fraction, None)?);
                    Ok(out)
                }
            }
        }
        (Some(a), Some(b)) => {
            let (ia, fa) = split(a)?;
            let (ib, fb) = split(b)?;
            if ia == ib {
                let mut out = ia.to_vec();
                out.extend(midpoint(fa, Some(fb))?);
                return Ok(out);
            }
            let next = increment(ia)?.ok_or(RankError::TooLong)?;
            if next.as_slice() < b {
                Ok(next)
            } else {
                let mut out = ia.to_vec();
                out.extend(midpoint(fa, None)?);
                Ok(out)
            }
        }
    }
}

impl Rank {
    /// Checks a stored rank.
    pub fn parse(text: &str) -> Result<Self, RankError> {
        validate(text.as_bytes())?;
        Ok(Self(text.to_owned()))
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
        let key = key_between(low.map(|r| r.0.as_bytes()), high.map(|r| r.0.as_bytes()))?;
        if key.len() > RANK_MAX {
            return Err(RankError::TooLong);
        }
        let text = String::from_utf8(key).map_err(|_| RankError::Invalid)?;
        Rank::parse(&text)
    }

    /// `count` short, evenly increasing ranks – for spreading out a crowded order.
    pub fn spread(count: usize) -> Vec<Rank> {
        let mut out: Vec<Rank> = Vec::with_capacity(count);
        for _ in 0..count {
            match Rank::between(out.last(), None) {
                Ok(next) => out.push(next),
                Err(_) => break,
            }
        }
        out
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
    fn first_rank() {
        assert_eq!(Rank::between(None, None).unwrap().as_str(), "a0");
    }

    #[test]
    fn appending_counts_up_the_integer_part() {
        assert_eq!(Rank::between(Some(&r("a0")), None).unwrap().as_str(), "a1");
        assert_eq!(Rank::between(Some(&r("az")), None).unwrap().as_str(), "b00");
        assert_eq!(Rank::between(None, Some(&r("a0"))).unwrap().as_str(), "Zz");
    }

    #[test]
    fn between_neighbours_uses_the_fraction() {
        assert_eq!(
            Rank::between(Some(&r("a0")), Some(&r("a1")))
                .unwrap()
                .as_str(),
            "a0V"
        );
        let mid = Rank::between(Some(&r("a0V")), Some(&r("a1"))).unwrap();
        assert!(r("a0V") < mid && mid < r("a1"));
    }

    #[test]
    fn ten_thousand_appends_stay_short() {
        let mut last: Option<Rank> = None;
        for _ in 0..10_000 {
            let next = Rank::between(last.as_ref(), None).unwrap();
            if let Some(previous) = &last {
                assert!(*previous < next);
            }
            last = Some(next);
        }
        assert!(last.unwrap().as_str().len() <= 4);
    }

    #[test]
    fn ten_thousand_prepends_stay_short() {
        let mut first: Option<Rank> = None;
        for _ in 0..10_000 {
            let next = Rank::between(None, first.as_ref()).unwrap();
            if let Some(previous) = &first {
                assert!(next < *previous);
            }
            first = Some(next);
        }
        assert!(first.unwrap().as_str().len() <= 4);
    }

    #[test]
    fn always_inserting_at_the_same_spot_ends_with_an_error_not_a_panic() {
        let low = r("a0");
        let mut high = r("a1");
        let mut result = Ok(());
        for _ in 0..5_000 {
            match Rank::between(Some(&low), Some(&high)) {
                Ok(mid) => high = mid,
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        assert_eq!(result, Err(RankError::TooLong));
    }

    #[test]
    fn spread_gives_increasing_short_ranks() {
        let ranks = Rank::spread(5_000);
        assert_eq!(ranks.len(), 5_000);
        assert!(ranks.windows(2).all(|w| w[0] < w[1]));
        assert!(ranks.iter().all(|r| r.as_str().len() <= 4));
    }

    #[test]
    fn wrong_order_is_an_error() {
        assert_eq!(
            Rank::between(Some(&r("b00")), Some(&r("a1"))),
            Err(RankError::Order)
        );
        assert_eq!(
            Rank::between(Some(&r("a1")), Some(&r("a1"))),
            Err(RankError::Order)
        );
    }

    #[test]
    fn invalid_texts_are_rejected() {
        let smallest = format!("A{}", "0".repeat(26));
        for bad in ["", "a", "a00", "b0", "a-", "ä0", "?0", smallest.as_str()] {
            assert!(Rank::parse(bad).is_err(), "{bad}");
        }
        assert!(Rank::parse(&format!("a0{}", "z".repeat(RANK_MAX))).is_err());
        assert!(Rank::parse("a0V").is_ok() && Rank::parse("Zz").is_ok());
    }

    #[test]
    fn serde_checks_the_text() {
        assert_eq!(serde_json::to_string(&r("a0")).unwrap(), "\"a0\"");
        assert!(serde_json::from_str::<Rank>("\"a00\"").is_err());
    }

    proptest! {
        #[test]
        fn repeated_inserts_keep_strict_order(positions in proptest::collection::vec(0usize..1000, 1..300)) {
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
