// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! The 2CBH move-word table.
//!
//! A move word below `0xfffa` names one move from an enumeration of every move
//! each piece can make on an empty board, so a word means the same thing in any
//! position and a game decodes without a board. The enumeration is rebuilt here
//! once, on first use, from its generating rules.

use std::collections::HashMap;
use std::sync::OnceLock;

mod pieces;

pub use pieces::{decode_piece_word, encode_piece_word};

/// Piece color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    /// White
    White,
    /// Black
    Black,
}

/// Piece role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Piece {
    /// King
    King,
    /// Queen
    Queen,
    /// Knight
    Knight,
    /// Bishop
    Bishop,
    /// Rook
    Rook,
    /// Pawn
    Pawn,
}

/// What a move word says is standing on the destination square.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Captured {
    /// No capture
    Nothing,
    /// Captured Queen
    Queen,
    /// Captured Knight
    Knight,
    /// Captured Bishop
    Bishop,
    /// Captured Rook
    Rook,
    /// Captured Pawn
    Pawn,
    /// En passant capture
    EnPassant,
}

/// Side of castling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CastleSide {
    /// Queenside / O-O-O
    Long,
    /// Kingside / O-O
    Short,
}

/// A square numbered the usual rank-major way: `a1` = 0, `b1` = 1, … `h8` = 63.
///
/// ChessBase numbers squares file by file (`a1` = 0, `a2` = 1); this type is
/// always in rank-major order and conversion happens at the file boundary.
pub type Sq = u8;

/// Decoded representation of a 16-bit move word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoveWord {
    /// Null move (`0xfffa`)
    Null,
    /// Standard piece move
    Normal {
        /// Color of moving piece
        color: Color,
        /// Role of moving piece
        piece: Piece,
        /// Origin square (rank-major)
        from: Sq,
        /// Destination square (rank-major)
        to: Sq,
        /// Stored capture piece
        captured: Captured,
        /// Stored promotion role
        promotion: Option<Piece>,
    },
    /// Standard castling
    Castle {
        /// Color of player castling
        color: Color,
        /// Castling side
        side: CastleSide,
    },
    /// Castling in a Chess960 game from start position `position` (0-959).
    Castle960 {
        /// Chess960 position index (0..959)
        position: u16,
        /// Color of player castling
        color: Color,
        /// Castling side
        side: CastleSide,
    },
}

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

const FIRST_CASTLE: u16 = 0xb129;
const FIRST_CASTLE_960: u16 = 0xb12d;
/// First word above the move words: set-up pieces start here.
pub const FIRST_PIECE_WORD: u16 = 0xc02d;

/// Last set-up piece word.
pub const LAST_PIECE_WORD: u16 = 0xc30c;

/// ChessBase square (file-major) to rank-major.
pub fn from_cb_square(cb: u8) -> Sq {
    let (file, rank) = (cb / 8, cb % 8);
    rank * 8 + file
}

fn sq(file: i8, rank: i8) -> Sq {
    (rank * 8 + file) as Sq
}

const KING_DIRS: &[(i8, i8)] = &[(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];
const KNIGHT_DIRS: &[(i8, i8)] = &[(-2, -1), (-2, 1), (2, -1), (2, 1), (-1, -2), (-1, 2), (1, -2), (1, 2)];
const BISHOP_DIRS: &[(i8, i8)] = &[(-1, -1), (1, -1), (1, 1), (-1, 1)];
const ROOK_DIRS: &[(i8, i8)] = &[(-1, 0), (0, -1), (1, 0), (0, 1)];
const QUEEN_DIRS: &[(i8, i8)] = &[(-1, -1), (1, -1), (1, 1), (-1, 1), (-1, 0), (0, -1), (1, 0), (0, 1)];

const CAPTURE_ORDER: [Captured; 6] =
    [Captured::Nothing, Captured::Queen, Captured::Knight, Captured::Bishop, Captured::Rook, Captured::Pawn];
const PROMOTION_ORDER: [Piece; 4] = [Piece::Queen, Piece::Knight, Piece::Bishop, Piece::Rook];
const PROMOTION_CAPTURES: [Captured; 4] = [Captured::Queen, Captured::Knight, Captured::Bishop, Captured::Rook];

fn build() -> Vec<MoveWord> {
    let mut t = Vec::with_capacity(FIRST_CASTLE_960 as usize);
    t.push(MoveWord::Null); // word 0 is unused; never looked up
    for color in [Color::White, Color::Black] {
        for (piece, dirs, slides) in [
            (Piece::King, KING_DIRS, false),
            (Piece::Queen, QUEEN_DIRS, true),
            (Piece::Knight, KNIGHT_DIRS, false),
            (Piece::Bishop, BISHOP_DIRS, true),
            (Piece::Rook, ROOK_DIRS, true),
        ] {
            for cb in 0..64i8 {
                let (f, r) = (cb / 8, cb % 8);
                for &(df, dr) in dirs {
                    let mut step = 1;
                    loop {
                        let (nf, nr) = (f + df * step, r + dr * step);
                        if !(0..8).contains(&nf) || !(0..8).contains(&nr) {
                            break;
                        }
                        for captured in CAPTURE_ORDER {
                            t.push(MoveWord::Normal {
                                color,
                                piece,
                                from: sq(f, r),
                                to: sq(nf, nr),
                                captured,
                                promotion: None,
                            });
                        }
                        if !slides {
                            break;
                        }
                        step += 1;
                    }
                }
            }
        }
    }
    for color in [Color::White, Color::Black] {
        // (direction, start rank, rank before promotion, en passant rank), 1-based ranks
        let (d, start, pre, ep) = match color {
            Color::White => (1i8, 2i8, 7i8, 5i8),
            Color::Black => (-1, 7, 2, 4),
        };
        for f in 0..8i8 {
            for rank in 2..=7i8 {
                let r = rank - 1;
                let pawn = |to: Sq, captured, promotion| MoveWord::Normal {
                    color,
                    piece: Piece::Pawn,
                    from: sq(f, r),
                    to,
                    captured,
                    promotion,
                };
                if rank == start {
                    t.push(pawn(sq(f, r + 2 * d), Captured::Nothing, None));
                    t.push(pawn(sq(f, r + d), Captured::Nothing, None));
                } else if rank == pre {
                    for p in PROMOTION_ORDER {
                        t.push(pawn(sq(f, r + d), Captured::Nothing, Some(p)));
                    }
                } else {
                    t.push(pawn(sq(f, r + d), Captured::Nothing, None));
                }
                for df in [-1i8, 1] {
                    let nf = f + df;
                    if !(0..8).contains(&nf) {
                        continue;
                    }
                    let to = sq(nf, r + d);
                    if rank == pre {
                        for captured in PROMOTION_CAPTURES {
                            for p in PROMOTION_ORDER {
                                t.push(pawn(to, captured, Some(p)));
                            }
                        }
                    } else {
                        for captured in &CAPTURE_ORDER[1..] {
                            t.push(pawn(to, *captured, None));
                        }
                        if rank == ep {
                            t.push(pawn(to, Captured::EnPassant, None));
                        }
                    }
                }
            }
        }
    }
    debug_assert_eq!(t.len(), FIRST_CASTLE as usize);
    for (color, side) in [
        (Color::White, CastleSide::Long),
        (Color::White, CastleSide::Short),
        (Color::Black, CastleSide::Long),
        (Color::Black, CastleSide::Short),
    ] {
        t.push(MoveWord::Castle { color, side });
    }
    t
}

fn table() -> &'static [MoveWord] {
    static TABLE: OnceLock<Vec<MoveWord>> = OnceLock::new();
    TABLE.get_or_init(build)
}

/// Decodes a move word. Markers other than the null move, and piece words, return `None`.
pub fn decode(word: u16) -> Option<MoveWord> {
    match word {
        NULL_MOVE => Some(MoveWord::Null),
        0 => None,
        w if w < FIRST_CASTLE_960 => Some(table()[w as usize]),
        w if w < FIRST_PIECE_WORD => {
            let n = w - FIRST_CASTLE_960;
            let (position, k) = (n / 4, n % 4);
            Some(MoveWord::Castle960 {
                position,
                color: if k < 2 { Color::White } else { Color::Black },
                side: if k % 2 == 0 { CastleSide::Long } else { CastleSide::Short },
            })
        }
        _ => None,
    }
}

fn words() -> &'static HashMap<MoveWord, u16> {
    static WORDS: OnceLock<HashMap<MoveWord, u16>> = OnceLock::new();
    WORDS.get_or_init(|| table().iter().enumerate().skip(1).map(|(w, m)| (*m, w as u16)).collect())
}

fn castle_index(color: Color, side: CastleSide) -> u16 {
    2 * (color == Color::Black) as u16 + (side == CastleSide::Short) as u16
}

/// Encodes a move as its word, the inverse of [`decode`]. A move no word names
/// (a knight moving like a bishop, castling from a Chess960 position above 959) returns `None`.
pub fn encode(mv: MoveWord) -> Option<u16> {
    match mv {
        MoveWord::Null => Some(NULL_MOVE),
        MoveWord::Normal { .. } => words().get(&mv).copied(),
        MoveWord::Castle { color, side } => Some(FIRST_CASTLE + castle_index(color, side)),
        MoveWord::Castle960 { position, color, side } => {
            (position < 960).then(|| FIRST_CASTLE_960 + 4 * position + castle_index(color, side))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(pred: impl Fn(&MoveWord) -> bool) -> usize {
        table().iter().skip(1).position(pred).unwrap() + 1
    }

    #[test]
    fn block_boundaries_match_the_format() {
        let is = |c: Color, p: Piece| move |m: &MoveWord| matches!(m, MoveWord::Normal { color, piece, .. } if *color == c && *piece == p);
        assert_eq!(table().len(), FIRST_CASTLE_960 as usize);
        assert_eq!(first(is(Color::White, Piece::Queen)), 0x09d9);
        assert_eq!(first(is(Color::White, Piece::Knight)), 0x2bf9);
        assert_eq!(first(is(Color::White, Piece::Bishop)), 0x33d9);
        assert_eq!(first(is(Color::White, Piece::Rook)), 0x40f9);
        assert_eq!(first(is(Color::Black, Piece::King)), 0x55f9);
        assert_eq!(first(is(Color::Black, Piece::Queen)), 0x5fd1);
        assert_eq!(first(is(Color::Black, Piece::Knight)), 0x81f1);
        assert_eq!(first(is(Color::Black, Piece::Bishop)), 0x89d1);
        assert_eq!(first(is(Color::Black, Piece::Rook)), 0x96f1);
        assert_eq!(first(is(Color::White, Piece::Pawn)), 0xabf1);
        assert_eq!(first(is(Color::Black, Piece::Pawn)), 0xae8d);
    }

    #[test]
    fn first_king_words() {
        // a1-a2 is word 1, a1-b1 word 7 (six words per destination).
        assert_eq!(
            decode(1),
            Some(MoveWord::Normal {
                color: Color::White,
                piece: Piece::King,
                from: 0,
                to: 8,
                captured: Captured::Nothing,
                promotion: None
            })
        );
        assert!(matches!(decode(7), Some(MoveWord::Normal { from: 0, to: 1, .. })));
    }

    #[test]
    fn castling_and_960() {
        assert_eq!(decode(0xb12a), Some(MoveWord::Castle { color: Color::White, side: CastleSide::Short }));
        assert_eq!(
            decode(0xb12d + 4 * 7 + 3),
            Some(MoveWord::Castle960 { position: 7, color: Color::Black, side: CastleSide::Short })
        );
        assert_eq!(decode(FIRST_PIECE_WORD), None);
    }

    #[test]
    fn encode_inverts_decode() {
        for w in 1..FIRST_PIECE_WORD {
            assert_eq!(encode(decode(w).unwrap()), Some(w), "{w:#06x}");
        }
        assert_eq!(encode(MoveWord::Null), Some(NULL_MOVE));
        assert_eq!(decode(NULL_MOVE), Some(MoveWord::Null));
        let knight_as_bishop = MoveWord::Normal {
            color: Color::White,
            piece: Piece::Knight,
            from: 0,
            to: 9,
            captured: Captured::Nothing,
            promotion: None,
        };
        assert_eq!(encode(knight_as_bishop), None);
        let beyond = MoveWord::Castle960 { position: 960, color: Color::White, side: CastleSide::Long };
        assert_eq!(encode(beyond), None);
    }
}
