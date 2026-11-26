// Copyright (c) 2026 the cbvault contributors (MIT License). See LICENSE.

//! Synthetic regression tests and test vectors for CTG opening book support.
//!
//! All tests use purely synthetic, procedurally generated CTG fixtures from
//! `cbvault-fixtures`. Zero proprietary ChessBase files or bytes are committed.

use cbvault::book::Book;
use cbvault_chess::ctg as chess_ctg;
use cbvault_chess::ctg::MoveError;
use cbvault_fixtures::TempDb;
use cbvault_fixtures::ctg::{
    connected_parent_child_book, malformed_unclosed_book, minimal_valid_book, promoted_test_vectors_book,
    unknown_move_byte_book,
};
use cbvault_format::ctg::{Kind, MoveCode};
use gigachess::Board;

#[test]
fn minimal_synthetic_book_opens_and_probes() {
    let temp = TempDb::create("ctg_minimal_valid");
    let ctg_bytes = minimal_valid_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let book = Book::open(&path).expect("synthetic book must open cleanly");
    assert_eq!(book.pages(), 2);
    assert!(book.damaged().is_empty(), "clean book must have no damaged pages");

    let start = Board::startpos();
    let root = book.probe(&start).expect("probe succeeds").expect("startpos is in book");
    assert_eq!(root.stats.games, 1000);
    assert_eq!(root.stats.wins, 400);
    assert_eq!(root.stats.draws, 300);
    assert_eq!(root.stats.losses, 300);
    assert_eq!(root.moves.len(), 1);

    let report = book.verify(false).expect("verification succeeds");
    assert_eq!(report.entries, 1);
    assert_eq!(report.damaged_pages, 0);
    assert_eq!(report.bad_keys, 0);
    assert_eq!(report.invalid_positions, 0);
    assert_eq!(report.key_round_trips, 1);
    assert_eq!(report.key_mismatches, 0);
    assert_eq!(report.moves_decoded, 1);
    assert_eq!(report.moves_unknown, 0);
    assert_eq!(report.moves_illegal, 0);
}

#[test]
fn connected_synthetic_parent_child_verifies_children() {
    let temp = TempDb::create("ctg_parent_child");
    let ctg_bytes = connected_parent_child_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let book = Book::open(&path).expect("synthetic book must open cleanly");
    assert!(book.damaged().is_empty());

    let report = book.verify(true).expect("child verification succeeds");
    assert_eq!(report.entries, 2);
    assert_eq!(report.moves_decoded, 2);
    assert_eq!(report.children_found, 1);
    assert_eq!(report.children_missing, 1); // 1...e5 child is leaf
}

/// Regression test for indexer handling of unclosed page boundaries:
/// `index_pages()` checks `!walk.closed()` and records the page in `book.damaged()`.
#[test]
fn bug_a_index_pages_detects_unclosed_page() {
    let temp = TempDb::create("ctg_bug_a_index");
    let ctg_bytes = malformed_unclosed_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let book = Book::open(&path).expect("open handles recoverable damaged page");
    assert_eq!(
        book.damaged().len(),
        1,
        "index_pages must report the page where used-bytes boundary does not match entry end"
    );
    assert_eq!(book.damaged()[0].page, 1);
    assert!(book.damaged()[0].detail.contains("bytes-used"), "damage detail must mention bytes-used boundary mismatch");
}

/// Regression test for Bug A in `verify_pages()`:
/// `verify_pages()` checks `!walk.closed()` and counts the unclosed page as damaged.
#[test]
fn bug_a_verify_pages_detects_unclosed_page() {
    let temp = TempDb::create("ctg_bug_a_verify");
    let ctg_bytes = malformed_unclosed_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let book = Book::open(&path).expect("open handles recoverable damaged page");
    let report = book.verify(false).expect("verify completes");
    assert_eq!(
        report.damaged_pages, 1,
        "Book::verify(false) must report damaged_pages == 1 when entries do not end at bytes-used mark"
    );
}

#[test]
fn unknown_move_byte_exposes_typed_error() {
    let temp = TempDb::create("ctg_unknown_move");
    let ctg_bytes = unknown_move_byte_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let book = Book::open(&path).expect("open book");
    let report = book.verify(false).expect("verify book");
    assert_eq!(report.entries, 1);
    assert_eq!(report.moves_unknown, 1);
    assert_eq!(report.moves_decoded, 0);

    let start = Board::startpos();
    let pos = book.probe(&start).unwrap().unwrap();
    assert_eq!(pos.moves.len(), 1);
    assert_eq!(pos.moves[0].byte, 0xff);
    assert!(matches!(pos.moves[0].mv, Err(MoveError::UnknownByte(0xff))));
}

/// Validates that the mathematical `MoveCode` coordinates proposed for the 5 rare
/// promoted-piece moves decode to the exact expected legal chess moves.
#[test]
fn rare_promoted_move_codes_decode_to_expected_chess_moves() {
    // 1. Vector 0x89: 3rd knight (ordinal 2) moving dfile: 1, drank: 6 (g8h6)
    let b1 = gigachess::fen::parse_fen("rnk1bbNr/pp4pp/8/2p5/4p3/2N5/PPPB1PPP/R3K1NR w KQ - 0 1").unwrap();
    let stored1 = chess_ctg::stored_form(&b1);
    let code1 = MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 1, drank: 6 };
    let mv1 = chess_ctg::decode_move(&b1, &stored1, code1).expect("code1 must decode to legal move");
    assert_eq!(mv1.to_string(), "g8h6", "vector 1 must decode to Nh6 (g8h6)");

    // 2. Vector 0x2b: 3rd knight (ordinal 2) moving dfile: 7, drank: 2 (f3e5)
    let b2 = gigachess::fen::parse_fen("rNb1qk1r/ppQ1n2p/5b2/5pp1/2Bp4/5N2/PPP2PPP/RN2RK2 w - - 0 1").unwrap();
    let stored2 = chess_ctg::stored_form(&b2);
    let code2 = MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 7, drank: 2 };
    let mv2 = chess_ctg::decode_move(&b2, &stored2, code2).expect("code2 must decode to legal move");
    assert_eq!(mv2.to_string(), "f3e5", "vector 2 must decode to Ne5 (f3e5)");

    // 3. Vector 0x19: 3rd knight (ordinal 2) moving dfile: 6, drank: 1 (f3d4)
    let b3 = gigachess::fen::parse_fen("rNb3nr/pp2k1pp/8/1B3p2/3p4/4bN2/PPP3PP/RN2RK2 w - - 0 1").unwrap();
    let stored3 = chess_ctg::stored_form(&b3);
    let code3 = MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 6, drank: 1 };
    let mv3 = chess_ctg::decode_move(&b3, &stored3, code3).expect("code3 must decode to legal move");
    assert_eq!(mv3.to_string(), "f3d4", "vector 3 must decode to Nxd4 (f3d4)");

    // 4. Vector 0xbc: 2nd queen (ordinal 1) moving dfile: 5, drank: 0 (f3c3)
    let b4 = gigachess::fen::parse_fen("rnQ3nr/p3kppp/8/2b1p3/8/5Q2/PPqP1PPP/RNB1K1NR w KQ - 0 1").unwrap();
    let stored4 = chess_ctg::stored_form(&b4);
    let code4 = MoveCode { kind: Kind::Queen, ordinal: 1, dfile: 5, drank: 0 };
    let mv4 = chess_ctg::decode_move(&b4, &stored4, code4).expect("code4 must decode to legal move");
    assert_eq!(mv4.to_string(), "f3c3", "vector 4 must decode to Qc3 (f3c3)");

    // 5. Vector 0xb4: 3rd knight (ordinal 2) moving dfile: 7, drank: 6 (g8f6)
    let b5 = gigachess::fen::parse_fen("rn2qbNr/pp4pp/1k6/2p2Q2/2b1pB2/2N5/PPP2PPP/3RK1NR w K - 0 1").unwrap();
    let stored5 = chess_ctg::stored_form(&b5);
    let code5 = MoveCode { kind: Kind::Knight, ordinal: 2, dfile: 7, drank: 6 };
    let mv5 = chess_ctg::decode_move(&b5, &stored5, code5).expect("code5 must decode to legal move");
    assert_eq!(mv5.to_string(), "g8f6", "vector 5 must decode to Nf6 (g8f6)");
}

/// End-to-end integration test for the 5 rare promoted move bytes via `MOVE_TABLE`.
/// Verifies full byte-to-move decoding through `Book::probe` and `Book::verify`.
#[test]
fn rare_promoted_move_bytes_decode_via_move_table() {
    let temp = TempDb::create("ctg_promoted_vectors_table");
    let ctg_bytes = promoted_test_vectors_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let book = Book::open(&path).expect("open book");
    assert!(book.damaged().is_empty());

    let report = book.verify(false).expect("verify book");
    assert_eq!(report.entries, 5);
    assert_eq!(report.moves_unknown, 0, "all 5 promoted bytes should be mapped in MOVE_TABLE");
    assert_eq!(report.moves_decoded, 5, "all 5 promoted bytes should decode cleanly");

    let b1 = gigachess::fen::parse_fen("rnk1bbNr/pp4pp/8/2p5/4p3/2N5/PPPB1PPP/R3K1NR w KQ - 0 1").unwrap();
    let pos1 = book.probe(&b1).unwrap().expect("b1 in book");
    assert_eq!(pos1.moves[0].mv.as_ref().unwrap().to_string(), "g8h6");

    let b2 = gigachess::fen::parse_fen("rNb1qk1r/ppQ1n2p/5b2/5pp1/2Bp4/5N2/PPP2PPP/RN2RK2 w - - 0 1").unwrap();
    let pos2 = book.probe(&b2).unwrap().expect("b2 in book");
    assert_eq!(pos2.moves[0].mv.as_ref().unwrap().to_string(), "f3e5");

    let b3 = gigachess::fen::parse_fen("rNb3nr/pp2k1pp/8/1B3p2/3p4/4bN2/PPP3PP/RN2RK2 w - - 0 1").unwrap();
    let pos3 = book.probe(&b3).unwrap().expect("b3 in book");
    assert_eq!(pos3.moves[0].mv.as_ref().unwrap().to_string(), "f3d4");

    let b4 = gigachess::fen::parse_fen("rnQ3nr/p3kppp/8/2b1p3/8/5Q2/PPqP1PPP/RNB1K1NR w KQ - 0 1").unwrap();
    let pos4 = book.probe(&b4).unwrap().expect("b4 in book");
    assert_eq!(pos4.moves[0].mv.as_ref().unwrap().to_string(), "f3c3");

    let b5 = gigachess::fen::parse_fen("rn2qbNr/pp4pp/1k6/2p2Q2/2b1pB2/2N5/PPP2PPP/3RK1NR w K - 0 1").unwrap();
    let pos5 = book.probe(&b5).unwrap().expect("b5 in book");
    assert_eq!(pos5.moves[0].mv.as_ref().unwrap().to_string(), "g8f6");
}

// Synthetic one-slot native directory: every key routes to page one.
fn native_single_slot(temp: &TempDb, raw: u32) {
    let mut cto = vec![0u8; 20];
    cto[..4].copy_from_slice(&0x0010_0001u32.to_be_bytes());
    cto[4..8].copy_from_slice(&1u32.to_be_bytes());
    cto[16..].copy_from_slice(&raw.to_be_bytes());
    temp.write(".cto", &cto);
    let mut ctb = vec![0u8; 12];
    ctb[..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
    temp.write(".ctb", &ctb);
}

#[test]
fn native_reader_exact_hits_and_validated_absence() {
    use cbvault::book::{NativeBook, NativeProbe};
    let temp = TempDb::create("native_exact");
    let path = temp.write(".ctg", &connected_parent_child_book());
    native_single_slot(&temp, 0);
    let fallback = Book::open(&path).unwrap();
    let mut native = NativeBook::open(&path).unwrap();
    assert_eq!(native.positions(), None);
    let root = Board::startpos();
    let mut child = root;
    child.play(gigachess::san::san_to_move(&root, "e4").unwrap()).unwrap();
    let mut missing = child;
    missing.play(gigachess::san::san_to_move(&child, "c5").unwrap()).unwrap();
    for board in [&root, &child] {
        assert_eq!(native.probe(board).unwrap(), NativeProbe::Found(fallback.probe(board).unwrap().unwrap()));
    }
    assert!(fallback.probe(&missing).unwrap().is_none());
    assert_eq!(native.probe(&missing).unwrap(), NativeProbe::UnverifiedMiss);
    assert_eq!(native.validate().unwrap(), 2);
    assert_eq!(native.positions(), Some(2));
    assert_eq!(native.probe(&missing).unwrap(), NativeProbe::Absent);
}

#[test]
fn native_reader_refuses_damaged_and_missing_companions() {
    use cbvault::book::{NativeBook, NativeProbe};
    let root = Board::startpos();
    for (label, raw) in [("missing", u32::MAX), ("out_of_range", 999)] {
        let temp = TempDb::create(label);
        let path = temp.write(".ctg", &minimal_valid_book());
        assert!(NativeBook::open(&path).is_err());
        native_single_slot(&temp, raw);
        let mut native = NativeBook::open(&path).unwrap();
        assert!(native.validate().is_err());
        assert_eq!(native.positions(), None);
        if raw == u32::MAX {
            assert_eq!(native.probe(&root).unwrap(), NativeProbe::UnverifiedMiss);
        } else {
            assert!(native.probe(&root).is_err());
        }
        assert!(Book::open(&path).unwrap().probe(&root).unwrap().is_some());
    }
    let temp = TempDb::create("native_unclosed");
    let path = temp.write(".ctg", &malformed_unclosed_book());
    native_single_slot(&temp, 0);
    let mut native = NativeBook::open(&path).unwrap();
    assert!(native.validate().is_err());
    assert!(native.probe(&root).is_err());
}
