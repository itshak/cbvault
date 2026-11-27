//! Read-only ChessBase opening books (`.ctg`): look a position up, read its book
//! moves and statistics, and check a whole book.
//!
//! # How lookups work
//!
//! [`NativeBook`] opens `.cto` and `.ctb` companion files directly in
//! milliseconds with zero synthesized index RAM. Because unvalidated native
//! companions might miss a position if externally corrupted, native misses
//! return [`NativeProbe::UnverifiedMiss`] until [`NativeBook::validate`]
//! establishes complete routing. [`AdaptiveBook`] combines fast native open
//! with automatic validation on first miss and automatic fallback to
//! [`Book::open`] on missing or corrupt native companions.
//!
//! [`Book::open`] scans the `.ctg` once and builds a bounded-memory index of
//! its own: one `u64` per position, packing high bits of a hash of the
//! position's key with the number of the page that holds it. A lookup binary-
//! searches that array and then compares the key byte for byte on the page, so
//! a hash collision costs a page read, never a wrong answer.
//!
//! # Provenance
//!
//! Original cbvault code, clean-room. It consumes `cbvault-format` for bytes and
//! `cbvault-chess` for chess.

mod stream;
pub use stream::{BookStream, StreamEnd, StreamEvent, StreamOptions, StreamPolicy, StreamStats};

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use cbvault_chess::ctg::{self as chess_ctg, MoveError, Stored};
use cbvault_format::ctg::cto::{NativeDirectory, SlotTable};
use cbvault_format::ctg::position;
use cbvault_format::ctg::{Ctg, Entry, PAGE_SIZE, Stats};
use gigachess::{Board, Move};
use rayon::prelude::*;

pub use cbvault_format::ctg::{Annotation, Colour, Evaluation, Stats as BookStats, colour};
pub use cbvault_format::error::{Error, Result};
pub use cbvault_format::file::ReadMode;

/// Pages handed to one worker at a time while the index is built.
const PAGES_PER_TASK: u32 = 2048;

/// An opened book with its in-memory index.
#[derive(Debug)]
pub struct Book {
    ctg: Ctg,
    /// Sorted: `(hash & !page_mask) | page`.
    index: Vec<u64>,
    page_mask: u64,
    damaged: Vec<Damage>,
}

/// A page the index build could not read completely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damage {
    /// The data page.
    pub page: u32,
    /// What was wrong.
    pub detail: String,
}

/// A position found in the book.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookPosition {
    /// Its statistics, from the point of view of the side to move.
    pub stats: Stats,
    /// Its book moves, in stored order.
    pub moves: Vec<BookMove>,
    /// The evaluation set on the move that leads here, oriented for the probed
    /// position (`None` when there is none).
    pub evaluation: Option<Evaluation>,
    /// The entry's byte offset in the `.ctg`.
    pub offset: u64,
}

/// Statistics and continuation count without decoding the continuation moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookPositionStats {
    /// Statistics from the side to move of this position.
    pub stats: Stats,
    /// Evaluation oriented to the actual board, as in [`BookPosition`].
    pub evaluation: Option<Evaluation>,
    /// Number of stored continuation move bytes (including unknown codes).
    pub continuation_count: usize,
    /// Source entry offset.
    pub offset: u64,
}

/// A one-level opening tree, preserving stored move order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookTree {
    /// Statistics and move codes of the parent.
    pub parent: BookPosition,
    /// Child statistics parallel to parent.moves. An absent child is None;
    /// an undecoded parent move retains its typed error in parent.moves.
    pub children: Vec<Option<BookPositionStats>>,
}

/// One move of a book position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookMove {
    /// The stored move byte.
    pub byte: u8,
    /// The stored annotation byte: see [`Annotation`] for what is established.
    pub annotation: u8,
    /// The legal move it stands for, or why it could not be decoded.
    pub mv: std::result::Result<Move, MoveError>,
}

impl BookMove {
    /// The annotation byte, split into its parts.
    pub fn annotation(&self) -> Annotation {
        Annotation(self.annotation)
    }
}

/// A hash of a key, to spread keys over the index. FNV-1a with a SplitMix64
/// finaliser: not a format fact, only this index's own choice.
fn key_hash(key: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in key {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

impl Book {
    /// Opens a book (the `.ctg` or its base name) and builds its index.
    ///
    /// A malformed page does not fail the open: its readable entries are
    /// indexed and the page is listed in [`Book::damaged`].
    pub fn open(path: impl AsRef<Path>) -> Result<Book> {
        Self::open_with_mode(path, ReadMode::from_environment())
    }

    /// Build a scan index with explicit I/O policy.
    pub fn open_with_mode(path: impl AsRef<Path>, mode: ReadMode) -> Result<Self> {
        let ctg = Ctg::open_with_mode(path, mode)?;
        let pages = ctg.pages();
        let page_bits = 32 - pages.leading_zeros();
        let page_mask = (1u64 << page_bits) - 1;
        let tasks: Vec<(u32, u32)> = (1..pages)
            .step_by(PAGES_PER_TASK as usize)
            .map(|s| (s, s.saturating_add(PAGES_PER_TASK).min(pages)))
            .collect();
        let (mut index, damaged) = build_index(&ctg, &tasks, page_mask)?;
        index.par_sort_unstable();
        Ok(Book { ctg, index, page_mask, damaged })
    }

    /// The `.ctg` path.
    pub fn path(&self) -> &Path {
        self.ctg.path()
    }

    /// Positions indexed.
    pub fn positions(&self) -> usize {
        self.index.len()
    }

    /// Pages in the `.ctg`, the header page included.
    pub fn pages(&self) -> u32 {
        self.ctg.pages()
    }

    /// Pages the index build could not read completely.
    pub fn damaged(&self) -> &[Damage] {
        &self.damaged
    }

    /// Bytes the index occupies.
    pub fn index_bytes(&self) -> usize {
        self.index.capacity() * std::mem::size_of::<u64>()
    }

    /// Looks `board` up. `Ok(None)` when the book does not have the position.
    pub fn probe(&self, board: &Board) -> Result<Option<BookPosition>> {
        let stored = chess_ctg::stored_form(board);
        for (key, len) in chess_ctg::key_candidates(&stored) {
            if let Some(found) = self.find(&key[..len], |entry| book_position(board, &stored, entry))? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Looks up the position after `mv` in `board`. `Ok(None)` when the move is
    /// illegal or the book does not have the position.
    pub fn probe_after(&self, board: &Board, mv: Move) -> Result<Option<BookPosition>> {
        match child_after(board, mv) {
            Some(next) => self.probe(&next),
            None => Ok(None),
        }
    }

    /// Look up statistics without allocating or decoding a continuation list.
    pub fn probe_stats(&self, board: &Board) -> Result<Option<BookPositionStats>> {
        let stored = chess_ctg::stored_form(board);
        for (key, len) in chess_ctg::key_candidates(&stored) {
            if let Some(found) = self.find(&key[..len], |entry| entry_stats(&stored, entry))? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Statistics of the child after a legal move, without decoding its moves.
    pub fn probe_stats_after(&self, board: &Board, mv: Move) -> Result<Option<BookPositionStats>> {
        match child_after(board, mv) {
            Some(next) => self.probe_stats(&next),
            None => Ok(None),
        }
    }

    /// Read one level of the opening tree. Child results are from the child
    /// side to move (the opposite of the parent mover); unknown moves remain
    /// typed errors. No grandchildren are decoded.
    pub fn probe_tree(&self, board: &Board) -> Result<Option<BookTree>> {
        let Some(parent) = self.probe(board)? else {
            return Ok(None);
        };
        let children = parent
            .moves
            .iter()
            .map(|m| match m.mv {
                Ok(mv) => self.probe_stats_after(board, mv),
                Err(_) => Ok(None),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(BookTree { parent, children }))
    }

    /// Whether the book has an entry with exactly `key`.
    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.find(key, |_| ())?.is_some())
    }

    /// Finds the entry whose key is `key` and maps it with `f`.
    fn find<T>(&self, key: &[u8], f: impl Fn(&Entry<'_>) -> T) -> Result<Option<T>> {
        let lo = key_hash(key) & !self.page_mask;
        let hi = lo | self.page_mask;
        let start = self.index.partition_point(|&x| x < lo);
        let mut last_page = None;
        for &x in self.index[start..].iter().take_while(|&&x| x <= hi) {
            let page = (x & self.page_mask) as u32;
            if last_page == Some(page) {
                continue;
            }
            last_page = Some(page);
            if let Some(found) = find_on_page(&self.ctg, page, key, &f)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Checks every entry of the book: that its key decodes to a valid position
    /// and encodes back to the same bytes, that its move bytes decode to legal
    /// moves, that no duplicate position keys appear across readable entries,
    /// and (with `children`) that the position after each move is in the book
    /// too. Read-only, like everything here.
    pub fn verify(&self, children: bool) -> Result<VerifyReport> {
        let pages = self.ctg.pages();
        let tasks: Vec<(u32, u32)> = (1..pages)
            .step_by(PAGES_PER_TASK as usize)
            .map(|s| (s, s.saturating_add(PAGES_PER_TASK).min(pages)))
            .collect();
        let parts: Vec<Result<VerifyReport>> =
            tasks.par_iter().map(|&(start, end)| self.verify_pages(start, end, children)).collect();
        let mut report = VerifyReport::default();
        for part in parts {
            report.add(&part?);
        }
        report.duplicate_keys = self.count_duplicate_keys()?;
        Ok(report)
    }

    fn count_duplicate_keys(&self) -> Result<u64> {
        let mut dups = 0u64;
        let mut i = 0;
        let mut buf = [0u8; PAGE_SIZE];
        while i < self.index.len() {
            let hash_hi = self.index[i] & !self.page_mask;
            let mut j = i + 1;
            while j < self.index.len() && (self.index[j] & !self.page_mask) == hash_hi {
                j += 1;
            }
            if j - i > 1 {
                let mut seen_pages = Vec::with_capacity(j - i);
                let mut keys: Vec<Vec<u8>> = Vec::with_capacity(j - i);
                for &slot in &self.index[i..j] {
                    let page = (slot & self.page_mask) as u32;
                    if seen_pages.contains(&page) {
                        continue;
                    }
                    seen_pages.push(page);
                    let bytes = match self.ctg.mapped_page(page) {
                        Some(b) => b,
                        None => {
                            self.ctg.read_page(page, &mut buf)?;
                            &buf[..]
                        }
                    };
                    if let Ok(entries) = self.ctg.entries(page, bytes) {
                        for entry in entries.flatten() {
                            if (key_hash(entry.key) & !self.page_mask) == hash_hi {
                                keys.push(entry.key.to_vec());
                            }
                        }
                    }
                }
                keys.sort_unstable();
                dups += keys.windows(2).filter(|w| w[0] == w[1]).count() as u64;
            }
            i = j;
        }
        Ok(dups)
    }

    fn verify_pages(&self, start: u32, end: u32, children: bool) -> Result<VerifyReport> {
        let mut r = VerifyReport::default();
        let mut buf = [0u8; PAGE_SIZE];
        for page in start..end {
            let bytes = match self.ctg.mapped_page(page) {
                Some(b) => b,
                None => {
                    self.ctg.read_page(page, &mut buf)?;
                    &buf[..]
                }
            };
            let Ok(entries) = self.ctg.entries(page, bytes) else {
                r.damaged_pages += 1;
                continue;
            };
            let mut page_has_err = false;
            let mut walk = entries;
            for entry in walk.by_ref() {
                let Ok(entry) = entry else {
                    r.damaged_pages += 1;
                    page_has_err = true;
                    break;
                };
                r.entries += 1;
                let Ok(raw) = position::decode(entry.key) else {
                    r.bad_keys += 1;
                    continue;
                };
                let Some(board) = chess_ctg::board_of(&raw) else {
                    r.invalid_positions += 1;
                    continue;
                };
                let stored = chess_ctg::stored_form(&board);
                let round_trips = chess_ctg::key_candidates(&stored).any(|(k, l)| &k[..l] == entry.key);
                if round_trips {
                    r.key_round_trips += 1;
                } else {
                    r.key_mismatches += 1;
                    continue;
                }
                if entry.moves.is_empty() {
                    continue;
                }
                let legal = board.legal_moves();
                for (byte, _) in entry.move_pairs() {
                    match chess_ctg::decode_byte_with_legal(&board, &stored, byte, &legal) {
                        Ok(mv) => {
                            r.moves_decoded += 1;
                            if mv.promotion().is_some() {
                                r.promotions += 1;
                            }
                            if children {
                                let mut next = board;
                                let found = next.play(mv).is_ok() && self.probe_key_of(&next)?;
                                if found {
                                    r.children_found += 1;
                                } else {
                                    r.children_missing += 1;
                                }
                            }
                        }
                        Err(MoveError::UnknownByte(_)) => r.moves_unknown += 1,
                        Err(MoveError::NoSuchPiece | MoveError::Illegal) => r.moves_illegal += 1,
                    }
                }
            }
            if !page_has_err && !walk.closed() {
                r.damaged_pages += 1;
            }
        }
        Ok(r)
    }

    fn probe_key_of(&self, board: &Board) -> Result<bool> {
        let stored = chess_ctg::stored_form(board);
        for (key, len) in chess_ctg::key_candidates(&stored) {
            if self.contains_key(&key[..len])? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Bounds temporary index arenas to two tasks per worker. For mapped files,
/// page headers give a reserve hint, never an authority for the final count.
/// Every entry is still parsed and every damaged page is reported.
fn build_index(ctg: &Ctg, tasks: &[(u32, u32)], page_mask: u64) -> Result<(Vec<u64>, Vec<Damage>)> {
    let mut index = Vec::new();
    let mut damaged = Vec::new();
    if ctg.mapped_page(1).is_some() {
        let hint: u64 = tasks
            .par_iter()
            .map(|&(start, end)| {
                (start..end)
                    .map(|page| {
                        let Some(bytes) = ctg.mapped_page(page) else {
                            return 0;
                        };
                        let count = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
                        let used = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
                        // Never reserve more than can physically fit. Even a corrupt
                        // count cannot request arbitrary memory.
                        if !(4..=PAGE_SIZE).contains(&used) {
                            return 0;
                        }
                        let maximum = (used - 4) / (2 + 1 + cbvault_format::ctg::STATS_LEN);
                        count.min(maximum) as u64
                    })
                    .sum::<u64>()
            })
            .sum();
        let hint = usize::try_from(hint).map_err(|_| Error::corrupt(ctg.path(), 0, "index exceeds address space"))?;
        index.try_reserve_exact(hint).map_err(|e| Error::corrupt(ctg.path(), 0, format!("index reserve: {e}")))?;
    }
    let wave = rayon::current_num_threads().saturating_mul(2).max(1);
    for tasks in tasks.chunks(wave) {
        let parts: Vec<_> = tasks.par_iter().map(|&(start, end)| index_pages(ctg, start, end, page_mask)).collect();
        for (v, d) in parts {
            index.try_reserve(v.len()).map_err(|e| Error::corrupt(ctg.path(), 0, format!("index reserve: {e}")))?;
            index.extend_from_slice(&v);
            damaged.extend(d);
        }
    }
    Ok((index, damaged))
}

/// Scans `page`, checks all entries and the bytes-used closing boundary, and
/// maps the first entry matching `key`.
fn find_on_mapped_page<T>(
    ctg: &Ctg,
    page: u32,
    bytes: &[u8],
    key: &[u8],
    f: &impl Fn(&Entry<'_>) -> T,
) -> Result<Option<T>> {
    let mut entries = ctg.entries(page, bytes)?;
    let mut found = None;
    for entry in entries.by_ref() {
        let entry = entry.map_err(|e| e.into_error(ctg.path()))?;
        if entry.key == key && found.is_none() {
            found = Some(f(&entry));
        }
    }
    if !entries.closed() {
        return Err(Error::corrupt(ctg.path(), u64::from(page) * PAGE_SIZE as u64, "unclosed book page"));
    }
    Ok(found)
}

#[cold]
fn find_on_unmapped_page<T>(ctg: &Ctg, page: u32, key: &[u8], f: &impl Fn(&Entry<'_>) -> T) -> Result<Option<T>> {
    let mut buf = [0u8; PAGE_SIZE];
    ctg.read_page(page, &mut buf)?;
    find_on_mapped_page(ctg, page, &buf, key, f)
}

#[inline]
fn find_on_page<T>(ctg: &Ctg, page: u32, key: &[u8], f: &impl Fn(&Entry<'_>) -> T) -> Result<Option<T>> {
    match ctg.mapped_page(page) {
        Some(bytes) => find_on_mapped_page(ctg, page, bytes, key, f),
        None => find_on_unmapped_page(ctg, page, key, f),
    }
}

#[inline]
fn child_after(board: &Board, mv: Move) -> Option<Board> {
    let mut next = *board;
    next.play(mv).ok()?;
    Some(next)
}

#[inline]
fn entry_stats(stored: &Stored, entry: &Entry<'_>) -> BookPositionStats {
    let stats = entry.stats();
    BookPositionStats {
        stats,
        evaluation: stats.evaluation().map(|e| e.for_real_position(stored.flipped)),
        continuation_count: entry.moves.len() / 2,
        offset: entry.offset,
    }
}

/// Native directory reader with zero synthesized scan index.
///
/// Exact hits are authoritative immediately; misses return
/// [`NativeProbe::UnverifiedMiss`] until [`NativeBook::validate`] checks the
/// entire book and `.cto`/`.ctb` routing.
#[derive(Debug)]
pub struct NativeBook {
    ctg: Ctg,
    directory: NativeDirectory,
    count: OnceLock<u64>,
}

/// Result of querying a [`NativeBook`] or [`AdaptiveBook`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeProbe<T> {
    /// Exact encoded key found on a candidate page.
    Found(T),
    /// Absence established after full validation or complete scan.
    Absent,
    /// No exact hit, and completeness has not been established.
    UnverifiedMiss,
}

impl NativeBook {
    /// Opens `.ctg`, `.cto`, and `.ctb` companions without scanning CTG data pages.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_mode(path, ReadMode::from_environment())
    }

    /// Open native companions using explicit I/O policy.
    pub fn open_with_mode(path: impl AsRef<Path>, mode: ReadMode) -> Result<Self> {
        let ctg = Ctg::open_with_mode(path, mode)?;
        let directory = NativeDirectory::open(&ctg)?;
        Ok(Self { ctg, directory, count: OnceLock::new() })
    }

    /// The `.ctg` path.
    pub fn path(&self) -> &Path {
        self.ctg.path()
    }

    /// Pages in the `.ctg`, the header page included.
    pub fn pages(&self) -> u32 {
        self.ctg.pages()
    }

    /// Native directory `(slots, low, high)` geometry.
    pub fn bounds(&self) -> (u32, u32, u32) {
        self.directory.bounds()
    }

    /// Count of validated positions; unknown before exhaustive validation.
    pub fn positions(&self) -> Option<u64> {
        self.count.get().copied()
    }

    fn verify_and_count(&self) -> Result<u64> {
        self.directory.validate_pages()?;
        let table: SlotTable<'_> = self.directory.slot_table()?;
        let ctg = &self.ctg;
        let dir = &self.directory;
        (1..ctg.pages())
            .into_par_iter()
            .map_init(
                || [0u8; PAGE_SIZE],
                |buf, page| {
                    let bytes = match ctg.mapped_page(page) {
                        Some(bytes) => bytes,
                        None => {
                            ctg.read_page(page, buf)?;
                            &buf[..]
                        }
                    };
                    let mut entries = ctg.entries(page, bytes)?;
                    let mut count = 0u64;
                    for entry in entries.by_ref() {
                        let entry = entry.map_err(|e| e.into_error(ctg.path()))?;
                        if !table.reaches(dir, entry.key, page)? {
                            return Err(Error::corrupt(
                                ctg.path(),
                                entry.offset,
                                "native directory cannot reach stored key",
                            ));
                        }
                        count += 1;
                    }
                    if !entries.closed() {
                        return Err(Error::corrupt(
                            ctg.path(),
                            u64::from(page) * PAGE_SIZE as u64,
                            "unclosed native page",
                        ));
                    }
                    Ok(count)
                },
            )
            .try_reduce(|| 0u64, |a, b| Ok(a + b))
    }

    /// Validates every stored key's native route and page closure without
    /// allocating a fallback index. Errors leave the reader unverified.
    pub fn validate(&self) -> Result<u64> {
        if let Some(&count) = self.count.get() {
            return Ok(count);
        }
        let count = self.verify_and_count()?;
        let _ = self.count.set(count);
        Ok(count)
    }

    fn find<T>(&self, key: &[u8], f: impl Fn(&Entry<'_>) -> T) -> Result<Option<T>> {
        let mut best: Option<(u32, T)> = None;
        for slot in self.directory.slots_for(key) {
            let Some(page) = self.directory.page_at(slot)? else {
                continue;
            };
            if let Some(found) = find_on_page(&self.ctg, page, key, &f)?
                && best.as_ref().is_none_or(|(p, _)| page < *p)
            {
                best = Some((page, found));
            }
        }
        Ok(best.map(|(_, v)| v))
    }

    fn miss_status<T>(&self) -> NativeProbe<T> {
        if self.count.get().is_some() { NativeProbe::Absent } else { NativeProbe::UnverifiedMiss }
    }

    /// Looks up `board` using the same canonical key candidates as [`Book::probe`].
    pub fn probe(&self, board: &Board) -> Result<NativeProbe<BookPosition>> {
        let stored = chess_ctg::stored_form(board);
        for (key, len) in chess_ctg::key_candidates(&stored) {
            if let Some(found) = self.find(&key[..len], |entry| book_position(board, &stored, entry))? {
                return Ok(NativeProbe::Found(found));
            }
        }
        Ok(self.miss_status())
    }

    /// Looks up the position reached by playing `mv` on `board`.
    pub fn probe_after(&self, board: &Board, mv: Move) -> Result<NativeProbe<BookPosition>> {
        match child_after(board, mv) {
            Some(next) => self.probe(&next),
            None => Ok(self.miss_status()),
        }
    }

    /// Looks up statistics for `board` without decoding continuation moves.
    pub fn probe_stats(&self, board: &Board) -> Result<NativeProbe<BookPositionStats>> {
        let stored = chess_ctg::stored_form(board);
        for (key, len) in chess_ctg::key_candidates(&stored) {
            if let Some(found) = self.find(&key[..len], |entry| entry_stats(&stored, entry))? {
                return Ok(NativeProbe::Found(found));
            }
        }
        Ok(self.miss_status())
    }

    /// Looks up statistics of the child position reached by playing `mv`.
    pub fn probe_stats_after(&self, board: &Board, mv: Move) -> Result<NativeProbe<BookPositionStats>> {
        match child_after(board, mv) {
            Some(next) => self.probe_stats(&next),
            None => Ok(self.miss_status()),
        }
    }

    /// Reads one level of the opening tree. Before full validation, an
    /// uncertain child miss surfaces [`NativeProbe::UnverifiedMiss`] so absence
    /// of a child is never fabricated.
    pub fn probe_tree(&self, board: &Board) -> Result<NativeProbe<BookTree>> {
        let parent = match self.probe(board)? {
            NativeProbe::Found(p) => p,
            NativeProbe::Absent => return Ok(NativeProbe::Absent),
            NativeProbe::UnverifiedMiss => return Ok(NativeProbe::UnverifiedMiss),
        };
        let mut children = Vec::with_capacity(parent.moves.len());
        for m in &parent.moves {
            let child = match m.mv {
                Ok(mv) => match self.probe_stats_after(board, mv)? {
                    NativeProbe::Found(st) => Some(st),
                    NativeProbe::Absent => None,
                    NativeProbe::UnverifiedMiss => return Ok(NativeProbe::UnverifiedMiss),
                },
                Err(_) => None,
            };
            children.push(child);
        }
        Ok(NativeProbe::Found(BookTree { parent, children }))
    }

    /// Probes a raw encoded key without decoding chess moves.
    pub fn contains_key(&self, key: &[u8]) -> Result<NativeProbe<()>> {
        if self.find(key, |_| ())?.is_some() {
            return Ok(NativeProbe::Found(()));
        }
        Ok(self.miss_status())
    }
}

/// Which lookup backend [`AdaptiveBook`] currently uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupMode {
    /// Native directory, exact hits only until validation.
    NativeUnverified,
    /// Native directory whose entire routing has been validated.
    NativeValidated,
    /// Scan-based fallback. See [`AdaptiveBook::fallback_reason`].
    Scan,
}

/// Fast-opening, thread-safe opening-book reader with automatic validation and
/// scan fallback.
///
/// Queries take `&self` and can execute concurrently across threads. Native
/// hits are returned immediately with zero index RAM; the first uncertain miss
/// triggers a one-time exhaustive validation or automatic fallback to [`Book`].
/// Before validation, a hit proves the exact key exists but cannot establish
/// precedence over an unreachable duplicate or earlier canonical candidate.
/// Use [`Self::open_verified`] when that guarantee is required.
#[derive(Debug)]
pub struct AdaptiveBook {
    path: PathBuf,
    native: Option<NativeBook>,
    native_validated: OnceLock<u64>,
    scan: OnceLock<Book>,
    fallback_reason: OnceLock<String>,
    transition: Mutex<()>,
    read_mode: ReadMode,
}

impl AdaptiveBook {
    /// Opens native companions first; if missing or invalid, scans the `.ctg`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_mode(path, ReadMode::from_environment())
    }

    /// Preserve explicit I/O policy through every native-to-scan transition.
    pub fn open_with_mode(path: impl AsRef<Path>, read_mode: ReadMode) -> Result<Self> {
        let path = cbvault_format::ctg::ctg_path(path.as_ref());
        let scan = OnceLock::new();
        let fallback_reason = OnceLock::new();
        let native = match NativeBook::open_with_mode(&path, read_mode) {
            Ok(book) => Some(book),
            Err(error) => {
                let _ = fallback_reason.set(error.to_string());
                let _ = scan.set(Book::open_with_mode(&path, read_mode)?);
                None
            }
        };
        Ok(Self {
            path,
            native,
            native_validated: OnceLock::new(),
            scan,
            fallback_reason,
            transition: Mutex::new(()),
            read_mode,
        })
    }

    /// Open and establish complete routing (or fallback) before any lookup.
    /// Use this for authoritative duplicate and canonical-key precedence.
    /// A damaged scan remains recoverable; inspect `damaged()` before treating
    /// results as a complete view of the source.
    pub fn open_verified(path: impl AsRef<Path>, mode: ReadMode) -> Result<Self> {
        let book = Self::open_with_mode(path, mode)?;
        book.validate()?;
        Ok(book)
    }

    /// Requested I/O policy, also used by the fallback.
    pub fn read_mode(&self) -> ReadMode {
        self.read_mode
    }

    /// The `.ctg` path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Pages in the `.ctg`, header included.
    pub fn pages(&self) -> u32 {
        if let Some(scan) = self.scan.get() {
            scan.pages()
        } else if let Some(native) = &self.native {
            native.pages()
        } else {
            0
        }
    }

    /// Current lookup mode.
    pub fn mode(&self) -> LookupMode {
        if self.scan.get().is_some() {
            LookupMode::Scan
        } else if self.native_validated.get().is_some() {
            LookupMode::NativeValidated
        } else {
            LookupMode::NativeUnverified
        }
    }

    /// Why native lookup fell back to a full scan, if applicable.
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.get().map(String::as_str)
    }

    /// Indexed or validated position count; `None` before native validation.
    pub fn positions(&self) -> Option<u64> {
        if let Some(scan) = self.scan.get() {
            Some(scan.positions() as u64)
        } else {
            self.native_validated.get().copied()
        }
    }

    /// Damage detected by the scan fallback; `Some(&[])` once native routing is
    /// validated; `None` while unvalidated.
    pub fn damaged(&self) -> Option<&[Damage]> {
        if let Some(scan) = self.scan.get() {
            Some(scan.damaged())
        } else if self.native_validated.get().is_some() {
            Some(&[])
        } else {
            None
        }
    }

    fn activate_scan_locked(&self, reason: Error) -> Result<()> {
        if self.scan.get().is_some() {
            return Ok(());
        }
        let book = Book::open_with_mode(&self.path, self.read_mode)?;
        let _ = self.fallback_reason.set(reason.to_string());
        let _ = self.scan.set(book);
        Ok(())
    }

    fn fallback(&self, reason: Error) -> Result<()> {
        if self.scan.get().is_some() {
            return Ok(());
        }
        let _guard = self.transition.lock().unwrap_or_else(|e| e.into_inner());
        self.activate_scan_locked(reason)
    }

    /// Establishes native routing completeness once, or switches to [`Book`] if
    /// native validation fails. Concurrent callers synchronize once on this transition.
    pub fn validate(&self) -> Result<()> {
        if self.scan.get().is_some() || self.native_validated.get().is_some() {
            return Ok(());
        }
        let _guard = self.transition.lock().unwrap_or_else(|e| e.into_inner());
        if self.scan.get().is_some() || self.native_validated.get().is_some() {
            return Ok(());
        }
        if let Some(native) = &self.native {
            match native.validate() {
                Ok(count) => {
                    let _ = self.native_validated.set(count);
                }
                Err(error) => {
                    self.activate_scan_locked(error)?;
                }
            }
        }
        Ok(())
    }

    fn dispatch<T>(
        &self,
        run_native: impl Fn(&NativeBook) -> Result<NativeProbe<T>>,
        run_scan: impl Fn(&Book) -> Result<Option<T>>,
    ) -> Result<NativeProbe<T>> {
        if let Some(scan) = self.scan.get() {
            return Ok(match run_scan(scan)? {
                Some(v) => NativeProbe::Found(v),
                None if scan.damaged().is_empty() => NativeProbe::Absent,
                None => NativeProbe::UnverifiedMiss,
            });
        }
        if let Some(native) = &self.native {
            match run_native(native) {
                Ok(NativeProbe::UnverifiedMiss) => self.validate()?,
                Ok(res) => return Ok(res),
                Err(error) => self.fallback(error)?,
            }
            if let Some(scan) = self.scan.get() {
                return Ok(match run_scan(scan)? {
                    Some(v) => NativeProbe::Found(v),
                    None if scan.damaged().is_empty() => NativeProbe::Absent,
                    None => NativeProbe::UnverifiedMiss,
                });
            }
            match run_native(native) {
                Ok(res) => return Ok(res),
                Err(error) => {
                    self.fallback(error)?;
                    if let Some(scan) = self.scan.get() {
                        return Ok(match run_scan(scan)? {
                            Some(v) => NativeProbe::Found(v),
                            None if scan.damaged().is_empty() => NativeProbe::Absent,
                            None => NativeProbe::UnverifiedMiss,
                        });
                    }
                }
            }
        }
        Ok(NativeProbe::UnverifiedMiss)
    }

    /// Probes `board`, resolving uncertain misses transparently via validation or fallback.
    pub fn probe(&self, board: &Board) -> Result<NativeProbe<BookPosition>> {
        self.dispatch(|n| n.probe(board), |s| s.probe(board))
    }

    /// Probes the child position reached by playing `mv` on `board`.
    pub fn probe_after(&self, board: &Board, mv: Move) -> Result<NativeProbe<BookPosition>> {
        self.dispatch(|n| n.probe_after(board, mv), |s| s.probe_after(board, mv))
    }

    /// Probes statistics for `board` without decoding continuation moves.
    pub fn probe_stats(&self, board: &Board) -> Result<NativeProbe<BookPositionStats>> {
        self.dispatch(|n| n.probe_stats(board), |s| s.probe_stats(board))
    }

    /// Probes statistics of the child position reached by playing `mv`.
    pub fn probe_stats_after(&self, board: &Board, mv: Move) -> Result<NativeProbe<BookPositionStats>> {
        self.dispatch(|n| n.probe_stats_after(board, mv), |s| s.probe_stats_after(board, mv))
    }

    /// Probes one level of the opening tree (parent moves + child statistics).
    pub fn probe_tree(&self, board: &Board) -> Result<NativeProbe<BookTree>> {
        self.dispatch(|n| n.probe_tree(board), |s| s.probe_tree(board))
    }

    /// Probes a raw encoded key without decoding chess moves.
    pub fn contains_key(&self, key: &[u8]) -> Result<NativeProbe<()>> {
        self.dispatch(|n| n.contains_key(key), |s| s.contains_key(key).map(|found| found.then_some(())))
    }
}

/// Indexes data pages `start..end`.
fn index_pages(ctg: &Ctg, start: u32, end: u32, page_mask: u64) -> (Vec<u64>, Vec<Damage>) {
    let mut out = Vec::with_capacity(((end - start) as usize) * 56);
    let mut damaged = Vec::new();
    let mut buf = [0u8; PAGE_SIZE];
    for page in start..end {
        let bytes = match ctg.mapped_page(page) {
            Some(b) => b,
            None => match ctg.read_page(page, &mut buf) {
                Ok(()) => &buf[..],
                Err(e) => {
                    damaged.push(Damage { page, detail: e.to_string() });
                    continue;
                }
            },
        };
        let entries = match ctg.entries(page, bytes) {
            Ok(e) => e,
            Err(e) => {
                damaged.push(Damage { page, detail: e.to_string() });
                continue;
            }
        };
        let mut walk = entries;
        for entry in walk.by_ref() {
            match entry {
                Ok(entry) => out.push((key_hash(entry.key) & !page_mask) | u64::from(page)),
                Err(e) => {
                    damaged.push(Damage { page, detail: e.detail });
                    break;
                }
            }
        }
        if damaged.last().is_none_or(|d| d.page != page) && !walk.closed() {
            damaged.push(Damage { page, detail: "entries do not end at the bytes-used mark".into() });
        }
    }
    (out, damaged)
}

fn book_position(board: &Board, stored: &Stored, entry: &Entry<'_>) -> BookPosition {
    let moves = if entry.moves.is_empty() {
        Vec::new()
    } else {
        let legal = board.legal_moves();
        entry
            .move_pairs()
            .map(|(byte, annotation)| BookMove {
                byte,
                annotation,
                mv: chess_ctg::decode_byte_with_legal(board, stored, byte, &legal),
            })
            .collect()
    };
    let stats = entry.stats();
    let evaluation = stats.evaluation().map(|e| e.for_real_position(stored.flipped));
    BookPosition { stats, moves, evaluation, offset: entry.offset }
}

/// What [`Book::verify`] counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VerifyReport {
    /// Entries read.
    pub entries: u64,
    /// Pages whose entries could not all be read.
    pub damaged_pages: u64,
    /// Keys that do not decode.
    pub bad_keys: u64,
    /// Keys that decode to something that is not a chess position.
    pub invalid_positions: u64,
    /// Keys that encode back to the same bytes.
    pub key_round_trips: u64,
    /// Keys that encode back to different bytes.
    pub key_mismatches: u64,
    /// Move bytes decoded to a legal move.
    pub moves_decoded: u64,
    /// Move bytes whose meaning is not established.
    pub moves_unknown: u64,
    /// Decoded moves that promote (always to a queen: see `cbvault_chess::ctg`).
    pub promotions: u64,
    /// Move bytes that decode to no legal move.
    pub moves_illegal: u64,
    /// Decoded moves whose resulting position is in the book.
    pub children_found: u64,
    /// Decoded moves whose resulting position is not.
    pub children_missing: u64,
    /// Extra duplicate occurrences of previously seen position keys.
    pub duplicate_keys: u64,
}

impl VerifyReport {
    fn add(&mut self, o: &VerifyReport) {
        self.entries += o.entries;
        self.damaged_pages += o.damaged_pages;
        self.bad_keys += o.bad_keys;
        self.invalid_positions += o.invalid_positions;
        self.key_round_trips += o.key_round_trips;
        self.key_mismatches += o.key_mismatches;
        self.moves_decoded += o.moves_decoded;
        self.moves_unknown += o.moves_unknown;
        self.promotions += o.promotions;
        self.moves_illegal += o.moves_illegal;
        self.children_found += o.children_found;
        self.children_missing += o.children_missing;
        self.duplicate_keys += o.duplicate_keys;
    }

    /// True when no structural, decoding, duplicate, or (when checked) missing
    /// child errors were recorded.
    pub fn ok(&self, children: bool) -> bool {
        self.damaged_pages == 0
            && self.bad_keys == 0
            && self.invalid_positions == 0
            && self.key_mismatches == 0
            && self.moves_unknown == 0
            && self.moves_illegal == 0
            && self.duplicate_keys == 0
            && (!children || self.children_missing == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_spreads_neighbouring_keys() {
        assert_ne!(key_hash(&[0x02, 0x00]), key_hash(&[0x02, 0x01]));
        assert_ne!(key_hash(&[]) >> 40, key_hash(&[0]) >> 40);
    }

    /// Runs against a real book named by `CBVAULT_TEST_CTG` (never committed).
    /// Without it the test does not run, and says so.
    #[test]
    fn a_real_book_answers_the_start_position() {
        let Ok(path) = std::env::var("CBVAULT_TEST_CTG") else {
            eprintln!("CBVAULT_TEST_CTG is unset: the real-book test DID NOT RUN (it did not pass either)");
            return;
        };
        let book = Book::open(&path).unwrap();
        assert!(book.damaged().is_empty(), "{:?}", &book.damaged()[..book.damaged().len().min(5)]);
        let board = Board::startpos();
        let root = book.probe(&board).unwrap().expect("the start position is in the book");
        assert!(root.stats.games > 0);
        for m in &root.moves {
            let mv = m.mv.unwrap_or_else(|e| panic!("root byte {:#04x}: {e}", m.byte));
            assert!(book.probe_after(&board, mv).unwrap().is_some(), "child after {mv} is missing");
        }
    }
}
