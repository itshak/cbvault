#![no_main]

use cbvault_format::ctg::cto::{native_hash, validate_directory_bytes};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = native_hash(data);
    if data.len() >= 32 {
        let pages = (u32::from(data[0]) % 32).saturating_add(1);
        let split = 16 + ((usize::from(data[1]) * 4) % (data.len() - 16).max(1));
        if let Some((cto, ctb)) = data.get(2..).and_then(|s| s.split_at_checked(split.min(s.len()))) {
            let _ = validate_directory_bytes(pages, cto, ctb);
        }
    }
});
