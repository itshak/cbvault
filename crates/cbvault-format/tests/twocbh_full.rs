// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Synthetic read-only 2CBH database family tests: headers, frames and .2lid.

use cbvault_format::twocbh::testdata::{Game, write_set, remove_set};
use cbvault_format::twocbh::{self, Database, word::{self, MoveWord, Captured, Piece, Color}};

#[test]
fn complete_family_resolves_player_entities() {
    let stem = write_set("full-entities", &[Game::new(1, 1, 2), Game::new(2, 3, 4)], true);
    let db = Database::open(&stem).unwrap();
    assert_eq!(db.record_count(), 2);
    let one = db.record(1).unwrap();
    assert_eq!((one.white(), one.black()), (1, 2));
    let frame = db.moves_of(&one).unwrap();
    let moves = twocbh::GameMoves::parse(db.moves().path(), frame.bytes()).unwrap();
    assert_eq!(moves.content(), &[0xfc, 0xff, 0xff, 0xff]);

    let mut scratch = Vec::with_capacity(1024);
    let (mut last, mut first) = (String::with_capacity(32), String::with_capacity(32));
    assert!(db.entities().player_into(one.white(), &mut scratch, &mut last, &mut first).unwrap());
    assert_eq!((&last[..], &first[..]), ("Test1", "Example"));
    assert!(db.entities().player_into(one.black(), &mut scratch, &mut last, &mut first).unwrap());
    assert_eq!((&last[..], &first[..]), ("Test2", "Example"));
    let capacity = scratch.capacity();
    for _ in 0..100 {
        db.entities().player_into(1, &mut scratch, &mut last, &mut first).unwrap();
    }
    assert_eq!(scratch.capacity(), capacity);
    remove_set(&stem);
}

#[test]
fn standard_move_words_are_reversible() {
    let encoded = MoveWord::Normal {
        color: Color::White,
        piece: Piece::Pawn,
        from: 12, to: 28, captured: Captured::Nothing, promotion: None,
    }; // e2e4 (rank-major indices)
    let word = word::encode(encoded).expect("pawn double push is in table");
    assert_eq!(word::decode(word), Some(encoded));
    assert!(word::decode(word::MOVES).is_none());
}
