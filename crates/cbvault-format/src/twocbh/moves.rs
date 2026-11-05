// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! A game's `.2cbg` move record: variant tag, start position and move tree.

use std::path::Path;

use super::record::Record;
use crate::error::{Error, Result};

/// Marker for null move.
pub const NULL_MOVE: u16 = 0xfffa;
/// Marker for setup start position.
pub const START_POSITION: u16 = 0xfffb;
/// Marker for moves stream beginning.
pub const MOVES: u16 = 0xfffc;
/// Marker for alternative variation start.
pub const ALTERNATIVE: u16 = 0xfffd;
/// Marker for end of variation line.
pub const END_OF_LINE: u16 = 0xffff;

/// One token of the move tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// A move word, including the null move.
    Move(u16),
    /// The move just given has a further alternative, stored later.
    Alternative,
    /// End of the current line.
    EndOfLine,
}

/// One game's `.2cbg` record, split into its start section and move tree.
#[derive(Clone, Copy, Debug)]
pub struct GameMoves<'a> {
    record: Record<'a>,
    tag: u16,
    start: &'a [u8],
    tree: &'a [u8],
}

impl<'a> GameMoves<'a> {
    /// Splits a whole `.2cbg` record of `path`, its magic and trailer included.
    #[inline]
    pub fn parse(path: &Path, raw: &'a [u8]) -> Result<Self> {
        let record = Record::parse(path, 0, raw)?;
        Self::from_record(path, record)
    }

    /// Splits a parsed record into its start section and move tree.
    pub fn from_record(path: &Path, record: Record<'a>) -> Result<Self> {
        let tag = record.tag();
        let content = record.content();
        if !content.len().is_multiple_of(2) {
            return Err(Error::corrupt(path, 0, "odd move stream length in 2CBH record"));
        }
        let n = content.len() / 2;
        if n == 0 {
            return Ok(GameMoves { record, tag, start: &[], tree: &[] });
        }
        let word = |i: usize| u16::from_le_bytes([content[2 * i], content[2 * i + 1]]);
        let mut i = 0;
        if word(0) == START_POSITION {
            i = 1;
            while i < n && word(i) != MOVES {
                i += 1;
            }
        }
        if i >= n || word(i) != MOVES {
            return Err(Error::corrupt(path, 0, "no move section marker in 2CBH record"));
        }
        Ok(GameMoves { record, tag, start: &content[..2 * i], tree: &content[2 * (i + 1)..] })
    }

    /// The record's framing.
    #[inline]
    pub fn record(&self) -> &Record<'a> {
        &self.record
    }

    /// The variant tag (1 = standard chess, 2 = Chess960).
    #[inline]
    pub fn tag(&self) -> u16 {
        self.tag
    }

    /// Whether this is a Chess960 game.
    #[inline]
    pub fn is_chess960(&self) -> bool {
        self.tag & 0xff == 2
    }

    /// The record's codec parameter `B`.
    #[inline]
    pub fn parameter_b(&self) -> u32 {
        self.record.parameter_b()
    }

    /// The full content bytes of the frame.
    #[inline]
    pub fn content(&self) -> &'a [u8] {
        self.record.content()
    }

    /// Raw start position bytes (empty for standard start).
    #[inline]
    pub fn start_slice(&self) -> &'a [u8] {
        self.start
    }

    /// Raw move tree bytes.
    #[inline]
    pub fn tree_slice(&self) -> &'a [u8] {
        self.tree
    }

    /// Whether the move codec is decoded in cbvault.
    #[inline]
    pub fn is_decoded(&self) -> bool {
        true
    }

    /// Tokens of the move tree in stored order.
    pub fn tokens(&self) -> impl Iterator<Item = Token> + 'a {
        self.tree.as_chunks::<2>().0.iter().map(|c| match u16::from_le_bytes([c[0], c[1]]) {
            ALTERNATIVE => Token::Alternative,
            END_OF_LINE => Token::EndOfLine,
            w => Token::Move(w),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twocbh::testdata::record;

    #[test]
    fn parse_move_record() {
        let content = [0xfc, 0xff, 0x01, 0x00, 0xff, 0xff];
        let raw = record(10, &content);
        let g = GameMoves::parse(Path::new("db.2cbg"), &raw).unwrap();
        assert_eq!(g.parameter_b(), 10);
        assert_eq!(g.content().len(), 6);
        assert!(g.is_decoded());
        assert!(!g.is_chess960());
        assert!(g.start_slice().is_empty());
        let toks: Vec<_> = g.tokens().collect();
        assert_eq!(toks, vec![Token::Move(1), Token::EndOfLine]);
    }
}
