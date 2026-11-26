//! First-hit latency, complete fingerprint and cancellation workload.
#![allow(missing_docs)]
use cbvault::bridge::{Database, PositionQuery, for_each_position_key};
use gigachess::Board;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let db = Database::open(&args[1])?;
    let threads = args[2].parse::<usize>()?;
    let limit = args[3].parse::<u64>()?;
    let flag = AtomicBool::new(false);
    let cancel = || flag.load(Ordering::Relaxed);
    let query = PositionQuery::of(Board::startpos().zobrist()).cancel_with(&cancel);
    let mut count = 0u64;
    let mut hash = 0xcbf29ce484222325u64;
    let mut first = None;
    let t = Instant::now();
    let r = for_each_position_key(&db, &query, threads, |h| {
        first.get_or_insert_with(|| t.elapsed().as_nanos());
        count += 1;
        for v in [u64::from(h.game), u64::from(h.ply), h.key] {
            hash = (hash ^ v).wrapping_mul(1099511628211);
        }
        if count == limit {
            flag.store(true, Ordering::Relaxed);
        }
    })?;
    assert_eq!(count, r.hits);
    println!(
        "{{\"games\":{},\"plies\":{},\"hits\":{},\"failures\":{},\"complete\":{},\"fingerprint\":\"{hash:016x}\",\"seconds\":{:.9},\"first_hit_ns\":{}}}",
        r.games,
        r.plies,
        r.hits,
        r.failures,
        r.complete,
        t.elapsed().as_secs_f64(),
        first.unwrap_or(0)
    );
    Ok(())
}
