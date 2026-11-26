//! Deterministic CTG lookup and child lookup workload.
#![allow(missing_docs)]
use cbvault::book::Book;
use gigachess::Board;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

struct CountAlloc;
static ARMED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ARMED.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOC: CountAlloc = CountAlloc;

fn hash(h: &mut u64, data: &[u8]) {
    for b in data {
        *h = (*h ^ u64::from(*b)).wrapping_mul(1099511628211);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let measure_alloc = args.iter().any(|s| s == "--alloc");
    ARMED.store(measure_alloc, Relaxed);
    let t = Instant::now();
    let book = Book::open(&args[1])?;
    let open_seconds = t.elapsed().as_secs_f64();
    ARMED.store(false, Relaxed);
    let open_allocations = ALLOCS.swap(0, Relaxed);
    assert!(book.damaged().is_empty());
    let mut seed = 0x58b0a13d_u64;
    let mut fingerprint = 14695981039346656037u64;
    let mut lat = Vec::with_capacity(32768);
    let mut hits = 0u64;
    let mut misses = 0u64;
    let mut children = 0u64;
    ARMED.store(measure_alloc, Relaxed);
    let start = Instant::now();
    for _ in 0..2048 {
        let mut board = Board::startpos();
        for _ in 0..16 {
            let t = Instant::now();
            let position = book.probe(&board)?;
            lat.push(t.elapsed().as_nanos());
            let Some(p) = position else {
                misses += 1;
                hash(&mut fingerprint, &[0]);
                break;
            };
            hits += 1;
            hash(&mut fingerprint, &[1]);
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
                hash(&mut fingerprint, &n.to_le_bytes());
            }
            hash(&mut fingerprint, &p.stats.raw_tail);
            hash(&mut fingerprint, &(p.moves.len() as u64).to_le_bytes());
            for m in &p.moves {
                hash(&mut fingerprint, &[m.byte, m.annotation]);
                let mv = m.mv.map_err(|_| "invalid book move")?;
                hash(&mut fingerprint, &mv.word().to_le_bytes());
            }
            if p.moves.is_empty() {
                break;
            }
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mv = p.moves[((seed >> 32) % p.moves.len() as u64) as usize].mv.map_err(|_| "invalid book move")?;
            // A real opening explorer needs each child's statistics,
            // not the child's decoded continuation moves.
            for m in &p.moves {
                let candidate = m.mv.map_err(|_| "invalid book move")?;
                let child = book.probe_stats_after(&board, candidate)?;
                if let Some(c) = child {
                    children += 1;
                    hash(&mut fingerprint, &[1]);
                    for n in [
                        c.stats.games,
                        c.stats.wins,
                        c.stats.losses,
                        c.stats.draws,
                        c.stats.raw_score,
                        c.stats.rated_games,
                        c.stats.rating_sum,
                        c.stats.opponent_rated_games,
                        c.stats.opponent_rating_sum,
                    ] {
                        hash(&mut fingerprint, &n.to_le_bytes());
                    }
                    hash(&mut fingerprint, &c.stats.raw_tail);
                    hash(&mut fingerprint, &(c.continuation_count as u64).to_le_bytes());
                    hash(&mut fingerprint, &c.offset.to_le_bytes());
                } else {
                    hash(&mut fingerprint, &[0]);
                }
            }
            board.play(mv).map_err(|_| "illegal book move")?;
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    ARMED.store(false, Relaxed);
    let probe_allocations = ALLOCS.load(Relaxed);
    lat.sort_unstable();
    let pct = |p: usize| lat[(lat.len() - 1) * p / 100];
    assert!(hits >= 2048, "book must contain the initial position");
    println!(
        "{{\"positions\":{},\"pages\":{},\"index_bytes\":{},\"open_seconds\":{open_seconds:.9},\"seconds\":{seconds:.9},\"hits\":{hits},\"misses\":{misses},\"children\":{children},\"fingerprint\":\"{fingerprint:016x}\",\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"open_allocations\":{open_allocations},\"probe_allocations\":{probe_allocations}}}",
        book.positions(),
        book.pages(),
        book.index_bytes(),
        pct(50),
        pct(95),
        pct(99)
    );
    Ok(())
}
