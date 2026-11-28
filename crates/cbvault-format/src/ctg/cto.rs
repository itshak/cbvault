//! Native `.cto`/`.ctb` directory lookup based on the public-domain CTG
//! format specification v0.2 (hash-trie split directory).
//!
//! Exhaustively validated across all tested commercial, Mega-derived, CC0
//! scale, and synthetic `.ctg` books (0 missed routes, 0 ambiguous routes,
//! exact CTB split-trie correspondence).

use std::path::{Path, PathBuf};

use super::{Ctg, PAGE_SIZE};
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

const CTO_MAGIC: u32 = 0x0010_0001;
const CTB_MAGIC: u32 = 0x000c_0001;
const CTO_HEADER_BYTES: u64 = 16;
const CTB_HEADER_BYTES: u64 = 12;
const CHUNK_SLOTS: usize = 16384; // 64 KiB chunk for buffered unmapped scans

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

/// Resolves `.cto` / `.ctb` companion files, trying lowercase first and uppercase
/// on case-sensitive filesystems when lowercase is absent.
fn companion_path(ctg_path: &Path, lower_ext: &str) -> PathBuf {
    let lower = ctg_path.with_extension(lower_ext);
    if lower.exists() {
        return lower;
    }
    let upper = ctg_path.with_extension(lower_ext.to_ascii_uppercase());
    if upper.exists() { upper } else { lower }
}

/// Native directory (`.cto` slot table + `.ctb` split bounds).
///
/// Opening validates headers, exact file sizes, and split bounds without
/// scanning CTG data pages.
#[derive(Debug)]
pub struct NativeDirectory {
    file: DbFile,
    bounds_file: DbFile,
    slots: u32,
    low: u32,
    high: u32,
    pages: u32,
}

/// Borrowed (when memory-mapped) or transient chunk-loaded snapshot of the
/// `.cto` slot array for fast exhaustive validation without per-probe syscalls.
pub enum SlotTable<'a> {
    /// Zero-copy slice into the mapped `.cto` slot table (past the 16-byte header).
    Mapped(&'a [u8]),
    /// Chunk-loaded native-endian slots used when memory-mapping is disabled.
    Buffered(Vec<u32>),
}

impl SlotTable<'_> {
    /// Looks up the physical CTG page referenced by `slot`.
    #[inline]
    pub fn page_at(&self, dir: &NativeDirectory, slot: u32) -> Result<Option<u32>> {
        if slot >= dir.slots {
            return Err(Error::corrupt(dir.file.path(), 0, "native slot out of bounds"));
        }
        let raw = match self {
            Self::Mapped(slots_bytes) => {
                let off = usize::try_from(slot)
                    .ok()
                    .and_then(|i| i.checked_mul(4))
                    .ok_or_else(|| Error::corrupt(dir.file.path(), 0, "native slot offset overflow"))?;
                let bytes = slots_bytes
                    .get(off..off + 4)
                    .ok_or_else(|| Error::corrupt(dir.file.path(), 0, "native slot out of range"))?;
                u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            }
            Self::Buffered(vec) => *vec
                .get(slot as usize)
                .ok_or_else(|| Error::corrupt(dir.file.path(), 0, "short buffered native directory"))?,
        };
        if raw == u32::MAX {
            return Ok(None);
        }
        if raw >= dir.pages.saturating_sub(1) {
            let at = CTO_HEADER_BYTES + u64::from(slot) * 4;
            return Err(Error::corrupt(dir.file.path(), at, "native page out of bounds"));
        }
        Ok(Some(raw + 1))
    }

    /// Whether `page` is among the candidate slots for `key`.
    #[inline]
    pub fn reaches(&self, dir: &NativeDirectory, key: &[u8], page: u32) -> Result<bool> {
        for slot in dir.slots_for(key) {
            if self.page_at(dir, slot)? == Some(page) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Validates raw `.cto` (16-byte) and `.ctb` (12-byte) headers and file sizes,
/// returning `(slots, low, high)`.
pub fn validate_headers(
    cto_header: &[u8; 16],
    cto_size: u64,
    ctb_header: &[u8; 12],
    ctb_size: u64,
) -> std::result::Result<(u32, u32, u32), &'static str> {
    let word = |x: &[u8]| u32::from_be_bytes([x[0], x[1], x[2], x[3]]);
    let slots = word(&cto_header[4..8]);
    let low = word(&ctb_header[4..8]);
    let high = word(&ctb_header[8..12]);
    let expected_cto = u64::from(slots).checked_mul(4).and_then(|n| n.checked_add(CTO_HEADER_BYTES));
    let expected_ctb = u64::from(high.div_ceil(8)).checked_add(CTB_HEADER_BYTES);
    if word(&cto_header[..4]) != CTO_MAGIC
        || word(&ctb_header[..4]) != CTB_MAGIC
        || slots == 0
        || expected_cto != Some(cto_size)
        || low > high
        || low >= slots
        || high >= 0x7fff_ffff
    {
        return Err("invalid native directory size or bounds");
    }
    if expected_ctb != Some(ctb_size) {
        return Err("invalid native bounds bitmap size");
    }
    Ok((slots, low, high))
}

/// Validates complete in-memory `.cto` and `.ctb` buffers against `pages`
/// total `.ctg` pages (header included).
pub fn validate_directory_bytes(
    pages: u32,
    cto: &[u8],
    ctb: &[u8],
) -> std::result::Result<(u32, u32, u32), &'static str> {
    let cto_hdr: &[u8; 16] = cto.get(..16).and_then(|s| s.try_into().ok()).ok_or("short cto header")?;
    let ctb_hdr: &[u8; 12] = ctb.get(..12).and_then(|s| s.try_into().ok()).ok_or("short ctb header")?;
    let (slots, low, high) = validate_headers(cto_hdr, cto.len() as u64, ctb_hdr, ctb.len() as u64)?;
    let data_pages = pages.saturating_sub(1);
    let mut seen = vec![false; pages as usize];
    for (i, chunk) in cto[16..].as_chunks::<4>().0.iter().enumerate() {
        let slot = i as u32;
        let raw = u32::from_be_bytes(*chunk);
        if raw == u32::MAX {
            continue;
        }
        if slot < low {
            return Err("occupied native slot below lower bound");
        }
        if raw >= data_pages {
            return Err("native page out of bounds");
        }
        let page = raw + 1;
        if std::mem::replace(&mut seen[page as usize], true) {
            return Err("duplicate native page slot");
        }
    }
    if seen.get(1..).is_some_and(|s| s.iter().any(|&x| !x)) {
        return Err("unreferenced native data page");
    }
    Ok((slots, low, high))
}

impl NativeDirectory {
    /// Opens `.cto` and `.ctb` companions; no CTG data page scan is performed.
    pub fn open(ctg: &Ctg) -> Result<Self> {
        let file = DbFile::open_with_mode(companion_path(ctg.path(), "cto"), ctg.read_mode())?;
        let bounds = DbFile::open_with_mode(companion_path(ctg.path(), "ctb"), ctg.read_mode())?;
        let mut header = [0u8; 16];
        file.read_into(0, &mut header)?;
        let mut b = [0u8; 12];
        bounds.read_into(0, &mut b)?;
        let (slots, low, high) = validate_headers(&header, file.size()?, &b, bounds.size()?)
            .map_err(|msg| Error::corrupt(file.path(), 0, msg))?;
        Ok(Self { file, bounds_file: bounds, slots, low, high, pages: ctg.pages() })
    }

    /// Returns `(slots, low, high)` directory geometry.
    pub fn bounds(&self) -> (u32, u32, u32) {
        (self.slots, self.low, self.high)
    }

    /// Candidate slot indices in level-order trie traversal between `low` and `high`.
    #[inline]
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

    /// A slot's physical CTG page (1-based; header page excluded from native numbering).
    #[inline]
    pub fn page_at(&self, slot: u32) -> Result<Option<u32>> {
        if slot >= self.slots {
            return Err(Error::corrupt(self.file.path(), 0, "native slot out of bounds"));
        }
        let at = CTO_HEADER_BYTES + u64::from(slot) * 4;
        let raw = if let Some(bytes) = self.file.slice_at(at, 4) {
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            let mut bytes = [0; 4];
            self.file.read_into(at, &mut bytes)?;
            u32::from_be_bytes(bytes)
        };
        if raw == u32::MAX {
            return Ok(None);
        }
        if raw >= self.pages.saturating_sub(1) {
            return Err(Error::corrupt(self.file.path(), at, "native page out of bounds"));
        }
        Ok(Some(raw + 1))
    }

    fn for_each_raw_slot(&self, mut f: impl FnMut(u32, u32) -> Result<()>) -> Result<()> {
        if let Some(bytes) = self.file.as_slice() {
            let slots_bytes = bytes
                .get(CTO_HEADER_BYTES as usize..)
                .ok_or_else(|| Error::corrupt(self.file.path(), 0, "short native directory"))?;
            let (chunks, rem) = slots_bytes.as_chunks::<4>();
            if !rem.is_empty() || chunks.len() != self.slots as usize {
                return Err(Error::corrupt(self.file.path(), 0, "native directory length mismatch"));
            }
            for (i, chunk) in chunks.iter().enumerate() {
                f(i as u32, u32::from_be_bytes(*chunk))?;
            }
            return Ok(());
        }
        let mut buf = vec![0u8; CHUNK_SLOTS * 4];
        let mut done = 0u32;
        while done < self.slots {
            let take = (self.slots - done).min(CHUNK_SLOTS as u32) as usize;
            let slice = &mut buf[..take * 4];
            self.file.read_into(CTO_HEADER_BYTES + u64::from(done) * 4, slice)?;
            for (j, chunk) in slice.as_chunks::<4>().0.iter().enumerate() {
                f(done + j as u32, u32::from_be_bytes(*chunk))?;
            }
            done += take as u32;
        }
        Ok(())
    }

    /// Provides a borrowed or chunk-buffered view of the entire slot table.
    pub fn slot_table(&self) -> Result<SlotTable<'_>> {
        if let Some(bytes) = self.file.as_slice() {
            let slots_bytes = bytes
                .get(CTO_HEADER_BYTES as usize..)
                .ok_or_else(|| Error::corrupt(self.file.path(), 0, "short native directory"))?;
            return Ok(SlotTable::Mapped(slots_bytes));
        }
        let mut table = Vec::new();
        table
            .try_reserve_exact(self.slots as usize)
            .map_err(|_| Error::corrupt(self.file.path(), 0, "native slot table allocation failed"))?;
        self.for_each_raw_slot(|_, raw| {
            table.push(raw);
            Ok(())
        })?;
        Ok(SlotTable::Buffered(table))
    }

    /// Checks that every directory page is in range, uniquely referenced, no
    /// occupied slot sits below `low`, and every data page `1..pages` has a slot.
    pub fn validate_pages(&self) -> Result<()> {
        let data_pages = self.pages.saturating_sub(1);
        let mut seen = vec![false; self.pages as usize];
        self.for_each_raw_slot(|slot, raw| {
            if raw == u32::MAX {
                return Ok(());
            }
            let at = CTO_HEADER_BYTES + u64::from(slot) * 4;
            if slot < self.low {
                return Err(Error::corrupt(self.file.path(), at, "occupied native slot below lower bound"));
            }
            if raw >= data_pages {
                return Err(Error::corrupt(self.file.path(), at, "native page out of bounds"));
            }
            let page = raw + 1;
            if std::mem::replace(&mut seen[page as usize], true) {
                return Err(Error::corrupt(self.file.path(), at, "duplicate native page slot"));
            }
            Ok(())
        })?;

        if seen[1..].iter().any(|&x| !x) {
            return Err(Error::corrupt(self.file.path(), 0, "unreferenced native data page"));
        }
        let _ = PAGE_SIZE;
        Ok(())
    }

    /// Audit the CTB split bitmap against occupied CTO leaves.
    ///
    /// This additional strict check is opt-in: native routing itself uses the
    /// validated bounds and exact CTG keys, not the bitmap. Walk only reachable
    /// trie nodes; a leaf beyond the bitmap range is implicit. Require every
    /// leaf to reference a page and every occupied CTO slot to be a leaf.
    pub fn validate_bitmap(&self) -> Result<u64> {
        self.validate_pages()?;
        let table = self.slot_table()?;
        let len = self.high.div_ceil(8) as usize;
        let mut scratch = Vec::new();
        let bitmap = if let Some(bytes) = self.bounds_file.slice_at(CTB_HEADER_BYTES, len) {
            bytes
        } else {
            scratch
                .try_reserve_exact(len)
                .map_err(|_| Error::corrupt(self.bounds_file.path(), 0, "CTB bitmap allocation failed"))?;
            scratch.resize(len, 0);
            self.bounds_file.read_into(CTB_HEADER_BYTES, &mut scratch)?;
            &scratch
        };
        let bad = |slot: u32, detail: &str| {
            Error::corrupt(self.bounds_file.path(), CTB_HEADER_BYTES + u64::from(slot / 8), detail)
        };
        // Depth-first traversal requires at most 32 pending nodes, regardless
        // of directory size. Each child occupies a strictly deeper level.
        let mut stack = [(0u32, 0u32); 32];
        let mut pending = 1;
        let mut leaves = 0u64;
        while pending != 0 {
            pending -= 1;
            let (slot, depth) = stack[pending];
            if slot >= self.slots {
                return Err(bad(slot, "CTB trie node outside CTO slots"));
            }
            let split = slot < self.high && bitmap.get((slot / 8) as usize).is_some_and(|b| b & (1 << (slot % 8)) != 0);
            let page = table.page_at(self, slot)?;
            if split {
                if page.is_some() || depth >= 30 || pending + 2 > stack.len() {
                    return Err(bad(slot, "invalid occupied or overflowing CTB split"));
                }
                let step = 1u32 << depth;
                stack[pending] = (slot + step, depth + 1);
                stack[pending + 1] = (slot + 2 * step, depth + 1);
                pending += 2;
            } else {
                if page.is_none() {
                    return Err(bad(slot, "CTB leaf has no CTO page"));
                }
                leaves += 1;
            }
        }
        if leaves != u64::from(self.pages.saturating_sub(1)) {
            return Err(bad(0, "CTB leaves do not cover all CTO pages"));
        }
        Ok(leaves)
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
