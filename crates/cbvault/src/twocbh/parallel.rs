// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Bounded, record-ordered parallel 2CBH PGN export.
//!
//! Worker-local PgnWriter and ReadScratch avoid per-game heap churn.
//! At most 2× the selected thread count of chunks can be outstanding, even
//! when an early slow chunk blocks the ordered writer stage. This prevents
//! multi-GB PGN buffering while retaining reproducible exact output order.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;

use cbvault_format::game::RecordKind;
use crate::pgn::PgnWriter;
use super::{Database, ReadScratch, ScanStats, io_error};

type ExportChunk = io::Result<(Vec<u8>, ScanStats)>;

fn merge_into(a: &mut ScanStats, b: ScanStats) {
    a.records += b.records;
    a.games += b.games;
    a.deleted += b.deleted;
    a.texts += b.texts;
    a.analyses += b.analyses;
    a.verified += b.verified;
    a.failures += b.failures;
}

impl Database {
    /// Export records from inclusive `first` to inclusive `last`, or to
    /// the end when `last=0`, in byte-identical source order. Threads own
    /// their chess/writer/record scratch. Bounded in-flight chunks control
    /// memory regardless of input size or worker scheduling.
    pub fn export_parallel(
        &self, out: &mut impl Write, first: u32, last: u32,
        threads: usize, batch_size: usize,
    ) -> io::Result<ScanStats> {
        if threads == 0 || batch_size == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "threads and batch_size must be positive"));
        }
        let first = first.max(1) as u64;
        let last = if last == 0 { self.records() as u64 } else { (last as u64).min(self.records() as u64) };
        if first > last { return Ok(ScanStats::default()); }
        let nrecords = last - first + 1;
        let size = batch_size as u64;
        let chunks = nrecords.div_ceil(size);
        let workers = (threads as u64).min(chunks) as usize;
        let window = workers.saturating_mul(2).max(2) as u64;

        if workers == 1 {
            let mut writer = PgnWriter::new();
            let mut scratch = ReadScratch::default();
            return self.export_chunk(out, first, last, &mut writer, &mut scratch);
        }

        let next = AtomicU64::new(0);
        let completed = AtomicU64::new(0);
        let stopped = AtomicBool::new(false);

        thread::scope(|scope| -> io::Result<ScanStats> {
            let (tx, rx) = mpsc::sync_channel::<(u64, ExportChunk)>(window as usize);
            for _ in 0..workers {
                let sender = tx.clone();
                let next = &next;
                let completed = &completed;
                let stopped = &stopped;
                scope.spawn(move || {
                    let mut writer = PgnWriter::new();
                    let mut scratch = ReadScratch::default();
                    loop {
                        if stopped.load(Ordering::Acquire) { break; }
                        let index = next.fetch_add(1, Ordering::AcqRel);
                        if index >= chunks { break; }
                        // Claim a bounded window: this prevents workers from
                        // outrunning an early, unusually expensive batch.
                        while index >= completed.load(Ordering::Acquire).saturating_add(window) {
                            if stopped.load(Ordering::Acquire) { return; }
                            thread::yield_now();
                        }
                        let lo = first + index * size;
                        let hi = (lo + size - 1).min(last);
                        let mut bytes = Vec::new();
                        let result = self.export_chunk(&mut bytes, lo, hi, &mut writer, &mut scratch)
                            .map(|stats| (bytes, stats));
                        if sender.send((index, result)).is_err() { return; }
                    }
                });
            }
            drop(tx);

            let mut pending = BTreeMap::new();
            let mut expected = 0u64;
            let mut result = ScanStats::default();
            while expected < chunks {
                let (index, batch) = rx.recv().map_err(|_| {
                    stopped.store(true, Ordering::Release);
                    io::Error::other("2CBH parallel worker ended without returning its chunk")
                })?;
                pending.insert(index, batch);
                while let Some(batch) = pending.remove(&expected) {
                    let (bytes, stats) = match batch {
                        Ok(good) => good,
                        Err(err) => {
                            stopped.store(true, Ordering::Release);
                            return Err(err);
                        }
                    };
                    if let Err(err) = out.write_all(&bytes) {
                        stopped.store(true, Ordering::Release);
                        return Err(err);
                    }
                    merge_into(&mut result, stats);
                    expected += 1;
                    completed.store(expected, Ordering::Release);
                }
            }
            Ok(result)
        })
    }

    fn export_chunk(
        &self, out: &mut impl Write, first: u64, last: u64,
        writer: &mut PgnWriter, scratch: &mut ReadScratch,
    ) -> io::Result<ScanStats> {
        let mut stats = ScanStats::default();
        for id in first..=last {
            let id = id as u32;
            let header = self.record(id).map_err(io_error)?;
            stats.records += 1;
            if header.is_deleted() { stats.deleted += 1; continue; }
            match header.kind() {
                RecordKind::Game => {
                    self.write_game_reuse(id, writer, out, scratch)?;
                    stats.games += 1;
                    stats.verified += 1;
                }
                RecordKind::Text => stats.texts += 1,
                RecordKind::Analysis => stats.analyses += 1,
                RecordKind::Unknown(kind) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData, format!("unknown 2CBH record kind {kind} at record {id}"),
                    ));
                }
            }
        }
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbvault_format::twocbh::testdata::{Game, remove_set, write_set};

    #[test]
    fn parallel_and_sequential_v2_pgn_are_identical() {
        let records: Vec<_> = (1..=12).map(|id| Game::new(id as u64, id, id + 1)).collect();
        let stem = write_set("parallel-identical", &records, true);
        let db = Database::open(&stem).unwrap();
        let mut serial = Vec::new();
        let mut parallel = Vec::new();
        let before = db.export_parallel(&mut serial, 1, 0, 1, 3).unwrap();
        let after = db.export_parallel(&mut parallel, 1, 0, 4, 2).unwrap();
        assert_eq!(serial, parallel, "ordered worker scheduling changed PGN bytes");
        assert_eq!(before, after);
        assert_eq!(before.verified, 12);
        remove_set(&stem);
    }
}
