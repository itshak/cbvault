//! Reproducible native database workloads, shared by baseline and candidate.
#![allow(missing_docs)]

use cbvault::bridge::{Database, Filter, GameBuf, scan};
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

fn hash(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h = (*h ^ u64::from(*b)).wrapping_mul(1099511628211);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let db = Database::open(&a[1])?;
    let games: u32 = a[2].parse()?;
    assert!(games > 0);
    let op = &a[3];
    let threads: usize = a[4].parse()?;
    let alloc = a.get(5).is_some_and(|x| x == "--alloc");
    let sample = 2048;
    let mut lat = Vec::with_capacity(sample);
    let mut buf = GameBuf::with_capacity(512);
    let mut seed = 0x58b0a13d_u64;
    let mut ids = Vec::with_capacity(sample);
    for _ in 0..sample {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ids.push(1 + ((seed >> 32) % u64::from(games)) as u32);
    }
    let mut digest = 14695981039346656037u64;
    let mut count = 0u64;
    let mut units = 0u64;
    if alloc {
        ARMED.store(true, Relaxed);
    }
    let start = Instant::now();
    match op.as_str() {
        "headers" => {
            count = db.game_count()?;
            hash(&mut digest, &count.to_le_bytes());
        }
        "broad" | "selective" => {
            let filter = if op == "broad" {
                Filter::EloAtLeast(1800)
            } else {
                Filter::WhiteEloAtLeast(2100).and(Filter::BlackEloAtLeast(2100))
            };
            let result = scan(&db, &filter, threads)?;
            count = result.len() as u64;
            units = result.stats().records;
            let mut last = 0;
            for m in result.matches() {
                assert!(m.id > last, "unordered or duplicate search result");
                last = m.id;
                hash(&mut digest, &m.id.to_le_bytes());
            }
            std::hint::black_box(&result);
        }
        "random-moves" | "random-pgn" | "random-pgn-noann" => {
            for id in ids {
                let t = Instant::now();
                if op == "random-moves" {
                    let g = db.game_with(id, &mut buf, false)?;
                    lat.push(t.elapsed().as_nanos());
                    units += g.moves.len() as u64;
                    hash(&mut digest, &id.to_le_bytes());
                    for m in g.moves {
                        hash(&mut digest, &m.to_le_bytes());
                    }
                } else {
                    let pgn = db.game_pgn_with(id, &mut buf, op == "random-pgn")?;
                    lat.push(t.elapsed().as_nanos());
                    units += pgn.len() as u64;
                    hash(&mut digest, &id.to_le_bytes());
                    hash(&mut digest, pgn.as_bytes());
                }
                count += 1;
            }
        }
        _ => return Err("unknown workload".into()),
    }
    let elapsed = start.elapsed().as_secs_f64();
    ARMED.store(false, Relaxed);
    lat.sort_unstable();
    let pct = |p: usize| lat.get((lat.len().saturating_sub(1)) * p / 100).copied().unwrap_or(0);
    println!(
        "{{\"count\":{count},\"units\":{units},\"fingerprint\":\"{digest:016x}\",\"seconds\":{elapsed:.9},\"p50_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"allocation_calls\":{}}}",
        pct(50),
        pct(95),
        pct(99),
        ALLOCS.load(Relaxed)
    );
    Ok(())
}
