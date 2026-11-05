// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Unit tests for 2CBH move decoding.

use std::path::Path;

use cbvault_chess::decode::{GameRef, MoveSink};
use cbvault_chess::start::{Start, start_board};
use cbvault_chess::tree::{MovesBuf, decode_v2_into};
use cbvault_chess::v2::{decode_v2_start, walk_v2};
use cbvault_fixtures::twocbh::{framed, words, words_960};
use cbvault_format::twocbh::moves::GameMoves;
use gigachess::{Board, Move, Square};

struct CollectMoves(Vec<u16>);

impl MoveSink for CollectMoves {
    fn play(&mut self, _board: &Board, mv: u16, _main: bool) {
        self.0.push(mv);
    }
    fn played(&mut self, _board: &Board) {}
    fn branch(&mut self) {}
    fn resume(&mut self) {}
}

#[test]
fn test_v2_standard_game_replay() {
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 e7e5 g1f3 b8c6 f1c4 g8f6");
    let mut content = Vec::new();
    content.extend(0xfffcu16.to_le_bytes()); // MOVES
    for w in move_words {
        content.extend(w.to_le_bytes());
    }
    content.extend(0xffffu16.to_le_bytes()); // END_OF_LINE

    let raw = framed(1, &content);
    let game = GameMoves::parse(Path::new("test.2cbg"), &raw).expect("parse game");
    assert!(!game.is_chess960());

    let mut sink = CollectMoves(Vec::new());
    let stats = walk_v2(GameRef::new(1), &game, &mut sink).expect("walk game");
    assert_eq!(stats.total_plies, 6);
    assert_eq!(stats.main_line_plies, 6);
    assert_eq!(stats.lines, 1);
    assert_eq!(sink.0.len(), 6);

    // Test with MovesBuf
    let mut buf = MovesBuf::default();
    let stats2 = decode_v2_into(GameRef::new(1), &game, &mut buf).expect("decode into");
    assert_eq!(stats2.total_plies, 6);
    assert_eq!(buf.len(), 6);
}

#[test]
fn test_v2_null_move_replay() {
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 -- g1f3");
    let mut content = Vec::new();
    content.extend(0xfffcu16.to_le_bytes());
    for w in move_words {
        content.extend(w.to_le_bytes());
    }
    content.extend(0xffffu16.to_le_bytes());

    let raw = framed(1, &content);
    let game = GameMoves::parse(Path::new("test.2cbg"), &raw).expect("parse game");

    let mut sink = CollectMoves(Vec::new());
    let stats = walk_v2(GameRef::new(1), &game, &mut sink).expect("walk game");
    assert_eq!(stats.total_plies, 3);
}

#[test]
fn test_v2_castling_standard() {
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 e7e5 g1f3 b8c6 f1c4 f8c5 e1g1 g8f6");
    let mut content = Vec::new();
    content.extend(0xfffcu16.to_le_bytes());
    for w in move_words {
        content.extend(w.to_le_bytes());
    }
    content.extend(0xffffu16.to_le_bytes());

    let raw = framed(1, &content);
    let game = GameMoves::parse(Path::new("test.2cbg"), &raw).expect("parse game");

    let mut sink = CollectMoves(Vec::new());
    let stats = walk_v2(GameRef::new(1), &game, &mut sink).expect("walk game");
    assert_eq!(stats.total_plies, 8);
    // White castling e1g1 is at ply 7 (index 6), king-to-rook: e1h1 (e1 = 4, h1 = 7)
    let castle_move = sink.0[6];
    let mv = Move::from_word(castle_move);
    assert_eq!(mv.from(), Square(4));
    assert_eq!(mv.to(), Square(7));
}

#[test]
fn test_v2_chess960_castling() {
    let mut board = start_board(&Start::Chess960(0)).unwrap();
    let move_words = words_960(&mut board, 0, "d2d4");
    let mut content = Vec::new();
    content.extend(0xfffbu16.to_le_bytes()); // START_POSITION
    content.extend(0u16.to_le_bytes()); // Chess960 pos 0
    content.extend(0xfffcu16.to_le_bytes()); // MOVES
    for w in move_words {
        content.extend(w.to_le_bytes());
    }
    content.extend(0xffffu16.to_le_bytes());

    let raw = framed(2, &content); // tag 2 = Chess960
    let game = GameMoves::parse(Path::new("test.2cbg"), &raw).expect("parse game");
    assert!(game.is_chess960());

    let start = decode_v2_start(&game).expect("start");
    assert_eq!(start, Start::Chess960(0));

    let mut sink = CollectMoves(Vec::new());
    let stats = walk_v2(GameRef::new(1), &game, &mut sink).expect("walk game");
    assert_eq!(stats.total_plies, 1);
}

#[test]
fn test_v2_variations() {
    // 1. e4 e5 (1... c5) 2. Nf3
    let mut b0 = Board::startpos();
    let w_e4 = words(&mut b0, "e2e4")[0];
    let mut b1 = b0;
    let w_e5 = words(&mut b1, "e7e5")[0];
    let w_nf3 = words(&mut b1, "g1f3")[0];
    let mut b_alt = b0;
    let w_c5 = words(&mut b_alt, "c7c5")[0];

    let mut content = Vec::new();
    content.extend(0xfffcu16.to_le_bytes()); // MOVES
    content.extend(w_e4.to_le_bytes());
    content.extend(w_e5.to_le_bytes());
    content.extend(0xfffdu16.to_le_bytes()); // ALTERNATIVE on e5
    content.extend(w_nf3.to_le_bytes()); // continues main line
    content.extend(0xffffu16.to_le_bytes()); // END_OF_LINE for main line
    content.extend(w_c5.to_le_bytes()); // alternative c5
    content.extend(0xffffu16.to_le_bytes()); // END_OF_LINE for c5

    let raw = framed(1, &content);
    let game = GameMoves::parse(Path::new("test.2cbg"), &raw).expect("parse game");

    let mut buf = MovesBuf::default();
    let stats = decode_v2_into(GameRef::new(1), &game, &mut buf).expect("decode into");
    assert_eq!(stats.lines, 2);
    assert_eq!(stats.total_plies, 4);
    assert_eq!(stats.main_line_plies, 3);
    assert_eq!(buf.is_main(), &[true, true, true, false]);
}
