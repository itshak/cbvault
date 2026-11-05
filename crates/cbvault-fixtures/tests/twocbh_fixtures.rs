//! Tests for synthetic 2CBH fixture builder.

use cbvault_fixtures::TempDb;
use cbvault_fixtures::twocbh::{
    Builder, arrows_annotation, corrupted_checksum_frame, corrupted_trailer_frame, squares_annotation,
    symbols_annotation, text_annotation, truncated_frame, words,
};
use cbvault_format::twocbh::frame::parse_frame;
use gigachess::Board;

#[test]
fn test_synthetic_2cbh_builder_and_frames() {
    let temp = TempDb::create("twocbh_test");
    let mut builder = Builder::new();

    // 1. A standard game: 1. e4 e5 2. Nf3 Nc6 (in UCI: e2e4 e7e5 g1f3 b8c6)
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 e7e5 g1f3 b8c6");
    let moves_offset = builder.moves(1, &move_words);

    // 2. Annotations for the game
    let ann_bytes = cbvault_fixtures::twocbh::annotations(&[
        (-1, vec![text_annotation(true, 1, "Pre-game comment")]),
        (0, vec![symbols_annotation(1, 0, 0)]),
        (1, vec![squares_annotation(&[(1, "e5")])]),
        (2, vec![arrows_annotation(&[(1, "f3", "e5")])]),
    ]);
    let ann_offset = builder.annotations(&ann_bytes);

    // Add game
    builder.annotated_game(moves_offset, ann_offset);

    // Add guiding text
    builder.text(1, 1, 12);

    // Add analysis
    builder.analysis(2, 1, moves_offset);

    // Add deleted game
    builder.deleted_game(moves_offset);

    // Write to disk
    builder.write_to(&temp);

    // Verify files were created
    assert!(temp.path(".2cbh").exists());
    assert!(temp.path(".2cbg").exists());
    assert!(temp.path(".2cba").exists());
    assert!(temp.path(".2lid").exists());

    // Verify move frame parsing
    let cbg_bytes = std::fs::read(temp.path(".2cbg")).unwrap();
    let (tag, content) = parse_frame(&cbg_bytes[12..], 12, true).unwrap();
    assert_eq!(tag, 1);
    assert_eq!(content.len(), (move_words.len() + 2) * 2);
}

#[test]
fn test_corrupted_and_truncated_frames() {
    let content = b"game move tokens";
    let bad_csum = corrupted_checksum_frame(1, content);
    assert!(parse_frame(&bad_csum, 0, true).is_err());
    assert!(parse_frame(&bad_csum, 0, false).is_ok());

    let bad_tail = corrupted_trailer_frame(1, content);
    assert!(parse_frame(&bad_tail, 0, true).is_err());

    let trunc = truncated_frame(1, content, 20);
    assert!(parse_frame(&trunc, 0, true).is_err());
}
