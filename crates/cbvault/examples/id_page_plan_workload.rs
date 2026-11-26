//! Repeated page requests compared with one-shot exact-count pages.
#![allow(missing_docs)]
use cbvault::bridge::{Database, Filter, SearchContext};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let db = Database::open(&args[1])?;
    let ctx = SearchContext::new(args[2].parse()?)?;
    let filter = match args[3].as_str() {
        "all" => Filter::All,
        "broad" => Filter::EloAtLeast(1800),
        "selective" => Filter::WhiteEloAtLeast(2100).and(Filter::BlackEloAtLeast(2100)),
        _ => return Err("unknown filter".into()),
    };
    let mode = &args[4];
    let start = Instant::now();
    let plan = if mode == "plan" { Some(ctx.prepare_id_pages(&db, &filter)?) } else { None };
    let setup = start.elapsed().as_secs_f64();
    let mut hash = 14695981039346656037u64;
    let mut count = 0;
    let mut total = None;
    let mut lat = Vec::new();
    let t = Instant::now();
    for i in 0..48u64 {
        let offset = if i < 24 { i * 100 } else { u64::from(db.records()) * (i - 24) / 24 };
        let begin = Instant::now();
        let page =
            if let Some(plan) = &plan { plan.page(offset, 100)? } else { ctx.scan_id_page(&db, &filter, offset, 100)? };
        lat.push(begin.elapsed().as_nanos());
        let n = page.stats().matches;
        assert_eq!(*total.get_or_insert(n), n);
        assert_eq!(page.ids().len() as u64, 100.min(n.saturating_sub(offset)));
        count += page.ids().len();
        for id in page.ids() {
            hash = (hash ^ u64::from(*id)).wrapping_mul(1099511628211);
        }
    }
    let seconds = t.elapsed().as_secs_f64();
    lat.sort_unstable();
    println!(
        "{{\"setup_seconds\":{setup:.9},\"seconds\":{seconds:.9},\"p50_ns\":{},\"p95_ns\":{},\"count\":{count},\"total\":{},\"fingerprint\":\"{hash:016x}\"}}",
        lat[23],
        lat[44],
        total.unwrap_or(0)
    );
    Ok(())
}
