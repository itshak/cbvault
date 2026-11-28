//! Exhaustive positive routing oracle for the native CTO/CTB index.
#![allow(missing_docs)]

use cbvault_format::ctg::{Ctg, PAGE_SIZE, cto::NativeDirectory};
use rayon::prelude::*;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: native_cto_audit book.ctg")?;
    let t = Instant::now();
    let ctg = Ctg::open(path)?;
    let directory = NativeDirectory::open(&ctg)?;
    let opened = t.elapsed().as_secs_f64();
    let (slots, low, high) = directory.bounds();
    let leaves = directory.validate_bitmap()?;
    let table = directory.slot_table()?;
    let mut occupied = 0u64;
    let mut seen = vec![false; ctg.pages() as usize];
    let mut duplicate_pages = 0u64;
    for slot in 0..slots {
        if let Some(page) = table.page_at(&directory, slot)? {
            occupied += 1;
            if std::mem::replace(&mut seen[page as usize], true) {
                duplicate_pages += 1;
            }
        }
    }
    let unreferenced = seen[1..].iter().filter(|&&x| !x).count();
    let t = Instant::now();
    let parts: Vec<_> = (1..ctg.pages())
        .into_par_iter()
        .map_init(
            || [0u8; PAGE_SIZE],
            |buf, page| {
                let bytes = match ctg.mapped_page(page) {
                    Some(b) => b,
                    None => {
                        ctg.read_page(page, buf)?;
                        &buf[..]
                    }
                };
                let mut entries = ctg.entries(page, bytes)?;
                let mut total = 0u64;
                let mut missed = 0u64;
                let mut ambiguous = 0u64;
                for entry in entries.by_ref() {
                    let e = entry.map_err(|e| e.into_error(ctg.path()))?;
                    total += 1;
                    let mut matches = 0;
                    for slot in directory.slots_for(e.key) {
                        if table.page_at(&directory, slot)? == Some(page) {
                            matches += 1;
                        }
                    }
                    if matches == 0 {
                        missed += 1;
                    }
                    if matches > 1 {
                        ambiguous += 1;
                    }
                }
                if !entries.closed() {
                    return Err(cbvault_format::error::Error::corrupt(
                        ctg.path(),
                        u64::from(page) * PAGE_SIZE as u64,
                        "unclosed page",
                    ));
                }
                Ok((total, missed, ambiguous))
            },
        )
        .collect::<Vec<cbvault_format::error::Result<(u64, u64, u64)>>>();
    let mut total = 0;
    let mut missed = 0;
    let mut ambiguous = 0;
    for part in parts {
        let (n, m, a) = part?;
        total += n;
        missed += m;
        ambiguous += a;
    }
    println!(
        "{{\"ctb_leaves\":{leaves},\"positions\":{total},\"missed\":{missed},\"ambiguous_routes\":{ambiguous},\"slots\":{slots},\"lower\":{low},\"upper\":{high},\"occupied\":{occupied},\"duplicate_page_slots\":{duplicate_pages},\"unreferenced_pages\":{unreferenced},\"open_seconds\":{opened:.9},\"audit_seconds\":{:.9}}}",
        t.elapsed().as_secs_f64()
    );
    if missed != 0 || ambiguous != 0 || duplicate_pages != 0 || unreferenced != 0 {
        return Err("native lookup directory anomaly".into());
    }
    Ok(())
}
