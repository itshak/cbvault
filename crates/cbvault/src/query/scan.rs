//! Running a [`GameDataQuery`]: the parallel header scan, then the ending
//! stage on the survivors.

use cbvault_format::error::Error;
use rayon::prelude::*;

use super::header::{Compiled, Record};
use super::{GameDataQuery, QueryResult, ending};
use crate::bridge::{Database, GameBuf, Generation};

/// Records per chunk of the header scan: the bridge's own scan batch.
const CHUNK: u32 = 8_192;
/// Games per chunk of the ending stage, which replays every game it is given.
const ENDING_CHUNK: usize = 512;

/// How to run a search.
#[derive(Clone, Copy, Default)]
pub struct SearchOptions<'a> {
    /// Worker threads; 0 or 1 searches on the calling thread.
    pub threads: usize,
    /// The first game number searched; 0 or 1 is the first game.
    pub first: u32,
    /// The last game number searched; 0 is the last game.
    pub last: u32,
    /// Polled between chunks; `true` stops the search with what it found so
    /// far, and [`Found::complete`] false.
    pub cancelled: Option<&'a (dyn Fn() -> bool + Sync)>,
}

/// What a search found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The matching game numbers, ascending.
    pub ids: Vec<u32>,
    /// Header records scanned.
    pub records: u64,
    /// Games replayed by the ending stage.
    pub replayed: u64,
    /// Games the ending stage could not replay; they are not matches.
    pub failed: u64,
    /// Whether the search ran to the end, rather than being cancelled.
    pub complete: bool,
}

/// Every game of `db` matching `query`, on `threads` threads.
pub fn search(db: &Database, query: &GameDataQuery, threads: usize) -> QueryResult<Found> {
    search_with(db, query, SearchOptions { threads, ..SearchOptions::default() })
}

/// [`search`] with every option.
pub fn search_with(db: &Database, query: &GameDataQuery, options: SearchOptions<'_>) -> QueryResult<Found> {
    let compiled = Compiled::new(db, query)?;
    let cancelled = || options.cancelled.is_some_and(|f| f());
    let total = db.records();
    let last = if options.last == 0 { total } else { options.last.min(total) };
    let mut chunks = Vec::new();
    let mut at = options.first.max(1);
    while at <= last {
        let end = at.saturating_add(CHUNK - 1).min(last);
        chunks.push((at, end));
        at = end + 1;
    }

    let scan_chunk = |&(first, last): &(u32, u32)| -> Result<Option<Vec<u32>>, Error> {
        if cancelled() {
            return Ok(None);
        }
        let mut out = Vec::new();
        match db.generation() {
            Generation::Classic => {
                for header in db.record_batch(first, last)? {
                    if keep(&compiled, &header)
                        && compiled.extra.as_ref().is_none_or(|f| f.matches(header.id(), &header))
                    {
                        out.push(header.id());
                    }
                }
            }
            _ => {
                for id in first..=last {
                    let header = db.header_ref(id)?;
                    if keep(&compiled, &header) && compiled.extra.as_ref().is_none_or(|f| f.matches(id, &header)) {
                        out.push(id);
                    }
                }
            }
        }
        Ok(Some(out))
    };
    let parts = run(db, options.threads, &chunks, scan_chunk)?;
    let mut complete = parts.iter().all(Option::is_some);
    let records = chunks
        .iter()
        .zip(&parts)
        .filter(|(_, part)| part.is_some())
        .map(|((first, last), _)| u64::from(last - first + 1))
        .sum();
    let mut ids: Vec<u32> = parts.into_iter().flatten().flatten().collect();

    let (mut replayed, mut failed) = (0, 0);
    if compiled.endings.is_set() && complete {
        let batches: Vec<&[u32]> = ids.chunks(ENDING_CHUNK).collect();
        let replay_chunk = |batch: &&[u32]| -> Result<Option<(Vec<u32>, u64)>, Error> {
            if cancelled() {
                return Ok(None);
            }
            let mut buf = GameBuf::new();
            let (mut kept, mut bad) = (Vec::new(), 0u64);
            for &id in batch.iter() {
                // A game that cannot be read or replayed is not a match; it is
                // counted, so a caller can tell "no mate" from "unreadable".
                match db.game_with(id, &mut buf, false).ok().and_then(|game| ending::ending(&game)) {
                    Some(end) if compiled.endings.accepts(end) => kept.push(id),
                    Some(_) => {}
                    None => bad += 1,
                }
            }
            Ok(Some((kept, bad)))
        };
        let parts = run(db, options.threads, &batches, replay_chunk)?;
        complete = parts.iter().all(Option::is_some);
        replayed = batches.iter().zip(&parts).filter(|(_, p)| p.is_some()).map(|(b, _)| b.len() as u64).sum();
        let mut kept = Vec::new();
        for (part, bad) in parts.into_iter().flatten() {
            kept.extend(part);
            failed += bad;
        }
        ids = kept;
    }
    Ok(Found { ids, records, replayed, failed, complete })
}

/// Whether a header record is searched and passes the compiled query.
#[inline]
fn keep<H: Record>(compiled: &Compiled, header: &H) -> bool {
    !header.is_deleted() && compiled.wants(header.kind()) && compiled.matches(header)
}

/// `work` over `items`, in order, on `threads` threads.
fn run<T: Sync, R: Send>(
    db: &Database,
    threads: usize,
    items: &[T],
    work: impl Fn(&T) -> Result<R, Error> + Sync + Send,
) -> Result<Vec<R>, Error> {
    if threads <= 1 || items.len() <= 1 {
        return items.iter().map(work).collect();
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| Error::corrupt(db.base(), 0, format!("thread pool creation failed: {e}")))?;
    pool.install(|| items.par_iter().map(&work).collect())
}
