//! Named scan timing, with full result validation outside the timed interval.
#![allow(missing_docs)]
use cbvault::bridge::{Database, Filter, scan};
use std::time::Instant;

fn hash(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h = (*h ^ u64::from(b)).wrapping_mul(1099511628211);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let db = Database::open(&a[1])?;
    let threads = a[2].parse()?;
    let filter = match a[3].as_str() {
        "broad" => Filter::EloAtLeast(1800),
        "selective" => Filter::WhiteEloAtLeast(2100).and(Filter::BlackEloAtLeast(2100)),
        _ => return Err("unknown filter".into()),
    };
    let t = Instant::now();
    let found = scan(&db, &filter, threads)?;
    let seconds = t.elapsed().as_secs_f64();
    let mut digest = 14695981039346656037u64;
    let mut prev = 0;
    for m in found.matches() {
        assert!(m.id > prev);
        prev = m.id;
        hash(&mut digest, &m.id.to_le_bytes());
        hash(&mut digest, m.header.bytes());
        for name in [&m.white, &m.black, &m.event] {
            for bytes in [name.last(), name.first()] {
                hash(&mut digest, &(bytes.len() as u64).to_le_bytes());
                hash(&mut digest, bytes);
            }
        }
    }
    println!(
        "{{\"seconds\":{seconds:.9},\"count\":{},\"units\":{},\"fingerprint\":\"{digest:016x}\"}}",
        found.len(),
        found.stats().records
    );
    Ok(())
}
