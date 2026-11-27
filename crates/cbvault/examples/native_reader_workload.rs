//! Native CTO, Adaptive, and Scan query parity & latency workload.
#![allow(missing_docs)]

use cbvault::book::{AdaptiveBook, Book, BookPosition, NativeBook, NativeProbe};
use cbvault_chess::ctg::board_of;
use cbvault_format::ctg::{Ctg, PAGE_SIZE, position};
use gigachess::Board;
use std::time::Instant;

enum Reader {
    Scan(Book),
    Native(NativeBook),
    Adaptive(Box<AdaptiveBook>),
}

impl Reader {
    fn probe(&self, b: &Board) -> Result<Option<BookPosition>, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Scan(book) => book.probe(b)?,
            Self::Native(book) => match book.probe(b)? {
                NativeProbe::Found(v) => Some(v),
                NativeProbe::Absent => None,
                NativeProbe::UnverifiedMiss => return Err("unverified negative during parity workload".into()),
            },
            Self::Adaptive(book) => match book.probe(b)? {
                NativeProbe::Found(v) => Some(v),
                NativeProbe::Absent => None,
                NativeProbe::UnverifiedMiss => return Err("unverified negative during adaptive parity workload".into()),
            },
        })
    }
}

fn mix(h: &mut u64, v: u64) {
    *h = (*h ^ v).wrapping_mul(1_099_511_628_211);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let path = args.get(1).ok_or("usage: native_reader_workload book.ctg <scan|native|quick|adaptive> [open-only]")?;
    let mode = args.get(2).ok_or("missing mode")?;
    let t = Instant::now();
    let book = match mode.as_str() {
        "scan" => Reader::Scan(Book::open(path)?),
        "adaptive" => Reader::Adaptive(Box::new(AdaptiveBook::open(path)?)),
        _ => Reader::Native(NativeBook::open(path)?),
    };
    let open = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut count = None;
    let index_bytes = match &book {
        Reader::Scan(b) => {
            count = Some(b.positions() as u64);
            b.index_bytes()
        }
        Reader::Native(b) => {
            if mode != "quick" {
                count = Some(b.validate()?);
            }
            0
        }
        Reader::Adaptive(b) => {
            if mode != "quick" {
                b.validate()?;
                count = b.positions();
            }
            0
        }
    };
    let validate = t.elapsed().as_secs_f64();
    if args.get(3).is_some_and(|a| a == "open-only") {
        println!(
            "{{\"open_seconds\":{open:.9},\"validate_seconds\":{validate:.9},\"index_bytes\":{index_bytes},\"positions\":{}}}",
            count.map_or("null".to_string(), |n| n.to_string())
        );
        return Ok(());
    }
    let ctg = Ctg::open(path)?;
    let mut buf = [0u8; PAGE_SIZE];
    let mut boards = Vec::new();
    for page in (1..ctg.pages()).step_by(4) {
        let bytes = match ctg.mapped_page(page) {
            Some(b) => b,
            None => {
                ctg.read_page(page, &mut buf)?;
                &buf[..]
            }
        };
        let mut entries = ctg.entries(page, bytes)?;
        let n = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if n == 0 {
            continue;
        }
        let ix = (u64::from(page).wrapping_mul(6_364_136_223_846_793_005) >> 32) as usize % n;
        let e = entries.nth(ix).ok_or("short page")?.map_err(|e| e.into_error(ctg.path()))?;
        let raw = position::decode(e.key).map_err(|_| "bad key")?;
        boards.push(board_of(&raw).ok_or("bad board")?);
    }
    let positives = boards.len();
    let mut state = 0xf23_abc9u64;
    for _ in 0..16384 {
        let mut b = Board::startpos();
        for ply in 0..48 {
            let legal = b.legal_moves();
            if legal.is_empty() {
                break;
            }
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            b.play(legal[(state >> 32) as usize % legal.len()]).map_err(|_| "illegal generated move")?;
            if ply % 4 == 3 {
                boards.push(b);
            }
        }
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut hits = 0usize;
    let mut misses = 0usize;
    let mut lat = Vec::with_capacity(boards.len());
    let t = Instant::now();
    for (i, b) in boards.iter().enumerate() {
        let start = Instant::now();
        let p = book.probe(b)?;
        lat.push(start.elapsed().as_nanos());
        if let Some(p) = p {
            hits += 1;
            mix(&mut hash, 1);
            mix(&mut hash, p.offset);
            for n in [
                p.stats.games,
                p.stats.wins,
                p.stats.losses,
                p.stats.draws,
                p.stats.raw_score,
                p.stats.rated_games,
                p.stats.rating_sum,
                p.stats.opponent_rated_games,
                p.stats.opponent_rating_sum,
            ] {
                mix(&mut hash, u64::from(n));
            }
            for n in p.stats.raw_tail {
                mix(&mut hash, u64::from(n));
            }
            mix(&mut hash, p.evaluation.map_or(256, |e| u64::from(e.0)));
            mix(&mut hash, p.moves.len() as u64);
            for m in p.moves {
                mix(&mut hash, u64::from(m.byte));
                mix(&mut hash, u64::from(m.annotation));
                let mv = m.mv.map_err(|_| "unknown or illegal move")?;
                mix(&mut hash, u64::from(mv.word()));
            }
        } else {
            if i < positives {
                return Err(format!("sampled positive missed: {i}").into());
            }
            misses += 1;
            mix(&mut hash, 0);
        }
    }
    let seconds = t.elapsed().as_secs_f64();
    lat.sort_unstable();
    let p = |n: usize| lat[(lat.len() - 1) * n / 100];
    assert!(misses > 0);
    println!(
        "{{\"open_seconds\":{open:.9},\"validate_seconds\":{validate:.9},\"index_bytes\":{index_bytes},\"positions\":{},\"positive_samples\":{positives},\"hits\":{hits},\"misses\":{misses},\"fingerprint\":\"{hash:016x}\",\"seconds\":{seconds:.9},\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{}}}",
        count.or_else(|| if let Reader::Adaptive(b) = &book { b.positions() } else { None }).unwrap(),
        p(50),
        p(95),
        p(99)
    );
    Ok(())
}
