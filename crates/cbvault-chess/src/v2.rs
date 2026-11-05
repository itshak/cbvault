// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! 2CBH move decoding and tree walk on `gigachess`.
//!
//! Replays 16-bit 2CBH move words onto a `gigachess::Board`, verifying legal moves,
//! turn correctness, captured pieces, promotions, Chess960/standard castling (as king→rook),
//! null moves, and variation stacks, reporting each move as 16-bit `moves2` to a [`MoveSink`].

use cbvault_format::error::{Error, Result};
use cbvault_format::twocbh::moves::{GameMoves, Token};
use gigachess::{Board, Color, Move, Role, Square};

use super::decode::{GameRef, MoveSink, NULL_MOVE, TreeStats};
use super::movetable::{self, Captured, CastleSide, MoveWord};
use super::start::{Setup, Start, start_board};

/// The most variations open at once in one walk.
pub const MAX_VARIATION_DEPTH: usize = 1024;

/// Decodes the start position of a 2CBH game.
pub fn decode_v2_start(game: &GameMoves<'_>) -> Result<Start> {
    let start_bytes = game.start_slice();
    if start_bytes.is_empty() {
        return Ok(Start::Standard);
    }
    // Must start with START_POSITION (0xfffb)
    let w: Vec<u16> = start_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .skip(1) // skip 0xfffb
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    if game.is_chess960() && w.len() == 1 {
        return Ok(Start::Chess960(w[0]));
    }

    let (chess960, s) =
        if game.is_chess960() && w.first() == Some(&1000) { (true, &w[1..]) } else { (game.is_chess960(), &w[..]) };

    if s.len() < 3 {
        return Err(Error::corrupt("<moves>", 0, "short 2CBH set-up section"));
    }

    let move_number = s[0];
    let side_to_move = if s[1] & 0xff == 0 { Color::White } else { Color::Black };
    let castling = (s[1] >> 8) as u8;
    let ep_raw = s[2];
    let en_passant_file = if (1..=8).contains(&ep_raw) { Some((ep_raw - 1) as u8) } else { None };

    let mut pieces = [None; 64];
    for &pw in &s[3..] {
        let (c, p, sq) = movetable::decode_piece_word(pw)
            .ok_or_else(|| Error::corrupt("<moves>", 0, format!("bad piece word {pw:#06x}")))?;
        let color = match c {
            movetable::Color::White => Color::White,
            movetable::Color::Black => Color::Black,
        };
        let role = match p {
            movetable::Piece::King => Role::King,
            movetable::Piece::Queen => Role::Queen,
            movetable::Piece::Rook => Role::Rook,
            movetable::Piece::Bishop => Role::Bishop,
            movetable::Piece::Knight => Role::Knight,
            movetable::Piece::Pawn => Role::Pawn,
        };
        pieces[sq as usize] = Some((color, role));
    }

    Ok(Start::Setup(Setup {
        chess960,
        move_number,
        side_to_move,
        castling,
        castling_rooks: [None; 4],
        castling_kings: [None; 2],
        en_passant_file,
        en_passant_raw: ep_raw,
        pieces,
    }))
}

/// Converts a 16-bit 2CBH move word into a `gigachess::Move`, checking turn,
/// piece placement, and captures against `board`.
pub fn word_to_move(board: &Board, word: u16) -> Result<Move> {
    if word == movetable::NULL_MOVE {
        return Ok(Move::NULL);
    }
    let decoded = movetable::decode(word)
        .ok_or_else(|| Error::corrupt("<move>", 0, format!("not a 2CBH move word: {word:#06x}")))?;

    match decoded {
        MoveWord::Null => Ok(Move::NULL),
        MoveWord::Castle { color: c, side: s } | MoveWord::Castle960 { color: c, side: s, .. } => {
            let turn = match c {
                movetable::Color::White => Color::White,
                movetable::Color::Black => Color::Black,
            };
            if board.turn() != turn {
                return Err(Error::corrupt(
                    "<move>",
                    0,
                    format!("castling out of turn: word {word:#06x}, turn {:?}", board.turn()),
                ));
            }
            let kingside = s == CastleSide::Short;
            let index = gigachess::types::castle_right_bit(turn, kingside);
            if board.castling_rights() & (1 << index) == 0 {
                return Err(Error::corrupt(
                    "<move>",
                    0,
                    format!(
                        "no castling right for {}: word {word:#06x}, rights {:#06x}",
                        if kingside { "O-O" } else { "O-O-O" },
                        board.castling_rights()
                    ),
                ));
            }
            let rook = board.castling_rook_square(index);
            let king = board.king_square(turn);
            Ok(Move::new(king, rook, None))
        }
        MoveWord::Normal { color: c, piece: p, from, to, captured, promotion } => {
            let turn = match c {
                movetable::Color::White => Color::White,
                movetable::Color::Black => Color::Black,
            };
            if board.turn() != turn {
                return Err(Error::corrupt(
                    "<move>",
                    0,
                    format!("move out of turn: word {word:#06x}, turn {:?}", board.turn()),
                ));
            }
            let from_sq = Square(from);
            let to_sq = Square(to);
            let expected_role = match p {
                movetable::Piece::King => Role::King,
                movetable::Piece::Queen => Role::Queen,
                movetable::Piece::Rook => Role::Rook,
                movetable::Piece::Bishop => Role::Bishop,
                movetable::Piece::Knight => Role::Knight,
                movetable::Piece::Pawn => Role::Pawn,
            };

            let piece_at_from = board.piece_at(from_sq);
            if piece_at_from != Some(gigachess::Piece { color: turn, role: expected_role }) {
                return Err(Error::corrupt(
                    "<move>",
                    0,
                    format!(
                        "expected {turn:?} {expected_role:?} at {from_sq}, found {piece_at_from:?} (word {word:#06x})"
                    ),
                ));
            }

            // Cannot capture own piece
            if board.occ_color(turn) & (1u64 << to_sq.index()) != 0 {
                return Err(Error::corrupt(
                    "<move>",
                    0,
                    format!("destination {to_sq} is occupied by friendly piece (word {word:#06x})"),
                ));
            }

            // Check captured piece agreement
            match captured {
                Captured::Nothing => {
                    if board.piece_at(to_sq).is_some() {
                        return Err(Error::corrupt(
                            "<move>",
                            0,
                            format!("quiet move onto occupied square {to_sq} (word {word:#06x})"),
                        ));
                    }
                }
                Captured::EnPassant => {
                    if board.en_passant() != Some(to_sq) {
                        return Err(Error::corrupt(
                            "<move>",
                            0,
                            format!("en passant capture onto {to_sq} without ep square (word {word:#06x})"),
                        ));
                    }
                }
                cap => {
                    let expected_cap_role = match cap {
                        Captured::Queen => Role::Queen,
                        Captured::Rook => Role::Rook,
                        Captured::Bishop => Role::Bishop,
                        Captured::Knight => Role::Knight,
                        Captured::Pawn => Role::Pawn,
                        _ => unreachable!(),
                    };
                    if board.piece_at(to_sq) != Some(gigachess::Piece { color: turn.other(), role: expected_cap_role })
                    {
                        return Err(Error::corrupt(
                            "<move>",
                            0,
                            format!(
                                "capture of {expected_cap_role:?} on {to_sq} contradicted by board: found {:?} (word {word:#06x})",
                                board.piece_at(to_sq)
                            ),
                        ));
                    }
                }
            }

            let promo_role = promotion.map(|pr| match pr {
                movetable::Piece::Queen => Role::Queen,
                movetable::Piece::Rook => Role::Rook,
                movetable::Piece::Bishop => Role::Bishop,
                movetable::Piece::Knight => Role::Knight,
                _ => Role::Queen,
            });

            Ok(Move::new(from_sq, to_sq, promo_role))
        }
    }
}

/// Walks every line of a 2CBH move tree, checking each move and reporting it to `sink`.
pub fn walk_v2(what: GameRef, game: &GameMoves<'_>, sink: &mut impl MoveSink) -> Result<TreeStats> {
    let start = decode_v2_start(game)?;
    walk_v2_from(what, game, &start, sink)
}

/// Walks every line of a 2CBH move tree from an explicit start position.
pub fn walk_v2_from(what: GameRef, game: &GameMoves<'_>, start: &Start, sink: &mut impl MoveSink) -> Result<TreeStats> {
    let mut board = start_board(start)?;
    let mut stack: Vec<Board> = Vec::new();
    let mut saved: Option<Board> = None;
    let mut stats = TreeStats { lines: 1, ..Default::default() };
    let mut main = true;
    let mut ended = false;
    let mut tokens = game.tokens().peekable();

    while let Some(token) = tokens.next() {
        if ended {
            return Err(Error::corrupt(
                "<tree>",
                what.offset.unwrap_or(0),
                "words after the final end of line in 2CBH tree",
            ));
        }
        if sink.stopped() {
            return Ok(stats);
        }
        match token {
            Token::Move(w) => {
                let ply = stats.total_plies + 1;
                let mv = word_to_move(&board, w).map_err(|e| Error::Move {
                    game: what.id,
                    ply,
                    offset: what.offset,
                    detail: format!("{e}"),
                })?;
                let mv_word = if w == movetable::NULL_MOVE { NULL_MOVE } else { mv.word() };

                saved = (tokens.peek() == Some(&Token::Alternative)).then_some(board);

                sink.play(&board, mv_word, main);

                if w == movetable::NULL_MOVE {
                    board.play(Move::NULL).map_err(|why| Error::Move {
                        game: what.id,
                        ply,
                        offset: what.offset,
                        detail: format!("null move illegal: {why:?}"),
                    })?;
                } else if sink.wants_zobrist() || sink.wants_checkers() {
                    board.play(mv).map_err(|why| Error::Move {
                        game: what.id,
                        ply,
                        offset: what.offset,
                        detail: format!("move illegal: {why:?}"),
                    })?;
                } else {
                    board.play_fast(mv).map_err(|why| Error::Move {
                        game: what.id,
                        ply,
                        offset: what.offset,
                        detail: format!("move illegal: {why:?}"),
                    })?;
                }

                sink.played(&board);
                stats.total_plies += 1;
                if main {
                    stats.main_line_plies += 1;
                }
            }
            Token::Alternative => {
                let b = saved.take().ok_or_else(|| {
                    Error::corrupt("<tree>", what.offset.unwrap_or(0), "alternative marker not after a move")
                })?;
                if stack.len() >= MAX_VARIATION_DEPTH {
                    return Err(Error::corrupt(
                        "<tree>",
                        what.offset.unwrap_or(0),
                        format!("variations nested deeper than {MAX_VARIATION_DEPTH}"),
                    ));
                }
                stack.push(b);
                sink.branch();
            }
            Token::EndOfLine => {
                main = false;
                saved = None;
                match stack.pop() {
                    Some(b) => {
                        board = b;
                        stats.lines += 1;
                        sink.resume();
                    }
                    None => ended = true,
                }
            }
        }
    }

    if !ended && !sink.stopped() && !game.tree_slice().is_empty() {
        return Err(Error::corrupt("<tree>", what.offset.unwrap_or(0), "move tree not terminated"));
    }

    Ok(stats)
}
