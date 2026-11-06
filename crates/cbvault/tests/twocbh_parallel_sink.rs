//! True multiworker 2CBH conversion must have byte-for-byte-equivalent
//! game payloads, keys, native annotations and failure IDs in input order.
//! Synthetic fixture only; no licensed real data in GitHub.

use cbvault::bridge::{Database, GameRef, GameSink, convert_parallel, for_each_game};
use cbvault_fixtures::TempDb;
use cbvault_fixtures::twocbh::{Builder, annotations, corrupted_checksum_frame, text_annotation, words};
use gigachess::Board;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    id: u32,
    moves: Vec<u16>,
    keys: Vec<u64>,
    annotations: Option<Vec<u8>>,
}

#[derive(Default)]
struct Collector {
    games: Vec<Seen>,
    failed: Vec<u32>,
}

impl GameSink for Collector {
    fn wants_annotations(&self) -> bool {
        true
    }
    fn wants_keys(&self) -> bool {
        true
    }
    fn game(&mut self, game: GameRef<'_>) {
        assert_eq!(game.keys.len(), game.moves.len() + 1);
        let annotations = game.twocbh_annotations.as_ref().map(|a| {
            a.iter_items()
                .collect::<Result<Vec<_>, _>>()
                .expect("all requested annotations must parse without silent termination");
            a.content().to_vec()
        });
        self.games.push(Seen { id: game.id, moves: game.moves.to_vec(), keys: game.keys.to_vec(), annotations });
    }
    fn failed(&mut self, id: u32, _: &cbvault::bridge::Error) {
        self.failed.push(id);
    }
}

#[test]
fn multiworker_v2_conversion_matches_sequential_annotated_and_keyed_stream() {
    let temp = TempDb::create("v2_parallel_annotated_sink");
    let mut b = Builder::new();
    for id in 1..=12 {
        if id == 6 {
            let bad = corrupted_checksum_frame(1, &[1, 2, 3, 4]);
            let at = b.move_frame_raw(&bad);
            b.game(at);
            continue;
        }
        let mut board = Board::startpos();
        let line = if id % 2 == 0 { "e2e4 e7e5 g1f3" } else { "d2d4 d7d5 c2c4" };
        let at = b.moves(1, &words(&mut board, line));
        if id % 3 == 0 {
            // Include an unknown native annotation kind as an independent
            // failure, not merely an invalid move-record checksum.
            let item =
                if id == 9 { 0x77u16.to_le_bytes().to_vec() } else { text_annotation(false, 1, "Synthetic note") };
            let data = annotations(&[(0, vec![item])]);
            let ann_at = b.annotations(&data);
            b.annotated_game(at, ann_at);
        } else {
            b.game(at);
        }
    }
    b.write_to(&temp);
    let db = Database::open(temp.base()).unwrap();

    let mut serial = Collector::default();
    let a = for_each_game(&db, &mut serial).unwrap();
    assert_eq!(a.games, 10);
    assert_eq!(a.failures, 2);
    assert_eq!(serial.failed, vec![6, 9]);
    let populated = serial
        .games
        .iter()
        .filter(|g| g.annotations.as_ref().is_some_and(|bytes| !bytes.is_empty()))
        .count();
    assert_eq!(populated, 2);

    for threads in [2, 4] {
        let mut parallel = Collector::default();
        let b = convert_parallel(&db, &mut parallel, threads, 2).unwrap();
        assert_eq!(b.records, a.records, "threads={threads}");
        assert_eq!(b.games, a.games, "threads={threads}");
        assert_eq!(b.failures, a.failures, "threads={threads}");
        assert_eq!(b.plies, a.plies, "threads={threads}");
        assert_eq!(b.keys, a.keys, "threads={threads}");
        assert_eq!(parallel.games, serial.games, "threads={threads}");
        assert_eq!(parallel.failed, serial.failed, "threads={threads}");
        assert!(b.complete);
    }
}

#[derive(Default)]
struct StopAfterFirst {
    seen: Vec<u32>,
}

impl GameSink for StopAfterFirst {
    fn game(&mut self, game: GameRef<'_>) {
        self.seen.push(game.id);
    }

    fn cancelled(&self) -> bool {
        !self.seen.is_empty()
    }
}

#[test]
fn multiworker_v2_cancel_is_an_ordered_prefix_not_an_entire_prefetched_wave() {
    let temp = TempDb::create("v2_parallel_cancel_prefix");
    let mut builder = Builder::new();
    for _ in 0..8 {
        let mut board = Board::startpos();
        let at = builder.moves(1, &words(&mut board, "e2e4 e7e5"));
        builder.game(at);
    }
    builder.write_to(&temp);
    let db = Database::open(temp.base()).unwrap();

    for threads in [2, 4] {
        let mut sink = StopAfterFirst::default();
        let stats = convert_parallel(&db, &mut sink, threads, 2).unwrap();
        assert!(!stats.complete, "cancelled 2CBH conversion cannot be reported complete");
        assert_eq!(sink.seen, vec![1, 2], "only the first chunk may have been delivered");
        assert_eq!(stats.games, 2);
        assert_eq!(stats.records, 2);
    }
}
