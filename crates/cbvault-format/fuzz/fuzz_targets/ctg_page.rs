#![no_main]

use cbvault_format::ctg::{Entries, PAGE_SIZE, position};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < PAGE_SIZE {
        return;
    }
    let page = &data[..PAGE_SIZE];
    if let Ok(mut entries) = Entries::parse(page, 1) {
        for entry in entries.by_ref() {
            let Ok(entry) = entry else {
                break;
            };
            let _ = entry.stats();
            let _ = entry.move_pairs().count();
            let _ = position::decode(entry.key);
        }
    }
});
