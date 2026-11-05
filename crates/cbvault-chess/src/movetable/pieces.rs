// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! Set-up piece words: one word per piece and square of a set-up position.

use super::{Color, FIRST_PIECE_WORD, LAST_PIECE_WORD, Piece, Sq, from_cb_square};

/// Rank-major square to ChessBase (file-major).
fn to_cb_square(sq: Sq) -> u8 {
    let (rank, file) = (sq / 8, sq % 8);
    file * 8 + rank
}

/// The order of the non-pawn blocks of set-up piece words.
const PIECE_ORDER: [Piece; 5] = [Piece::King, Piece::Queen, Piece::Knight, Piece::Bishop, Piece::Rook];

/// First words of the set-up piece blocks: white pieces, black pieces, white
/// pawns, black pawns. Pieces take 64 words per kind, one per square; pawns 48,
/// one per square of ranks 2-7.
const WHITE_PIECES: u16 = FIRST_PIECE_WORD;
const BLACK_PIECES: u16 = WHITE_PIECES + 5 * 64;
const WHITE_PAWNS: u16 = BLACK_PIECES + 5 * 64;
const BLACK_PAWNS: u16 = WHITE_PAWNS + 48;

/// Decodes a set-up piece word (`0xc02d..=0xc30c`) as the piece and its square.
pub fn decode_piece_word(word: u16) -> Option<(Color, Piece, Sq)> {
    let piece = |base: u16, color| {
        let i = word - base;
        (color, PIECE_ORDER[(i / 64) as usize], from_cb_square((i % 64) as u8))
    };
    let pawn = |base: u16, color| {
        let i = word - base;
        (color, Piece::Pawn, from_cb_square(((i / 6) * 8 + i % 6 + 1) as u8))
    };
    match word {
        FIRST_PIECE_WORD..BLACK_PIECES => Some(piece(WHITE_PIECES, Color::White)),
        BLACK_PIECES..WHITE_PAWNS => Some(piece(BLACK_PIECES, Color::Black)),
        WHITE_PAWNS..BLACK_PAWNS => Some(pawn(WHITE_PAWNS, Color::White)),
        BLACK_PAWNS..=LAST_PIECE_WORD => Some(pawn(BLACK_PAWNS, Color::Black)),
        _ => None,
    }
}

/// Encodes a set-up piece, the inverse of [`decode_piece_word`]. A pawn on the
/// first or last rank, or a square above 63, returns `None`.
pub fn encode_piece_word(color: Color, piece: Piece, sq: Sq) -> Option<u16> {
    if sq >= 64 {
        return None;
    }
    let cb = to_cb_square(sq) as u16;
    match piece {
        Piece::Pawn => {
            let (file, rank) = (cb / 8, cb % 8);
            let base = if color == Color::White { WHITE_PAWNS } else { BLACK_PAWNS };
            (1..=6).contains(&rank).then(|| base + file * 6 + rank - 1)
        }
        _ => {
            let kind = PIECE_ORDER.iter().position(|&p| p == piece)? as u16;
            let base = if color == Color::White { WHITE_PIECES } else { BLACK_PIECES };
            Some(base + kind * 64 + cb)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piece_words() {
        assert_eq!(BLACK_PAWNS + 48 - 1, LAST_PIECE_WORD);
        // Blocks as documented: white king a1, black king a1, white pawn a2, black pawn a2.
        assert_eq!(decode_piece_word(0xc02d), Some((Color::White, Piece::King, 0)));
        assert_eq!(decode_piece_word(0xc16d), Some((Color::Black, Piece::King, 0)));
        assert_eq!(decode_piece_word(0xc2ad), Some((Color::White, Piece::Pawn, 8)));
        assert_eq!(decode_piece_word(0xc2dd), Some((Color::Black, Piece::Pawn, 8)));
        // White rook g1: kind 4, ChessBase square g1 = 6 * 8.
        assert_eq!(decode_piece_word(0xc02d + 4 * 64 + 6 * 8), Some((Color::White, Piece::Rook, 6)));
        assert_eq!(decode_piece_word(FIRST_PIECE_WORD - 1), None);
        assert_eq!(decode_piece_word(LAST_PIECE_WORD + 1), None);
        for w in FIRST_PIECE_WORD..=LAST_PIECE_WORD {
            let (c, p, sq) = decode_piece_word(w).unwrap();
            assert_eq!(encode_piece_word(c, p, sq), Some(w), "{w:#06x}");
        }
        assert_eq!(encode_piece_word(Color::White, Piece::Pawn, 0), None);
        assert_eq!(encode_piece_word(Color::Black, Piece::Pawn, 63), None);
        assert_eq!(encode_piece_word(Color::White, Piece::King, 64), None);
    }
}
