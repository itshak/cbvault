//! A game's `.2cba` record: its framing, and the clear text inside it.
//!
//! Unlike the `.2cbg` codec, 2CBA content is **plain text in the clear** — the
//! local sets contain free-text result notes, each a NUL-terminated string
//! preceded by its length (spec §5.4). So unlike the moves, these bytes are
//! genuinely readable.
//!
//! The example below is synthetic. Real annotation text is copied verbatim from
//! a licensed database and must never be committed, so the fixture is written
//! here rather than taken from a set.
//!
//! What is deliberately **not** done here is a per-ply item parse. The
//! classic format's `.cba` maps each item to a move position, and a 2CBA
//! record's item boundaries are only **partially** mapped (spec §5.4): the
//! length word is confirmed by the string it introduces, but the small
//! integers around it are unknown. A wrong boundary would place a comment on
//! the wrong ply — a plausible-looking wrong answer, which is exactly what this
//! module refuses to produce. So [`Annotations`] exposes the whole record and
//! the strings in it, and says so.

use crate::cbh::bytes::text;
use crate::error::Result;

use super::record::Record;

/// One game's `.2cba` annotation record.
#[derive(Clone, Copy, Debug)]
pub struct Annotations<'a> {
    record: Record<'a>,
}

impl<'a> Annotations<'a> {
    /// Splits a whole `.2cba` record of `path`, its magic and trailer
    /// included.
    #[inline]
    pub fn parse(path: &std::path::Path, raw: &'a [u8]) -> Result<Self> {
        Ok(Annotations { record: Record::parse(path, 0, raw)? })
    }

    /// The record's framing.
    #[inline]
    pub fn record(&self) -> &Record<'a> {
        &self.record
    }

    /// The record's content, exactly as stored.
    #[inline]
    pub fn content(&self) -> &'a [u8] {
        self.record.content()
    }

    /// Inspect printable byte runs without allocating or attributing them to
    /// a ply. This is a diagnostic view ONLY, never a PGN annotation decoder:
    /// structure bytes may coincidentally be printable and Unicode content
    /// needs its own typed decoding.
    pub fn visit_printable_runs(&self, mut visitor: impl FnMut(&[u8])) {
        let bytes = self.content();
        let mut start = 0;
        while start < bytes.len() {
            let rel = bytes[start..].iter().position(|&b| !(0x20..0x7f).contains(&b));
            match rel {
                Some(n) => {
                    if n >= MIN_TEXT {
                        visitor(&bytes[start..start + n]);
                    }
                    start += n + 1;
                }
                None => {
                    if bytes.len() - start >= MIN_TEXT {
                        visitor(&bytes[start..]);
                    }
                    break;
                }
            }
        }
    }

    /// Allocating convenience wrapper around visit_printable_runs, for
    /// diagnostics only; do not use these unlocated strings in PGN output.
    pub fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.visit_printable_runs(|run| out.push(text(run)));
        out
    }
}

/// Shortest run of printable bytes [`Annotations::texts`] reports. Below this
/// it is a length word or a flag that happens to be printable, not text; the
/// real annotations observed are sentences many bytes long.
const MIN_TEXT: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twocbh::testdata::record;

    fn p() -> &'static std::path::Path {
        std::path::Path::new("db.2cba")
    }

    #[test]
    fn clear_text_is_recovered() {
        // A record shaped like the local ones: a fixed sub-header, then a
        // length word and the sentence it introduces.
        let mut body = vec![0u8; 40];
        body.extend_from_slice(&7u16.to_le_bytes());
        body.extend_from_slice(&33u32.to_le_bytes());
        body.extend_from_slice(b"synthetic fixture text, not from any database\0");
        let raw = record(198, &body);
        let ann = Annotations::parse(p(), &raw).unwrap();
        assert_eq!(ann.texts(), vec!["synthetic fixture text, not from any database".to_owned()]);
    }

    #[test]
    fn a_record_with_no_text_yields_nothing() {
        let raw = record(198, &[0u8; 24]);
        assert!(Annotations::parse(p(), &raw).unwrap().texts().is_empty());
    }

    #[test]
    fn the_framing_is_still_validated() {
        let mut raw = record(198, &[0u8; 9]);
        raw[1] ^= 0xff;
        assert!(Annotations::parse(p(), &raw).is_err());
    }
}
