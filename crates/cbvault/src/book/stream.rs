//! Index-free physical traversal of CTG data pages.

use super::{BookPosition, Error, ReadMode, Result, book_position};
use cbvault_chess::ctg;
use cbvault_format::ctg::{Ctg, Entry, PAGE_SIZE, position};
use gigachess::Board;
use std::path::Path;

/// Whether to stop at the first damaged entry or report it and continue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StreamPolicy {
    #[default]
    Strict,
    Recover,
}

/// Why traversal ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StreamEnd {
    #[default]
    Running,
    Exhausted,
    Cancelled,
    PageBudget,
    EntryBudget,
    SinkStopped,
}

/// Physical scan counters. A recovered scan can exhaust the file with errors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamStats {
    pub pages: u32,
    pub entries: u64,
    pub delivered: u64,
    pub errors: u64,
    pub end: StreamEnd,
}

impl StreamStats {
    /// True only for an exhaustive traversal with no reported damage.
    pub fn complete(self) -> bool {
        self.end == StreamEnd::Exhausted && self.errors == 0
    }
}

/// Bound the work by data pages and/or physical entries. Cancellation is
/// polled before each page and each entry, including in recovery mode.
#[derive(Default)]
pub struct StreamOptions<'a> {
    pub policy: StreamPolicy,
    pub decode: bool,
    pub max_pages: Option<u32>,
    pub max_entries: Option<u64>,
    pub progress_every_pages: u32,
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}

/// Events delivered synchronously, in physical order. Borrowed records cannot
/// outlive the callback. Return false to stop without scanning further pages.
#[allow(clippy::large_enum_variant)]
pub enum StreamEvent<'a> {
    Raw(Entry<'a>),
    /// Position in its canonical stored orientation (White to move).
    /// No child lookups are performed.
    Decoded {
        raw: Entry<'a>,
        board: Board,
        position: BookPosition,
    },
    Damage {
        page: u32,
        error: &'a Error,
    },
    Progress(StreamStats),
}

/// A reader that opens only CTG, with no synthesized or native directory.
/// Raw traversal uses one 4 KiB scratch page, independent of book size;
/// decoded traversal additionally owns one bounded record's move list.
pub struct BookStream {
    ctg: Ctg,
}

impl BookStream {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_mode(path, ReadMode::from_environment())
    }

    pub fn open_with_mode(path: impl AsRef<Path>, mode: ReadMode) -> Result<Self> {
        Ok(Self { ctg: Ctg::open_with_mode(path, mode)? })
    }

    pub fn pages(&self) -> u32 {
        self.ctg.pages()
    }

    pub fn visit(
        &self,
        options: &StreamOptions<'_>,
        mut sink: impl for<'a> FnMut(StreamEvent<'a>) -> bool,
    ) -> Result<StreamStats> {
        let mut stats = StreamStats::default();
        let mut scratch = [0; PAGE_SIZE];
        let cancelled = || options.cancelled.is_some_and(|f| f());
        'pages: for page in 1..self.ctg.pages() {
            if cancelled() {
                stats.end = StreamEnd::Cancelled;
                break;
            }
            if options.max_pages.is_some_and(|max| stats.pages >= max) {
                stats.end = StreamEnd::PageBudget;
                break;
            }
            if options.max_entries.is_some_and(|max| stats.entries >= max) {
                stats.end = StreamEnd::EntryBudget;
                break;
            }
            stats.pages += 1;
            let result = match self.ctg.mapped_page(page) {
                Some(bytes) => Ok(bytes),
                None => self.ctg.read_page(page, &mut scratch).map(|()| &scratch[..]),
            };
            let entries = result.and_then(|bytes| self.ctg.entries(page, bytes));
            match entries {
                Err(error) => {
                    if !report_damage(options.policy, &mut stats, &mut sink, page, error)? {
                        stats.end = StreamEnd::SinkStopped;
                        break;
                    }
                }
                Ok(mut entries) => {
                    let mut broken = false;
                    loop {
                        if cancelled() {
                            stats.end = StreamEnd::Cancelled;
                            break 'pages;
                        }
                        if entries.closed() {
                            break;
                        }
                        if options.max_entries.is_some_and(|max| stats.entries >= max) {
                            stats.end = StreamEnd::EntryBudget;
                            break 'pages;
                        }
                        let Some(next) = entries.next() else {
                            break;
                        };
                        let entry = match next {
                            Ok(entry) => entry,
                            Err(error) => {
                                broken = true;
                                if !report_damage(
                                    options.policy,
                                    &mut stats,
                                    &mut sink,
                                    page,
                                    error.into_error(self.ctg.path()),
                                )? {
                                    stats.end = StreamEnd::SinkStopped;
                                    break 'pages;
                                }
                                break;
                            }
                        };
                        stats.entries += 1;
                        let event =
                            if options.decode { decode(self.ctg.path(), entry) } else { Ok(StreamEvent::Raw(entry)) };
                        match event {
                            Ok(event) => {
                                stats.delivered += 1;
                                if !sink(event) {
                                    stats.end = StreamEnd::SinkStopped;
                                    break 'pages;
                                }
                            }
                            Err(error) => {
                                if !report_damage(options.policy, &mut stats, &mut sink, page, error)? {
                                    stats.end = StreamEnd::SinkStopped;
                                    break 'pages;
                                }
                            }
                        }
                    }
                    if !broken && !entries.closed() {
                        let error = Error::corrupt(
                            self.ctg.path(),
                            u64::from(page) * PAGE_SIZE as u64,
                            "entries do not end at the bytes-used mark",
                        );
                        if !report_damage(options.policy, &mut stats, &mut sink, page, error)? {
                            stats.end = StreamEnd::SinkStopped;
                            break;
                        }
                    }
                }
            }
            if options.progress_every_pages != 0
                && stats.pages.is_multiple_of(options.progress_every_pages)
                && !sink(StreamEvent::Progress(stats))
            {
                stats.end = StreamEnd::SinkStopped;
                break;
            }
        }
        if stats.end == StreamEnd::Running {
            stats.end = StreamEnd::Exhausted;
        }
        Ok(stats)
    }
}

fn report_damage(
    policy: StreamPolicy,
    stats: &mut StreamStats,
    sink: &mut impl for<'a> FnMut(StreamEvent<'a>) -> bool,
    page: u32,
    error: Error,
) -> Result<bool> {
    if policy == StreamPolicy::Strict {
        return Err(error);
    }
    stats.errors += 1;
    Ok(sink(StreamEvent::Damage { page, error: &error }))
}

fn decode<'a>(path: &Path, entry: Entry<'a>) -> Result<StreamEvent<'a>> {
    let bad = |detail: &str| Error::corrupt(path, entry.offset, detail);
    let raw = position::decode(entry.key).map_err(|_| bad("invalid CTG position key"))?;
    let board = ctg::board_of(&raw).ok_or_else(|| bad("invalid CTG board"))?;
    let stored = ctg::stored_form(&board);
    if !ctg::key_candidates(&stored).any(|(key, len)| &key[..len] == entry.key) {
        return Err(bad("CTG key round trip failed"));
    }
    let position = book_position(&board, &stored, &entry);
    if position.moves.iter().any(|m| m.mv.is_err()) {
        return Err(bad("unknown or illegal CTG move"));
    }
    Ok(StreamEvent::Decoded { raw: entry, board, position })
}
