// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! End-to-end 2CBH -> gigachess -> shared PGN output from fully synthetic
//! records. Nothing from a user database or ChessBase PGN enters this fixture.

use cbvault::pgn::PgnWriter;
use cbvault::twocbh::Database;
use cbvault_format::twocbh::testdata::{Game, record, remove_set, segment, write_set};
use cbvault_format::twocbh::word::{self, Captured, Color, MoveWord, Piece};

fn normal(side: Color, role: Piece, from: u8, to: u8) -> u16 {
    word::encode(MoveWord::Normal {
        color: side,
        piece: role,
        from,
        to,
        captured: Captured::Nothing,
        promotion: None,
    })
    .expect("the move is part of the published v2 move-word enumeration")
}

fn replace_moves(stem: &std::path::Path, words: &[u16]) {
    let payload: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let path = format!("{}.2cbg", stem.display());
    std::fs::write(path, segment(&[record(0, &payload)])).unwrap();
}

#[test]
fn real_move_words_export_legal_san_and_entity_names() {
    let stem = write_set("v2-pgn-real-moves", &[Game::new(1, 1, 2)], true);
    replace_moves(
        &stem,
        &[
            word::MOVES,
            normal(Color::White, Piece::Pawn, 12, 28), // e2e4
            normal(Color::Black, Piece::Pawn, 52, 36), // e7e5
            normal(Color::White, Piece::Knight, 6, 21), // g1f3
            word::END_OF_LINE,
        ],
    );
    let db = Database::open(&stem).unwrap();
    let mut serial = Vec::new();
    db.write_game(1, &mut PgnWriter::new(), &mut serial).unwrap();
    let pgn = String::from_utf8(serial.clone()).unwrap();
    assert!(pgn.contains("[White \"Test1, Example\"]"), "{pgn}");
    assert!(pgn.contains("[Black \"Test2, Example\"]"), "{pgn}");
    assert!(pgn.contains("1. e4 e5 2. Nf3"), "{pgn}");
    let mut ordered_parallel = Vec::new();
    let stats = db.export_parallel(&mut ordered_parallel, 1, 0, 2, 1).unwrap();
    assert_eq!(stats.verified, 1);
    assert_eq!(serial, ordered_parallel, "parallel PGN differs byte for byte");
    remove_set(&stem);
}

#[test]
fn stored_order_variation_exports_in_pgn_order() {
    let stem = write_set("v2-pgn-variation", &[Game::new(1, 1, 2)], false);
    replace_moves(
        &stem,
        &[
            word::MOVES,
            normal(Color::White, Piece::Pawn, 12, 28), // 1. e4
            normal(Color::Black, Piece::Pawn, 52, 36), // 1... e5
            word::ALTERNATIVE,
            word::END_OF_LINE, // next decoded line starts from before 1...e5
            normal(Color::Black, Piece::Pawn, 51, 35), // 1...d5
            word::END_OF_LINE,
        ],
    );
    let db = Database::open(&stem).unwrap();
    let mut out = Vec::new();
    db.write_game(1, &mut PgnWriter::new(), &mut out).unwrap();
    let pgn = String::from_utf8(out).unwrap();
    assert!(pgn.contains("1. e4"), "{pgn}");
    assert!(pgn.contains("e5"), "{pgn}");
    assert!(pgn.contains("(1... d5)"), "{pgn}");
    remove_set(&stem);
}
