//! The text forms of the dialog's fields, as a user types them: `2020-2026.11`,
//! `A00-E99/99`, `2500-3500`, `0-22`.

use std::str::FromStr;

use super::{DateRange, EcoCode, EcoRange, PartialDate};
use crate::bridge::Range;

/// A field whose text could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// What was being read.
    pub what: &'static str,
    /// The text that was given.
    pub text: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot read {} from {:?}", self.what, self.text)
    }
}

impl std::error::Error for ParseError {}

fn error(what: &'static str, text: &str) -> ParseError {
    ParseError { what, text: text.to_owned() }
}

/// `low-high`, `low-`, `-high` or a single value, split on the first `-`.
fn split_range(text: &str) -> (&str, &str) {
    match text.split_once('-') {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (text.trim(), text.trim()),
    }
}

impl FromStr for PartialDate {
    type Err = ParseError;

    /// `2026`, `2026.11` or `2026.11.03`.
    fn from_str(text: &str) -> Result<PartialDate, ParseError> {
        let bad = || error("a date (year[.month[.day]])", text);
        let mut parts = text.trim().split('.');
        let year: u16 = parts.next().and_then(|y| y.parse().ok()).filter(|y| *y > 0).ok_or_else(bad)?;
        let month = match parts.next() {
            Some(m) => Some(m.parse::<u8>().ok().filter(|m| (1..=12).contains(m)).ok_or_else(bad)?),
            None => None,
        };
        let day = match parts.next() {
            Some(d) => Some(d.parse::<u8>().ok().filter(|d| (1..=31).contains(d)).ok_or_else(bad)?),
            None => None,
        };
        if parts.next().is_some() {
            return Err(bad());
        }
        Ok(PartialDate { year, month, day })
    }
}

impl FromStr for DateRange {
    type Err = ParseError;

    /// `2020-2026.11`, `2020-`, `-2026` or `2026`.
    fn from_str(text: &str) -> Result<DateRange, ParseError> {
        let (a, b) = split_range(text);
        let end = |t: &str| if t.is_empty() { Ok(None) } else { t.parse().map(Some) };
        let range = DateRange { from: end(a)?, to: end(b)? };
        if range.from.is_none() && range.to.is_none() {
            return Err(error("a date range", text));
        }
        Ok(range)
    }
}

impl FromStr for EcoCode {
    type Err = ParseError;

    /// `B20` or `B20/03`.
    fn from_str(text: &str) -> Result<EcoCode, ParseError> {
        let bad = || error("an ECO code (A00-E99, optionally /sub)", text);
        let (code, sub) = match text.trim().split_once('/') {
            Some((c, s)) => (c, Some(s.parse::<u8>().ok().filter(|s| *s < 128).ok_or_else(bad)?)),
            None => (text.trim(), None),
        };
        let bytes = code.as_bytes();
        if bytes.len() != 3 || !bytes[1].is_ascii_digit() || !bytes[2].is_ascii_digit() {
            return Err(bad());
        }
        let letter = match bytes[0].to_ascii_uppercase() {
            l @ b'A'..=b'E' => u16::from(l - b'A'),
            _ => return Err(bad()),
        };
        let code = letter * 100 + u16::from(bytes[1] - b'0') * 10 + u16::from(bytes[2] - b'0');
        Ok(EcoCode { code, sub })
    }
}

impl FromStr for EcoRange {
    type Err = ParseError;

    /// `A00-E99/99`, or one code for that code alone.
    fn from_str(text: &str) -> Result<EcoRange, ParseError> {
        let (a, b) = split_range(text);
        let from = if a.is_empty() { EcoCode { code: 0, sub: None } } else { a.parse()? };
        let to = if b.is_empty() { EcoCode { code: 499, sub: None } } else { b.parse()? };
        Ok(EcoRange { from, to })
    }
}

/// A numeric range as typed: `2500-3500`, `2500-`, `-22` or `40`.
pub fn parse_range(text: &str) -> Result<Range, ParseError> {
    let bad = || error("a numeric range (low-high)", text);
    let (a, b) = split_range(text);
    let end = |t: &str| if t.is_empty() { Ok(None) } else { t.parse::<i32>().map(Some).map_err(|_| bad()) };
    match (end(a)?, end(b)?) {
        (Some(low), Some(high)) => Ok(Range::new(low, high)),
        (Some(low), None) => Ok(Range::at_least(low)),
        (None, Some(high)) => Ok(Range::at_most(high)),
        (None, None) => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_with_one_two_or_three_parts() {
        let range: DateRange = "2020-2026.11".parse().unwrap();
        assert_eq!(range.from, Some(PartialDate { year: 2020, month: None, day: None }));
        assert_eq!(range.to, Some(PartialDate { year: 2026, month: Some(11), day: None }));
        assert_eq!("2026.11.03".parse::<PartialDate>().unwrap().day, Some(3));
        assert!("2026.13".parse::<PartialDate>().is_err());
        assert!("-".parse::<DateRange>().is_err());
    }

    #[test]
    fn eco_codes_read_with_and_without_sub_codes() {
        assert_eq!("b20".parse::<EcoCode>().unwrap(), EcoCode { code: 120, sub: None });
        assert_eq!("E99/99".parse::<EcoCode>().unwrap(), EcoCode { code: 499, sub: Some(99) });
        assert!("F00".parse::<EcoCode>().is_err());
        assert!("B2".parse::<EcoCode>().is_err());
        let range: EcoRange = "A00-E99/99".parse().unwrap();
        assert_eq!(range.from.code, 0);
        assert_eq!(range.to.sub, Some(99));
    }

    #[test]
    fn numeric_ranges_may_be_open_at_either_end() {
        assert!(parse_range("2500-3500").unwrap().contains(2600));
        assert!(parse_range("2500-").unwrap().contains(2900));
        assert!(!parse_range("-22").unwrap().contains(23));
        assert!(parse_range("40").unwrap().contains(40));
        assert!(parse_range("x").is_err());
    }
}
