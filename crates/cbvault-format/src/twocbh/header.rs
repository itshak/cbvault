// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! 192-byte .2cbh record, with field semantics from the pinned MIT v1.0.3
//! oschess-cb-bridge v2/record.rs baseline.
//!
//! All integer fields are little-endian. Player names are NOT inline: the
//! values at offsets 0x18 and 0x20 are .2lid entity IDs. At 0x68/0x78 the
//! metadata is rating-related, not text. No incorrect placeholder PGN tags
//! are invented. The full 192 bytes are held by value, without allocation.

use crate::game::{Date, Eco, GameResult, Head, RecordKind};
use super::bytes::RECORD_SIZE;

#[derive(Clone, Copy)]
pub struct GameHeader {
    id: u32,
    b: [u8; RECORD_SIZE],
}

#[inline]
fn le_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn le_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn le_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().expect("192-byte record"))
}
#[inline]
fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().expect("192-byte record"))
}
#[inline]
fn le_i64(b: &[u8], o: usize) -> i64 {
    i64::from_le_bytes(b[o..o + 8].try_into().expect("192-byte record"))
}
#[inline]
fn le_u64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().expect("192-byte record"))
}

impl GameHeader {
    pub fn from_bytes(id: u32, b: &[u8; RECORD_SIZE]) -> Self {
        Self { id, b: *b }
    }
    #[inline]
    pub fn id(&self) -> u32 { self.id }
    #[inline]
    pub fn bytes(&self) -> &[u8] { &self.b }

    /// Only valid for file header (record 0).
    pub fn generation_byte(&self) -> u8 { self.b[0x08] }
    /// Only valid for file header (record 0).
    pub fn stated_record_size(&self) -> u16 { le_u16(&self.b, 0x0a) }
    /// Only valid for file header (record 0).
    pub fn stated_records(&self) -> u32 { le_u32(&self.b, 0x10) }

    /// Record flags at byte 0; bit 0x80 is the deletion flag.
    pub fn flags_byte(&self) -> u8 { self.b[0] }
    /// Type discriminator at byte 2 (1 game/text, 2 analysis).
    pub fn kind_byte(&self) -> u8 { self.b[2] }
    pub fn is_deleted(&self) -> bool { self.b[0] & 0x80 != 0 }
    pub fn kind(&self) -> RecordKind {
        match self.b[2] {
            1 if self.b[0] & 2 != 0 => RecordKind::Text,
            1 => RecordKind::Game,
            2 => RecordKind::Analysis,
            other => RecordKind::Unknown(other),
        }
    }

    /// Signed byte offset of moves or guiding text in .2cbg.
    pub fn moves_offset(&self) -> i64 { le_i64(&self.b, 0x08) }
    /// Signed byte offset of annotations in .2cba.
    pub fn annotations_offset(&self) -> i64 { le_i64(&self.b, 0x10) }

    /// Entity IDs in .2lid. These are NOT UTF-8 or inline names.
    pub fn white(&self) -> i64 { le_i64(&self.b, 0x18) }
    pub fn black(&self) -> i64 { le_i64(&self.b, 0x20) }
    pub fn tournament(&self) -> i64 { le_i64(&self.b, 0x28) }
    pub fn annotator(&self) -> i64 { le_i64(&self.b, 0x30) }
    pub fn source(&self) -> i64 { le_i64(&self.b, 0x38) }
    pub fn white_team(&self) -> i64 { le_i64(&self.b, 0x40) }
    pub fn black_team(&self) -> i64 { le_i64(&self.b, 0x48) }
    pub fn game_tag(&self) -> i64 { le_i64(&self.b, 0x50) }
    pub fn text_author(&self) -> i64 { le_i64(&self.b, 0x20) }
    pub fn text_title(&self) -> i64 { le_i64(&self.b, 0x28) }
    pub fn analysis_title(&self) -> i64 { le_i64(&self.b, 0x18) }
    pub fn analysis_author(&self) -> i64 { le_i64(&self.b, 0x28) }

    pub fn result(&self) -> GameResult { GameResult::from_field(self.b[0x58]) }
    pub fn eco(&self) -> Eco { Eco::from_field(le_u16(&self.b, 0x80)) }
    pub fn flags(&self) -> u32 { le_u32(&self.b, 0x84) }
    pub fn played_date(&self) -> Date { Date(le_i32(&self.b, 0xbc)) }
    pub fn board(&self) -> i16 { le_i16(&self.b, 0x5e) }
    pub fn round(&self) -> (i32, i32) { (i32::from(le_i16(&self.b, 0x5a)), i32::from(le_i16(&self.b, 0x5c))) }
    pub fn elo(&self) -> (i32, i32) { (i32::from(le_i16(&self.b, 0x60)), i32::from(le_i16(&self.b, 0x70))) }
    pub fn move_count(&self) -> i32 { i32::from(le_i16(&self.b, 0x8a)) }
    /// Original uninterpreted per-record value (not yet proven unique key).
    pub fn key(&self) -> u64 { le_u64(&self.b, 0x98) }

    /// Raw rating-related fields: never interpret as player names.
    pub fn rating_white_bytes(&self) -> &[u8] { &self.b[0x68..0x78] }
    pub fn rating_black_bytes(&self) -> &[u8] { &self.b[0x78..0x88] }
}

impl Head for GameHeader {
    fn id(&self) -> u32 { GameHeader::id(self) }
    fn kind(&self) -> RecordKind { GameHeader::kind(self) }
    fn is_deleted(&self) -> bool { GameHeader::is_deleted(self) }
    fn white(&self) -> i64 { GameHeader::white(self) }
    fn black(&self) -> i64 { GameHeader::black(self) }
    fn tournament(&self) -> i64 { GameHeader::tournament(self) }
    fn annotator(&self) -> i64 { GameHeader::annotator(self) }
    fn other(&self) -> Option<(i64, i64)> {
        match self.kind() {
            RecordKind::Game => None,
            RecordKind::Text => Some((self.text_title(), self.text_author())),
            RecordKind::Analysis => Some((self.analysis_title(), self.analysis_author())),
            RecordKind::Unknown(_) => Some((-1, -1)),
        }
    }
    fn result(&self) -> GameResult { GameHeader::result(self) }
    fn eco(&self) -> Eco { GameHeader::eco(self) }
    fn played_date(&self) -> Date { GameHeader::played_date(self) }
    fn round(&self) -> (i32, i32) { GameHeader::round(self) }
    fn elo(&self) -> (i32, i32) { GameHeader::elo(self) }
    fn move_count(&self) -> i32 { GameHeader::move_count(self) }
    fn bytes(&self) -> &[u8] { GameHeader::bytes(self) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_actual_v2_field_offsets_without_heap() {
        let mut b = [0u8; RECORD_SIZE];
        b[2] = 1;
        b[0x18..0x20].copy_from_slice(&345i64.to_le_bytes());
        b[0x20..0x28].copy_from_slice(&678i64.to_le_bytes());
        b[0x58] = 2;
        b[0x60..0x62].copy_from_slice(&2450i16.to_le_bytes());
        b[0x70..0x72].copy_from_slice(&2200i16.to_le_bytes());
        b[0x80..0x82].copy_from_slice(&(500u16 * 128 + 5).to_le_bytes());
        b[0x8a..0x8c].copy_from_slice(&42i16.to_le_bytes());
        let h = GameHeader::from_bytes(1, &b);
        assert_eq!((h.white(), h.black()), (345, 678));
        assert_eq!(h.result(), GameResult::WhiteWins);
        assert_eq!(h.elo(), (2450, 2200));
        assert_eq!(h.eco(), Eco::Code { code: 499, sub: 5 });
        assert_eq!(h.move_count(), 42);
        assert_eq!(h.kind(), RecordKind::Game);
    }

    #[test]
    fn distinguishing_game_text_analysis_deleted() {
        let mut b = [0u8; RECORD_SIZE];
        b[2] = 1;
        assert_eq!(GameHeader::from_bytes(1, &b).kind(), RecordKind::Game);
        b[0] = 2;
        assert_eq!(GameHeader::from_bytes(1, &b).kind(), RecordKind::Text);
        b[0] = 0;
        b[2] = 2;
        assert_eq!(GameHeader::from_bytes(1, &b).kind(), RecordKind::Analysis);
        b[0] = 0x80;
        assert!(GameHeader::from_bytes(1, &b).is_deleted());
    }
}
