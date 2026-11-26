//! Compare compact ID-only search with the prior named scan plus ids().
#![allow(missing_docs)]
use cbvault::bridge::{Database, Filter, scan_ids};
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
    let args: Vec<_> = std::env::args().collect();
    let db = Database::open(&args[1])?;
    let filter = match args[2].as_str() {
        "all" => Filter::All,
        "broad" => Filter::EloAtLeast(1800),
        "selective" => Filter::WhiteEloAtLeast(2100).and(Filter::BlackEloAtLeast(2100)),
        _ => return Err("invalid operation".into()),
    };
    let threads = args[3].parse()?;
    ARMED.store(args.get(4).is_some_and(|s| s == "--alloc"), Relaxed);
    let t = Instant::now();
    let result = scan_ids(&db, &filter, threads)?;
    let ids = result.ids();
    let seconds = t.elapsed().as_secs_f64();
    ARMED.store(false, Relaxed);
    let mut digest = 14695981039346656037u64;
    let mut last = 0;
    for &id in ids.iter() {
        assert!(id > last);
        last = id;
        hash(&mut digest, &id.to_le_bytes());
    }
    println!(
        "{{\"count\":{},\"units\":{},\"fingerprint\":\"{digest:016x}\",\"seconds\":{seconds:.9},\"allocation_calls\":{}}}",
        ids.len(),
        result.stats().records,
        ALLOCS.load(Relaxed)
    );
    Ok(())
}
