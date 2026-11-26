//! Compare exact-count pages to materializing every matching ID.
#![allow(missing_docs)]
use cbvault::bridge::{Database, Filter, SearchContext};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let db = Database::open(&a[1])?;
    let threads = a[2].parse()?;
    let filter = match a[3].as_str() {
        "all" => Filter::All,
        "broad" => Filter::EloAtLeast(1800),
        "selective" => Filter::WhiteEloAtLeast(2100).and(Filter::BlackEloAtLeast(2100)),
        _ => return Err("unknown filter".into()),
    };
    let offset: u64 = a[4].parse()?;
    let limit: usize = a[5].parse()?;
    let ctx = SearchContext::new(threads)?;
    let t = Instant::now();
    let (stats, ids) = match a[6].as_str() {
        "full" => {
            let all = ctx.scan_ids(&db, &filter)?;
            let start = offset.min(all.ids().len() as u64) as usize;
            let end = start.saturating_add(limit).min(all.ids().len());
            (all.stats(), all.ids()[start..end].to_vec())
        }
        "page" => {
            let page = ctx.scan_id_page(&db, &filter, offset, limit)?;
            (page.stats(), page.ids().to_vec())
        }
        _ => return Err("unknown mode".into()),
    };
    let seconds = t.elapsed().as_secs_f64();
    let mut digest = 14695981039346656037u64;
    for id in &ids {
        digest = (digest ^ u64::from(*id)).wrapping_mul(1099511628211);
    }
    println!(
        "{{\"seconds\":{seconds:.9},\"count\":{},\"total\":{},\"records\":{},\"fingerprint\":\"{digest:016x}\"}}",
        ids.len(),
        stats.matches,
        stats.records
    );
    Ok(())
}
