// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! Framing of records in `.2cbg` and `.2cba`: magic, sizes, checksum, tag, content,
//! spare padding and trailing length.

use super::bytes::{FRAME_HEADER, MAX_FRAME_PART, RECORD_MAGIC, be_u64, le_i32, le_i64, le_u16};
use crate::error::{Error, Result};

/// The content (A) and spare (B) sizes of a frame, from its first bytes.
pub fn frame_sizes(frame: &[u8], offset: i64) -> Result<(usize, usize)> {
    let bad = |what: &str| Error::corrupt(std::path::Path::new("<frame>"), offset.max(0) as u64, what);
    if frame.len() < FRAME_HEADER {
        return Err(bad("offset out of range"));
    }
    if frame[..8] != RECORD_MAGIC {
        return Err(bad("bad magic"));
    }
    let a = usize::try_from(le_i32(frame, 8)).map_err(|_| bad("negative size A"))?;
    let b = usize::try_from(le_i32(frame, 12)).map_err(|_| bad("negative size B"))?;
    if a > MAX_FRAME_PART || b > MAX_FRAME_PART {
        return Err(bad("record larger than 64 MiB"));
    }
    Ok((a, b))
}

/// Checks one framed record from `.2cbg` or `.2cba` held from its first byte in `frame`:
/// magic, sizes, trailing length and checksum. Returns its variant tag and content slice.
pub fn parse_frame(frame: &[u8], offset: i64, verify_checksum: bool) -> Result<(u16, &[u8])> {
    let bad = |what: &str| Error::corrupt(std::path::Path::new("<frame>"), offset.max(0) as u64, what);
    let (a, b) = frame_sizes(frame, offset)?;
    if FRAME_HEADER + a + b + 8 > frame.len() {
        return Err(bad("runs past end of file"));
    }
    let tail = le_i64(frame, FRAME_HEADER + a + b);
    if tail != (a + b + 34) as i64 {
        return Err(bad("trailing length mismatch"));
    }
    let content = &frame[FRAME_HEADER..FRAME_HEADER + a];
    if verify_checksum && be_u64(frame, 0x10) != checksum(content) {
        return Err(bad("checksum mismatch"));
    }
    Ok((le_u16(frame, 0x18), content))
}

/// The 2CBH record checksum:
///
/// Byte `i` of the 64-bit value is the wrapping sum (modulo 256) of the `i`-th of eight
/// equal runs of the content; a trailing remainder is ignored. For content shorter than
/// 8 bytes, the content is copied into a zeroed 8-byte buffer and read little-endian.
pub fn checksum(content: &[u8]) -> u64 {
    let m = content.len() / 8;
    if m == 0 {
        let mut buf = [0u8; 8];
        buf[..content.len()].copy_from_slice(content);
        return u64::from_le_bytes(buf);
    }
    let mut v = 0u64;
    for i in 0..8 {
        let s = content[i * m..(i + 1) * m].iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
        v |= (s as u64) << (8 * i);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_short_and_runs() {
        assert_eq!(checksum(&[1, 2, 3]), 0x030201);
        let content: Vec<u8> = (0..17).collect();
        // m = 2: runs (0,1) (2,3) ... (14,15); byte 16 ignored
        let expect: u64 = (0..8).map(|i| ((2 * i + 2 * i + 1) as u64) << (8 * i)).sum();
        assert_eq!(checksum(&content), expect);
    }

    #[test]
    fn parse_valid_frame() {
        let content = b"test payload 1234";
        let a = content.len();
        let b = 10;
        let mut frame = Vec::new();
        frame.extend_from_slice(&RECORD_MAGIC);
        frame.extend_from_slice(&(a as i32).to_le_bytes());
        frame.extend_from_slice(&(b as i32).to_le_bytes());
        frame.extend_from_slice(&checksum(content).to_be_bytes());
        frame.extend_from_slice(&0x0001u16.to_le_bytes()); // tag
        frame.extend_from_slice(content);
        frame.resize(FRAME_HEADER + a + b, 0); // padding B
        frame.extend_from_slice(&((a + b + 34) as i64).to_le_bytes()); // trailer

        let (tag, parsed) = parse_frame(&frame, 0x100, true).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(parsed, content);
    }

    #[test]
    fn parse_frame_bad_checksum() {
        let content = b"test payload 1234";
        let a = content.len();
        let b: usize = 0;
        let mut frame = Vec::new();
        frame.extend_from_slice(&RECORD_MAGIC);
        frame.extend_from_slice(&(a as i32).to_le_bytes());
        frame.extend_from_slice(&(b as i32).to_le_bytes());
        frame.extend_from_slice(&0xdeadbeefu64.to_be_bytes()); // corrupted checksum
        frame.extend_from_slice(&0x0001u16.to_le_bytes());
        frame.extend_from_slice(content);
        frame.extend_from_slice(&((a + b + 34) as i64).to_le_bytes());

        let err = parse_frame(&frame, 0x100, true).unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));

        // If verify_checksum is false, it succeeds
        let (tag, parsed) = parse_frame(&frame, 0x100, false).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(parsed, content);
    }

    #[test]
    fn parse_frame_bad_trailer() {
        let content = b"test payload 1234";
        let a = content.len();
        let b: usize = 0;
        let mut frame = Vec::new();
        frame.extend_from_slice(&RECORD_MAGIC);
        frame.extend_from_slice(&(a as i32).to_le_bytes());
        frame.extend_from_slice(&(b as i32).to_le_bytes());
        frame.extend_from_slice(&checksum(content).to_be_bytes());
        frame.extend_from_slice(&0x0001u16.to_le_bytes());
        frame.extend_from_slice(content);
        frame.extend_from_slice(&(9999i64).to_le_bytes()); // bad trailer

        let err = parse_frame(&frame, 0x100, true).unwrap_err();
        assert!(err.to_string().contains("trailing length mismatch"));
    }
}
