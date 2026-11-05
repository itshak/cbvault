// MIT-licensed layout adapted from oschess-cb-bridge v1.0.3
// (commit bffebfc498d52b4e4631aed32b2f8e8194d125ff).
// Copyright (c) 2026 the oschess-cb-bridge contributors.
// Modifications copyright (c) 2026 the cbvault contributors.
// SPDX-License-Identifier: MIT
//! Read-only .2lid registry: zero allocation per warmed entity lookup.
//!
//! The registry stores fixed-size entity containers in interleaved type
//! blocks, one block per entity id. Each container starts with a little-endian
//! payload byte length. Names in the payload are length-prefixed UTF-8.

use std::path::PathBuf;
use crate::error::{Error, Result};
use crate::file::DbFile;

/// Player-name entities.
pub const PLAYER: usize = 0;
/// Tournament and venue entities.
pub const TOURNAMENT: usize = 1;
/// Source-name entities.
pub const SOURCE: usize = 2;
/// Team-name entities.
pub const TEAM: usize = 4;
/// Multilingual game-text title entities.
pub const GAME_TAG: usize = 5;
const TYPES_MAX: usize = 32;
const MAX_CONTAINER: usize = 1 << 20;
const MAX_HEADER: usize = 64 << 10;

/// One type's container shape in the interleaved registry.
#[derive(Clone, Copy, Debug, Default)]
struct Kind {
    size: usize,
    count: i64,
    offset: u64,
}

/// The .2lid namebase, borrowed from a mapped file wherever possible.
#[derive(Debug)]
pub struct Entities {
    file: DbFile,
    length: u64,
    head_bytes: u64,
    kinds: [Kind; TYPES_MAX],
    types: usize,
    block_bytes: u64,
}

fn be_i32(b: &[u8], o: usize) -> i32 {
    i32::from_be_bytes(b[o..o + 4].try_into().expect("checked header"))
}
fn be_i64(b: &[u8], o: usize) -> i64 {
    i64::from_be_bytes(b[o..o + 8].try_into().expect("checked header"))
}

impl Entities {
    /// Open and validate .2lid without copying its contents.
    pub fn open(path: PathBuf) -> Result<Self> {
        let file = DbFile::open(path.clone())?;
        let length = file.size()?;
        if length < 8 {
            return Err(Error::corrupt(&path, 0, ".2lid: missing header"));
        }
        let mut head = [0u8; 8 + 20 * TYPES_MAX];
        file.read_into(0, &mut head[..8])?;
        let header_size = be_i32(&head, 0);
        let count = be_i32(&head, 4);
        if !(1..=TYPES_MAX as i32).contains(&count) {
            return Err(Error::corrupt(&path, 4, ".2lid: invalid type count"));
        }
        let types = count as usize;
        let min_header = 8 + 20 * types;
        if header_size < min_header as i32 || header_size as usize > MAX_HEADER || header_size as u64 > length {
            return Err(Error::corrupt(&path, 0, ".2lid: invalid header size"));
        }
        file.read_into(8, &mut head[8..min_header])?;
        let mut kinds = [Kind::default(); TYPES_MAX];
        let mut block_bytes = 0u64;
        for (i, kind) in kinds[..types].iter_mut().enumerate() {
            let o = 8 + i * 20;
            let sz = be_i32(&head, o);
            let n = be_i64(&head, o + 4);
            if sz < 0 || sz as usize > MAX_CONTAINER || n < 0 {
                return Err(Error::corrupt(&path, o as u64, ".2lid: invalid container size/count"));
            }
            *kind = Kind { size: sz as usize, count: n, offset: block_bytes };
            block_bytes = block_bytes
                .checked_add(sz as u64)
                .ok_or_else(|| Error::corrupt(&path, o as u64, ".2lid: block size overflow"))?;
        }
        Ok(Self { file, length, head_bytes: header_size as u64, kinds, types, block_bytes })
    }

    /// Number of IDs the file header declares for the given registry kind.
    pub fn count(&self, kind: usize) -> i64 {
        if kind >= self.types { 0 } else { self.kinds[kind].count }
    }

    fn location(&self, kind: usize, id: i64) -> Option<(u64, usize)> {
        let k = self.kinds.get(kind).filter(|_| kind < self.types)?;
        if id < 0 || id >= k.count || k.size < 4 { return None; }
        let start = (id as u64).checked_mul(self.block_bytes)?
            .checked_add(self.head_bytes)?.checked_add(k.offset)?;
        let end = start.checked_add(k.size as u64)?;
        (end <= self.length).then_some((start, k.size))
    }

    /// Borrows the payload of a valid entity (no copying with mmap). If
    /// unmapped, reuses the caller's `scratch` capacity for positional reads.
    /// An empty/unused id returns None, malformed lengths fail closed.
    pub fn raw_ref<'a>(&'a self, kind: usize, id: i64, scratch: &'a mut Vec<u8>) -> Result<Option<&'a [u8]>> {
        let Some((offset, size)) = self.location(kind, id) else { return Ok(None) };
        if let Some(mapped) = self.file.slice_at(offset, size) {
            return self.payload(offset, mapped);
        }
        scratch.clear();
        scratch.resize(size, 0);
        self.file.read_into(offset, scratch)?;
        self.payload(offset, scratch)
    }

    fn payload<'a>(&self, offset: u64, raw: &'a [u8]) -> Result<Option<&'a [u8]>> {
        let n = i32::from_le_bytes(raw[..4].try_into().expect("container has at least four bytes"));
        if n == 0 { return Ok(None); }
        if n < 0 || n as usize > raw.len() - 4 {
            return Err(Error::corrupt(self.file.path(), offset, ".2lid: entity length outside container"));
        }
        Ok(Some(&raw[4..4 + n as usize]))
    }

    /// Resolve player ID into name parts. Reuses both string buffers.
    /// Returns false for an empty/missing ID, and errors on malformed data.
    pub fn player_into(&self, id: i64, scratch: &mut Vec<u8>, last: &mut String, first: &mut String) -> Result<bool> {
        last.clear();
        first.clear();
        let Some(payload) = self.raw_ref(PLAYER, id, scratch)? else { return Ok(false) };
        let mut reader = Cursor::new(payload);
        let a = reader.string(self.file.path())?;
        let b = reader.string(self.file.path())?;
        append_text(last, a);
        append_text(first, b);
        Ok(true)
    }

    /// Resolve tournament title/place into reused strings, skipping its
    /// packed start date (the game's date comes from the game header).
    pub fn tournament_into(&self, id: i64, scratch: &mut Vec<u8>, title: &mut String, place: &mut String) -> Result<bool> {
        title.clear();
        place.clear();
        let Some(payload) = self.raw_ref(TOURNAMENT, id, scratch)? else { return Ok(false) };
        let mut reader = Cursor::new(payload);
        let p = reader.string(self.file.path())?;
        let t = reader.string(self.file.path())?;
        let _ = reader.take(4, self.file.path())?;
        append_text(place, p);
        append_text(title, t);
        Ok(true)
    }

    /// Resolve an entity type whose payload is two length-prefixed names, for
    /// sources/teams. The first name is the display value.
    pub fn first_field_into(&self, kind: usize, id: i64, scratch: &mut Vec<u8>, out: &mut String) -> Result<bool> {
        out.clear();
        let Some(payload) = self.raw_ref(kind, id, scratch)? else { return Ok(false) };
        let mut reader = Cursor::new(payload);
        append_text(out, reader.string(self.file.path())?);
        Ok(true)
    }

    /// Resolve a guiding-text/analysis title from the first nonempty
    /// language-specific string in its game-tag entity.
    pub fn title_into(&self, id: i64, scratch: &mut Vec<u8>, out: &mut String) -> Result<bool> {
        out.clear();
        let Some(payload) = self.raw_ref(GAME_TAG, id, scratch)? else { return Ok(false) };
        let mut reader = Cursor::new(payload);
        let n = reader.i32(self.file.path())?;
        if n < 0 || (n as usize) > payload.len() / 8 {
            return Err(Error::corrupt(self.file.path(), 0, ".2lid: invalid title language count"));
        }
        for _ in 0..n {
            let _language = reader.i32(self.file.path())?;
            let title = reader.string(self.file.path())?;
            if !title.is_empty() {
                append_text(out, title);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Direct access to the underlying positional-read/mmap file.
    pub fn db_file(&self) -> &DbFile { &self.file }
}

/// Preserve UTF-8, lossily replacing malformed bytes only at the output
/// boundary. A warm `String` writes without allocation for typical names.
fn append_text(into: &mut String, src: &[u8]) {
    match std::str::from_utf8(src) {
        Ok(text) => into.push_str(text),
        Err(_) => into.push_str(&String::from_utf8_lossy(src)),
    }
}

struct Cursor<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, offset: 0 } }
    fn take(&mut self, n: usize, path: &std::path::Path) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(n)
            .ok_or_else(|| Error::corrupt(path, self.offset as u64, ".2lid: overflowing entity field"))?;
        let bytes = self.bytes.get(self.offset..end)
            .ok_or_else(|| Error::corrupt(path, self.offset as u64, ".2lid: truncated entity field"))?;
        self.offset = end;
        Ok(bytes)
    }
    fn i32(&mut self, path: &std::path::Path) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4, path)?.try_into().expect("four bytes")))
    }
    fn string(&mut self, path: &std::path::Path) -> Result<&'a [u8]> {
        let n = self.i32(path)?;
        if n < 0 { return Err(Error::corrupt(path, self.offset as u64, ".2lid: negative text length")); }
        self.take(n as usize, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_rejects_truncated_names() {
        let path = std::path::Path::new("fixture.2lid");
        let mut c = Cursor::new(&[5, 0, 0, 0, b'A']);
        assert!(c.string(path).is_err());
    }
}
