//! The magic every 2CBH `.2cbg`/`.2cba` record starts with, and the field
//! widths around it.
//!
//! Observed on the ten 2CBH sets on the development machine; see
//! `docs/format-spec-2cbh.md` §5 for the evidence. The four invariants this
//! module relies on were checked on 220,418 records with no violations.

/// The eight bytes a `.2cbg`/`.2cba` record starts with, as stored:
/// the byte image of the `u64` [`RECORD_MAGIC`].
pub const RECORD_MAGIC: [u8; 8] = [0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11];

/// The record magic as a number, for a reader that prefers to compare `u64`s.
pub const RECORD_MAGIC_U64: u64 = u64::from_le_bytes(RECORD_MAGIC);

/// Size of a `.2cbg`/`.2cba` file header.
pub const FILE_HEADER_SIZE: usize = 12;

/// Size of a `.2cbh` record, and of the file header before the first one.
pub const RECORD_SIZE: usize = 192;

/// Conservative maximum framed record size; corrupted lengths must not cause
/// unbounded buffer allocation. The MIT v1.0.3 baseline uses 64 MiB
/// limits for each of A and B; this bound applies to their sum.
pub const MAX_RECORD: usize = 64 << 20;

/// The fixed framing bytes: magic (8), content size A (4), spare size B (4),
/// big-endian checksum (8), 16-bit little-endian tag (2), and trailer (8).
/// Content begins at offset 0x1a. Total record size is 34 + A + B.
/// The move-stream 0xffff marker belongs inside A, not after it.
pub const FIXED_OVERHEAD: usize = 34;
/// First content byte, immediately after the two-byte tag.
pub const CONTENT_OFFSET: usize = 0x1a;

/// Metadata fields at 0x68 and 0x78 are rating-related, not inline names.
/// Player names are referenced by entity IDs in the .2lid registry.
