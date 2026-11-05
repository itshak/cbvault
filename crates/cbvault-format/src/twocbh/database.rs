// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Standalone read-only 2CBH database. Reader and sidecar resolution only;
//! move legality and PGN remain the responsibility of cbvault-chess/cbvault.

use super::{Entities, GameHeader, Headers, Segment, SegmentRecord, sibling};
use crate::error::{Error, Result, Role};
use std::path::{Path, PathBuf};

/// Open database family, with entity names and optional annotation frames.
#[derive(Debug)]
pub struct Database {
    stem: PathBuf,
    headers: Headers,
    moves: Segment,
    annotations: Option<Segment>,
    entities: Entities,
}

impl Database {
    /// Open a 2CBH family by stem or any of its six known sidecar names.
    /// All files are opened read-only; no namebase index is ever written.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let base = match path.extension().and_then(|e| e.to_str()) {
            Some(ext)
                if ["2cbh", "2cbg", "2cba", "2lid", "2lcd", "2lgd"].iter().any(|v| v.eq_ignore_ascii_case(ext)) =>
            {
                path.with_extension("")
            }
            _ => path.to_path_buf(),
        };
        let hp = sibling(&base, ".2cbh");
        if !hp.is_file() {
            return Err(Error::MissingFile { path: hp, role: Role::Headers });
        }
        let mp = sibling(&base, ".2cbg");
        if !mp.is_file() {
            return Err(Error::MissingFile { path: mp, role: Role::Moves });
        }
        let ep = sibling(&base, ".2lid");
        if !ep.is_file() {
            return Err(Error::MissingFile { path: ep, role: Role::Entities });
        }
        let headers = Headers::open_path(hp)?;
        let moves = Segment::open(mp)?;
        let entities = Entities::open(ep)?;
        let ap = sibling(&base, ".2cba");
        let annotations = if ap.is_file() { Some(Segment::open(ap)?) } else { None };
        Ok(Self { stem: base, headers, moves, annotations, entities })
    }

    /// Base path shared by the set.
    pub fn stem(&self) -> &Path {
        &self.stem
    }
    /// The 192-byte record index.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }
    /// All record slots excluding the file header, including deleted/text/analysis.
    pub fn record_count(&self) -> u32 {
        self.headers.games()
    }
    /// The required player/tournament namebase.
    pub fn entities(&self) -> &Entities {
        &self.entities
    }
    /// The required move frames.
    pub fn moves(&self) -> &Segment {
        &self.moves
    }
    /// The optional annotation frames.
    pub fn annotations(&self) -> Option<&Segment> {
        self.annotations.as_ref()
    }
    /// Read the 192-byte metadata record for one 1-based record ID.
    pub fn record(&self, id: u32) -> Result<GameHeader> {
        self.headers.record(id)
    }

    /// Read a record's move frame. The frame borrows mapped bytes when possible,
    /// otherwise holds its own buffer. For the unmapped zero-allocation path,
    /// call `moves().record_into(offset, scratch)`.
    pub fn moves_of(&self, record: &GameHeader) -> Result<SegmentRecord<'_>> {
        let at = u64::try_from(record.moves_offset())
            .map_err(|_| Error::corrupt(self.moves.path(), 0, "negative .2cbg move offset"))?;
        self.moves.record_at(at)
    }

    /// Read the optional annotation frame; a zero offset denotes none.
    pub fn annotations_of(&self, record: &GameHeader) -> Result<Option<SegmentRecord<'_>>> {
        if record.annotations_offset() == 0 {
            return Ok(None);
        }
        let segment = self
            .annotations
            .as_ref()
            .ok_or_else(|| Error::MissingFile { path: sibling(&self.stem, ".2cba"), role: Role::Annotations })?;
        let at = u64::try_from(record.annotations_offset())
            .map_err(|_| Error::corrupt(segment.path(), 0, "negative .2cba offset"))?;
        segment.record_at(at).map(Some)
    }
}
