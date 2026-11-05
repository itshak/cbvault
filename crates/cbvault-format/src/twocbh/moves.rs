// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Allocation-free structural reader for a 2CBH v2 move-word stream.
//! Word tokens are NOT gigachess moves2; they must be resolved against a
//! legal board in cbvault-chess before any PGN or search result is emitted.
//! Structure follows the pinned MIT v1.0.3 v2/moves.rs baseline.

use super::record::Record;
use crate::error::{Error, Result};

const START_POSITION: u16 = 0xfffb;
const MOVES: u16 = 0xfffc;
const ALTERNATIVE: u16 = 0xfffd;
const END_OF_LINE: u16 = 0xffff;

/// Structural move token, deliberately not a resolved chess move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// The encoded move number, including 0xfffa (null).
    MoveWord(u16),
    /// Variation marker: subsequent words start an alternative line.
    Alternative,
    /// Marker terminating the current variation or main line.
    EndOfLine,
}

#[derive(Clone, Copy, Debug)]
/// Borrowed move frame and its parsed structural word boundaries.
pub struct GameMoves<'a> {
    record: Record<'a>,
    /// The index of the first move word (past the FFFC marker), in bytes.
    tree_start: usize,
}

impl<'a> GameMoves<'a> {
    #[inline]
    /// Parse and validate move-frame boundaries without translating moves to chess-core moves.
    pub fn parse(path: &std::path::Path, raw: &'a [u8]) -> Result<Self> {
        let record = Record::parse(path, 0, raw)?;
        let content = record.content();
        if content.is_empty() || !content.len().is_multiple_of(2) {
            return Err(Error::corrupt(path, 0, "2CBH moves: empty or odd-length word stream"));
        }
        let first = u16::from_le_bytes([content[0], content[1]]);
        let tree_start = if first == START_POSITION {
            // SETUP follows START_POSITION and finishes at MOVES. Reject an
            // incomplete section; we cannot guess where the game begins.
            let Some(i) = content.as_chunks::<2>().0.iter().position(|b| u16::from_le_bytes([b[0], b[1]]) == MOVES) else {
                return Err(Error::corrupt(path, 0, "2CBH moves: missing MOVES marker after setup"));
            };
            (i + 1) * 2
        } else if first == MOVES {
            2
        } else {
            return Err(Error::corrupt(path, 0, "2CBH moves: missing MOVES marker"));
        };
        Ok(Self { record, tree_start })
    }

    #[inline]
    /// Borrow the validated underlying frame.
    pub fn record(&self) -> &Record<'a> {
        &self.record
    }
    #[inline]
    /// Return B, the spare-capacity byte count.
    pub fn parameter_b(&self) -> u32 {
        self.record.parameter_b()
    }
    #[inline]
    /// Return the original encoded word bytes.
    pub fn content(&self) -> &'a [u8] {
        self.record.content()
    }
    /// Variant 2 is Chess960 in the low tag byte.
    #[inline]
    pub fn is_chess960(&self) -> bool {
        self.record.tag() & 0xff == 2
    }

    /// Raw setup words after FFFB and before FFFC, if any.
    pub fn start_words(&self) -> impl Iterator<Item = u16> + '_ {
        let begin = if self.tree_start > 2 { 2 } else { 0 };
        self.content()[begin..self.tree_start.saturating_sub(2)]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    /// Structural move tree tokens. Iteration uses no heap memory. The words
    /// are NOT moves2 and have not undergone chess-legality validation.
    pub fn tokens(&self) -> impl Iterator<Item = Token> + '_ {
        self.content()[self.tree_start..] .as_chunks::<2>().0.iter().map(|b| match u16::from_le_bytes([b[0], b[1]]) {
            ALTERNATIVE => Token::Alternative,
            END_OF_LINE => Token::EndOfLine,
            w => Token::MoveWord(w),
        })
    }

    /// The actual move-to-moves2 conversion is not yet implemented.
    #[inline]
    pub fn is_decoded(&self) -> bool {
        false
    }
    /// Explicitly unavailable: never cast raw move words into moves2.
    #[inline]
    pub fn stream(&self) -> Option<&'a [u16]> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twocbh::testdata::record;
    use std::path::Path;

    #[test]
    fn empty_mainline_has_an_end_marker_and_no_decoded_moves() {
        let raw = record(102, &[0xfc, 0xff, 0xff, 0xff]);
        let g = GameMoves::parse(Path::new("db.2cbg"), &raw).unwrap();
        assert_eq!(g.parameter_b(), 102);
        assert_eq!(g.record().parameter_a(), 4);
        assert_eq!(g.tokens().collect::<Vec<_>>(), vec![Token::EndOfLine]);
        assert!(!g.is_decoded());
        assert!(g.stream().is_none());
    }

    #[test]
    fn setup_and_tree_markers_are_structural() {
        let words = [START_POSITION, 9, 10, MOVES, 0xfffa, ALTERNATIVE, 42, END_OF_LINE];
        let bytes: Vec<u8> = words.into_iter().flat_map(u16::to_le_bytes).collect();
        let raw = record(4, &bytes);
        let g = GameMoves::parse(Path::new("db.2cbg"), &raw).unwrap();
        assert_eq!(g.start_words().collect::<Vec<_>>(), vec![9, 10]);
        assert_eq!(
            g.tokens().collect::<Vec<_>>(),
            vec![Token::MoveWord(0xfffa), Token::Alternative, Token::MoveWord(42), Token::EndOfLine,]
        );
    }

    #[test]
    fn bad_word_section_is_an_error() {
        for body in [&[][..], &[0][..], &[1, 0][..], &[0xfb, 0xff, 1, 0][..]] {
            let raw = record(4, body);
            assert!(GameMoves::parse(Path::new("db.2cbg"), &raw).is_err());
        }
    }
}
