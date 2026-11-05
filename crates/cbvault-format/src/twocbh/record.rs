// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! One `.2cbg`/`.2cba` record: its magic, its two parameters, its checksum, its
//! 2-byte tag, its content, spare padding and its self-delimiting length trailer.

use std::path::Path;

use super::bytes::{FIXED_OVERHEAD, FRAME_HEADER, MAX_FRAME_PART, RECORD_MAGIC, be_u64, le_i32, le_u16};
use super::frame::checksum;
use crate::error::{Error, Result};

/// Offset of the record's total-length trailer, counted back from its end.
pub const TRAILER: usize = 8;

/// One record of a `.2cbg` or `.2cba`, split into its parts.
#[derive(Clone, Copy, Debug)]
pub struct Record<'a> {
    a: u32,
    b: u32,
    checksum: u64,
    tag: u16,
    content: &'a [u8],
}

impl<'a> Record<'a> {
    /// Splits and validates the whole record at `offset`, whose bytes are `raw`.
    #[inline]
    pub fn parse(path: &Path, offset: u64, raw: &'a [u8]) -> Result<Self> {
        let bad = |what: String| Error::corrupt(path, offset, format!("2CBH record: {what}"));
        if raw.len() < FIXED_OVERHEAD {
            return Err(bad(format!("{} bytes, shorter than the {FIXED_OVERHEAD}-byte frame", raw.len())));
        }
        if raw[..8] != RECORD_MAGIC {
            return Err(bad(format!("magic {:02x?}, expected {RECORD_MAGIC:02x?}", &raw[..8])));
        }
        let a = u32::try_from(le_i32(raw, 8)).map_err(|_| bad("negative size A".into()))?;
        let b = u32::try_from(le_i32(raw, 12)).map_err(|_| bad("negative size B".into()))?;
        if a as usize > MAX_FRAME_PART || b as usize > MAX_FRAME_PART {
            return Err(bad("record larger than 64 MiB limit".into()));
        }
        let total = u64::from_le_bytes(raw[raw.len() - TRAILER..].try_into().expect("8 bytes"));
        if total != raw.len() as u64 {
            return Err(bad(format!("length field {total} for a {}-byte record", raw.len())));
        }
        let expect = (FIXED_OVERHEAD + a as usize + b as usize) as u64;
        if expect != total {
            return Err(bad(format!("length field {total}, but 34 + {a} + {b} = {expect}")));
        }
        let csum = be_u64(raw, 0x10);
        let tag = le_u16(raw, 0x18);
        let start = FRAME_HEADER;
        let end = start + a as usize;
        let content = raw.get(start..end).ok_or_else(|| bad(format!("content runs past the {total}-byte record")))?;
        if csum != checksum(content) {
            return Err(bad("checksum mismatch".into()));
        }
        Ok(Record { a, b, checksum: csum, tag, content })
    }

    /// Splits bytes already accepted by [`Record::parse`]. Cheap enough to call
    /// per access without re-validating.
    #[inline]
    pub fn parse_unchecked(raw: &[u8]) -> Record<'_> {
        let a = u32::from_le_bytes(raw[8..12].try_into().expect("validated"));
        let b = u32::from_le_bytes(raw[12..16].try_into().expect("validated"));
        let csum = be_u64(raw, 0x10);
        let tag = le_u16(raw, 0x18);
        let start = FRAME_HEADER;
        let end = start + a as usize;
        Record { a, b, checksum: csum, tag, content: &raw[start..end] }
    }

    /// The record's content length `A`.
    #[inline]
    pub fn parameter_a(&self) -> u32 {
        self.a
    }

    /// The record's spare capacity `B`.
    #[inline]
    pub fn parameter_b(&self) -> u32 {
        self.b
    }

    /// The record's 64-bit checksum.
    #[inline]
    pub fn checksum(&self) -> u64 {
        self.checksum
    }

    /// The 2-byte variant tag at offset `0x18`.
    #[inline]
    pub fn tag(&self) -> u16 {
        self.tag
    }

    /// The record's content.
    #[inline]
    pub fn content(&self) -> &'a [u8] {
        self.content
    }

    /// The record's total length in bytes, as its trailer states it.
    #[inline]
    pub fn total(&self) -> usize {
        FIXED_OVERHEAD + self.a as usize + self.b as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twocbh::frame::checksum;

    fn p() -> &'static Path {
        Path::new("db.2cbg")
    }

    fn make_test_record(tag: u16, a: u32, b: u32, content: &[u8]) -> Vec<u8> {
        let mut r = Vec::new();
        r.extend_from_slice(&RECORD_MAGIC);
        r.extend_from_slice(&(a as i32).to_le_bytes());
        r.extend_from_slice(&(b as i32).to_le_bytes());
        r.extend_from_slice(&checksum(content).to_be_bytes());
        r.extend_from_slice(&tag.to_le_bytes());
        r.extend_from_slice(content);
        r.resize(FRAME_HEADER + a as usize + b as usize, 0);
        r.extend_from_slice(&(a as i64 + b as i64 + 34).to_le_bytes());
        r
    }

    #[test]
    fn a_record_splits_into_its_parts() {
        let content = [7u8; 28];
        let raw = make_test_record(1, 28, 102, &content);
        let r = Record::parse(p(), 12, &raw).unwrap();
        assert_eq!(r.parameter_a(), 28);
        assert_eq!(r.parameter_b(), 102);
        assert_eq!(r.tag(), 1);
        assert_eq!(r.content(), &content);
        assert_eq!(r.total(), 164);
        assert_eq!(raw.len(), r.total());
    }

    #[test]
    fn a_bad_magic_is_corruption_at_the_record() {
        let mut raw = make_test_record(1, 4, 0, &[1, 2, 3, 4]);
        raw[0] = 0x00;
        assert!(Record::parse(p(), 12, &raw).is_err());
    }

    #[test]
    fn bad_checksum_rejected() {
        let mut raw = make_test_record(1, 4, 0, &[1, 2, 3, 4]);
        raw[0x10] ^= 0xff; // corrupt checksum
        assert!(Record::parse(p(), 12, &raw).is_err());
    }

    #[test]
    fn bad_trailer_rejected() {
        let mut raw = make_test_record(1, 4, 0, &[1, 2, 3, 4]);
        let len = raw.len();
        raw[len - 8..].copy_from_slice(&9999i64.to_le_bytes());
        assert!(Record::parse(p(), 12, &raw).is_err());
    }
}
