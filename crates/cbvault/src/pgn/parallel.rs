//! Parallel PGN export: the Rayon pipeline of [`crate::replay::verify_parallel`]
//! over the export path, with the output written in record order.
//!
//! Records are independent and the database files are memory-mapped, so the id
//! space is cut into chunks (8,192 records, the batch the replay path uses),
//! every worker renders whole chunks into its own buffer with a private
//! [`PgnWriter`], and one writer stage emits the chunks in id order — so the
//! byte stream is identical to the sequential export's, whatever the thread
//! count. Only a bounded number of chunks is in flight (one wave per worker
//! plus one), and the next wave renders while the current one is written, so
//! the *buffer* peak is a few MiB per worker rather than a database's worth of
//! text. The mapped files sit on top: a full-database export faults what it
//! touches into resident memory (1.9 GB on the reference set, `.cbj` skipped;
//! ~30–110 MB with `CBVAULT_NO_MMAP=1`, for roughly a quarter more wall time).

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use cbvault_format::cbh::{Annotations, Batch, Entities, Headers, Wide};
use cbvault_format::error::Result;
use cbvault_format::file::DbFile;
use cbvault_format::game::RecordKind;
use rayon::prelude::*;

use super::PgnWriter;

/// The records one chunk holds when the caller gives no batch size: the same
/// batch the replay path reads.
pub const DEFAULT_BATCH: u32 = 8192;

/// What an export wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportStats {
    /// The records walked.
    pub records: u64,
    /// The games written.
    pub games: u64,
    /// The guiding texts skipped.
    pub texts: u64,
    /// The bytes written.
    pub bytes: u64,
    /// The largest writer footprint (its buffers) any worker held, the
    /// high-water mark of one game's worth of text.
    pub peak_writer: u64,
    /// The records a worker could not write.
    pub failures: u64,
}

/// Writes the database's PGN to `out` across `threads` workers, in record
/// order, with the failures collected as `verify_parallel` collects them.
///
/// `threads` of 0 or 1 runs on the calling thread. The output is byte-for-byte
/// the sequential export's.
pub fn export_parallel(
    base: &Path,
    out: &mut impl Write,
    threads: usize,
    batch_size: u32,
    failure_limit: usize,
) -> Result<ExportStats> {
    export_range(base, out, threads, batch_size, failure_limit, 0)
}

/// [`export_parallel`] over the records `first..=last` of the id space, so a
/// tool can export a range (the gold comparison exports the gold range). Ids
/// are 1-based, as everywhere else; `last` 0 or below means the whole
/// database.
pub fn export_range(
    base: &Path,
    out: &mut impl Write,
    threads: usize,
    batch_size: u32,
    failure_limit: usize,
    last: u32,
) -> Result<ExportStats> {
    export_span(base, out, threads, batch_size, failure_limit, 1, last)
}

/// [`export_span`], named for the common case: everything from the first record
/// up to `last`.
pub fn export_range_from(
    base: &Path,
    out: &mut impl Write,
    threads: usize,
    batch_size: u32,
    failure_limit: usize,
    first: u32,
    last: u32,
) -> Result<ExportStats> {
    export_span(base, out, threads, batch_size, failure_limit, first, last)
}

/// Writes the records `first..=last` in record order. Ids are 1-based; a `last`
/// of 0 or below means "to the end", and a `first` of 0 or below is clamped to
/// 1, so a caller can pass an unset value straight through.
///
/// The output is byte-for-byte the sequential export's over the same span: a
/// range is a slice of the same stream, not a differently-shaped one.
pub fn export_span(
    base: &Path,
    out: &mut impl Write,
    threads: usize,
    batch_size: u32,
    failure_limit: usize,
    first: u32,
    last: u32,
) -> Result<ExportStats> {
    Ok(export_span_report(base, out, threads, batch_size, failure_limit, first, last)?.0)
}

/// [`export_span`], additionally returning the failure messages (up to
/// `failure_limit`), in record order — the [`crate::replay::verify_parallel`]
/// shape, so a caller can name what the stats count.
pub fn export_span_report(
    base: &Path,
    out: &mut impl Write,
    threads: usize,
    batch_size: u32,
    failure_limit: usize,
    first: u32,
    last: u32,
) -> Result<(ExportStats, Vec<String>)> {
    if cbvault_format::twocbh::sibling(base, ".2cbh").exists() {
        return export_span_report_v2(base, out, threads, batch_size, failure_limit, first, last);
    }
    let headers = Headers::open(base)?;
    let entities = Entities::open(base)?;
    let annotations = Annotations::open(base)?;
    // See `Database::open`: skip the `.cbj` below 4 GiB; `CBVAULT_WIDE=on` forces it.
    let wide = Wide::open_auto(base);
    let cbg_path = base.with_extension("cbg");
    let cbg = DbFile::open(cbg_path)?;
    let total = headers.records();

    let batch = if batch_size == 0 { DEFAULT_BATCH } else { batch_size };
    let first = first.max(1);
    let last = if last == 0 { total } else { last.min(total) };
    let mut chunks: Vec<(u32, u32)> = Vec::new();
    let mut id = first;
    while id <= last {
        let end = id.saturating_add(batch - 1).min(last);
        chunks.push((id, end));
        id = end + 1;
    }

    let failures = Mutex::new(Vec::new());
    let mut stats = ExportStats::default();
    let sink = &mut *out;

    // The chunk buffers, recycled: a megabase chunk is several megabytes, so
    // allocating one per chunk is a page-fault storm (1,361 chunks of the
    // reference database) on top of the realloc copy when the first guess at
    // the size is short. The writer stage hands each buffer back once its bytes
    // are out, so the pool holds about one buffer per worker.
    let buffers: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    // The largest chunk rendered so far, which sizes the buffers a worker
    // takes: a megabase chunk is ~5.6 MiB, and growing into it by doubling
    // would copy megabytes per worker before the pool settles.
    let high_water = AtomicUsize::new(0);

    // One wave of chunks per worker, each rendered into its own buffer; the
    // next wave renders while the current one is written.
    let render = |wave: &[(u32, u32)]| -> Vec<(u32, Vec<u8>, ExportStats)> {
        wave.par_iter()
            .map(|&(first, last)| {
                let mut bytes = buffers.lock().unwrap_or_else(|e| e.into_inner()).pop().unwrap_or_default();
                bytes.clear();
                let hint = high_water.load(Ordering::Relaxed);
                if bytes.capacity() < hint {
                    bytes.reserve(hint - bytes.len());
                }
                let s = render_chunk(
                    &headers,
                    &entities,
                    &annotations,
                    wide.as_ref(),
                    &cbg,
                    first,
                    last,
                    &mut bytes,
                    &failures,
                    failure_limit,
                );
                (first, bytes, s)
            })
            .collect()
    };

    let mut write = |rendered: Vec<(u32, Vec<u8>, ExportStats)>, stats: &mut ExportStats| {
        for (_, bytes, s) in rendered {
            let _ = (*sink).write_all(&bytes);
            stats.records += s.records;
            stats.games += s.games;
            stats.texts += s.texts;
            stats.bytes += s.bytes;
            stats.failures += s.failures;
            // The bytes are out: the buffer goes back for the next chunk, and
            // the high-water mark is what the next worker's buffer is sized for.
            high_water.fetch_max(bytes.len(), Ordering::Relaxed);
            if bytes.capacity() > 0 {
                buffers.lock().unwrap_or_else(|e| e.into_inner()).push(bytes);
            }
        }
    };

    if threads <= 1 {
        for wave in chunks.chunks(1) {
            write(render(wave), &mut stats);
        }
    } else {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| cbvault_format::error::Error::corrupt(base, 0, format!("thread pool creation failed: {e}")))?;
        // A wave of chunks per worker, rendered in the pool and written here
        // in id order: the workers borrow the open database, so nothing is
        // shared behind an `Arc`, and peak memory is one buffer per worker.
        for wave in chunks.chunks(threads) {
            let rendered = pool.install(|| render(wave));
            write(rendered, &mut stats);
        }
    }
    let _ = out.flush();
    let failures = failures.into_inner().unwrap_or_else(|e| e.into_inner());
    Ok((stats, failures))
}

/// One chunk: every game of it written through a private writer, its own
/// buffers, into `bytes`.
#[allow(clippy::too_many_arguments)]
fn render_chunk(
    headers: &Headers,
    entities: &Entities,
    annotations: &Annotations,
    wide: Option<&Wide>,
    cbg: &DbFile,
    first: u32,
    last: u32,
    bytes: &mut Vec<u8>,
    failures: &Mutex<Vec<String>>,
    failure_limit: usize,
) -> ExportStats {
    let mut stats = ExportStats::default();
    let mut writer = PgnWriter::new();
    let mut peak = 0usize;
    // Two scratch buffers: a parsed game borrows the first, an annotation
    // record borrows the second.
    let (mut move_scratch, mut ann_scratch) = (Vec::new(), Vec::new());
    let batch = match Batch::open(headers, cbg, wide, first, last) {
        Ok(b) => b,
        Err(e) => {
            record(failures, failure_limit, format!("batch open {first}..={last}: {e}"));
            stats.failures += 1;
            return stats;
        }
    };
    for header in batch.iter_records() {
        stats.records += 1;
        if header.is_deleted() {
            continue;
        }
        match header.kind() {
            RecordKind::Game => {}
            RecordKind::Text => {
                stats.texts += 1;
                continue;
            }
            _ => continue,
        }
        let id = header.id();
        let game = match batch.moves_of_ref(&header, &mut move_scratch) {
            Ok(g) => g,
            Err(e) => {
                record(failures, failure_limit, format!("game {id}: {e}"));
                stats.failures += 1;
                continue;
            }
        };
        let anns = match annotations.of_ref(&header, wide, &mut ann_scratch) {
            Ok(a) => Some(a),
            Err(e) => {
                record(failures, failure_limit, format!("game {id} annotations: {e}"));
                stats.failures += 1;
                None
            }
        };
        // A game whose moves will not write is a failure, never an exported
        // game: counting it would report games whose bytes are not in the
        // stream (seven such games on the reference set, all also rejected by
        // the ancestor). An unreadable annotation block alone keeps the game —
        // the failure above already named it — since the moves are intact.
        match writer.write_game(bytes, &header, entities, &game, anns.as_ref()) {
            Ok(()) => stats.games += 1,
            Err(e) => {
                record(failures, failure_limit, format!("game {id}: {e}"));
                stats.failures += 1;
                continue;
            }
        }
        // The writer's high-water mark: the largest game this worker has seen.
        peak = peak.max(writer.capacity());
    }
    stats.bytes = bytes.len() as u64;
    stats.peak_writer = peak as u64;
    stats
}

/// A failure message, up to the caller's limit — the `verify_parallel` shape.
fn record(failures: &Mutex<Vec<String>>, limit: usize, message: String) {
    let mut list = failures.lock().unwrap_or_else(|e| e.into_inner());
    if list.len() < limit {
        list.push(message);
    }
}

/// Exports 2CBH databases in parallel.
pub fn export_span_report_v2(
    base: &Path,
    out: &mut impl Write,
    threads: usize,
    batch_size: u32,
    failure_limit: usize,
    first: u32,
    last: u32,
) -> Result<(ExportStats, Vec<String>)> {
    let headers = cbvault_format::twocbh::Headers::open(base)?;
    let lid_path = cbvault_format::twocbh::sibling(base, ".2lid");
    let entities = cbvault_format::twocbh::Entities::open(lid_path)?;
    let cbg_path = cbvault_format::twocbh::sibling(base, ".2cbg");
    let cbg = cbvault_format::twocbh::Segment::open(cbg_path)?;
    let cba_path = cbvault_format::twocbh::sibling(base, ".2cba");
    let cba = if cba_path.exists() { Some(cbvault_format::twocbh::Segment::open(cba_path)?) } else { None };
    let total = headers.games();

    let batch = if batch_size == 0 { DEFAULT_BATCH } else { batch_size };
    let first = first.max(1);
    let last = if last == 0 { total } else { last.min(total) };
    let mut chunks: Vec<(u32, u32)> = Vec::new();
    let mut id = first;
    while id <= last {
        let end = id.saturating_add(batch - 1).min(last);
        chunks.push((id, end));
        id = end + 1;
    }

    let failures = Mutex::new(Vec::new());
    let mut stats = ExportStats::default();
    let sink = &mut *out;

    let buffers: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    let high_water = AtomicUsize::new(0);

    let render_chunk = |(start_id, end_id): (u32, u32)| -> (Vec<u8>, ExportStats) {
        let mut local_stats = ExportStats::default();
        let mut writer = PgnWriter::new();
        let target_capacity = high_water.load(Ordering::Relaxed);
        let mut buf = buffers.lock().unwrap().pop().unwrap_or_else(|| Vec::with_capacity(target_capacity));
        buf.clear();

        for rec_id in start_id..=end_id {
            local_stats.records += 1;
            let header = match headers.record(rec_id) {
                Ok(h) => h,
                Err(e) => {
                    local_stats.failures += 1;
                    record(&failures, failure_limit, format!("game {rec_id}: {e}"));
                    continue;
                }
            };

            if header.is_deleted() {
                continue;
            }
            match header.kind() {
                cbvault_format::game::RecordKind::Game | cbvault_format::game::RecordKind::Analysis => {}
                cbvault_format::game::RecordKind::Text => {
                    local_stats.texts += 1;
                    continue;
                }
                _ => continue,
            }

            let at = header.moves_offset();
            let seg_rec = match cbg.record_at(at) {
                Ok(r) => r,
                Err(e) => {
                    local_stats.failures += 1;
                    record(&failures, failure_limit, format!("game {rec_id}: {e}"));
                    continue;
                }
            };
            let moves = match cbvault_format::twocbh::moves::GameMoves::from_record(cbg.path(), seg_rec.record()) {
                Ok(m) => m,
                Err(e) => {
                    local_stats.failures += 1;
                    record(&failures, failure_limit, format!("game {rec_id}: {e}"));
                    continue;
                }
            };

            let anns_rec = if header.annotations_offset() != 0 {
                match &cba {
                    Some(segment) => match segment.record_at(header.annotations_offset()) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            local_stats.failures += 1;
                            record(&failures, failure_limit, format!("game {rec_id} annotations: {e}"));
                            continue;
                        }
                    },
                    None => {
                        local_stats.failures += 1;
                        record(&failures, failure_limit, format!("game {rec_id}: referenced 2CBA annotation file is missing"));
                        continue;
                    }
                }
            } else {
                None
            };
            let anns = anns_rec.as_ref().map(|r| cbvault_format::twocbh::Annotations::from_record(r.record()));

            match writer.write_game_v2(&mut buf, &header, &entities, &moves, anns.as_ref()) {
                Ok(()) => local_stats.games += 1,
                Err(e) => {
                    local_stats.failures += 1;
                    record(&failures, failure_limit, format!("game {rec_id}: {e}"));
                }
            }
        }

        high_water.fetch_max(buf.capacity(), Ordering::Relaxed);
        local_stats.bytes = buf.len() as u64;
        local_stats.peak_writer = writer.capacity() as u64;
        (buf, local_stats)
    };

    let merge_chunk = |stats: &mut ExportStats, s: &ExportStats| {
        stats.records += s.records;
        stats.games += s.games;
        stats.texts += s.texts;
        stats.bytes += s.bytes;
        stats.failures += s.failures;
        stats.peak_writer = stats.peak_writer.max(s.peak_writer);
    };

    if threads <= 1 {
        for chunk in chunks {
            let (buf, chunk_stats) = render_chunk(chunk);
            if let Err(e) = sink.write_all(&buf) {
                return Err(cbvault_format::error::Error::corrupt(base, 0, format!("write failed: {e}")));
            }
            merge_chunk(&mut stats, &chunk_stats);
            buffers.lock().unwrap().push(buf);
        }
    } else {
        use rayon::prelude::*;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| cbvault_format::error::Error::corrupt(base, 0, format!("thread pool creation failed: {e}")))?;
        let wave_size = threads * 2;
        for wave in chunks.chunks(wave_size) {
            let rendered: Vec<(Vec<u8>, ExportStats)> =
                pool.install(|| wave.par_iter().copied().map(render_chunk).collect());
            for (buf, chunk_stats) in rendered {
                if let Err(e) = sink.write_all(&buf) {
                    return Err(cbvault_format::error::Error::corrupt(base, 0, format!("write failed: {e}")));
                }
                merge_chunk(&mut stats, &chunk_stats);
                buffers.lock().unwrap().push(buf);
            }
        }
    }

    Ok((stats, failures.into_inner().unwrap()))
}
