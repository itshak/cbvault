// MIT-licensed replay rules adapted from oschess-cb-bridge v1.0.3,
// commit bffebfc498d52b4e4631aed32b2f8e8194d125ff.
// Copyright (c) 2026 the oschess-cb-bridge contributors.
// Modifications copyright (c) 2026 the cbvault contributors.
// SPDX-License-Identifier: MIT
//! 2CBH word-to-gigachess replay, with full capture/legality validation.
//!
//! This is not a second chess core. The static word table is a file-format
//! enumeration; gigachess determines legality, FEN, SAN and castling.
//! Scratch stacks are retained between games, so a warmed walk allocates
//! zero per ply even in variation-heavy workloads.

use cbvault_format::error::{Error, Result};
use cbvault_format::twocbh::moves::{GameMoves, Token};
use cbvault_format::twocbh::word::{self, Captured, CastleSide, MoveWord, Piece as WPiece};
use gigachess::{Board, Color, Move, Role, Square};

use crate::decode::{GameRef, MAX_VARIATION_DEPTH, MoveSink, TreeStats};
#[cfg(test)]
use crate::start::standard_board;
use crate::start::{Setup, Start, StartCache, start_board_cached};

/// Scratch buffers retained across 2CBH games.
#[derive(Default, Debug)]
pub struct V2Scratch {
    /// Parent boards for nested variations, reused across games.
    pub stack: Vec<Board>,
    /// Cache of non-standard start positions, reused across games.
    pub starts: StartCache,
}

fn color(c: word::Color) -> Color {
    match c {
        word::Color::White => Color::White,
        word::Color::Black => Color::Black,
    }
}
fn role(p: WPiece) -> Role {
    match p {
        WPiece::King => Role::King,
        WPiece::Queen => Role::Queen,
        WPiece::Knight => Role::Knight,
        WPiece::Bishop => Role::Bishop,
        WPiece::Rook => Role::Rook,
        WPiece::Pawn => Role::Pawn,
    }
}
fn other_role(p: Captured) -> Option<Role> {
    match p {
        Captured::Nothing | Captured::EnPassant => None,
        Captured::Queen => Some(Role::Queen),
        Captured::Knight => Some(Role::Knight),
        Captured::Bishop => Some(Role::Bishop),
        Captured::Rook => Some(Role::Rook),
        Captured::Pawn => Some(Role::Pawn),
    }
}

fn error(what: GameRef, ply: u32, detail: impl Into<String>) -> Error {
    Error::Move { game: what.id, ply, offset: what.offset, detail: detail.into() }
}

/// Start position from the v2 setup section, built without a Vec allocation.
/// The words after FFFB encode either a Chess960 start ID or three control
/// fields followed by piece words. Start::Setup is a fixed-sized array.
pub fn start_of(moves: &GameMoves<'_>) -> Result<Start> {
    let raw = moves.content();
    if u16::from_le_bytes([raw[0], raw[1]]) != word::START_POSITION {
        return Ok(Start::Standard);
    }
    let mut words = moves.start_words();
    let first = words.next().ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: empty setup section"))?;
    if moves.is_chess960() && moves.start_words().count() == 1 {
        if first >= 960 {
            return Err(Error::corrupt("<.2cbg>", 0, "2CBH: invalid Chess960 start index"));
        }
        return Ok(Start::Chess960(first));
    }

    let (move_number, side_and_castling, ep) = if moves.is_chess960() && first == 1000 {
        let num = words.next().ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: missing setup move number"))?;
        let sc = words.next().ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: missing setup side/castling"))?;
        let ep = words.next().ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: missing setup en passant"))?;
        (num, sc, ep)
    } else {
        let sc = words.next().ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: missing setup side/castling"))?;
        let ep = words.next().ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: missing setup en passant"))?;
        (first, sc, ep)
    };
    let mut pieces: [Option<(Color, Role)>; 64] = [None; 64];
    for w in words {
        let (side, piece, sq) =
            word::decode_piece_word(w).ok_or_else(|| Error::corrupt("<.2cbg>", 0, "2CBH: unknown setup piece word"))?;
        let slot = &mut pieces[sq as usize];
        if slot.is_some() {
            return Err(Error::corrupt("<.2cbg>", 0, "2CBH: duplicate setup square"));
        }
        *slot = Some((color(side), role(piece)));
    }
    if ep > 8 {
        return Err(Error::corrupt("<.2cbg>", 0, "2CBH: invalid en-passant file"));
    }
    Ok(Start::Setup(Setup {
        chess960: moves.is_chess960(),
        move_number,
        side_to_move: if side_and_castling & 0xff == 0 { Color::White } else { Color::Black },
        castling: (side_and_castling >> 8) as u8,
        castling_rooks: [None; 4],
        castling_kings: [None; 2],
        en_passant_file: if ep == 0 { None } else { Some((ep - 1) as u8) },
        en_passant_raw: ep,
        pieces,
    }))
}

/// Translate the on-disk move word to a gigachess move *after* validating
/// the named mover, side and captured piece against the current board.
/// gigachess still checks king safety and every move's full legality.
#[inline]
pub fn to_move(board: &Board, word_value: u16, start: &Start) -> std::result::Result<Move, &'static str> {
    let word = word::decode(word_value).ok_or("not a legal 2CBH move word")?;
    match word {
        MoveWord::Null => Ok(Move::NULL),
        MoveWord::Castle { color: c, side } | MoveWord::Castle960 { color: c, side, .. } => {
            let c = color(c);
            if board.turn() != c {
                return Err("castling by wrong side");
            }
            if let MoveWord::Castle960 { position, .. } = word
                && let Start::Chess960(start_position) = start
                && position != *start_position
            {
                return Err("Chess960 castle word has wrong start index");
            }
            let kingside = matches!(side, CastleSide::Short);
            let bit = gigachess::types::castle_right_bit(c, kingside);
            if board.castling_rights() & (1 << bit) == 0 {
                return Err("missing castling right");
            }
            Ok(Move::new(board.king_square(c), board.castling_rook_square(bit), None))
        }
        MoveWord::Normal { color: c, piece, from, to, captured, promotion } => {
            let (c, p) = (color(c), role(piece));
            if board.turn() != c {
                return Err("move out of turn");
            }
            let from = Square(from);
            let to = Square(to);
            if board.piece_at(from).is_none_or(|v| v.color != c || v.role != p) {
                return Err("moving piece absent or wrong type");
            }
            if board.occ_color(c) & (1u64 << to.index()) != 0 {
                return Err("normal move would capture own piece");
            }
            let target = board.piece_at(to);
            if let Some(named) = other_role(captured) {
                if target.is_none_or(|v| v.color == c || v.role != named) {
                    return Err("captured piece does not match stored word");
                }
            } else if target.is_some() {
                return Err("unexpected capture at target square");
            }
            if captured == Captured::EnPassant && board.en_passant() != Some(to) {
                return Err("en passant target differs from stored word");
            }
            if captured == Captured::Nothing
                && p == Role::Pawn
                && board.en_passant() == Some(to)
                && from.file() != to.file()
            {
                return Err("en passant must be explicitly encoded");
            }
            Ok(Move::new(from, to, promotion.map(role)))
        }
    }
}

/// Replay one game with a reusable variation stack; the sink receives
/// gigachess moves2 (including its canonical null word), plus branch events.
pub fn walk_from(
    what: GameRef,
    game: &GameMoves<'_>,
    start: &Start,
    sink: &mut impl MoveSink,
    stack: &mut Vec<Board>,
    cache: &mut StartCache,
) -> Result<TreeStats> {
    let mut board = start_board_cached(start, cache)?;
    stack.clear();
    let mut saved = None;
    let mut stats = TreeStats { lines: 1, ..TreeStats::default() };
    // The main line is stored first; alternatives follow it after an
    // end-of-line sentinel. Mark subsequent decoded lines as variations.
    let mut main = true;
    let mut ended = false;
    let mut tokens = game.tokens().peekable();
    while let Some(token) = tokens.next() {
        if ended {
            return Err(error(what, stats.total_plies, "words after final end-of-line"));
        }
        if sink.stopped() {
            return Ok(stats);
        }
        match token {
            Token::MoveWord(w) => {
                let mv = to_move(&board, w, start).map_err(|why| error(what, stats.total_plies, why))?;
                if tokens.peek() == Some(&Token::Alternative) {
                    saved = Some(board);
                } else {
                    saved = None;
                }
                sink.play(&board, mv.word(), main);
                let result = match (sink.wants_zobrist(), sink.wants_checkers()) {
                    (true, true) => board.play(mv).map(|_| ()),
                    (true, false) => board.play_hashed(mv).map(|_| ()),
                    (false, true) => board.play_checkered(mv).map(|_| ()),
                    (false, false) => board.play_fast(mv).map(|_| ()),
                };
                result.map_err(|_| error(what, stats.total_plies, "gigachess rejected the encoded move"))?;
                sink.played(&board);
                stats.total_plies += 1;
                if main {
                    stats.main_line_plies += 1;
                }
            }
            Token::Alternative => {
                let before = saved
                    .take()
                    .ok_or_else(|| error(what, stats.total_plies, "alternative without a preceding move"))?;
                if stack.len() >= MAX_VARIATION_DEPTH {
                    return Err(error(what, stats.total_plies, "variation nesting beyond 1024"));
                }
                stack.push(before);
                sink.branch();
            }
            Token::EndOfLine => {
                main = false;
                saved = None;
                match stack.pop() {
                    Some(before) => {
                        board = before;
                        stats.lines += 1;
                        sink.resume();
                    }
                    None => ended = true,
                }
            }
        }
    }
    if !ended && !sink.stopped() {
        return Err(error(what, stats.total_plies, "move tree missing final end-of-line"));
    }
    Ok(stats)
}

/// Replay with all allocation-managed scratch owned by the caller.
pub fn walk_reuse(
    what: GameRef,
    game: &GameMoves<'_>,
    sink: &mut impl MoveSink,
    scratch: &mut V2Scratch,
) -> Result<TreeStats> {
    let start = start_of(game)?;
    walk_from(what, game, &start, sink, &mut scratch.stack, &mut scratch.starts)
}

/// Validate a game without persisting its moves.
pub fn verify(what: GameRef, game: &GameMoves<'_>, scratch: &mut V2Scratch) -> Result<TreeStats> {
    struct Silent;
    impl MoveSink for Silent {
        fn play(&mut self, _: &Board, _: u16, _: bool) {}
        fn played(&mut self, _: &Board) {}
        fn branch(&mut self) {}
        fn resume(&mut self) {}
    }
    walk_reuse(what, game, &mut Silent, scratch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbvault_format::twocbh::testdata::record;
    use std::path::Path;

    #[test]
    fn empty_game_replays_without_moves() {
        let frame = record(0, &[0xfc, 0xff, 0xff, 0xff]);
        let parsed = GameMoves::parse(Path::new("test.2cbg"), &frame).unwrap();
        let mut scratch = V2Scratch::default();
        let stats = verify(GameRef::new(1), &parsed, &mut scratch).unwrap();
        assert_eq!(stats.main_line_plies, 0);
        assert_eq!(stats.lines, 1);
    }

    #[test]
    fn first_white_king_word_is_illegal_from_start() {
        let frame = record(0, &[0xfc, 0xff, 1, 0, 0xff, 0xff]);
        let parsed = GameMoves::parse(Path::new("test.2cbg"), &frame).unwrap();
        let mut scratch = V2Scratch::default();
        assert!(verify(GameRef::new(1), &parsed, &mut scratch).is_err());
    }

    fn framed_words(words: &[u16]) -> Vec<u8> {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        record(0, &bytes)
    }

    fn move_word(color: word::Color, piece: WPiece, from: u8, to: u8, captured: Captured) -> u16 {
        word::encode(MoveWord::Normal { color, piece, from, to, captured, promotion: None })
            .expect("legal geometry is represented by a 2CBH word")
    }

    #[test]
    fn standard_opening_replays_and_counts_both_colours() {
        use word::Color::{Black, White};
        let words = [
            word::MOVES,
            move_word(White, WPiece::Pawn, 12, 28, Captured::Nothing), // 1. e4
            move_word(Black, WPiece::Pawn, 52, 36, Captured::Nothing), // ... e5
            move_word(White, WPiece::Knight, 6, 21, Captured::Nothing), // 2. Nf3
            move_word(Black, WPiece::Knight, 57, 42, Captured::Nothing), // ... Nc6
            word::END_OF_LINE,
        ];
        let frame = framed_words(&words);
        let moves = GameMoves::parse(Path::new("opening.2cbg"), &frame).unwrap();
        let stats = verify(GameRef::new(1), &moves, &mut V2Scratch::default()).unwrap();
        assert_eq!(stats.total_plies, 4);
        assert_eq!(stats.main_line_plies, 4);
        assert_eq!(stats.lines, 1);
    }

    #[test]
    fn variation_ends_and_restores_the_main_line() {
        use word::Color::{Black, White};
        let words = [
            word::MOVES,
            move_word(White, WPiece::Pawn, 12, 28, Captured::Nothing), // main 1. e4
            move_word(Black, WPiece::Pawn, 52, 36, Captured::Nothing), // main ... e5
            word::ALTERNATIVE,
            word::END_OF_LINE, // main line ends, restore board before ...e5
            move_word(Black, WPiece::Pawn, 51, 35, Captured::Nothing), // (1... d5)
            word::END_OF_LINE,
        ];
        let frame = framed_words(&words);
        let moves = GameMoves::parse(Path::new("variation.2cbg"), &frame).unwrap();
        let stats = verify(GameRef::new(2), &moves, &mut V2Scratch::default()).unwrap();
        assert_eq!(stats.total_plies, 3);
        assert_eq!(stats.main_line_plies, 2, "only the primary two moves count toward the main line");
        assert_eq!(stats.lines, 2);
    }

    #[test]
    fn impossible_capture_and_missing_final_end_are_rejected() {
        let bad_capture = framed_words(&[
            word::MOVES,
            move_word(word::Color::White, WPiece::Pawn, 12, 28, Captured::Nothing),
            move_word(word::Color::Black, WPiece::Pawn, 52, 36, Captured::Nothing),
            move_word(word::Color::White, WPiece::Knight, 6, 21, Captured::Pawn),
            word::END_OF_LINE,
        ]);
        let a = GameMoves::parse(Path::new("bad-capture.2cbg"), &bad_capture).unwrap();
        assert!(verify(GameRef::new(3), &a, &mut V2Scratch::default()).is_err());

        let incomplete =
            framed_words(&[word::MOVES, move_word(word::Color::White, WPiece::Pawn, 12, 28, Captured::Nothing)]);
        let b = GameMoves::parse(Path::new("missing-end.2cbg"), &incomplete).unwrap();
        assert!(verify(GameRef::new(4), &b, &mut V2Scratch::default()).is_err());
    }

    #[test]
    fn standard_start_is_reused() {
        let frame = record(0, &[0xfc, 0xff, 0xff, 0xff]);
        let parsed = GameMoves::parse(Path::new("test.2cbg"), &frame).unwrap();
        assert_eq!(start_of(&parsed).unwrap(), Start::Standard);
        assert_eq!(standard_board().to_fen(), standard_board().to_fen());
    }
}
