//! The "Game data" search against the bridge's own tag scan.
//!
//! - The **fixture** benches scan a generated set of 20,000 games on one
//!   thread: the bridge's `scan` with `EloAtLeast(1)`, the same criterion as a
//!   [`GameDataQuery`], and a query that sets every header field at once. The
//!   first two compare the two scans on equal work; the third is the cost of a
//!   full form.
//! - The **report** times a set of queries over a whole database, on one
//!   thread and ten. It is asked for with `CBVAULT_QUERY_REPORT=1` and needs
//!   `CBVAULT_TEST_DB` (a `.cbh` or `.2cbh` path or base); without it, it says
//!   so and stops.
//!
//! ```text
//! cargo bench -p cbvault --bench query
//! CBVAULT_QUERY_REPORT=1 CBVAULT_TEST_DB="Mega Database 2026.2cbh" cargo bench -p cbvault --bench query
//! ```
#![allow(missing_docs)]

use std::time::Instant;

use cbvault::bridge::{Database, Filter, Range, scan};
use cbvault::query::{EloMode, EloQuery, GameDataQuery, PlayerName, search};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

/// How many games the fixture benches scan per iteration.
const GAMES: u32 = 20_000;

fn fixture(criterion: &mut Criterion) {
    if std::env::var_os("CBVAULT_QUERY_REPORT").is_some() {
        return;
    }
    let db = build();
    let database = Database::open(db.base()).expect("the fixture set");
    let mut group = criterion.benchmark_group("query_fixture");
    group.throughput(Throughput::Elements(u64::from(GAMES)));
    group.bench_function("bridge_scan_elo_1_thread", |b| {
        b.iter(|| std::hint::black_box(scan(&database, &Filter::EloAtLeast(1), 1).expect("a scan").len()))
    });
    let elo = GameDataQuery {
        elo: Some(EloQuery { range: Range::at_least(1), mode: EloMode::One }),
        ..GameDataQuery::default()
    };
    group.bench_function("query_elo_1_thread", |b| {
        b.iter(|| std::hint::black_box(search(&database, &elo, 1).expect("a search").ids.len()))
    });
    let full = full_query();
    group.bench_function("query_every_field_1_thread", |b| {
        b.iter(|| std::hint::black_box(search(&database, &full, 1).expect("a search").ids.len()))
    });
    group.finish();
}

/// A query that sets every header field the fixture can satisfy.
fn full_query() -> GameDataQuery {
    let mut q = GameDataQuery {
        white: PlayerName::parse("Keres"),
        ignore_colours: true,
        tournament: Some("Zurich".into()),
        date: "1950-1960".parse().expect("a date range"),
        eco: Some("A00-E99/99".parse().expect("an ECO range")),
        moves: Some(Range::new(0, 80)),
        elo: Some(EloQuery { range: Range::new(2500, 2800), mode: EloMode::Average }),
        ..GameDataQuery::default()
    };
    q.results.white_wins = true;
    q.results.draw = true;
    q
}

/// [`GAMES`] games of Keres in the Zurich Candidates, 1953, between 2600 and
/// 2700, with an ECO code and 41 moves.
fn build() -> cbvault_fixtures::TempDb {
    use cbvault_fixtures::classic::{self, Builder, Tok};
    use gigachess::Board;

    let toks = [Tok::Mv("e2e4"), Tok::Mv("e7e5"), Tok::Mv("g1f3"), Tok::End];
    let record = classic::move_record(0, None, None, &classic::encode(&Board::startpos(), &toks, 0, false));
    let mut b = Builder::new();
    b.player("Keres", "Paul");
    b.tournament("Candidates", "Zurich");
    for i in 0..GAMES {
        let h = b.game(&record);
        h[0x09..0x0c].copy_from_slice(&2u32.to_be_bytes()[1..]);
        h[0x0f..0x12].copy_from_slice(&1u32.to_be_bytes()[1..]);
        h[0x18..0x1b].copy_from_slice(&(1953u32 << 9 | 9 << 5 | 1).to_be_bytes()[1..]);
        h[0x1b] = (i % 3) as u8;
        h[0x1f..0x21].copy_from_slice(&(2600 + (i % 100) as u16).to_be_bytes());
        h[0x21..0x23].copy_from_slice(&(2700 - (i % 100) as u16).to_be_bytes());
        h[0x23..0x25].copy_from_slice(&(((i % 500) as u16 + 1) * 128).to_be_bytes());
        h[0x2d] = 41;
    }
    b.write("query-bench")
}

/// The published numbers: each query over the whole database, one thread and
/// ten, after one warm pass.
fn report(_criterion: &mut Criterion) {
    if std::env::var_os("CBVAULT_QUERY_REPORT").is_none() {
        return;
    }
    let Some(path) = std::env::var_os("CBVAULT_TEST_DB") else {
        eprintln!("no report: CBVAULT_TEST_DB is not set. Nothing was measured; nothing passed.");
        return;
    };
    let db = Database::open(&path).expect("the reference set");
    println!("\n=== query report: {} ({} records) ===", path.to_string_lossy(), db.records());
    let elo = |low, mode| GameDataQuery {
        elo: Some(EloQuery { range: Range::at_least(low), mode }),
        ..GameDataQuery::default()
    };
    let mut mate = GameDataQuery { date: "2025-".parse().expect("a date range"), ..GameDataQuery::default() };
    mate.endings.mate = true;
    let queries: Vec<(&str, GameDataQuery)> = vec![
        (
            "bridge Filter::EloAtLeast(2700)",
            GameDataQuery { extra: Some(Filter::EloAtLeast(2700)), ..Default::default() },
        ),
        ("elo >= 2700, one", elo(2700, EloMode::One)),
        ("elo >= 2700, both", elo(2700, EloMode::Both)),
        (
            "white Carlsen, black Caruana, ignore colours",
            GameDataQuery {
                white: PlayerName::parse("Carlsen, Magnus"),
                black: PlayerName::parse("Caruana"),
                ignore_colours: true,
                ..Default::default()
            },
        ),
        ("tournament contains Wijk", GameDataQuery { tournament: Some("Wijk".into()), ..Default::default() }),
        (
            "ECO B20-B99, moves <= 22",
            GameDataQuery {
                eco: Some("B20-B99".parse().expect("an ECO range")),
                moves: Some(Range::at_most(22)),
                ..Default::default()
            },
        ),
        ("mate, 2025 on (replays the year)", mate),
    ];
    let _ = search(&db, &GameDataQuery::default(), 10);
    println!("| query | matches | 1 thread | 10 threads |");
    println!("|---|---:|---:|---:|");
    for (name, query) in &queries {
        let time = |threads| {
            let started = Instant::now();
            let found = search(&db, query, threads).expect("a search");
            (found.ids.len(), started.elapsed().as_secs_f64())
        };
        let (matches, one) = time(1);
        let (_, ten) = time(10);
        println!("| {name} | {matches} | {one:.3} s | {ten:.3} s |");
    }
}

criterion_group!(benches, fixture, report);
criterion_main!(benches);
