// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! The 192-byte `.2cbh` records: the headers of 2CBH games.
//!
//! Integers are little-endian. Each record stores the offsets of the game's
//! `.2cbg` move frame and `.2cba` annotation frame, entity IDs into `.2lid`
//! for white and black players, tournament, annotator, source, teams and tags,
//! result, round, board, ratings, ECO, flags, move count, and played date.

use crate::cbh::bytes::NameBuf;
use crate::game::{Date, Eco, GameResult, Head, RecordKind};

use super::bytes::{NAME_FIELD, RECORD_SIZE};

/// The decoded header of one `.2cbh` record (a game, or the file header).
#[derive(Clone, Copy, Debug)]
pub struct GameHeader {
    id: u32,
    b: [u8; RECORD_SIZE],
}

fn le_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

fn le_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn le_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn le_i64(b: &[u8], o: usize) -> i64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    i64::from_le_bytes(v)
}

fn le_u64(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}

impl GameHeader {
    /// The header of the record `id` whose 192 bytes are `b`.
    pub fn from_bytes(id: u32, b: &[u8; RECORD_SIZE]) -> GameHeader {
        GameHeader { id, b: *b }
    }

    /// The record's id (1-based; id 0 is the file's own header record).
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The record as stored.
    pub fn bytes(&self) -> &[u8] {
        &self.b
    }

    /// The generation byte the file header states: `0x22` or `0x26` in the
    /// sets observed.
    pub fn generation_byte(&self) -> u8 {
        self.b[0x08]
    }

    /// The record size the file header states; [`RECORD_SIZE`] in every set observed.
    pub fn stated_record_size(&self) -> u16 {
        le_u16(&self.b, 0x0a)
    }

    /// The record count the file header states, **including record 0**.
    pub fn stated_records(&self) -> u32 {
        le_u32(&self.b, 0x10)
    }

    /// The record's kind byte.
    pub fn kind_byte(&self) -> u8 {
        self.b[0]
    }

    /// Whether bit `0x80` of the kind byte is set or kind byte is 0 (deleted record).
    pub fn is_deleted(&self) -> bool {
        self.b[2] == 0 || self.b[0] & 0x80 != 0
    }

    /// Classification: Game, Text, Analysis, or Unknown.
    pub fn kind(&self) -> RecordKind {
        match self.b[2] {
            1 if self.b[0] & 2 != 0 => RecordKind::Text,
            1 => RecordKind::Game,
            2 => RecordKind::Analysis,
            3 => RecordKind::Text,
            k => RecordKind::Unknown(k),
        }
    }

    /// The byte offset of the game's record in `.2cbg`.
    pub fn moves_offset(&self) -> u64 {
        le_u64(&self.b, 0x08)
    }

    /// The byte offset of the game's record in `.2cba`; 0 when none.
    pub fn annotations_offset(&self) -> u64 {
        le_u64(&self.b, 0x10)
    }

    /// White player entity ID in `.2lid`.
    pub fn white(&self) -> i64 {
        le_i64(&self.b, 0x18)
    }

    /// Black player entity ID in `.2lid`.
    pub fn black(&self) -> i64 {
        le_i64(&self.b, 0x20)
    }

    /// Tournament entity ID in `.2lid`.
    pub fn tournament(&self) -> i64 {
        le_i64(&self.b, 0x28)
    }

    /// Annotator entity ID in `.2lid`.
    pub fn annotator(&self) -> i64 {
        le_i64(&self.b, 0x30)
    }

    /// Source entity ID in `.2lid`.
    pub fn source(&self) -> i64 {
        le_i64(&self.b, 0x38)
    }

    /// White team entity ID in `.2lid`.
    pub fn white_team(&self) -> i64 {
        le_i64(&self.b, 0x40)
    }

    /// Black team entity ID in `.2lid`.
    pub fn black_team(&self) -> i64 {
        le_i64(&self.b, 0x48)
    }

    /// Game tag entity ID in `.2lid`.
    pub fn game_tag(&self) -> i64 {
        le_i64(&self.b, 0x50)
    }

    /// Guiding text author player entity ID in `.2lid`.
    pub fn text_author(&self) -> i64 {
        le_i64(&self.b, 0x20)
    }

    /// Guiding text title game tag entity ID in `.2lid`.
    pub fn text_title(&self) -> i64 {
        le_i64(&self.b, 0x18)
    }

    /// Analysis title game tag entity ID in `.2lid`.
    pub fn analysis_title(&self) -> i64 {
        le_i64(&self.b, 0x18)
    }

    /// Analysis author player entity ID in `.2lid`.
    pub fn analysis_author(&self) -> i64 {
        le_i64(&self.b, 0x28)
    }

    /// Result of the game (0=0-1, 1=1/2, 2=1-0, 3=*). Analysis records are always *.
    pub fn result(&self) -> GameResult {
        if self.kind() == RecordKind::Analysis {
            return GameResult::Line;
        }
        match self.b[0x58] {
            0 => GameResult::BlackWins,
            1 => GameResult::Draw,
            2 => GameResult::WhiteWins,
            3 => GameResult::Line,
            4 => GameResult::BlackWinsForfeit,
            5 => GameResult::DrawForfeit,
            6 => GameResult::WhiteWinsForfeit,
            7 => GameResult::BothLost,
            r => GameResult::Unknown(r),
        }
    }

    /// Round number.
    pub fn round(&self) -> i16 {
        le_i16(&self.b, 0x5a)
    }

    /// Subround number.
    pub fn subround(&self) -> i16 {
        le_i16(&self.b, 0x5c)
    }

    /// Board number.
    pub fn board(&self) -> i16 {
        le_i16(&self.b, 0x5e)
    }

    /// White Elo rating.
    pub fn white_elo(&self) -> i16 {
        le_i16(&self.b, 0x60)
    }

    /// Black Elo rating.
    pub fn black_elo(&self) -> i16 {
        le_i16(&self.b, 0x70)
    }

    /// ECO code.
    pub fn eco(&self) -> Eco {
        Eco::from_field(le_u16(&self.b, 0x80))
    }

    /// Game flags.
    pub fn flags(&self) -> u32 {
        le_u32(&self.b, 0x84)
    }

    /// Move count of the main line. For Analysis (kind 2), stored at 0x50; for Games, at 0x8a.
    pub fn move_count(&self) -> i16 {
        if self.b[2] == 2 { le_i16(&self.b, 0x50) } else { le_i16(&self.b, 0x8a) }
    }

    /// Date played. For Analysis (kind 2), stored at 0x4c; for Games, at 0xbc.
    pub fn played_date(&self) -> Date {
        if self.b[2] == 2 { Date(le_i32(&self.b, 0x4c)) } else { Date(le_i32(&self.b, 0xbc)) }
    }

    /// The 16 source bytes of the first inline text field.
    pub fn name1(&self) -> &[u8] {
        &self.b[0x68..0x68 + NAME_FIELD]
    }

    /// The 16 bytes of the second inline text field.
    pub fn name2(&self) -> &[u8] {
        &self.b[0x78..0x78 + NAME_FIELD]
    }

    /// [`GameHeader::name1`] decoded into the caller's buffer.
    pub fn name1_into<'b>(&self, buf: &'b mut NameBuf) -> &'b str {
        buf.set(self.name1());
        buf.as_str()
    }

    /// [`GameHeader::name2`] decoded into the caller's buffer.
    pub fn name2_into<'b>(&self, buf: &'b mut NameBuf) -> &'b str {
        buf.set(self.name2());
        buf.as_str()
    }

    /// Value at `0x98`.
    pub fn key(&self) -> u64 {
        le_u64(&self.b, 0x98)
    }
}

impl Head for GameHeader {
    fn id(&self) -> u32 {
        GameHeader::id(self)
    }
    fn kind(&self) -> RecordKind {
        GameHeader::kind(self)
    }
    fn is_deleted(&self) -> bool {
        GameHeader::is_deleted(self)
    }
    fn white(&self) -> i64 {
        GameHeader::white(self)
    }
    fn black(&self) -> i64 {
        GameHeader::black(self)
    }
    fn tournament(&self) -> i64 {
        GameHeader::tournament(self)
    }
    fn annotator(&self) -> i64 {
        GameHeader::annotator(self)
    }
    fn other(&self) -> Option<(i64, i64)> {
        match GameHeader::kind(self) {
            RecordKind::Game => None,
            RecordKind::Text => Some((self.text_title(), self.text_author())),
            RecordKind::Analysis => Some((self.analysis_title(), self.analysis_author())),
            RecordKind::Unknown(_) => Some((-1, -1)),
        }
    }
    fn result(&self) -> GameResult {
        GameHeader::result(self)
    }
    fn eco(&self) -> Eco {
        GameHeader::eco(self)
    }
    fn played_date(&self) -> Date {
        GameHeader::played_date(self)
    }
    fn round(&self) -> (i32, i32) {
        (i32::from(GameHeader::round(self)), i32::from(GameHeader::subround(self)))
    }
    fn elo(&self) -> (i32, i32) {
        (i32::from(GameHeader::white_elo(self)), i32::from(GameHeader::black_elo(self)))
    }
    fn move_count(&self) -> i32 {
        i32::from(GameHeader::move_count(self))
    }
    fn bytes(&self) -> &[u8] {
        GameHeader::bytes(self)
    }
}
