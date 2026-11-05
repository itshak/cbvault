//! Integration tests for reading 2CBH databases through `cbvault::bridge`.

use cbvault::bridge::{Database, GameBuf, GameSink, Generation, for_each_game};
use cbvault_fixtures::TempDb;
use cbvault_fixtures::twocbh::{Builder, set_elos, set_entities, words};
use gigachess::Board;

struct Collector {
    games: Vec<(u32, String, String, usize)>, // (id, white, black, ply_count)
    failed: Vec<u32>,
}

impl Collector {
    fn new() -> Self {
        Self { games: Vec::new(), failed: Vec::new() }
    }
}

impl GameSink for Collector {
    fn game(&mut self, game: cbvault::bridge::GameRef<'_>) {
        self.games.push((game.id, game.white.to_string(), game.black.to_string(), game.moves.len()));
    }

    fn failed(&mut self, id: u32, _error: &cbvault::bridge::Error) {
        self.failed.push(id);
    }
}

#[test]
fn test_2cbh_bridge_read_and_pgn() {
    let temp = TempDb::create("twocbh_bridge_test");
    let mut builder = Builder::new();

    let p1 = builder.player("Kasparov", "Garry");
    let p2 = builder.player("Deep Blue", "");
    let tour = builder.tournament("Man vs Machine", "Philadelphia", 19960210);

    // Game 1: 1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 c7c5 g1f3 d7d6 d2d4 c5d4 f3d4 g8f6 b1c3 a7a6");
    let moves_offset = builder.moves(1, &move_words);

    let g1 = builder.game(moves_offset);
    set_entities(g1, p1, p2, tour);
    set_elos(g1, 2785, 2700);

    // Add record 2: Guiding text
    builder.text(1, 1, 12);

    // Add record 3: Deleted game
    builder.deleted_game(moves_offset);

    // Add record 4: Second game (1. d4 d5 2. c4)
    let mut board2 = Board::startpos();
    let move_words2 = words(&mut board2, "d2d4 d7d5 c2c4");
    let moves_offset2 = builder.moves(4, &move_words2);
    let g4 = builder.game(moves_offset2);
    set_entities(g4, p2, p1, tour);

    builder.write_to(&temp);

    // 1. Open database through Database façade
    let db = Database::open(temp.base()).expect("2CBH database opens");
    assert_eq!(db.generation(), Generation::TwoCbh);
    assert_eq!(db.records(), 4);

    // 2. Walk headers
    let mut headers = db.headers();
    let mut seen_ids = Vec::new();
    for h in headers.by_ref() {
        let h = h.expect("valid header");
        seen_ids.push(h.id);
    }
    // Only games (1 and 4) are yielded by Headers iterator; texts and deleted are skipped
    assert_eq!(seen_ids, vec![1, 4]);
    assert_eq!(headers.stats().games, 2);
    assert_eq!(headers.stats().texts, 1);
    assert_eq!(headers.stats().deleted, 1);

    // 3. Read single game
    let mut buf = GameBuf::new();
    let game1 = db.game(1, &mut buf).expect("game 1 reads");
    assert_eq!(game1.id, 1);
    assert_eq!(game1.white, "Kasparov, Garry");
    assert_eq!(game1.black, "Deep Blue");
    assert_eq!(game1.event, "Man vs Machine");
    assert_eq!(game1.site, "Philadelphia");
    assert_eq!(game1.moves.len(), 10);

    // 4. Export single game PGN
    let pgn1 = db.game_pgn(1, &mut buf).expect("game 1 exports to PGN");
    assert!(pgn1.contains("[White \"Kasparov, Garry\"]"), "{pgn1}");
    assert!(pgn1.contains("[Black \"Deep Blue\"]"), "{pgn1}");
    assert!(pgn1.contains("[Event \"Man vs Machine\"]"), "{pgn1}");
    assert!(pgn1.contains("[Site \"Philadelphia\"]"), "{pgn1}");
    assert!(pgn1.contains("[WhiteElo \"2785\"]"), "{pgn1}");
    assert!(pgn1.contains("[BlackElo \"2700\"]"), "{pgn1}");
    assert!(pgn1.contains("1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6"), "{pgn1}");

    // 5. Sequential stream via for_each_game
    let mut collector = Collector::new();
    let stats = for_each_game(&db, &mut collector).expect("streaming conversion");
    assert_eq!(stats.games, 2);
    assert_eq!(collector.games.len(), 2);
    assert_eq!(collector.games[0], (1, "Kasparov, Garry".into(), "Deep Blue".into(), 10));
    assert_eq!(collector.games[1], (4, "Deep Blue".into(), "Kasparov, Garry".into(), 3));
    assert!(collector.failed.is_empty());
}

#[test]
fn test_2cbh_malformed_frame_recovery() {
    let temp = TempDb::create("twocbh_corrupt_test");
    let mut builder = Builder::new();

    // Game 1: valid moves
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 e7e5");
    let moves_offset = builder.moves(1, &move_words);
    builder.game(moves_offset);

    // Game 2: corrupted moves frame (bad checksum)
    let bad_moves = cbvault_fixtures::twocbh::corrupted_checksum_frame(2, &[0x00, 0x01, 0x02, 0x03]);
    let bad_offset = builder.move_frame_raw(&bad_moves);
    builder.game(bad_offset);

    // Game 3: valid moves
    let mut board3 = Board::startpos();
    let move_words3 = words(&mut board3, "d2d4 d7d5");
    let moves_offset3 = builder.moves(3, &move_words3);
    builder.game(moves_offset3);

    builder.write_to(&temp);

    let db = Database::open(temp.base()).expect("database opens");
    let mut collector = Collector::new();
    let stats = for_each_game(&db, &mut collector).expect("stream continues after failure");

    // Games 1 and 3 succeed; game 2 fails cleanly without panic
    assert_eq!(stats.games, 2);
    assert_eq!(stats.failures, 1);
    assert_eq!(collector.failed, vec![2]);
    assert_eq!(collector.games.len(), 2);
    assert_eq!(collector.games[0].0, 1);
    assert_eq!(collector.games[1].0, 3);
}

#[test]
fn test_twocbh_960_and_variations() {
    let temp = TempDb::create("twocbh_960_var_test");
    let mut builder = Builder::new();

    // 1. Chess960 game (position 0: BBNNRKRQ)
    let mut b960 = cbvault_chess::start::start_board(&cbvault_chess::start::Start::Chess960(0)).unwrap();
    let move_words_960 = cbvault_fixtures::twocbh::words_960(&mut b960, 0, "d2d4");
    let mut c960 = Vec::new();
    c960.extend(0xfffbu16.to_le_bytes()); // START_POSITION
    c960.extend(0u16.to_le_bytes()); // Chess960 pos 0
    c960.extend(0xfffcu16.to_le_bytes()); // MOVES
    for w in move_words_960 {
        c960.extend(w.to_le_bytes());
    }
    c960.extend(0xffffu16.to_le_bytes());
    let raw_960 = cbvault_fixtures::twocbh::framed(2, &c960); // tag 2 = Chess960
    let offset_960 = builder.move_frame_raw(&raw_960);
    builder.game(offset_960);

    // 2. Variations game: 1. e4 e5 (1... c5) 2. Nf3
    let mut b0 = Board::startpos();
    let w_e4 = words(&mut b0, "e2e4")[0];
    let mut b1 = b0;
    let w_e5 = words(&mut b1, "e7e5")[0];
    let w_nf3 = words(&mut b1, "g1f3")[0];
    let mut b_alt = b0;
    let w_c5 = words(&mut b_alt, "c7c5")[0];

    let mut c_var = Vec::new();
    c_var.extend(0xfffcu16.to_le_bytes()); // MOVES
    c_var.extend(w_e4.to_le_bytes());
    c_var.extend(w_e5.to_le_bytes());
    c_var.extend(0xfffdu16.to_le_bytes()); // ALTERNATIVE on e5
    c_var.extend(w_nf3.to_le_bytes()); // continues main line
    c_var.extend(0xffffu16.to_le_bytes()); // END_OF_LINE for main line
    c_var.extend(w_c5.to_le_bytes()); // alternative c5
    c_var.extend(0xffffu16.to_le_bytes()); // END_OF_LINE for c5
    let raw_var = cbvault_fixtures::twocbh::framed(1, &c_var);
    let offset_var = builder.move_frame_raw(&raw_var);
    builder.game(offset_var);

    builder.write_to(&temp);

    let db = Database::open(temp.base()).expect("database opens");
    assert_eq!(db.records(), 2);

    let mut buf = GameBuf::new();

    // Verify game 1 (Chess960)
    let g1 = db.game(1, &mut buf).expect("read 960 game");
    assert_eq!(g1.moves.len(), 1);
    assert!(g1.start_fen.is_some(), "Chess960 game has start FEN");
    let pgn1 = db.game_pgn(1, &mut buf).expect("export 960 PGN");
    assert!(pgn1.contains("[SetUp \"1\"]"), "{pgn1}");
    assert!(pgn1.contains("[Variant \"Chess960\"]") || pgn1.contains("[FEN "), "{pgn1}");

    // Verify game 2 (Variations)
    let g2 = db.game(2, &mut buf).expect("read variation game");
    assert_eq!(g2.moves.len(), 3); // Main line: 1. e4 e5 2. Nf3
    let pgn2 = db.game_pgn(2, &mut buf).expect("export variation PGN");
    assert!(pgn2.contains("1. e4 e5 (1... c5) 2. Nf3"), "{pgn2}");
}

#[test]
fn test_twocbh_analysis_record_recipe_6_1() {
    let temp = TempDb::create("twocbh_analysis_test");
    let mut builder = Builder::new();

    let author_id = builder.player("Kasparov", "Garry");
    let title_id = builder.tag("Scotch Game Analysis");

    // 4 plies: 1. e4 e5 2. Nf3 Nc6
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 e7e5 g1f3 b8c6");
    let moves_offset = builder.moves(1, &move_words);

    let packed_date = (2024u32 << 9) | (11u32 << 5) | 5u32;
    builder.analysis_full(title_id, author_id, moves_offset, packed_date, 4);

    builder.write_to(&temp);

    let db = Database::open(temp.base()).expect("database opens");
    assert_eq!(db.records(), 1);

    let mut buf = GameBuf::new();
    let game = db.game(1, &mut buf).expect("read analysis record");
    assert_eq!(game.id, 1);
    assert_eq!(game.event, "Scotch Game Analysis");
    assert_eq!(game.moves.len(), 4);
    assert_eq!(game.header.played_date().year(), 2024);
    assert_eq!(game.header.played_date().month(), 11);
    assert_eq!(game.header.played_date().day(), 5);

    let pgn = db.game_pgn(1, &mut buf).expect("export analysis PGN");
    assert!(pgn.contains("[Event \"Scotch Game Analysis\"]"), "{pgn}");
    assert!(pgn.contains("[Date \"2024.11.05\"]"), "{pgn}");
    assert!(pgn.contains("1. e4 e5 2. Nf3 Nc6"), "{pgn}");
}

#[test]
fn test_twocbh_results_and_tournament_types() {
    let temp = TempDb::create("twocbh_results_test");
    let mut builder = Builder::new();

    let p1 = builder.player("Carlsen", "Magnus");
    let p2 = builder.player("Nakamura", "Hikaru");
    let blitz_tour = builder.tournament_with_type("World Blitz", "Samarkand", 20231230, 0x20);

    // Moves: 1. e4 e5
    let mut board = Board::startpos();
    let move_words = words(&mut board, "e2e4 e7e5");
    let moves_offset = builder.moves(1, &move_words);

    // Game 1: result 3 -> *
    let g1 = builder.game(moves_offset);
    set_entities(g1, p1, p2, blitz_tour);
    g1[0x58] = 3;

    // Game 2: result 1 -> 1/2-1/2
    let g2 = builder.game(moves_offset);
    set_entities(g2, p1, p2, blitz_tour);
    g2[0x58] = 1;

    // Game 3: result 2 -> 1-0
    let g3 = builder.game(moves_offset);
    set_entities(g3, p1, p2, blitz_tour);
    g3[0x58] = 2;

    // Game 4: result 0 -> 0-1
    let g4 = builder.game(moves_offset);
    set_entities(g4, p1, p2, blitz_tour);
    g4[0x58] = 0;

    builder.write_to(&temp);

    let db = Database::open(temp.base()).expect("database opens");
    assert_eq!(db.records(), 4);

    let mut buf = GameBuf::new();

    let r1 = db.game(1, &mut buf).unwrap();
    assert_eq!(r1.header.result(), cbvault_format::game::GameResult::Line);
    let pgn1 = db.game_pgn(1, &mut buf).unwrap();
    assert!(pgn1.contains("[Result \"*\"]"), "{pgn1}");

    let r2 = db.game(2, &mut buf).unwrap();
    assert_eq!(r2.header.result(), cbvault_format::game::GameResult::Draw);
    let pgn2 = db.game_pgn(2, &mut buf).unwrap();
    assert!(pgn2.contains("[Result \"1/2-1/2\"]"), "{pgn2}");

    let r3 = db.game(3, &mut buf).unwrap();
    assert_eq!(r3.header.result(), cbvault_format::game::GameResult::WhiteWins);
    let pgn3 = db.game_pgn(3, &mut buf).unwrap();
    assert!(pgn3.contains("[Result \"1-0\"]"), "{pgn3}");

    let r4 = db.game(4, &mut buf).unwrap();
    assert_eq!(r4.header.result(), cbvault_format::game::GameResult::BlackWins);
    let pgn4 = db.game_pgn(4, &mut buf).unwrap();
    assert!(pgn4.contains("[Result \"0-1\"]"), "{pgn4}");

    // Check tournament entity
    let entities = db.twocbh_entities().unwrap();
    let tour = entities.tournament(blitz_tour).unwrap().unwrap();
    assert_eq!(tour.title, "World Blitz");
    assert_eq!(tour.place, "Samarkand");
    assert!(tour.is_blitz());
    assert!(!tour.is_rapid());
    assert!(!tour.is_correspondence());
}
