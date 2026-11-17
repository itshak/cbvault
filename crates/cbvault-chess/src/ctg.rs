//! Opening books: between a `gigachess` position and a `.ctg` entry.
//!
//! Original cbvault code, clean-room (see
//! `docs/research/ctg-observations-2026-11-14.md`).
//!
//! A book stores every position with **White to move**. A position with Black to
//! move is looked up with its colours swapped and its board turned upside down
//! (rank r ↔ 8 − r). After that, a position with **no castling rights** and the
//! white king on files a–d is mirrored left to right (file f ↔ h − f). Both
//! transforms are undone on the way back, when a book move is turned into a
//! legal `gigachess` move.

use cbvault_format::ctg::position::{
    self, AlignedPad, CASTLE_BLACK_KING, CASTLE_BLACK_QUEEN, CASTLE_WHITE_KING, CASTLE_WHITE_QUEEN, MAX_ENCODED,
};
use cbvault_format::ctg::{Kind, Man, MoveCode, RawPosition};
use gigachess::{Board, Color, Move, Role, Square};

/// A position in the book's stored form, with the transforms that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    /// The stored position.
    pub raw: RawPosition,
    /// Colours were swapped and ranks reversed (Black was to move).
    pub flipped: bool,
    /// Files were reversed (no castling rights, white king on a–d).
    pub mirrored: bool,
}

/// Why a book move could not be turned into a legal move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    /// The move byte's meaning is not established.
    UnknownByte(u8),
    /// The stored position has no such piece (kind and ordinal).
    NoSuchPiece,
    /// No legal move matches the decoded squares.
    Illegal,
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::UnknownByte(b) => write!(f, "move byte {b:#04x} is not established"),
            MoveError::NoSuchPiece => f.write_str("no piece of that kind and ordinal"),
            MoveError::Illegal => f.write_str("decodes to no legal move"),
        }
    }
}

fn kind_of(role: Role) -> Kind {
    match role {
        Role::Pawn => Kind::Pawn,
        Role::Knight => Kind::Knight,
        Role::Bishop => Kind::Bishop,
        Role::Rook => Kind::Rook,
        Role::Queen => Kind::Queen,
        Role::King => Kind::King,
    }
}

/// The real square of a stored square index (`rank * 8 + file`).
fn real_square(stored: &Stored, index: usize) -> Square {
    let (mut file, mut rank) = ((index % 8) as u8, (index / 8) as u8);
    if stored.mirrored {
        file = 7 - file;
    }
    if stored.flipped {
        rank = 7 - rank;
    }
    Square::from_coords(file, rank)
}

/// Whether a pawn of the side to move stands next to the pawn that just made a
/// double step, so that an en-passant capture is at least pseudo-legal. The
/// probed book only sets the en-passant flag in that case (2,752/2,752).
fn en_passant_file(board: &Board) -> Option<u8> {
    let ep = board.en_passant()?;
    let us = board.turn();
    let pawn_rank = if us == Color::White { 4 } else { 3 };
    let beside = [ep.file().checked_sub(1), Some(ep.file() + 1).filter(|&f| f < 8)];
    beside
        .into_iter()
        .flatten()
        .any(|f| board.piece_at(Square::from_coords(f, pawn_rank)) == Some(gigachess::Piece::new(us, Role::Pawn)))
        .then_some(ep.file())
}

/// The stored form of `board`.
pub fn stored_form(board: &Board) -> Stored {
    let flipped = board.turn() == Color::Black;
    let mut raw = RawPosition::empty();
    for i in 0..64u8 {
        let sq = Square::new(i);
        if let Some(p) = board.piece_at(sq) {
            let (file, mut rank) = (sq.file(), sq.rank());
            if flipped {
                rank = 7 - rank;
            }
            let white = (p.color == Color::White) != flipped;
            raw.squares[usize::from(rank * 8 + file)] = Some(Man { white, kind: kind_of(p.role) });
        }
    }
    // gigachess bits: 1 WK, 2 WQ, 4 BK, 8 BQ — the same values as the format's.
    let rights = board.castling_rights();
    raw.castling = if flipped {
        let has = |m: u8| rights & m != 0;
        let mut c = 0;
        if has(4) {
            c |= CASTLE_WHITE_KING;
        }
        if has(8) {
            c |= CASTLE_WHITE_QUEEN;
        }
        if has(1) {
            c |= CASTLE_BLACK_KING;
        }
        if has(2) {
            c |= CASTLE_BLACK_QUEEN;
        }
        c
    } else {
        rights & 0xF
    };
    raw.ep_file = en_passant_file(board);
    let white_king_file =
        raw.squares.iter().position(|m| *m == Some(Man { white: true, kind: Kind::King })).map(|i| i % 8);
    let mirrored = raw.castling == 0 && white_king_file.is_some_and(|f| f < 4);
    if mirrored {
        let mut squares = [None; 64];
        for (i, m) in raw.squares.iter().enumerate() {
            squares[(i / 8) * 8 + (7 - i % 8)] = *m;
        }
        raw.squares = squares;
        raw.ep_file = raw.ep_file.map(|f| 7 - f);
    }
    Stored { raw, flipped, mirrored }
}

/// The keys to look `stored` up under, most likely first.
///
/// Usually one. A key whose padding falls in a case never observed in a real
/// book gets the alternative too, so the lookup tries both rather than guess.
pub fn keys(stored: &Stored) -> Vec<([u8; MAX_ENCODED], usize)> {
    let raw = &stored.raw;
    let (pad, observed) = position::observed_aligned_pad(raw.castling != 0, raw.ep_file.is_some());
    let mut out = Vec::with_capacity(2);
    let mut buf = [0; MAX_ENCODED];
    if let Some(len) = position::encode_with(raw, pad, &mut buf) {
        out.push((buf, len));
    }
    if !observed {
        let other = if pad == AlignedPad::None { AlignedPad::FullByte } else { AlignedPad::None };
        if let Some(len) = position::encode_with(raw, other, &mut buf)
            && out.iter().all(|(k, l)| k[..*l] != buf[..len])
        {
            out.push((buf, len));
        }
    }
    out
}

/// The stored square (`rank * 8 + file`) of the `ordinal`-th white piece of
/// `kind`, counting in the key's square order (a1, a2, …, a8, b1, …, h8).
fn nth_piece(raw: &RawPosition, kind: Kind, ordinal: u8) -> Option<usize> {
    let wanted = Some(Man { white: true, kind });
    (0..8)
        .flat_map(|file| (0..8).map(move |rank| rank * 8 + file))
        .filter(|&i| raw.squares[i] == wanted)
        .nth(usize::from(ordinal))
}

/// Turns move code `code` of the stored position into a legal move of `board`.
pub fn decode_move(board: &Board, stored: &Stored, code: MoveCode) -> Result<Move, MoveError> {
    let from_i = nth_piece(&stored.raw, code.kind, code.ordinal).ok_or(MoveError::NoSuchPiece)?;
    let (file, rank) = (from_i % 8, from_i / 8);
    let to_i = ((rank + usize::from(code.drank)) % 8) * 8 + (file + usize::from(code.dfile)) % 8;
    let from = real_square(stored, from_i);
    let to = real_square(stored, to_i);
    let castle = code.kind == Kind::King && code.drank == 0 && (code.dfile == 2 || code.dfile == 6);
    let legal = board.legal_moves();
    let mut found: Option<Move> = None;
    for &mv in legal.iter() {
        if mv.from() != from {
            continue;
        }
        let hit = if castle {
            // gigachess castles king → rook; the book moves the king two files.
            let rook = Some(gigachess::Piece::new(board.turn(), Role::Rook));
            board.piece_at(mv.to()) == rook && (mv.to().file() > from.file()) == (to.file() > from.file())
        } else {
            mv.to() == to
        };
        // A move byte carries no promotion piece. A pawn reaching the last rank
        // is a queen promotion: in the probed book the position after the
        // queen promotion is in the book for 874/874 promotion bytes, and no
        // entry repeats a byte, so an under-promotion has no byte of its own.
        if hit && mv.promotion().is_none_or(|r| r == Role::Queen) {
            found = Some(mv);
            break;
        }
    }
    found.ok_or(MoveError::Illegal)
}

/// The stored position as a `gigachess` board with White to move, as the book
/// sees it. `None` when it is not a valid chess position.
pub fn board_of(raw: &RawPosition) -> Option<Board> {
    let mut fen = String::with_capacity(90);
    for rank in (0..8).rev() {
        let mut empty = 0;
        for file in 0..8 {
            match raw.squares[rank * 8 + file] {
                None => empty += 1,
                Some(m) => {
                    if empty > 0 {
                        fen.push(char::from(b'0' + empty));
                        empty = 0;
                    }
                    let c = match m.kind {
                        Kind::Pawn => 'p',
                        Kind::Knight => 'n',
                        Kind::Bishop => 'b',
                        Kind::Rook => 'r',
                        Kind::Queen => 'q',
                        Kind::King => 'k',
                    };
                    fen.push(if m.white { c.to_ascii_uppercase() } else { c });
                }
            }
        }
        if empty > 0 {
            fen.push(char::from(b'0' + empty));
        }
        if rank > 0 {
            fen.push('/');
        }
    }
    fen.push_str(" w ");
    let rights =
        [(CASTLE_WHITE_KING, 'K'), (CASTLE_WHITE_QUEEN, 'Q'), (CASTLE_BLACK_KING, 'k'), (CASTLE_BLACK_QUEEN, 'q')];
    let before = fen.len();
    for (m, c) in rights {
        if raw.castling & m != 0 {
            fen.push(c);
        }
    }
    if fen.len() == before {
        fen.push('-');
    }
    match raw.ep_file {
        Some(f) => {
            fen.push(' ');
            fen.push(char::from(b'a' + (f & 7)));
            fen.push_str("6 0 1");
        }
        None => fen.push_str(" - 0 1"),
    }
    gigachess::fen::parse_fen(&fen).ok()
}

/// [`decode_move`] for a raw move byte.
pub fn decode_byte(board: &Board, stored: &Stored, byte: u8) -> Result<Move, MoveError> {
    let code = cbvault_format::ctg::move_code(byte).ok_or(MoveError::UnknownByte(byte))?;
    decode_move(board, stored, code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gigachess::fen::parse_fen;

    /// The key of the initial position (the same in every book: it is only
    /// the encoding of the board).
    const ROOT_KEY: [u8; 22] = [
        0x56, 0xb6, 0x0f, 0x79, 0x60, 0xf3, 0xa6, 0x0f, 0x58, 0xb0, 0x78, 0xe0, 0xc1, 0xe1, 0xa6, 0x0f, 0x59, 0x60,
        0xf3, 0xb6, 0x0f, 0x7f,
    ];

    #[test]
    fn the_start_position_keys_to_the_probed_root() {
        let stored = stored_form(&Board::startpos());
        assert!(!stored.flipped && !stored.mirrored);
        let keys = keys(&stored);
        assert_eq!(keys.len(), 1);
        assert_eq!(&keys[0].0[..keys[0].1], &ROOT_KEY);
    }

    #[test]
    fn the_twenty_root_bytes_decode_to_the_twenty_legal_moves() {
        let board = Board::startpos();
        let stored = stored_form(&board);
        // The bytes of the 20 legal moves of the initial position, sorted.
        let root = [
            0x04, 0x18, 0x34, 0x4a, 0x4c, 0x55, 0x5f, 0x62, 0x85, 0x9b, 0xa3, 0xb1, 0xbb, 0xc8, 0xcb, 0xcf, 0xdb, 0xe6,
            0xec, 0xf3,
        ];
        let mut got: Vec<Move> = root.iter().map(|&b| decode_byte(&board, &stored, b).unwrap()).collect();
        let mut legal: Vec<Move> = board.legal_moves().into_iter().collect();
        got.sort_by_key(|m| m.word());
        legal.sort_by_key(|m| m.word());
        assert_eq!(got, legal);
    }

    #[test]
    fn black_to_move_is_stored_flipped_as_white_to_move() {
        let after_e4 = parse_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1").unwrap();
        let stored = stored_form(&after_e4);
        assert!(stored.flipped);
        // The flipped board has White's pawns on rank 2 except one on rank 5
        // (the e-pawn, now Black's, seen from the other side).
        assert_eq!(stored.raw.squares[8 + 4], Some(Man { white: true, kind: Kind::Pawn }));
        assert_eq!(stored.raw.squares[32 + 4], Some(Man { white: false, kind: Kind::Pawn }));
        assert_eq!(stored.raw.castling, 0xF);
        // 0xe6 = (pawn, ordinal 3, +0, +2): the fourth stored pawn (d-file)
        // steps twice, which for Black is 1...d5.
        let mv = decode_byte(&after_e4, &stored, 0xe6).unwrap();
        assert_eq!(mv.to_string(), "d7d5");
    }

    #[test]
    fn a_castle_free_king_on_the_queen_side_is_mirrored() {
        let board = parse_fen("8/8/8/8/8/8/5k2/1K6 w - - 0 1").unwrap();
        let stored = stored_form(&board);
        assert!(stored.mirrored && !stored.flipped);
        // b1 mirrored is g1.
        assert_eq!(stored.raw.squares[6], Some(Man { white: true, kind: Kind::King }));
        // King one file right (0xbe) in the stored form is one file left for real.
        let mv = decode_byte(&board, &stored, 0xbe).unwrap();
        assert_eq!(mv.to_string(), "b1a1");
    }

    #[test]
    fn castling_bytes_become_king_to_rook_moves() {
        let board = parse_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let stored = stored_form(&board);
        assert_eq!(decode_byte(&board, &stored, 0x6b).unwrap().to_string(), "e1h1");
        assert_eq!(decode_byte(&board, &stored, 0xf6).unwrap().to_string(), "e1a1");
    }

    #[test]
    fn en_passant_is_keyed_only_when_a_pawn_can_take() {
        let can = parse_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        assert_eq!(stored_form(&can).raw.ep_file, Some(3));
        let cannot = parse_fen("4k3/8/8/3p4/8/8/8/4K3 w - d6 0 1").unwrap();
        assert_eq!(stored_form(&cannot).raw.ep_file, None);
    }

    #[test]
    fn an_unknown_byte_is_a_typed_error() {
        let board = Board::startpos();
        let stored = stored_form(&board);
        assert_eq!(decode_byte(&board, &stored, 0x02), Err(MoveError::UnknownByte(0x02)));
    }
}
