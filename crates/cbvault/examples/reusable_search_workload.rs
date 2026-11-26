//! Paired repeated metadata requests with/without a reusable worker pool.
#![allow(missing_docs)]
use cbvault::bridge::{Database, Filter, SearchContext, scan_ids_range};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let db = Database::open(&args[1])?;
    let threads = args[2].parse::<usize>()?;
    let width = args[3].parse::<u32>()?.min(db.records());
    let reuse = args[4] == "reuse";
    let repeats = args[5].parse::<u32>()?;
    let t = Instant::now();
    let ctx = if reuse { Some(SearchContext::new(threads)?) } else { None };
    let setup = t.elapsed().as_secs_f64();
    let mut hash = 0xcbf29ce484222325u64;
    let mut count = 0u64;
    let mut lat = Vec::with_capacity(repeats as usize);
    let mut state = 0x12345678u64;
    for i in 0..repeats {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let first = 1 + ((state >> 32) % (u64::from(db.records() - width) + 1)) as u32;
        let last = first + width - 1;
        let filter = match i % 3 {
            0 => Filter::All,
            1 => Filter::EloAtLeast(1800),
            _ => Filter::WhiteEloAtLeast(2100).and(Filter::BlackEloAtLeast(2100)),
        };
        let t = Instant::now();
        let r = if let Some(ctx) = &ctx {
            ctx.scan_ids_range(&db, first, last, &filter)?
        } else {
            scan_ids_range(&db, first, last, &filter, threads)?
        };
        lat.push(t.elapsed().as_nanos());
        assert_eq!(r.stats().records, u64::from(width));
        let mut prev = 0;
        for &id in r.ids() {
            assert!(id >= first && id <= last && id > prev);
            prev = id;
            hash = (hash ^ u64::from(id)).wrapping_mul(1099511628211);
        }
        count += r.ids().len() as u64;
        hash = (hash ^ count).wrapping_mul(1099511628211);
    }
    let seconds = lat.iter().sum::<u128>() as f64 / 1e9;
    lat.sort_unstable();
    let p = |n| lat[(lat.len() - 1) * n / 100];
    println!(
        "{{\"count\":{count},\"fingerprint\":\"{hash:016x}\",\"seconds\":{seconds:.9},\"setup_seconds\":{setup:.9},\"p50_ns\":{},\"p95_ns\":{}}}",
        p(50usize),
        p(95usize)
    );
    Ok(())
}
