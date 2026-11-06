//! Generated 2CBH error and annotation-preservation regressions.
//! No licensed real ChessBase game or native database is included.

use cbvault::bridge::{Database, GameBuf, GameRef, GameSink, for_each_game};
use cbvault_fixtures::TempDb;
use cbvault_fixtures::twocbh::{Builder, annotations, text_annotation, words};
use gigachess::Board;

#[derive(Default)]
struct AnnotatedCollector {
    seen: usize,
    items: usize,
    failed: Vec<u32>,
}

impl GameSink for AnnotatedCollector {
    fn wants_annotations(&self) -> bool {
        true
    }

    fn game(&mut self, game: GameRef<'_>) {
        self.seen += 1;
        assert!(game.annotations.is_none(), "v2 is not classic CBA");
        let anns = game.twocbh_annotations.expect("requested native 2CBA must be present");
        self.items += anns.iter_items().collect::<Result<Vec<_>, _>>().unwrap().len();
    }

    fn failed(&mut self, id: u32, _: &cbvault::bridge::Error) {
        self.failed.push(id);
    }
}

#[test]
fn requested_v2_annotations_are_borrowed_and_available_in_the_sink() {
    let temp = TempDb::create("v2_anns_visible_to_sink");
    let mut builder = Builder::new();
    let mut board = Board::startpos();
    let offset = builder.moves(1, &words(&mut board, "e2e4 e7e5"));
    let content = annotations(&[(0, vec![text_annotation(false, 1, "A generated example")])]);
    let ann_offset = builder.annotations(&content);
    builder.annotated_game(offset, ann_offset);
    builder.write_to(&temp);

    let db = Database::open(temp.base()).unwrap();
    let mut buf = GameBuf::new();
    let g = db.game_with(1, &mut buf, true).unwrap();
    let a = g.twocbh_annotations.expect("native annotations");
    assert_eq!(a.texts(), vec!["A generated example".to_string()]);
    let pgn = db.game_pgn_with(1, &mut buf, true).unwrap();
    assert!(pgn.contains("A generated example"), "{pgn}");

    let mut sink = AnnotatedCollector::default();
    let stats = for_each_game(&db, &mut sink).unwrap();
    assert_eq!(stats.games, 1);
    assert_eq!(stats.failures, 0);
    assert_eq!(sink.seen, 1);
    assert_eq!(sink.items, 1);
}

#[test]
fn unknown_annotation_type_must_fail_the_game_and_pgn_not_strip_the_comment() {
    let temp = TempDb::create("v2_unknown_anns_are_errors");
    let mut builder = Builder::new();
    let mut board = Board::startpos();
    let offset = builder.moves(1, &words(&mut board, "d2d4 d7d5"));
    let content = annotations(&[(0, vec![0x77u16.to_le_bytes().to_vec()])]);
    let ann_offset = builder.annotations(&content);
    builder.annotated_game(offset, ann_offset);
    builder.write_to(&temp);

    let db = Database::open(temp.base()).unwrap();
    let mut buf = GameBuf::new();
    assert!(db.game_with(1, &mut buf, true).is_err());
    assert!(db.game_pgn_with(1, &mut buf, true).is_err());
    let mut sink = AnnotatedCollector::default();
    let stats = for_each_game(&db, &mut sink).unwrap();
    assert_eq!(stats.games, 0);
    assert_eq!(stats.failures, 1);
    assert_eq!(sink.failed, vec![1]);
    assert_eq!(sink.seen, 0);
}

#[test]
fn referenced_out_of_range_annotation_frame_must_fail() {
    let temp = TempDb::create("v2_missing_anns_are_errors");
    let mut builder = Builder::new();
    let mut board = Board::startpos();
    let offset = builder.moves(1, &words(&mut board, "e2e4"));
    builder.annotated_game(offset, 100_000_000);
    builder.write_to(&temp);

    let db = Database::open(temp.base()).unwrap();
    let mut buf = GameBuf::new();
    assert!(db.game_with(1, &mut buf, true).is_err());
    assert!(db.game_pgn_with(1, &mut buf, true).is_err());
    let mut sink = AnnotatedCollector::default();
    let stats = for_each_game(&db, &mut sink).unwrap();
    assert_eq!(stats.failures, 1);
    assert_eq!(sink.seen, 0);
}
