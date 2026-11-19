//! The "Game data" search over generated classic and 2CBH databases: every
//! field alone, combined, and the queries that must be refused.

use cbvault::bridge::{Database, Filter, IdSet, Range};
use cbvault::query::{
    AnyText, DateRange, EloDifference, EloMode, EloQuery, GameDataQuery, PlayerName, QueryError, TextMatch,
    TimeControl, search,
};
use cbvault_fixtures::classic::{self, Builder, Tok};
use cbvault_fixtures::twocbh;
use gigachess::Board;

/// The date field both formats pack: year, month and day in one integer.
fn packed(year: u32, month: u32, day: u32) -> u32 {
    year << 9 | month << 5 | day
}

/// The ECO field of `code` (0-499) and `sub`.
fn eco(code: u16, sub: u16) -> u16 {
    (code + 1) * 128 + sub
}

/// One game's header fields, as the test sets them.
struct Game {
    white: u32,
    black: u32,
    tournament: u32,
    annotator: u32,
    date: (u32, u32, u32),
    result: u8,
    elo: (u16, u16),
    eco: u16,
    moves: u8,
    mate: bool,
}

/// The five games both databases hold, in this order:
///
/// 1. Morphy-Anderssen, Paris 1858.12.27, 1-0, 2600/2500, C50, 30 moves
/// 2. Anderssen-Morphy, Paris 1858.12.28, 0-1, 2500/2600, B20, 40 moves
/// 3. González-Carlsen, Wijk aan Zee (NED) 2026.01.20, ½, 2700/2850, E99/3,
///    22 moves, annotated by Kasparov
/// 4. Carlsen-González, Wijk aan Zee 2026.11.05, 0-0, -/2700, A00, 10 moves
/// 5. Morphy-Anderssen 2025.06.01, 0-1 by fool's mate, no ECO, 2 moves
///
/// Players 0 and 1 are Morphy and Anderssen, 2 González and 3 Carlsen;
/// tournament 0 is Paris and 1 Wijk aan Zee; annotator 1 is Kasparov.
fn games() -> Vec<Game> {
    let g = |white, black, tournament, annotator, date, result, elo, eco, moves, mate| Game {
        white,
        black,
        tournament,
        annotator,
        date,
        result,
        elo,
        eco,
        moves,
        mate,
    };
    vec![
        g(0, 1, 0, 0, (1858, 12, 27), 2, (2600, 2500), eco(50, 0), 30, false),
        g(1, 0, 0, 0, (1858, 12, 28), 0, (2500, 2600), eco(120, 0), 40, false),
        g(2, 3, 1, 1, (2026, 1, 20), 1, (2700, 2850), eco(499, 3), 22, false),
        g(3, 2, 1, 0, (2026, 11, 5), 7, (0, 2700), eco(0, 0), 10, false),
        g(0, 1, 0, 0, (2025, 6, 1), 0, (0, 0), 0, 2, true),
    ]
}

/// The classic database: the five games and a guiding text (record 6).
fn classic_db(name: &str) -> cbvault_fixtures::TempDb {
    let board = Board::startpos();
    let quiet = classic::move_record(
        0,
        None,
        None,
        &classic::encode(&board, &[Tok::Mv("e2e4"), Tok::Mv("e7e5"), Tok::End], 0, false),
    );
    let mate = classic::move_record(
        0,
        None,
        None,
        &classic::encode(
            &board,
            &[Tok::Mv("f2f3"), Tok::Mv("e7e5"), Tok::Mv("g2g4"), Tok::Mv("d8h4"), Tok::End],
            0,
            false,
        ),
    );
    let mut b = Builder::new();
    b.player("González", "José");
    b.player("Carlsen", "Magnus");
    b.tournament("Wijk aan Zee", "NED");
    b.annotator("Kasparov");
    for g in games() {
        let h = b.game(if g.mate { &mate } else { &quiet });
        h[0x09..0x0c].copy_from_slice(&g.white.to_be_bytes()[1..]);
        h[0x0c..0x0f].copy_from_slice(&g.black.to_be_bytes()[1..]);
        h[0x0f..0x12].copy_from_slice(&g.tournament.to_be_bytes()[1..]);
        h[0x12..0x15].copy_from_slice(&g.annotator.to_be_bytes()[1..]);
        h[0x18..0x1b].copy_from_slice(&packed(g.date.0, g.date.1, g.date.2).to_be_bytes()[1..]);
        h[0x1b] = g.result;
        h[0x1f..0x21].copy_from_slice(&g.elo.0.to_be_bytes());
        h[0x21..0x23].copy_from_slice(&g.elo.1.to_be_bytes());
        h[0x23..0x25].copy_from_slice(&g.eco.to_be_bytes());
        h[0x2d] = g.moves;
    }
    b.text(&[(0, b"A guiding text")]);
    b.write(name)
}

/// The 2CBH database: the same five games, with Paris a classical event and
/// Wijk aan Zee a blitz one. Its annotator is a player, as in a real 2CBH
/// set: Kasparov is player 4.
fn twocbh_db(name: &str) -> cbvault_fixtures::TempDb {
    let temp = cbvault_fixtures::TempDb::create(name);
    let mut b = twocbh::Builder::new();
    for (last, first) in [("Morphy", "Paul"), ("Anderssen", "Adolf"), ("González", "José"), ("Carlsen", "Magnus")] {
        b.player(last, first);
    }
    let kasparov = b.player("Kasparov", "Garry");
    b.tournament_with_type("Paris", "FRA", 0, 0);
    b.tournament_with_type("Wijk aan Zee", "NED", 0, 0x20);
    let mut board = Board::startpos();
    let quiet = b.moves(1, &twocbh::words(&mut board, "e2e4 e7e5"));
    let mut board = Board::startpos();
    let mate = b.moves(1, &twocbh::words(&mut board, "f2f3 e7e5 g2g4 d8h4"));
    for g in games() {
        let rec = b.game(if g.mate { mate } else { quiet });
        twocbh::set_entities(rec, i64::from(g.white), i64::from(g.black), i64::from(g.tournament));
        let annotator = if g.annotator == 1 { kasparov } else { -1 };
        rec[0x30..0x38].copy_from_slice(&annotator.to_le_bytes());
        rec[0x38..0x40].copy_from_slice(&(-1i64).to_le_bytes());
        rec[0x58] = g.result;
        twocbh::set_elos(rec, g.elo.0 as i16, g.elo.1 as i16);
        rec[0x80..0x82].copy_from_slice(&g.eco.to_le_bytes());
        rec[0x8a..0x8c].copy_from_slice(&i16::from(g.moves).to_le_bytes());
        rec[0xbc..0xc0].copy_from_slice(&packed(g.date.0, g.date.1, g.date.2).to_le_bytes());
    }
    b.write_to(&temp);
    temp
}

fn ids(db: &Database, query: &GameDataQuery) -> Vec<u32> {
    let one = search(db, query, 1).expect("a search").ids;
    let many = search(db, query, 4).expect("a parallel search").ids;
    assert_eq!(one, many, "the thread count does not change the answer");
    one
}

fn white(last: &str) -> GameDataQuery {
    GameDataQuery { white: PlayerName::parse(last), ..GameDataQuery::default() }
}

/// The checks both generations must pass alike.
fn shared_checks(db: &Database) {
    let q = GameDataQuery::default;
    assert_eq!(ids(db, &q()), [1, 2, 3, 4, 5], "an empty query is every game, and only games");

    // Players, colours and wins.
    assert_eq!(ids(db, &white("Morphy")), [1, 5]);
    assert_eq!(ids(db, &GameDataQuery { ignore_colours: true, ..white("Morphy") }), [1, 2, 5]);
    assert_eq!(ids(db, &GameDataQuery { ignore_colours: true, wins_only: true, ..white("Morphy") }), [1, 2]);
    assert_eq!(ids(db, &GameDataQuery { wins_only: true, ..white("Morphy") }), [1]);
    assert_eq!(ids(db, &white("gonzalez")), [3], "case and accents fold away");
    assert_eq!(ids(db, &GameDataQuery { ignore_colours: true, ..white("GONZ") }), [3, 4]);
    assert_eq!(ids(db, &white(", Magnus")), [4], "a forename alone");
    assert!(ids(db, &white("arlsen")).is_empty(), "names match from their start by default");
    assert_eq!(ids(db, &GameDataQuery { names_match: TextMatch::Contains, ..white("arlsen") }), [4]);
    let both = GameDataQuery { black: PlayerName::parse("Carlsen"), ignore_colours: true, ..white("González") };
    assert_eq!(ids(db, &both), [3, 4]);

    // Tournament and annotator.
    assert_eq!(ids(db, &GameDataQuery { tournament: Some("wijk".into()), ..q() }), [3, 4]);
    assert_eq!(ids(db, &GameDataQuery { tournament: Some("NED".into()), ..q() }), [3, 4], "the place counts");
    assert_eq!(ids(db, &GameDataQuery { annotator: Some("kasp".into()), ..q() }), [3]);

    // Dates.
    let date = |text: &str| GameDataQuery { date: text.parse::<DateRange>().unwrap(), ..q() };
    assert_eq!(ids(db, &date("2026")), [3, 4]);
    assert_eq!(ids(db, &date("2026-2026.10")), [3]);
    assert_eq!(ids(db, &date("1858.12.28-")), [2, 3, 4, 5]);

    // ECO.
    let eco = |text: &str| GameDataQuery { eco: Some(text.parse().unwrap()), ..q() };
    assert_eq!(ids(db, &eco("B20-E99")), [2, 3], "an upper bound without a sub-code takes them all");
    assert_eq!(ids(db, &eco("B20-E99/02")), [2]);
    assert_eq!(ids(db, &eco("A00-E99/99")), [1, 2, 3, 4], "a game without ECO is outside every range");

    // Moves.
    assert_eq!(ids(db, &GameDataQuery { moves: Some(Range::new(0, 22)), ..q() }), [3, 4, 5]);

    // Elo, in each mode, and the difference.
    let elo = |low, high, mode| GameDataQuery { elo: Some(EloQuery { range: Range::new(low, high), mode }), ..q() };
    assert_eq!(ids(db, &elo(2550, 2650, EloMode::One)), [1, 2]);
    assert_eq!(ids(db, &elo(2600, 2900, EloMode::Both)), [3]);
    assert_eq!(ids(db, &elo(2700, 2800, EloMode::Average)), [3]);
    let diff = |d| GameDataQuery { elo_difference: Some(d), ..q() };
    assert_eq!(ids(db, &diff(EloDifference::AtMost(100))), [1, 2], "both ratings must be known");
    assert_eq!(ids(db, &diff(EloDifference::AtLeast(150))), [3]);

    // Results and endings.
    let mut r = q();
    r.results.white_wins = true;
    assert_eq!(ids(db, &r), [1]);
    let mut r = q();
    r.results.black_wins = true;
    assert_eq!(ids(db, &r), [2, 5]);
    let mut r = q();
    r.results.draw = true;
    r.results.both_lost = true;
    assert_eq!(ids(db, &r), [3, 4]);
    let mut r = q();
    r.endings.mate = true;
    let found = search(db, &r, 2).unwrap();
    assert_eq!(found.ids, [5], "the fool's mate is the one game that ends in mate");
    assert_eq!(found.replayed, 5);
    assert_eq!(found.failed, 0);
    let mut r = q();
    r.endings.stalemate = true;
    r.endings.check = true;
    assert!(ids(db, &r).is_empty());

    // Any text, with and without "not".
    let any = |text: &str, negate| GameDataQuery { any_text: Some(AnyText { text: text.into(), negate }), ..q() };
    assert_eq!(ids(db, &any("kasparov", false)), [3]);
    assert_eq!(ids(db, &any("kasparov", true)), [1, 2, 4, 5]);
    assert_eq!(ids(db, &any("paris", false)), [1, 2, 5]);

    // Everything at once.
    let all = GameDataQuery {
        ignore_colours: true,
        tournament: Some("Wijk".into()),
        date: "2026".parse().unwrap(),
        eco: Some("E00-E99/99".parse().unwrap()),
        moves: Some(Range::at_most(30)),
        elo: Some(EloQuery { range: Range::at_least(2700), mode: EloMode::Both }),
        ..white("Carlsen")
    };
    let mut all = all;
    all.results.draw = true;
    assert_eq!(ids(db, &all), [3]);

    // A filter of the bridge's own vocabulary combines with the rest.
    let extra = GameDataQuery { extra: Some(Filter::Ids(IdSet::from_ids([2, 3, 4]))), ..white("Anderssen") };
    assert_eq!(ids(db, &extra), [2]);

    // Refusals.
    let wins = GameDataQuery { wins_only: true, ..q() };
    assert!(matches!(search(db, &wins, 1), Err(QueryError::Invalid(_))));
    let mut text_mate = GameDataQuery { text_records: true, ..q() };
    text_mate.endings.mate = true;
    assert!(matches!(search(db, &text_mate, 1), Err(QueryError::Invalid(_))));
}

#[test]
fn every_game_data_field_works_on_a_classic_database() {
    let temp = classic_db("query-classic");
    let db = Database::open(temp.base()).expect("open");
    shared_checks(&db);
    assert_eq!(ids(&db, &GameDataQuery { text_records: true, ..GameDataQuery::default() }), [6]);
    let blitz = GameDataQuery { time_control: Some(TimeControl::Blitz), ..GameDataQuery::default() };
    assert!(
        matches!(search(&db, &blitz, 1), Err(QueryError::Unsupported(_))),
        "the classic tournament type is not decoded, so the time control is refused rather than guessed"
    );
}

#[test]
fn every_game_data_field_works_on_a_twocbh_database() {
    let temp = twocbh_db("query-twocbh");
    let db = Database::open(temp.base().with_extension("2cbh")).expect("open");
    shared_checks(&db);
    let tc = |t| GameDataQuery { time_control: Some(t), ..GameDataQuery::default() };
    assert_eq!(ids(&db, &tc(TimeControl::Blitz)), [3, 4]);
    assert_eq!(ids(&db, &tc(TimeControl::Classical)), [1, 2, 5]);
    assert!(ids(&db, &tc(TimeControl::Rapid)).is_empty());
}

#[test]
fn a_header_only_search_never_opens_the_moves_file() {
    let temp = classic_db("query-nomoves");
    let db = Database::open(temp.base()).expect("open");
    let hidden = temp.dir().join("query-nomoves.cbg.hidden");
    std::fs::rename(temp.path(".cbg"), &hidden).expect("hide .cbg");
    let found = search(&db, &GameDataQuery { ignore_colours: true, ..white("Morphy") }, 2);
    std::fs::rename(&hidden, temp.path(".cbg")).expect("put .cbg back");
    assert_eq!(found.expect("a search with .cbg absent").ids, [1, 2, 5]);
}
