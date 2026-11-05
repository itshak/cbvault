// Adapted from oschess-cb-bridge v1.0.3 (MIT), commit bffebfc498d52b4e4631aed32b2f8e8194d125ff.
// Copyright (c) 2026 the oschess-cb-bridge contributors.
// Modifications copyright (c) 2026 the cbvault contributors.
// SPDX-License-Identifier: MIT
//! One `.2cbg`/`.2cba` frame. The MIT v1.0.3 specification gives a
//! big-endian checksum at 0x10, a two-byte tag at 0x18, and content at 0x1a.
//! All content is borrowed; only error paths format strings.
//!
//! The checksum is verified on every record; no damaged payload is silently
//! handed to a consumer. The 0xffff move terminator is part of the content,
//! not an additional two bytes between the content and spare capacity.
//! Provenance: corrected using the pinned MIT v1.0.3 v2/frame.rs baseline.
//!
use crate::error::{Error, Result};

use super::bytes::{CONTENT_OFFSET, FIXED_OVERHEAD, MAX_RECORD, RECORD_MAGIC};

/// Offset of the record's total-length trailer, counted back from its end.
pub const TRAILER: usize = 8;
/// One record of a `.2cbg` or `.2cba`, split into its parts. The content is
/// borrowed; nothing is copied and nothing is interpreted.
#[derive(Clone, Copy, Debug)]
pub struct Record<'a> {
    a: u32,
    b: u32,
    tag: u16,
    stored_checksum: u64,
    content: &'a [u8],
}

impl<'a> Record<'a> {
    /// Splits the whole record at `offset`, whose bytes are `raw` and whose
    /// position within its file is `offset` (for error messages).
    #[inline]
    pub fn parse(path: &std::path::Path, offset: u64, raw: &'a [u8]) -> Result<Self> {
        let bad = |what: String| Error::corrupt(path, offset, format!("2CBH record: {what}"));
        if raw.len() < FIXED_OVERHEAD {
            return Err(bad(format!("{} bytes, shorter than the {FIXED_OVERHEAD}-byte frame", raw.len())));
        }
        if raw[..8] != RECORD_MAGIC {
            return Err(bad(format!("magic {:02x?}, expected {RECORD_MAGIC:02x?}", &raw[..8])));
        }
        let total = u64::from_le_bytes(raw[raw.len() - TRAILER..].try_into().expect("8 bytes"));
        if total != raw.len() as u64 {
            return Err(bad(format!("length field {total} for a {}-byte record", raw.len())));
        }
        if total > MAX_RECORD as u64 {
            return Err(bad(format!("length field {total} past the {MAX_RECORD}-byte limit")));
        }
        let a = u32::from_le_bytes(raw[8..12].try_into().expect("4 bytes"));
        let b = u32::from_le_bytes(raw[12..16].try_into().expect("4 bytes"));
        let expect = FIXED_OVERHEAD as u64 + u64::from(a) + u64::from(b);
        if expect != total {
            return Err(bad(format!("length field {total}, but 34 + {a} + {b} = {expect}")));
        }
        let end = CONTENT_OFFSET + a as usize;
        let content =
            raw.get(CONTENT_OFFSET..end).ok_or_else(|| bad(format!("content runs past the {total}-byte record")))?;
        let stored_checksum = u64::from_be_bytes(raw[16..24].try_into().expect("8 checksum bytes"));
        if stored_checksum != checksum(content) {
            return Err(bad("checksum mismatch".to_owned()));
        }
        let tag = u16::from_le_bytes([raw[0x18], raw[0x19]]);
        Ok(Record { a, b, tag, stored_checksum, content })
    }

    /// Splits bytes already accepted by [`Record::parse`]. Cheap enough to call
    /// per access, and it lets a reader that owns or holds the bytes hand out a
    /// [`Record`] without re-validating or re-reading.
    #[inline]
    pub(crate) fn parse_unchecked(raw: &[u8]) -> Record<'_> {
        let a = u32::from_le_bytes(raw[8..12].try_into().expect("a validated record"));
        let b = u32::from_le_bytes(raw[12..16].try_into().expect("a validated record"));
        let end = CONTENT_OFFSET + a as usize;
        Record {
            a,
            b,
            tag: u16::from_le_bytes([raw[0x18], raw[0x19]]),
            stored_checksum: u64::from_be_bytes(raw[16..24].try_into().expect("a validated record")),
            content: &raw[CONTENT_OFFSET..end],
        }
    }

    /// The record's `A` parameter: the content length less
    /// its exact content length, and always even in the files observed.
    #[inline]
    pub fn parameter_a(&self) -> u32 {
        self.a
    }

    /// Spare capacity B following the A content bytes.
    #[inline]
    pub fn parameter_b(&self) -> u32 {
        self.b
    }

    /// Two-byte little-endian content/variant tag.
    #[inline]
    pub fn tag(&self) -> u16 {
        self.tag
    }

    /// Stored big-endian checksum, verified when parsed.
    #[inline]
    pub fn stored_checksum(&self) -> u64 {
        self.stored_checksum
    }

    /// Exactly A content bytes; a move-stream end marker, if present,
    /// belongs inside this slice.
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

/// Allocation-free checksum from the pinned MIT v1.0.3 frame specification.
/// For content shorter than 8 bytes, zero-pad into a little-endian u64.
/// Otherwise sum each of eight equal contiguous runs modulo 256; ignore
/// trailing bytes after the eighth run.
#[inline]
pub fn checksum(content: &[u8]) -> u64 {
    let width = content.len() / 8;
    if width == 0 {
        let mut padded = [0u8; 8];
        padded[..content.len()].copy_from_slice(content);
        return u64::from_le_bytes(padded);
    }
    let mut sum = [0u8; 8];
    for (i, slot) in sum.iter_mut().enumerate() {
        let begin = i * width;
        *slot = content[begin..begin + width].iter().fold(0u8, |acc, &byte| acc.wrapping_add(byte));
    }
    u64::from_le_bytes(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twocbh::testdata::file_header as header_of;
    use crate::twocbh::testdata::{record, record_with};

    fn p() -> &'static std::path::Path {
        std::path::Path::new("db.2cbg")
    }

    #[test]
    fn a_record_splits_into_its_parts() {
        let raw = record(102, &[7u8; 28]);
        let r = Record::parse(p(), 12, &raw).unwrap();
        assert_eq!(r.parameter_a(), 28);
        assert_eq!(r.parameter_b(), 102);
        assert_eq!(r.tag(), 1, "the two-byte v2 tag");
        assert_eq!(r.content(), &[7u8; 28], "the content is exactly A bytes");
        assert_eq!(r.total(), 164);
        assert_eq!(raw.len(), r.total());
    }

    #[test]
    fn the_content_ends_with_the_terminator() {
        let mut body = vec![3u8; 28];
        body[26] = 0xff;
        body[27] = 0xff;
        let raw = record(102, &body);
        let r = Record::parse(p(), 12, &raw).unwrap();
        assert_eq!(&r.content()[26..], &[0xff, 0xff]);
    }

    #[test]
    fn a_bad_magic_is_corruption_at_the_record() {
        let mut raw = record(98, &[1u8; 4]);
        raw[3] ^= 0xff;
        let e = Record::parse(p(), 0x1_000, &raw).unwrap_err();
        match e {
            Error::Corrupt { offset, .. } => assert_eq!(offset, 0x1_000),
            other => panic!("expected Corrupt, got {other}"),
        }
    }

    #[test]
    fn a_length_field_that_disagrees_with_the_record_is_rejected() {
        // Trailer says 99 bytes for a 164-byte record.
        let mut raw = record(102, &[7u8; 37]);
        let n = raw.len();
        raw[n - 8..].copy_from_slice(&99u64.to_le_bytes());
        assert!(Record::parse(p(), 12, &raw).is_err());

        // Trailer agrees with the length but not with 34 + a + b.
        let mut raw = record(102, &[7u8; 28]);
        let n = raw.len() - 8;
        raw[n..].copy_from_slice(&(n as u64).to_le_bytes());
        assert!(Record::parse(p(), 12, &raw).is_err());
    }

    #[test]
    fn an_absurd_length_field_is_refused_before_allocating() {
        let mut raw = record(98, &[1u8; 4]);
        let n = raw.len();
        raw[n - 8..].copy_from_slice(&u64::MAX.to_le_bytes());
        let e = Record::parse(p(), 12, &raw).unwrap_err().to_string();
        assert!(e.contains("length field"), "{e}");
    }

    #[test]
    fn a_record_shorter_than_its_frame_is_rejected() {
        assert!(Record::parse(p(), 0, &[0u8; 20]).is_err());
    }

    #[test]
    fn a_zero_length_content_is_a_valid_record() {
        let raw = record_with(0, 98, &[]);
        let r = Record::parse(p(), 12, &raw).unwrap();
        assert!(r.content().is_empty(), "a record may carry no content at all");
        assert_eq!(r.total(), 132);
    }

    #[test]
    fn corrupt_content_is_rejected_by_checksum() {
        let mut raw = record(7, &[1u8; 16]);
        raw[CONTENT_OFFSET + 3] ^= 1;
        let e = Record::parse(p(), 12, &raw).unwrap_err().to_string();
        assert!(e.contains("checksum mismatch"), "{e}");
    }

    #[test]
    fn checksum_covers_eight_runs_and_ignores_remainder() {
        assert_eq!(checksum(&[1, 2, 3]), 0x030201);
        let content: Vec<u8> = (0..17).collect();
        let expected: u64 = (0..8).map(|i| ((4 * i + 1) as u64) << (8 * i)).sum();
        assert_eq!(checksum(&content), expected);
    }

    #[test]
    fn a_file_header_is_not_a_record() {
        let only_header = header_of(12);
        assert!(Record::parse(p(), 0, &only_header).is_err(), "a 12-byte header is not a record");
    }
}
