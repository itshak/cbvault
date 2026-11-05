//! A game's players and its tournament, as both formats name them.
//!
//! Ported from `cbformat` in `oschess-cb-bridge` @ `ca9e8f8e` (MIT); see
//! `docs/provenance.md`.

use super::Date;

/// A player, as the `.cbp` namebase stores one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Player {
    /// The family name.
    pub last: String,
    /// The given name; empty when the namebase has none.
    pub first: String,
}

impl Player {
    /// The name as PGN writes it: `Last, First`, or just `Last`.
    pub fn pgn(&self) -> String {
        if self.first.is_empty() { self.last.clone() } else { format!("{}, {}", self.last, self.first) }
    }
}

/// A tournament, as the `.cbt` namebase stores one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tournament {
    /// The tournament's title.
    pub title: String,
    /// Where it was played.
    pub place: String,
    /// The date it started; 0 parts are unknown.
    pub start: Date,
    /// Style and time control byte (bit 0x20 blitz, bit 0x40 rapid, bit 0x80 correspondence).
    pub event_type: u8,
}

impl Tournament {
    /// Whether bit `0x20` is set (Blitz time control).
    pub fn is_blitz(&self) -> bool {
        self.event_type & 0x20 != 0
    }

    /// Whether bit `0x40` is set (Rapid time control).
    pub fn is_rapid(&self) -> bool {
        self.event_type & 0x40 != 0
    }

    /// Whether bit `0x80` is set (Correspondence event).
    pub fn is_correspondence(&self) -> bool {
        self.event_type & 0x80 != 0
    }

    /// Tournament format or category code (bits 0x00..0x1f).
    pub fn category(&self) -> u8 {
        self.event_type & 0x1f
    }
}
