//! Experimental CTO/CTB directory lookup based on the public-domain CTG
//! specification v0.2, posted by Sesse on 2007-09-30, section 3:
//! <https://web.archive.org/web/20210129162445/http://rybkaforum.net/cgi-bin/rybkaforum/topic_show.pl?tid=2319>
//! Bucket reduction is a hypothesis until checked against actual CTG pages.
use super::Ctg;
use crate::{
    error::{Error, Result},
    file::DbFile,
};

const TABLE: [u32; 64] = [
    0x3100d2bf, 0x3118e3de, 0x34ab1372, 0x2807a847, 0x1633f566, 0x2143b359, 0x26d56488, 0x3b9e6f59, 0x37755656,
    0x3089ca7b, 0x18e92d85, 0x0cd0e9d8, 0x1a9e3b54, 0x3eaa902f, 0x0d9bfaae, 0x2f32b45b, 0x31ed6102, 0x3d3c8398,
    0x146660e3, 0x0f8d4b76, 0x02c77a5f, 0x146c8799, 0x1c47f51f, 0x249f8f36, 0x24772043, 0x1fbc1e4d, 0x1e86b3fa,
    0x37df36a6, 0x16ed30e4, 0x02c3148e, 0x216e5929, 0x0636b34e, 0x317f9f56, 0x15f09d70, 0x131026fb, 0x38c784b1,
    0x29ac3305, 0x2b485dc5, 0x3c049ddc, 0x35a9fbcd, 0x31d5373b, 0x2b246799, 0x0a2923d3, 0x08a96e9d, 0x30031a9f,
    0x08f525b5, 0x33611c06, 0x2409db98, 0x0ca4feb2, 0x1000b71e, 0x30566e32, 0x39447d31, 0x194e3752, 0x08233a95,
    0x0f38fe36, 0x29c7cd57, 0x0f7b3a39, 0x328e8a16, 0x1e7d1388, 0x0fba78f5, 0x274c7e7c, 0x1e8be65c, 0x2fa0b0bb,
    0x1eb6c371,
];

/// Native 32-bit hash of the complete encoded CTG key, including its header.
pub fn native_hash(key: &[u8]) -> u32 {
    let mut state = 0usize;
    let mut sum = 0u32;
    for &byte in key {
        for nibble in [byte & 15, byte >> 4] {
            state = (state + usize::from(15 - nibble) * 4 + 1) & 63;
            sum = sum.wrapping_add(TABLE[state]);
        }
    }
    sum
}

/// Native directory. Its structure is checked at open, but only a full audit
/// establishes that every stored key can be reached. Negative results from
/// this experimental directory must not be treated as authoritative yet.
#[derive(Debug)]
pub struct NativeDirectory {
    file: DbFile,
    slots: u32,
    low: u32,
    high: u32,
    pages: u32,
}

impl NativeDirectory {
    /// Opens companion files; no CTG page scan is performed.
    pub fn open(ctg: &Ctg) -> Result<Self> {
        let file = DbFile::open(ctg.path().with_extension("cto"))?;
        let bounds = DbFile::open(ctg.path().with_extension("ctb"))?;
        let mut header = [0u8; 16];
        file.read_into(0, &mut header)?;
        let mut b = [0u8; 12];
        bounds.read_into(0, &mut b)?;
        let word = |x: &[u8]| u32::from_be_bytes(x.try_into().unwrap());
        let slots = word(&header[4..8]);
        let low = word(&b[4..8]);
        let high = word(&b[8..12]);
        if slots == 0
            || file.size()? != 16 + u64::from(slots) * 4
            || low > high
            || low >= slots
            || high > slots
            || high >= 0x7fff_ffff
        {
            return Err(Error::corrupt(file.path(), 0, "invalid native directory size or bounds"));
        }
        Ok(Self { file, slots, low, high, pages: ctg.pages() })
    }

    /// Slot count and CTB lower/upper bounds.
    pub fn bounds(&self) -> (u32, u32, u32) {
        (self.slots, self.low, self.high)
    }

    /// Candidate slots from the documented hash-reduction hypothesis.
    pub fn slots_for(&self, key: &[u8]) -> impl Iterator<Item = u32> {
        let hash = native_hash(key);
        let low = self.low;
        let high = self.high;
        let slots = self.slots;
        let mut done = false;
        (0..31).filter_map(move |bits| {
            if done {
                return None;
            }
            let mask = (1u32 << bits) - 1;
            let slot = (hash & mask) + mask;
            if slot < low {
                return None;
            }
            if slot >= high {
                done = true;
            }
            (slot < slots).then_some(slot)
        })
    }

    /// A slot's physical CTG page (header page excluded from native numbering).
    pub fn page_at(&self, slot: u32) -> Result<Option<u32>> {
        if slot >= self.slots {
            return Err(Error::corrupt(self.file.path(), 0, "native slot out of bounds"));
        }
        let at = 16 + u64::from(slot) * 4;
        let mut bytes = [0; 4];
        self.file.read_into(at, &mut bytes)?;
        let raw = u32::from_be_bytes(bytes);
        if raw == u32::MAX {
            return Ok(None);
        }
        if raw >= self.pages.saturating_sub(1) {
            return Err(Error::corrupt(self.file.path(), at, "native page out of bounds"));
        }
        Ok(Some(raw + 1))
    }

    /// Whether the actual page of a stored key is among the native candidates.
    pub fn reaches(&self, key: &[u8], page: u32) -> Result<bool> {
        for slot in self.slots_for(key) {
            if self.page_at(slot)? == Some(page) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_hash_known_bytes() {
        assert_eq!(native_hash(&[]), 0);
        // For 0xff the two increments are both 1.
        assert_eq!(native_hash(&[255]), TABLE[1].wrapping_add(TABLE[2]));
        // For 0x00 the increments are both 61.
        assert_eq!(native_hash(&[0]), TABLE[61].wrapping_add(TABLE[58]));
    }
}
