//! The classic decoder against records the fixture builder writes: the
//! encoder is a second reading of the format description, so the decoder is
//! not checked against itself.

use cbvault_chess::decode::{GameRef, NULL_MOVE};
use cbvault_chess::tree::{MovesBuf, ROOT, decode_game_into};
use cbvault_fixtures::classic::{self, Tok};
use cbvault_format::cbh::moves::GameMoves;
use gigachess::{Board, Color, Move, Role, Square};

/// The `moves2` stream the tokens describe, walked on `board` as the fixture
/// encoder walks them (a `Var` opens a bracket that `End` closes): the
/// expected value the decoder must produce.
fn expected(board: &Board, toks: &[Tok<'_>]) -> Vec<u16> {
    let mut board = *board;
    let mut out = Vec::new();
    let mut brackets: Vec<Board> = Vec::new();
    for tok in toks {
        match tok {
            Tok::Var => brackets.push(board),
            Tok::End => {
                if let Some(outer) = brackets.pop() {
                    board = outer;
                }
            }
            Tok::Mv(alg) => {
                let mv = if *alg == "O-O" || *alg == "O-O-O" {
                    let kingside = *alg == "O-O";
                    let bit = gigachess::types::castle_right_bit(board.turn(), kingside);
                    Move::new(board.king_square(board.turn()), board.castling_rook_square(bit), None)
                } else {
                    let from = Square::from_alg(&alg[0..2]).expect("a square");
                    let to = Square::from_alg(&alg[2..4]).expect("a square");
                    let promo = alg.as_bytes().get(4).map(|c| match c {
                        b'r' => Role::Rook,
                        b'b' => Role::Bishop,
                        b'n' => Role::Knight,
                        _ => Role::Queen,
                    });
                    Move::new(from, to, promo)
                };
                out.push(mv.word());
                board.play(mv).expect("a legal token move");
            }
        }
    }
    out
}

/// A `.cbg` record of `toks` from `board`, encoded in `mode`: the record's
/// flags carry the mode, as the format stores it.
fn record(board: &Board, toks: &[Tok<'_>], mode: u8) -> Vec<u8> {
    classic::move_record(mode, None, None, &classic::encode(board, toks, mode, false))
}

/// Decodes one record and returns the buffer.
fn decode(record: &[u8]) -> MovesBuf {
    let game = GameMoves::parse(std::path::Path::new("db.cbg"), record).expect("a sound record");
    let mut buf = MovesBuf::with_capacity(64);
    decode_game_into(GameRef::at(1, 0), &game, &mut buf).expect("a decodable game");
    buf
}

#[test]
fn a_standard_game_decodes_to_the_encoders_moves() {
    let toks = [
        Tok::Mv("e2e4"),
        Tok::Mv("e7e5"),
        Tok::Mv("g1f3"),
        Tok::Mv("b8c6"),
        Tok::Mv("f1c4"),
        Tok::Mv("g8f6"),
        Tok::Mv("O-O"),
        Tok::Mv("f8e7"),
        Tok::End,
    ];
    let board = Board::startpos();
    let buf = decode(&record(&board, &toks, 0));
    assert_eq!(buf.moves(), expected(&board, &toks).as_slice());
    assert_eq!(buf.stats().total_plies, 8);
    assert_eq!(buf.stats().main_line_plies, 8);
    assert_eq!(buf.stats().lines, 1);
    assert!(buf.is_main().iter().all(|&m| m));
    assert_eq!(buf.parents()[0], ROOT);
    for (i, parent) in buf.parents().iter().enumerate().skip(1) {
        assert_eq!(*parent as usize, i - 1, "the main line chains");
    }
}

#[test]
fn variations_keep_their_shape() {
    // 1. e4 e5 2. Nf3 (2. Bc4): Nf3 has alternatives still to come, so the
    // stream brackets Nf3's line and the alternative follows its end.
    let toks = [Tok::Mv("e2e4"), Tok::Mv("e7e5"), Tok::Var, Tok::Mv("g1f3"), Tok::End, Tok::Mv("f1c4"), Tok::End];
    let board = Board::startpos();
    let buf = decode(&record(&board, &toks, 0));
    assert_eq!(buf.moves(), expected(&board, &toks).as_slice());
    assert_eq!(buf.len(), 4);
    assert_eq!(buf.stats().lines, 2);
    assert_eq!(buf.stats().main_line_plies, 3);
    assert_eq!(buf.stats().total_plies, 4);
    // Bc4 is an alternative to Nf3: it hangs from e5, as Nf3 does.
    assert_eq!(buf.parents(), &[ROOT, 0, 1, 1]);
    assert_eq!(buf.is_main(), &[true, true, true, false]);
    assert!(!buf.is_null(0));
}

#[test]
fn nested_variations_decode() {
    // 1. e4 e5 2. Nf3 Nc6 (2. Bc4): the bracket around Nf3's line contains
    // Nc6's, and Bc4's line is the alternative to Nf3.
    let toks = [
        Tok::Mv("e2e4"),
        Tok::Mv("e7e5"),
        Tok::Var,
        Tok::Mv("g1f3"),
        Tok::Var,
        Tok::Mv("b8c6"),
        Tok::End,
        Tok::End,
        Tok::Mv("f1c4"),
        Tok::End,
    ];
    let board = Board::startpos();
    let buf = decode(&record(&board, &toks, 0));
    assert_eq!(buf.moves(), expected(&board, &toks).as_slice());
    assert_eq!(buf.len(), 5);
    assert_eq!(buf.stats().lines, 3);
    assert_eq!(buf.stats().main_line_plies, 4, "1. e4 e5 2. Nf3 Nc6");
    assert_eq!(buf.parents(), &[ROOT, 0, 1, 2, 1]);
    assert_eq!(buf.is_main(), &[true, true, true, true, false]);
}

#[test]
fn two_alternatives_at_one_node_decode() {
    // 1. e4 e5 2. Nf3 Nc6 (2. Bc4 Nf6): one node, two alternatives after the
    // main line's bracket.
    let toks = [
        Tok::Mv("e2e4"),
        Tok::Mv("e7e5"),
        Tok::Var,
        Tok::Mv("g1f3"),
        Tok::Var,
        Tok::Mv("b8c6"),
        Tok::End,
        Tok::End,
        Tok::Mv("f1c4"),
        Tok::Mv("g8f6"),
        Tok::End,
    ];
    let board = Board::startpos();
    let buf = decode(&record(&board, &toks, 0));
    assert_eq!(buf.moves(), expected(&board, &toks).as_slice());
    assert_eq!(buf.parents(), &[ROOT, 0, 1, 2, 1, 4], "Bc4 and Nf6 branch from e5 and Bc4");
    assert_eq!(buf.stats().lines, 3);
    assert_eq!(buf.is_main(), &[true, true, true, true, false, false]);
}

/// The 28-byte start position of `board`'s placement, as the format stores it.
fn start_position_of(board: &Board, black_to_move: bool, castling: u8) -> [u8; 28] {
    let mut pieces = Vec::new();
    for file in 0..8u8 {
        for rank in 0..8u8 {
            let sq = Square::from_coords(file, rank);
            if let Some(p) = board.piece_at(sq) {
                pieces.push((String::from_utf8_lossy(&sq.to_alg()).to_string(), p.role, p.color));
            }
        }
    }
    let refs: Vec<(&str, Role, Color)> = pieces.iter().map(|(s, r, c)| (s.as_str(), *r, *c)).collect();
    classic::start_position(&refs, black_to_move, castling, 0)
}

#[test]
fn promotions_castling_and_en_passant_decode() {
    // A promotion from a set-up position, in mode 4: the record carries the
    // 28-byte start position and bit 6 of its flags.
    let board = gigachess::fen::parse_fen("8/P7/4k3/4p3/8/8/8/4K3 w - - 0 1").unwrap();
    let toks = [Tok::Mv("a7a8q"), Tok::Mv("e6e7"), Tok::End];
    let setup = start_position_of(&board, false, 0);
    let stream = classic::encode(&board, &toks, 4, false);
    let buf = decode(&classic::move_record(0x40 | 4, Some(&setup), None, &stream));
    assert_eq!(buf.moves(), expected(&board, &toks).as_slice());
    assert_eq!(buf.moves()[0] >> 12, Role::Queen as u16, "the promotion role is in the word");

    // En passant.
    let board = Board::startpos();
    let toks = [Tok::Mv("e2e4"), Tok::Mv("a7a6"), Tok::Mv("e4e5"), Tok::Mv("d7d5"), Tok::Mv("e5d6"), Tok::End];
    let buf = decode(&record(&board, &toks, 0));
    assert_eq!(buf.moves(), expected(&board, &toks).as_slice());

    // Castling is the king onto its rook in moves2.
    let toks = [
        Tok::Mv("e2e4"),
        Tok::Mv("e7e5"),
        Tok::Mv("g1f3"),
        Tok::Mv("b8c6"),
        Tok::Mv("f1c4"),
        Tok::Mv("g8f6"),
        Tok::Mv("O-O"),
        Tok::End,
    ];
    let buf = decode(&record(&board, &toks, 0));
    let castled = buf.moves()[6];
    assert_eq!(
        (Square((castled & 63) as u8).to_alg(), Square((castled >> 6 & 63) as u8).to_alg()),
        (*b"e1", *b"h1"),
        "castling is king-to-rook"
    );
}

#[test]
fn a_null_move_is_kept_as_its_marker() {
    // A pass in the middle of the line, written by the encoder like any
    // other token: the counter moves, the turn flips, and the move after the
    // pass plays the other side. After 1. e4 e5 White passes, so Black is to
    // move again and 1... Nf6 is legal.
    let board = Board::startpos();
    let toks = [Tok::Mv("e2e4"), Tok::Mv("e7e5"), Tok::Mv("--"), Tok::Mv("g8f6"), Tok::End];
    let rec = classic::move_record(0, None, None, &classic::encode(&board, &toks, 0, false));
    let buf = decode(&rec);
    let e4 = Move::new(Square::from_alg("e2").unwrap(), Square::from_alg("e4").unwrap(), None).word();
    let e5 = Move::new(Square::from_alg("e7").unwrap(), Square::from_alg("e5").unwrap(), None).word();
    let nf6 = Move::new(Square::from_alg("g8").unwrap(), Square::from_alg("f6").unwrap(), None).word();
    assert_eq!(buf.moves(), &[e4, e5, NULL_MOVE, nf6]);
    assert!(buf.is_null(2));
    assert_eq!(buf.stats().total_plies, 4);
    assert_eq!(buf.stats().main_line_plies, 4);
}

#[test]
fn a_long_game_decodes_in_every_mode() {
    // The same tree in each mode the reader knows: the modes differ only in
    // the byte translation.
    let toks = [
        Tok::Mv("d2d4"),
        Tok::Mv("g8f6"),
        Tok::Mv("c2c4"),
        Tok::Mv("e7e6"),
        Tok::Mv("g1f3"),
        Tok::Mv("b7b6"),
        Tok::Mv("g2g3"),
        Tok::Mv("c8b7"),
        Tok::Mv("f1g2"),
        Tok::Mv("f8e7"),
        Tok::Mv("O-O"),
        Tok::End,
    ];
    let board = Board::startpos();
    let want = expected(&board, &toks);
    for mode in [0u8, 4, 5] {
        let buf = decode(&record(&board, &toks, mode));
        assert_eq!(buf.moves(), want.as_slice(), "mode {mode}");
    }
}

/// `NULL_MOVE` is a convenience alias, not a second source of truth. It is
/// derived from `gigachess::Move::NULL`, and this pins that: if the engine ever
/// changes the sentinel, the alias follows it and the compile-time `const`
/// initialiser fails loudly rather than this crate silently decoding a word the
/// engine no longer treats as a pass.
#[test]
fn null_move_alias_is_the_engine_word() {
    assert_eq!(NULL_MOVE, gigachess::Move::NULL.word());
    assert!(gigachess::Move::from_word(NULL_MOVE).is_null());
    assert_eq!(NULL_MOVE, 0xffff, "the CBH wire value the format fixes");

    // And the engine's own view of a pass is what this crate relies on: legal out
    // of check, illegal in it, and never offered as a legal move. The position
    // is reached by playing, so the claim is about the position rather than a
    // hand-built FEN.
    let f2f3 = Move::new(Square::from_alg("f2").expect("f2"), Square::from_alg("f3").expect("f3"), None);
    let mut board = gigachess::Board::startpos();
    board.play(f2f3).expect("f2f3 is legal");
    assert!(!board.in_check());
    assert!(board.is_legal(gigachess::Move::NULL));
    assert!(board.play(gigachess::Move::NULL).is_ok());
    assert!(!board.legal_moves().iter().any(|m| m.is_null()));
}

// Native mode-0 records from the public CC0 Lichess benchmark corpus.
// Expected main lines independently decoded from the matching 2CBH records.

#[test]
fn promoted_overflow_native_226476() {
    let record: &[u8] = &[
        0, 0, 0, 177, 132, 219, 63, 135, 15, 16, 4, 228, 1, 54, 142, 126, 66, 34, 165, 1, 238, 24, 192, 186, 138, 104,
        238, 141, 106, 216, 32, 92, 46, 1, 32, 69, 106, 40, 11, 172, 214, 154, 76, 171, 128, 115, 116, 8, 102, 172,
        117, 129, 187, 147, 154, 69, 204, 35, 31, 205, 48, 140, 32, 142, 106, 193, 173, 10, 135, 111, 137, 225, 62,
        146, 131, 158, 28, 205, 148, 116, 252, 152, 133, 12, 38, 3, 137, 231, 105, 236, 184, 172, 186, 132, 9, 254,
        161, 168, 163, 21, 165, 182, 18, 120, 191, 199, 60, 201, 175, 148, 42, 150, 179, 74, 46, 127, 183, 42, 205,
        131, 185, 46, 37, 135, 175, 164, 213, 221, 193, 54, 217, 143, 197, 168, 210, 122, 49, 170, 57, 9, 201, 172, 45,
        204, 225, 210, 227, 120, 229, 221, 211, 211, 100, 218, 64, 241, 65, 186, 202, 7, 68, 188, 220, 61, 241, 96, 88,
        192, 65, 53, 73, 4, 247, 196, 69, 212, 168,
    ];
    let uci = "e2e3 c7c5 b1c3 e7e6 d2d4 d7d5 g1f3 b8c6 d4c5 a7a6 e3e4 f8c5 e4d5 e6d5 c1e3 c5e3 f2e3 g8f6 f1d3 d8e7 e1h1 e7e3 g1h1 e8h8 f1e1 e3h6 d3e2 c8g4 h2h3 g4f3 e2f3 a8d8 c3e2 f6e4 e2c3 e4f2 h1g1 f2d1 a1d1 d5d4 c3e4 c6e5 e4g3 e5f3 g2f3 h6h3 g1f2 f8e8 e1h1 h3d7 h1h4 h7h6 h4e4 e8e4 g3e4 d7c6 d1d3 c6c2 d3d2 c2c6 d2e2 d4d3 e2d2 c6c1 f2e3 d8e8 e3d4 f7f5 e4d6 c1d2 d6e8 d2c2 e8d6 d3d2 d6f5 d2d1q d4e5 c2f5 e5f5 d1f3 f5g6 f3e2 b2b4 e2b2 b4b5 b2a2 b5b6 a6a5 g6f5 a2a3 f5e6 a3b4 e6d7 b4b6 d7c8 b6a6 c8b8 b7b5 b8c7 b5b4 c7d7 b4b3 d7d8 a5a4 d8d7 a4a3 d7d8 g7g5 d8d7 h6h5 d7d8 g5g4 d8e7 h5h4 e7d8 g4g3 d8c7 h4h3 c7d8 a3a2 d8e7 b3b2 e7d8 g3g2 d8e7 h3h2 e7d8 h2h1q d8c7 g2g1q c7b8 a2a1q b8c7 a6b7 c7d6 g1b6 d6e5 b6e6 e5e6 b7d5 e6f6 d5e6 f6e6 h1d5 e6d5 b2b1q d5c5 a1a5 c5d4 b1b4 d4d3 a5a3 d3c2 b4b2 c2d1 a3a1";
    let tokens: Vec<_> = uci.split_whitespace().map(Tok::Mv).collect();
    let buf = decode(record);
    assert_eq!(buf.moves(), expected(&Board::startpos(), &tokens));
    assert_eq!(buf.stats().main_line_plies as usize, tokens.len());
}

#[test]
fn promoted_overflow_native_2712614() {
    let record: &[u8] = &[
        0, 0, 0, 205, 255, 188, 13, 56, 136, 128, 4, 204, 62, 18, 71, 32, 89, 180, 102, 24, 202, 220, 40, 26, 138, 139,
        104, 235, 69, 157, 49, 115, 83, 36, 118, 156, 218, 245, 43, 1, 124, 141, 28, 113, 30, 106, 93, 255, 94, 231,
        143, 55, 94, 112, 107, 106, 123, 118, 72, 149, 129, 79, 83, 49, 133, 80, 29, 32, 135, 118, 244, 203, 246, 91,
        141, 203, 250, 10, 251, 120, 36, 122, 135, 211, 1, 126, 4, 215, 44, 138, 159, 128, 254, 160, 48, 130, 113, 166,
        11, 215, 165, 174, 54, 136, 166, 148, 18, 138, 18, 228, 20, 154, 193, 123, 159, 184, 26, 146, 198, 250, 28,
        162, 201, 131, 183, 234, 34, 154, 35, 47, 52, 196, 54, 44, 78, 242, 42, 162, 46, 104, 215, 164, 51, 162, 84,
        111, 64, 96, 221, 119, 201, 153, 205, 151, 207, 176, 60, 1, 207, 155, 60, 157, 233, 159, 63, 184, 43, 23, 104,
        89, 84, 91, 108, 190, 220, 188, 72, 173, 209, 208, 225, 10, 78, 179, 96, 200, 81, 13, 233, 105, 235, 169, 1,
        33, 87, 22, 241, 224, 108, 115, 245, 214, 111, 79, 186,
    ];
    let uci = "e2e4 g7g6 d2d4 f8g7 e4e5 c7c6 g1f3 d7d6 e5d6 f7f6 b1c3 e7d6 d1e2 d8e7 c3e4 f6f5 e4c3 e7e2 f1e2 g8f6 e1h1 e8h8 f1e1 b8a6 a2a3 d6d5 b2b4 a6c7 c1b2 f6e4 c3e4 f5e4 f3d2 c7b5 f2f3 e4f3 d2f3 f8f3 e2f3 b5d4 f3g4 c8g4 h2h4 d4c2 e1c1 c2a1 c1a1 g7b2 a1b1 b2d4 g1h2 g4f5 h2g3 f5b1 h4h5 d4b2 g3g4 b2a3 h5g6 a3b4 g4g5 h7g6 g2g4 b1e4 g5f6 b4c3 f6e6 a8e8 e6d6 c3b4 d6c7 d5d4 c7b7 a7a5 b7a6 a5a4 a6b6 a4a3 b6c7 d4d3 c7b6 a3a2 b6a6 d3d2 a6b6 b4c3 b6b7 a2a1b b7c7 d2d1b c7b6 e4c2 b6b7 a1b2 b7c7 d1e2 c7b7 e2d3 b7a7 c2b3 a7b6 c3b4 b6c7 b2c3 c7b7 d3c4 b7a7 b3a4 a7b6 b4a5 b6b7 c6c5 b7a7 c4d5 a7a6 c3d4 a6a5 a4c2 a5b5 c5c4 b5a5 d5f3 a5b4 f3g4 b4c4 d4f2 c4c3 c2f5 c3d2 f2g3 d2c3 g3f4 c3c4 g6g5 c4c5 g4f3 c5b6 g5g4 b6a6 g4g3 a6b5 g3g2 b5a4 g2g1b a4b4 g1f2 b4b3 f2e3 b3c3 f3e4 c3b3 f4e5 b3c4 f5e6 c4b5 e8d8 b5a5 e5d6 a5a4 e4d5 a4a5 e3d4 a5a6 e6f5 a6b5 f5d3 b5a4 d8c8 a4a5 d3c4 a5a4 d4c5 a4a5 d5c6";
    let tokens: Vec<_> = uci.split_whitespace().map(Tok::Mv).collect();
    let buf = decode(record);
    assert_eq!(buf.moves(), expected(&Board::startpos(), &tokens));
    assert_eq!(buf.stats().main_line_plies as usize, tokens.len());
}
