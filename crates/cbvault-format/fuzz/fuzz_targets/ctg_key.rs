#![no_main]

use cbvault_format::ctg::position;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(raw) = position::decode(data) {
        let mut buf = [0u8; position::MAX_ENCODED];
        if let Some(len) = position::encode(&raw, &mut buf) {
            assert!(len <= position::MAX_ENCODED);
            assert!(position::decode(&buf[..len]).is_ok());
        }
    }
});
