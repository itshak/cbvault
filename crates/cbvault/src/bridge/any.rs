// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Non-breaking common entry point for both generations of ChessBase sets.
//!
//! The established `bridge::Database` remains the classic-only high-performance
//! handle (avoiding a breaking API change). New consumers may instead use
//! `AnyDatabase`, which routes record counts and per-game PGN by generation.
//! The v2-native methods (verification, bounded parallel export, frame scratch)
//! remain available through its `TwoCbh` variant.

use std::path::Path;

use super::{Database, GameBuf, Generation, generation_of};
use crate::twocbh;
use cbvault_format::error::{Error, Result};

/// A read-only database of either ChessBase generation. The classic and v2
/// variants expose their full native APIs; one Box is allocated on open, never per game.
#[derive(Debug)]
pub enum AnyDatabase {
    /// Established classic ChessBase family.
    Classic(Box<Database>),
    /// Second-generation ChessBase .2cbh family.
    TwoCbh(Box<twocbh::Database>),
}

impl AnyDatabase {
    /// Open the database family selected by the caller's extension or stem.
    /// Explicit `.2cbh` always selects v2 even when the stem also has `.cbh`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        match generation_of(path) {
            Generation::Classic => Database::open(path).map(|db| Self::Classic(Box::new(db))),
            Generation::TwoCbh => twocbh::Database::open(path).map(|db| Self::TwoCbh(Box::new(db))),
            Generation::Unknown => Err(Error::corrupt(path, 0, "no ChessBase .cbh or .2cbh header found")),
        }
    }

    /// The opened database's generation.
    pub fn generation(&self) -> Generation {
        match self {
            Self::Classic(_) => Generation::Classic,
            Self::TwoCbh(_) => Generation::TwoCbh,
        }
    }

    /// All index slots, including deleted/guiding-text/analysis records.
    pub fn records(&self) -> u32 {
        match self {
            Self::Classic(db) => db.records(),
            Self::TwoCbh(db) => db.records(),
        }
    }

    /// One game's full PGN, with annotations. The native v2 PGN writer
    /// resolves metadata from .2lid and all moves through gigachess.
    /// A damaged record or unrecognized annotation fails explicitly.
    pub fn game_pgn(&self, id: u32) -> Result<String> {
        match self {
            Self::Classic(db) => db.game_pgn_with(id, &mut GameBuf::new(), true),
            Self::TwoCbh(db) => {
                let mut result = Vec::new();
                db.write_game(id, &mut crate::pgn::PgnWriter::new(), &mut result)
                    .map_err(|e| Error::corrupt(db.inner().stem(), id as u64, e.to_string()))?;
                String::from_utf8(result).map_err(|e| Error::corrupt(db.inner().stem(), id as u64, e.to_string()))
            }
        }
    }

    /// Return the generation-specific classic handle when available.
    pub fn as_classic(&self) -> Option<&Database> {
        if let Self::Classic(db) = self { Some(db) } else { None }
    }

    /// Return the generation-specific native 2CBH handle when available.
    pub fn as_twocbh(&self) -> Option<&twocbh::Database> {
        if let Self::TwoCbh(db) = self { Some(db) } else { None }
    }
}

/// Open either generation without affecting existing classic-only code.
pub fn open_any(path: impl AsRef<Path>) -> Result<AnyDatabase> {
    AnyDatabase::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbvault_format::twocbh::testdata::{Game, remove_set, write_set};
    #[test]
    fn v2_set_opens_through_common_handle_and_exports_pgn() {
        let stem = write_set("bridge-any", &[Game::new(5, 1, 2)], true);
        let by_extension = stem.with_extension("2cbh");
        let db = AnyDatabase::open(&by_extension).unwrap();
        assert_eq!(db.generation(), Generation::TwoCbh);
        assert_eq!(db.records(), 1);
        let pgn = db.game_pgn(1).unwrap();
        assert!(pgn.contains("[White \"Test1, Example\"]"), "{pgn}");
        assert!(db.as_classic().is_none());
        assert!(db.as_twocbh().is_some());
        remove_set(&stem);
    }
}
