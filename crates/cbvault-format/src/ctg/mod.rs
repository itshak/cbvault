//! ChessBase opening books: the `.ctg` file, read page by page.
//!
//! Original cbvault code, clean-room: the layout was established by probing a
//! real book, not taken from any implementation (see
//! `docs/research/ctg-observations-2026-11-14.md` and `docs/provenance.md`).
//!
//! A book is a set of files sharing a base name. Only the `.ctg` is read:
//!
//! - **`.ctg`**: 4096-byte pages. Page 0 is a header (not decoded). Every other
//!   page starts with a big-endian `u16` entry count and a `u16` count of bytes
//!   used, followed by entries back to back.
//! - **`.cto`** and **`.ctb`**: native nibble-hash directory and split-trie
//!   bounds (see [`cto::NativeDirectory`]), enabling millisecond opens and
//!   exhaustive validation without a full `.ctg` scan when companions are intact.
//!
//! An entry is an encoded position ([`position`]), a move section (a length
//! byte, then two bytes per move: [`moves::move_code`] and an annotation byte)
//! and a 33-byte [`stats::Stats`] block.

pub mod colour;
pub mod cto;
pub mod evaluation;
pub mod moves;
pub mod position;
pub mod stats;

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::file::DbFile;

pub use colour::{Colour, colour};
pub use evaluation::Evaluation;
pub use moves::{Annotation, MoveCode, move_code};
pub use position::{Kind, Man, RawPosition};
pub use stats::{DONT_PLAY, MAIN_MOVE, STATS_LEN, Stats};

/// Size of a `.ctg` page.
pub const PAGE_SIZE: usize = 4096;
/// Size of a data page's header: entry count and bytes used.
const PAGE_HEADER: usize = 4;

/// The `.ctg` file of a book.
#[derive(Debug)]
pub struct Ctg {
    file: DbFile,
    pages: u32,
}

/// The `.ctg` path for `path`: the path itself when it already ends in `.ctg`
/// (any case), otherwise the base name with `.ctg` appended.
pub fn ctg_path(path: &Path) -> PathBuf {
    let is_ctg = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("ctg"));
    if is_ctg {
        return path.to_path_buf();
    }
    let mut s = path.as_os_str().to_owned();
    s.push(".ctg");
    PathBuf::from(s)
}

impl Ctg {
    /// Opens the `.ctg` of a book (pass the `.ctg` itself or the base name).
    pub fn open(path: impl AsRef<Path>) -> Result<Ctg> {
        let path = ctg_path(path.as_ref());
        let file = DbFile::open(path)?;
        let size = file.size()?;
        if size == 0 || size % PAGE_SIZE as u64 != 0 {
            return Err(Error::corrupt(
                file.path(),
                size,
                format!("size {size} is not a whole number of 4096-byte pages"),
            ));
        }
        let pages = u32::try_from(size / PAGE_SIZE as u64)
            .map_err(|_| Error::corrupt(file.path(), 0, "more pages than a u32 counts"))?;
        Ok(Ctg { file, pages })
    }

    /// The file's path.
    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Number of pages, the header page included.
    pub fn pages(&self) -> u32 {
        self.pages
    }

    /// Reads data page `number` (1-based: page 0 is the header) into `buf`.
    pub fn read_page(&self, number: u32, buf: &mut [u8; PAGE_SIZE]) -> Result<()> {
        if number == 0 || number >= self.pages {
            return Err(Error::corrupt(self.path(), 0, format!("no data page {number}")));
        }
        self.file.read_into(u64::from(number) * PAGE_SIZE as u64, buf)
    }

    /// Borrows data page `number` straight from the mapping, when the file is
    /// memory-mapped.
    pub fn mapped_page(&self, number: u32) -> Option<&[u8]> {
        if number == 0 || number >= self.pages {
            return None;
        }
        self.file.slice_at(u64::from(number) * PAGE_SIZE as u64, PAGE_SIZE)
    }

    /// The entries of a data page, checked against its header.
    pub fn entries<'a>(&self, number: u32, page: &'a [u8]) -> Result<Entries<'a>> {
        Entries::new(page, self.page_offset(number))
            .map_err(|(at, detail)| Error::corrupt(self.path(), self.page_offset(number) + at as u64, detail))
    }

    fn page_offset(&self, number: u32) -> u64 {
        u64::from(number) * PAGE_SIZE as u64
    }
}

/// One entry of a data page, borrowed from the page.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    /// The encoded position, header byte included.
    pub key: &'a [u8],
    /// The moves: pairs of (move byte, annotation byte).
    pub moves: &'a [u8],
    /// The statistics block.
    pub stats: &'a [u8; STATS_LEN],
    /// The entry's byte offset in the `.ctg`, for diagnostics.
    pub offset: u64,
}

impl<'a> Entry<'a> {
    /// The parsed statistics.
    pub fn stats(&self) -> Stats {
        Stats::parse(self.stats)
    }

    /// The moves as (move byte, annotation byte) pairs.
    pub fn move_pairs(&self) -> impl Iterator<Item = (u8, u8)> + 'a {
        self.moves.as_chunks::<2>().0.iter().map(|&[byte, annotation]| (byte, annotation))
    }
}

/// The entries of one page.
#[derive(Clone, Debug)]
pub struct Entries<'a> {
    page: &'a [u8],
    at: usize,
    left: u16,
    base: u64,
}

impl<'a> Entries<'a> {
    /// Parses entries from a raw `.ctg` data page slice (`page_number` only
    /// sets reported entry byte offsets).
    pub fn parse(page: &'a [u8], page_number: u32) -> std::result::Result<Entries<'a>, (usize, String)> {
        Self::new(page, u64::from(page_number) * PAGE_SIZE as u64)
    }

    /// Checks the page header: the bytes-used field must fit the page. A
    /// malformed entry later on ends the walk with an error item.
    fn new(page: &'a [u8], base: u64) -> std::result::Result<Entries<'a>, (usize, String)> {
        if page.len() < PAGE_HEADER {
            return Err((0, "page shorter than its header".into()));
        }
        let count = u16::from_be_bytes([page[0], page[1]]);
        let used = usize::from(u16::from_be_bytes([page[2], page[3]]));
        if used < PAGE_HEADER || used > page.len() {
            return Err((2, format!("page claims {used} bytes used")));
        }
        Ok(Entries { page: &page[..used], at: PAGE_HEADER, left: count, base })
    }

    fn next_entry(&mut self) -> std::result::Result<Entry<'a>, (usize, String)> {
        let start = self.at;
        let page = self.page;
        let key_len =
            usize::from(*page.get(start).ok_or_else(|| (start, "entry past the bytes used".to_string()))? & 0x1F);
        if key_len < 2 {
            return Err((start, format!("position length {key_len}")));
        }
        let moves_at = start + key_len;
        let moves_len =
            usize::from(*page.get(moves_at).ok_or_else(|| (moves_at, "move section past the bytes used".to_string()))?);
        if moves_len == 0 || moves_len % 2 == 0 {
            return Err((moves_at, format!("move section length {moves_len} is not 1 + 2n")));
        }
        let stats_at = moves_at + moves_len;
        let end = stats_at + STATS_LEN;
        let stats: &[u8; STATS_LEN] = page
            .get(stats_at..end)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| (stats_at, "statistics past the bytes used".to_string()))?;
        self.at = end;
        Ok(Entry {
            key: &page[start..moves_at],
            moves: &page[moves_at + 1..stats_at],
            stats,
            offset: self.base + start as u64,
        })
    }

    /// Whether the walk ended exactly at the page's bytes-used mark.
    pub fn closed(&self) -> bool {
        self.left == 0 && self.at == self.page.len()
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = std::result::Result<Entry<'a>, EntryError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        match self.next_entry() {
            Ok(e) => Some(Ok(e)),
            Err((at, detail)) => {
                self.left = 0;
                Some(Err(EntryError { offset: self.base + at as u64, detail }))
            }
        }
    }
}

/// A malformed entry: where, and what was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryError {
    /// Byte offset in the `.ctg`.
    pub offset: u64,
    /// What was wrong.
    pub detail: String,
}

impl EntryError {
    /// The typed error for `path`.
    pub fn into_error(self, path: &Path) -> Error {
        Error::corrupt(path, self.offset, self.detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page with one entry: a 2-byte key, one move, zero stats.
    fn page() -> Vec<u8> {
        let mut p = vec![0u8; PAGE_SIZE];
        let entry_len = 2 + 3 + STATS_LEN;
        p[0..2].copy_from_slice(&1u16.to_be_bytes());
        p[2..4].copy_from_slice(&((PAGE_HEADER + entry_len) as u16).to_be_bytes());
        p[4] = 0x02;
        p[5] = 0x00;
        p[6] = 3;
        p[7] = 0x6b;
        p[8] = 0x00;
        p[9] = 0x01;
        p
    }

    #[test]
    fn a_well_formed_page_walks_and_closes() {
        let p = page();
        let mut e = Entries::new(&p, 4096).unwrap();
        let entry = e.next().unwrap().unwrap();
        assert_eq!(entry.key, &[0x02, 0x00]);
        assert_eq!(entry.move_pairs().collect::<Vec<_>>(), vec![(0x6b, 0)]);
        assert_eq!(entry.stats().games, 1 << 16);
        assert_eq!(entry.offset, 4096 + 4);
        assert!(e.next().is_none());
        assert!(e.closed());
    }

    #[test]
    fn an_even_move_section_is_an_error_not_a_panic() {
        let mut p = page();
        p[6] = 4;
        let mut e = Entries::new(&p, 0).unwrap();
        let err = e.next().unwrap().unwrap_err();
        assert_eq!(err.offset, 6);
        assert!(e.next().is_none());
    }

    #[test]
    fn a_bytes_used_field_past_the_page_is_refused() {
        let mut p = page();
        p[2..4].copy_from_slice(&5000u16.to_be_bytes());
        assert!(Entries::new(&p, 0).is_err());
    }

    #[test]
    fn an_entry_past_the_bytes_used_mark_is_an_error() {
        let mut p = page();
        p[0..2].copy_from_slice(&2u16.to_be_bytes());
        let mut e = Entries::new(&p, 0).unwrap();
        assert!(e.next().unwrap().is_ok());
        assert!(e.next().unwrap().is_err());
    }

    #[test]
    fn the_ctg_path_is_derived_from_a_base_name() {
        assert_eq!(ctg_path(Path::new("Books/Power")), PathBuf::from("Books/Power.ctg"));
        assert_eq!(ctg_path(Path::new("Books/Power.CTG")), PathBuf::from("Books/Power.CTG"));
    }
}
