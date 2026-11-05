// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! Byte reading helpers and layout constants for the 2CBH format family.

/// The eight bytes a `.2cbg`/`.2cba` record starts with, as stored:
/// `0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11` (LE `0x1122334455667788`).
pub const RECORD_MAGIC: [u8; 8] = [0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11];

/// The record magic as a 64-bit integer.
pub const RECORD_MAGIC_U64: u64 = u64::from_le_bytes(RECORD_MAGIC);

/// Size of a `.2cbg`/`.2cba` file header.
pub const FILE_HEADER_SIZE: usize = 12;

/// Size of a `.2cbh` record and of the file header.
pub const RECORD_SIZE: usize = 192;

/// Synonym for [`RECORD_SIZE`].
pub const HEADER_RECORD_SIZE: usize = 192;

/// Size of a move/annotation frame header before its content:
/// 8 bytes magic + 4 bytes A + 4 bytes B + 8 bytes checksum + 2 bytes tag = 26 bytes (`0x1a`).
pub const FRAME_HEADER: usize = 0x1a;

/// Total overhead around content in a frame: 26 bytes header + 8 bytes trailer length = 34 bytes.
/// A record's total size on disk is `FIXED_OVERHEAD + A + B`.
pub const FIXED_OVERHEAD: usize = 34;

/// Largest content or spare area accepted in one move record (64 MiB).
pub const MAX_FRAME_PART: usize = 64 << 20;

/// Synonym for [`MAX_FRAME_PART`].
pub const MAX_RECORD: usize = MAX_FRAME_PART;

/// Deprecated legacy offset; use [`FRAME_HEADER`].
pub const CONTENT_OFFSET: usize = 24;

/// Deprecated legacy terminator size.
pub const TERMINATOR_SIZE: usize = 2;

/// Width of each of the two inline name fields in a `.2cbh` game record (§4.3).
pub const NAME_FIELD: usize = 16;

/// Bounded big-endian 64-bit integer read.
#[inline]
pub fn be_u64(b: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(b[at..at + 8].try_into().unwrap())
}

/// Bounded big-endian 64-bit signed integer read.
#[inline]
pub fn be_i64(b: &[u8], at: usize) -> i64 {
    i64::from_be_bytes(b[at..at + 8].try_into().unwrap())
}

/// Bounded big-endian 32-bit signed integer read.
#[inline]
pub fn be_i32(b: &[u8], at: usize) -> i32 {
    i32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

/// Bounded little-endian 64-bit signed integer read.
#[inline]
pub fn le_i64(b: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// Bounded little-endian 64-bit unsigned integer read.
#[inline]
pub fn le_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// Bounded little-endian 32-bit signed integer read.
#[inline]
pub fn le_i32(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Bounded little-endian 32-bit unsigned integer read.
#[inline]
pub fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Bounded little-endian 16-bit signed integer read.
#[inline]
pub fn le_i16(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}

/// Bounded little-endian 16-bit unsigned integer read.
#[inline]
pub fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
