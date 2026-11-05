// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Native read-only .2cbh database operations, using the same gigachess
//! legality and PGN writer as the established classic .cbh reader.
//! This is an independent entry point until the consumer-facing classic
//! GameRef representation can model both 46-byte and 192-byte headers.
//! No malformed game or annotation may silently become a partial PGN.

use std::io::{self, Write};
use std::path::Path;

use cbvault_chess::decode::GameRef;
use cbvault_chess::v2::V2Scratch;
use cbvault_format::error::{Error, Result};
use cbvault_format::game::RecordKind;
use cbvault_format::twocbh::{self as v2, annotation_items::GameAnnotations, moves::GameMoves};

use crate::pgn::PgnWriter;

mod parallel;

/// Summary of a read-only v2 database scan. A record may be deleted, a
/// guiding text, an analysis, or a playable game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// All non-header records examined.
    pub records: u64,
    /// Live game records.
    pub games: u64,
    /// Deleted records excluded from game output.
    pub deleted: u64,
    /// Guiding-text records, not playable games.
    pub texts: u64,
    /// Analysis records, not playable games.
    pub analyses: u64,
    /// Games whose move tree was checked successfully.
    pub verified: u64,
    /// Games that failed framing, annotations or legal replay.
    pub failures: u64,
}

/// Reusable non-mmap input buffers: steady-state frame reads borrow an mmap
/// or read into these preallocated vectors, rather than allocate per game.
#[derive(Debug, Default)]
pub struct ReadScratch {
    /// Caller-owned .2cbg frame storage for positional I/O.
    pub moves: Vec<u8>,
    /// Caller-owned .2cba frame storage for positional I/O.
    pub annotations: Vec<u8>,
}
impl ReadScratch {
    /// Reserved scratch memory, without any live record-specific allocation.
    pub fn capacity(&self) -> usize { self.moves.capacity() + self.annotations.capacity() }
}

/// Read a valid frame directly from the mapping, or into the caller's
/// capacity-reused buffer when mmap is disabled.
fn borrowed_frame<'a>(
    segment: &'a v2::Segment,
    at: u64,
    scratch: &'a mut Vec<u8>,
) -> Result<&'a [u8]> {
    let size = segment.total_at(at)? as usize;
    if let Some(raw) = segment.db_file().slice_at(at, size) {
        v2::Record::parse(segment.path(), at, raw)?;
        return Ok(raw);
    }
    segment.record_into(at, scratch)?;
    Ok(scratch)
}

/// Full native v2 database handle, owning headers, frame files and .2lid.
#[derive(Debug)]
pub struct Database {
    inner: v2::Database,
}

impl Database {
    /// Open a v2 database by stem or any recognised companion filename.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self { inner: v2::Database::open(path)? })
    }
    /// Read the byte-level native v2 database.
    pub fn inner(&self) -> &v2::Database { &self.inner }
    /// Number of record slots, including non-game entries and deletions.
    pub fn records(&self) -> u32 { self.inner.record_count() }
    /// The 192-byte metadata of one record.
    pub fn record(&self, id: u32) -> Result<v2::GameHeader> { self.inner.record(id) }

    /// Verify every live playable game. A damaged record is reported to the
    /// caller and the scan continues. Unknown record classifications are not
    /// counted as playable games; they are reported as failures.
    pub fn verify(&self, mut report: impl FnMut(u32, &Error)) -> Result<ScanStats> {
        let mut stats = ScanStats::default();
        let mut scratch = V2Scratch::default();
        let mut buffers = ReadScratch::default();
        for id in 1..=self.records() {
            let h = self.record(id)?;
            stats.records += 1;
            if h.is_deleted() { stats.deleted += 1; continue; }
            match h.kind() {
                RecordKind::Text => { stats.texts += 1; continue; }
                RecordKind::Analysis => { stats.analyses += 1; continue; }
                RecordKind::Game => { stats.games += 1; }
                RecordKind::Unknown(_) => {
                    stats.failures += 1;
                    let error = Error::corrupt(self.inner.headers().path(), id as u64 * 192, "unknown 2CBH record type");
                    report(id, &error);
                    continue;
                }
            }
            let result = (|| -> Result<()> {
                let at = u64::try_from(h.moves_offset())
                    .map_err(|_| Error::corrupt(self.inner.moves().path(), 0, "negative 2CBH move frame offset"))?;
                let bytes = borrowed_frame(self.inner.moves(), at, &mut buffers.moves)?;
                let moves = GameMoves::parse(self.inner.moves().path(), bytes)?;
                let tree = cbvault_chess::v2::verify(GameRef::at(id, at), &moves, &mut scratch)?;
                if let Some((path, bytes)) = self.annotation_bytes(&h, &mut buffers.annotations)? {
                    let ann = GameAnnotations::parse(path, bytes)?;
                    ann.check_positions(tree.total_plies)?;
                }
                Ok(())
            })();
            match result {
                Ok(()) => stats.verified += 1,
                Err(e) => { stats.failures += 1; report(id, &e); }
            }
        }
        Ok(stats)
    }

    /// Borrow the optional annotated frame from the mapped file or a
    /// caller-owned pread buffer. Returns a typed error if the record
    /// references a missing .2cba companion file.
    fn annotation_bytes<'a>(
        &'a self, h: &v2::GameHeader, scratch: &'a mut Vec<u8>,
    ) -> Result<Option<(&'a Path, &'a [u8])>> {
        if h.annotations_offset() == 0 { return Ok(None); }
        let seg = self.inner.annotations().ok_or_else(|| Error::corrupt(
            self.inner.headers().path(), h.id() as u64 * 192, "2CBH record references missing .2cba",
        ))?;
        let at = u64::try_from(h.annotations_offset())
            .map_err(|_| Error::corrupt(seg.path(), 0, "negative 2CBH annotation offset"))?;
        let bytes = borrowed_frame(seg, at, scratch)?;
        Ok(Some((seg.path(), bytes)))
    }

    /// Export one active record as PGN, refusing unknown annotation layouts
    /// and illegal moves. The convenience wrapper owns temporary I/O storage;
    /// repeatedly exporting should use write_game_reuse instead.
    pub fn write_game(&self, id: u32, writer: &mut PgnWriter, out: &mut impl Write) -> io::Result<()> {
        let mut scratch = ReadScratch::default();
        self.write_game_reuse(id, writer, out, &mut scratch)
    }

    /// Export with caller-owned per-worker frame buffers. In the mapped mode
    /// reads borrow the original files; in pread mode warmed buffers eliminate
    /// per-game input allocations.
    pub fn write_game_reuse(
        &self, id: u32, writer: &mut PgnWriter, out: &mut impl Write,
        scratch: &mut ReadScratch,
    ) -> io::Result<()> {
        let h = self.record(id).map_err(io_error)?;
        if h.is_deleted() || h.kind() != RecordKind::Game {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not an active v2 game record"));
        }
        let at = u64::try_from(h.moves_offset()).map_err(|_| io_error(Error::corrupt(
            self.inner.moves().path(), 0, "negative .2cbg offset",
        )))?;
        let data = borrowed_frame(self.inner.moves(), at, &mut scratch.moves).map_err(io_error)?;
        let game = GameMoves::parse(self.inner.moves().path(), data).map_err(io_error)?;
        let annotation = self.annotation_bytes(&h, &mut scratch.annotations).map_err(io_error)?;
        let parsed = annotation.map(|(path, bytes)| GameAnnotations::parse(path, bytes).map_err(io_error)).transpose()?;
        writer.write_game_v2(out, &h, self.inner.entities(), &game, parsed.as_ref())
    }

    /// Export all active playable games into a writable sink in stable source
    /// record order. Deleted/guiding-text/analysis records are skipped; unknown
    /// types and failed games stop with a typed error, never partial success.
    /// The caller owns the output and may stream to a file without collecting
    /// the database in memory.
    pub fn export(&self, out: &mut impl Write, mut writer: PgnWriter) -> io::Result<ScanStats> {
        let mut stats = ScanStats::default();
        let mut scratch = ReadScratch::default();
        for id in 1..=self.records() {
            let h = self.record(id).map_err(io_error)?;
            stats.records += 1;
            if h.is_deleted() { stats.deleted += 1; continue; }
            match h.kind() {
                RecordKind::Text => stats.texts += 1,
                RecordKind::Analysis => stats.analyses += 1,
                RecordKind::Game => {
                    stats.games += 1;
                    // The writer checks the complete tree and all annotations
                    // before its single write_all per game.
                    self.write_game_reuse(id, &mut writer, out, &mut scratch)?;
                    stats.verified += 1;
                }
                RecordKind::Unknown(_) => {
                    return Err(io_error(Error::corrupt(
                        self.inner.headers().path(), id as u64 * 192, "unknown v2 record type",
                    )));
                }
            }
        }
        Ok(stats)
    }
}

fn io_error(error: Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbvault_format::twocbh::testdata::{Game, remove_set, write_set};
    #[test]
    fn empty_move_tree_exports_complete_pgn_with_player_entities() {
        let stem = write_set("v2-pgn-empty", &[Game::new(1, 1, 2)], true);
        let db = Database::open(&stem).unwrap();
        let mut out = Vec::new();
        db.write_game(1, &mut PgnWriter::new(), &mut out).unwrap();
        let pgn = String::from_utf8(out).unwrap();
        assert!(pgn.contains("[White \"Test1, Example\"]"), "{pgn}");
        assert!(pgn.contains("[Black \"Test2, Example\"]"), "{pgn}");
        assert!(pgn.contains("[PlyCount \"0\"]"), "{pgn}");
        assert!(pgn.ends_with("0-1\n\n"), "{pgn}");
        remove_set(&stem);
    }
}
