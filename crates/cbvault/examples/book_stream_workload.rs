//! Full CTG stream workload: raw traversal vs independent physical page loop.
use cbvault::book::{BookStream, ReadMode, StreamEvent, StreamOptions};
use cbvault_format::ctg::{Ctg, Entry, PAGE_SIZE};
use std::time::Instant;

fn accumulate(e: Entry<'_>, n: &mut u64, moves: &mut u64, sig: &mut u64) {
    *n += 1;
    *moves += (e.moves.len() / 2) as u64;
    for &b in e.key.iter().chain(e.moves).chain(e.stats) {
        *sig = (*sig ^ u64::from(b)).wrapping_mul(1099511628211);
    }
    *sig = (*sig ^ e.offset).wrapping_mul(1099511628211);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("book path required")?;
    let op = args.next().ok_or("oracle, raw, or decoded")?;
    let mode = match args.next().as_deref() {
        Some("mapped") => ReadMode::MappedPreferred,
        Some("positioned") => ReadMode::Positioned,
        _ => return Err("mapped or positioned required".into()),
    };
    let mut n = 0;
    let mut moves = 0;
    let mut sig = 14695981039346656037u64;
    let start = Instant::now();
    if op == "oracle" {
        let ctg = Ctg::open_with_mode(&path, mode)?;
        let mut scratch = [0u8; PAGE_SIZE];
        for page in 1..ctg.pages() {
            let bytes = if let Some(bytes) = ctg.mapped_page(page) {
                bytes
            } else {
                ctg.read_page(page, &mut scratch)?;
                &scratch[..]
            };
            let mut entries = ctg.entries(page, bytes)?;
            for e in entries.by_ref() {
                let e = e.map_err(|e| e.into_error(ctg.path()))?;
                accumulate(e, &mut n, &mut moves, &mut sig);
            }
            assert!(entries.closed());
        }
    } else {
        assert!(op == "raw" || op == "decoded");
        let stream = BookStream::open_with_mode(&path, mode)?;
        let report = stream.visit(&StreamOptions { decode: op == "decoded", ..Default::default() }, |event| {
            match event {
                StreamEvent::Raw(e) => accumulate(e, &mut n, &mut moves, &mut sig),
                StreamEvent::Decoded { raw, position, .. } => {
                    assert_eq!(position.moves.len(), raw.moves.len() / 2);
                    accumulate(raw, &mut n, &mut moves, &mut sig);
                }
                _ => panic!("unexpected stream event"),
            }
            true
        })?;
        assert!(report.complete());
        assert_eq!(report.delivered, n);
    }
    println!(
        "{{\"entries\":{n},\"moves\":{moves},\"fingerprint\":\"{sig:016x}\",\"seconds\":{:.9}}}",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
