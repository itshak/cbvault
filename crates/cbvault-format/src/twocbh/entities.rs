// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! `.2lid`: entity database for players, tournaments, sources, teams and game tags.

use std::borrow::Cow;
use std::ops::Range;
use std::path::Path;

use super::bytes::{be_i32, be_i64, le_i32, le_i64};
use crate::error::{Error, Result};
use crate::file::DbFile;
use crate::game::{Date, Player, Tournament};

/// Largest container size accepted; real ones are at most 1,120 bytes.
const MAX_CONTAINER: i32 = 1 << 20;
/// Largest entity-file header accepted.
const MAX_ENTITY_HEADER: usize = 64 << 10;
/// Largest block of one id's containers read whole.
const MAX_RUN_BLOCK: usize = 64 << 10;

/// Entity type index for players.
pub const PLAYER: usize = 0;
/// Entity type index for tournaments.
pub const TOURNAMENT: usize = 1;
/// Entity type index for game tags (titles).
pub const GAME_TAG: usize = 2;
/// Entity type index for annotators and sources.
pub const ANNOTATOR_SOURCE: usize = 3;
/// Synonym for sources.
pub const SOURCE: usize = 3;
/// Entity type index for teams.
pub const TEAM: usize = 4;

/// Two entity display fields, borrowed from the mapping or owned when necessary.
pub type EntityName<'a> = (Cow<'a, str>, Cow<'a, str>);

/// The `.2lid` entity file reader.
#[derive(Debug)]
pub struct Entities {
    file: DbFile,
    len: u64,
    header_size: usize,
    /// (container size, count, first deleted id) per type
    types: Vec<(usize, i64, i64)>,
    container_offset: Vec<usize>,
    block_size: usize,
}

impl Entities {
    /// Opens the entity file at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = DbFile::open(path.as_ref().to_path_buf())?;
        Self::new(file)
    }

    /// Wraps an already opened [`DbFile`].
    pub fn new(file: DbFile) -> Result<Self> {
        let len = file.len()?;
        if len < 8 {
            return Err(Error::corrupt(file.path(), 0, ".2lid too short for header"));
        }
        let bad = |what: String| Error::corrupt(file.path(), 0, format!(".2lid header: {what}"));
        let head = file.read(0, 8)?;
        let header_size_be = be_i32(&head, 0);
        let header_size_le = le_i32(&head, 0);
        let is_le = !(64..=MAX_ENTITY_HEADER as i32).contains(&header_size_be)
            && (64..=MAX_ENTITY_HEADER as i32).contains(&header_size_le);
        let (header_size, ntypes) =
            if is_le { (header_size_le, le_i32(&head, 4)) } else { (header_size_be, be_i32(&head, 4)) };
        if !(1..=32).contains(&ntypes) {
            return Err(bad(format!("{ntypes} entity types")));
        }
        let ntypes = ntypes as usize;
        let header_size = usize::try_from(header_size).map_err(|_| bad(format!("size {header_size}")))?;
        if 8 + 20 * ntypes > header_size || header_size as u64 > len || header_size > MAX_ENTITY_HEADER {
            return Err(bad(format!("size {header_size} for {ntypes} types in a {len}-byte file")));
        }
        let d = file.read(0, header_size)?;
        let mut types = Vec::with_capacity(ntypes);
        let mut container_offset = Vec::with_capacity(ntypes);
        let mut block_size = 0usize;
        for i in 0..ntypes {
            let o = 8 + 20 * i;
            let (size, count, first_del) = if is_le {
                (le_i32(&d, o), le_i64(&d, o + 4), le_i64(&d, o + 12))
            } else {
                (be_i32(&d, o), be_i64(&d, o + 4), be_i64(&d, o + 12))
            };
            if !(0..=MAX_CONTAINER).contains(&size) {
                return Err(bad(format!("type {i} container size {size}")));
            }
            if count < 0 {
                return Err(bad(format!("type {i} count {count}")));
            }
            types.push((size as usize, count, first_del));
            container_offset.push(block_size);
            block_size += size as usize;
        }
        Ok(Entities { file, len, header_size, types, container_offset, block_size })
    }

    /// Number of registered entities of type `typ`.
    pub fn count(&self, typ: usize) -> i64 {
        self.types.get(typ).map_or(0, |t| t.1)
    }

    /// Entity counts `[players, tournaments, annotators, sources]`.
    pub fn counts(&self) -> [u32; 4] {
        [
            self.count(PLAYER).max(0) as u32,
            self.count(TOURNAMENT).max(0) as u32,
            self.count(GAME_TAG).max(0) as u32,
            self.count(SOURCE).max(0) as u32,
        ]
    }

    /// Stored count of type `typ` bounded by file size.
    pub fn stored_count(&self, typ: usize) -> i64 {
        let Some(&(_, count, _)) = self.types.get(typ) else { return 0 };
        if self.block_size == 0 {
            return 0;
        }
        let start = (self.header_size + self.container_offset[typ]) as u64;
        let fit = self.len.saturating_sub(start).div_ceil(self.block_size as u64);
        count.min(i64::try_from(fit).unwrap_or(i64::MAX))
    }

    /// Raw payload bytes of entity `id` of type `typ`.
    pub fn raw(&self, typ: usize, id: i64) -> Result<Option<Vec<u8>>> {
        self.raw_within(typ, id, usize::MAX)
    }

    /// Reads at most `limit` bytes of entity `id` of type `typ`.
    pub fn raw_within(&self, typ: usize, id: i64, limit: usize) -> Result<Option<Vec<u8>>> {
        Ok(self.raw_cow_within(typ, id, limit)?.map(Cow::into_owned))
    }

    /// Reads the entity payload, borrowing the validated mapped bytes when
    /// possible and owning the positioned-read fallback otherwise.
    fn raw_cow_within(&self, typ: usize, id: i64, limit: usize) -> Result<Option<Cow<'_, [u8]>>> {
        let Some(&(size, count, _)) = self.types.get(typ) else { return Ok(None) };
        if id < 0 || id >= count {
            return Ok(None);
        }
        let Some(o) = u64::try_from(id)
            .ok()
            .and_then(|id| id.checked_mul(self.block_size as u64))
            .and_then(|o| o.checked_add(self.header_size as u64))
            .and_then(|o| o.checked_add(self.container_offset[typ] as u64))
        else {
            return Ok(None);
        };
        let want = (size as u64).min(self.len.saturating_sub(o)).min(limit.saturating_add(4) as u64) as usize;
        if want < 4 {
            return Ok(None);
        }
        let length = |buf: &[u8]| usize::try_from(le_i32(buf, 0)).ok().filter(|&n| n != 0 && n <= want - 4);
        if let Some(buf) = self.file.slice_at(o, want) {
            return Ok(length(buf).map(|n| Cow::Borrowed(&buf[4..4 + n])));
        }
        let mut buf = self.file.read(o, want)?;
        let Some(n) = length(&buf) else { return Ok(None) };
        buf.truncate(4 + n);
        buf.drain(..4);
        Ok(Some(Cow::Owned(buf)))
    }

    /// Player display fields, borrowed from the mapping for valid UTF-8.
    /// Unmapped data and invalid UTF-8 retain the old owned/lossy semantics.
    pub fn player_name(&self, id: i64) -> Result<Option<EntityName<'_>>> {
        Ok(match self.raw_cow_within(PLAYER, id, usize::MAX)? {
            Some(Cow::Borrowed(raw)) => {
                let mut c = Cursor(raw, 0);
                c.text().zip(c.text())
            }
            Some(Cow::Owned(raw)) => player_of(&raw).map(|p| (Cow::Owned(p.last), Cow::Owned(p.first))),
            None => None,
        })
    }

    /// Tournament title and place, borrowed where the record permits.
    pub fn tournament_name(&self, id: i64) -> Result<Option<EntityName<'_>>> {
        Ok(match self.raw_cow_within(TOURNAMENT, id, usize::MAX)? {
            Some(Cow::Borrowed(raw)) => {
                let mut c = Cursor(raw, 0);
                let fields = c.text().zip(c.text()).map(|(place, title)| (title, place));
                if c.i32().is_some() { fields } else { None }
            }
            Some(Cow::Owned(raw)) => tournament_of(&raw).map(|t| (Cow::Owned(t.title), Cow::Owned(t.place))),
            None => None,
        })
    }

    /// Reads consecutive records in batches.
    pub fn read_within<E: From<Error>>(
        &self,
        typ: usize,
        ids: Range<i64>,
        buf: &mut [u8],
        limit: usize,
        each: &mut impl FnMut(Option<&[u8]>) -> std::result::Result<(), E>,
    ) -> std::result::Result<u64, E> {
        let first = ids.start;
        let Ok(wanted) = u64::try_from(ids.end.saturating_sub(first)) else { return Ok(0) };
        if wanted == 0 {
            return Ok(0);
        }
        let (size, count) = self.types.get(typ).map_or((0, 0), |t| (t.0, t.1));
        let start = u64::try_from(first)
            .ok()
            .filter(|_| first < count)
            .and_then(|id| id.checked_mul(self.block_size as u64))
            .and_then(|o| o.checked_add((self.header_size + self.container_offset[typ]) as u64))
            .filter(|&o| o < self.len);
        let fit = match buf.len().checked_sub(size) {
            Some(rest) if size > 0 && self.block_size <= MAX_RUN_BLOCK => 1 + rest / self.block_size,
            _ => 0,
        };
        let Some(start) = start.filter(|_| fit > 0) else {
            let raw = self.raw_within(typ, first, limit)?;
            each(raw.as_deref())?;
            return Ok(1);
        };
        let stored = u64::try_from(count - first).unwrap_or(u64::MAX);
        let n = (fit as u64).min(wanted).min(stored);
        let end = (start + (n - 1) * self.block_size as u64 + size as u64).min(self.len);
        let span = &mut buf[..(end - start) as usize];
        self.file.read_into(start, span)?;
        for i in 0..n as usize {
            let at = i * self.block_size;
            let want = size.min(span.len().saturating_sub(at)).min(limit.saturating_add(4));
            let record = (want >= 4)
                .then(|| usize::try_from(le_i32(span, at)).ok().filter(|&len| len != 0 && len <= want - 4))
                .flatten()
                .map(|len| &span[at + 4..at + 4 + len]);
            each(record)?;
        }
        Ok(n)
    }

    /// Decodes player entity `id`.
    pub fn player(&self, id: i64) -> Result<Option<Player>> {
        self.player_within(id, usize::MAX)
    }

    /// Decodes player entity `id` within `limit` bytes.
    pub fn player_within(&self, id: i64, limit: usize) -> Result<Option<Player>> {
        Ok(self.raw_cow_within(PLAYER, id, limit)?.and_then(|r| player_of(&r)))
    }

    /// Reads player entities in batch.
    pub fn read_players_within<E: From<Error>>(
        &self,
        ids: Range<i64>,
        buf: &mut [u8],
        limit: usize,
        each: &mut impl FnMut(Option<Player>) -> std::result::Result<(), E>,
    ) -> std::result::Result<u64, E> {
        self.read_within(PLAYER, ids, buf, limit, &mut |r| each(r.and_then(player_of)))
    }

    /// Decodes tournament entity `id`.
    pub fn tournament(&self, id: i64) -> Result<Option<Tournament>> {
        self.tournament_within(id, usize::MAX)
    }

    /// Decodes tournament entity `id` within `limit` bytes.
    pub fn tournament_within(&self, id: i64, limit: usize) -> Result<Option<Tournament>> {
        Ok(self.raw_cow_within(TOURNAMENT, id, limit)?.and_then(|r| tournament_of(&r)))
    }

    /// Reads tournament entities in batch.
    pub fn read_tournaments_within<E: From<Error>>(
        &self,
        ids: Range<i64>,
        buf: &mut [u8],
        limit: usize,
        each: &mut impl FnMut(Option<Tournament>) -> std::result::Result<(), E>,
    ) -> std::result::Result<u64, E> {
        self.read_within(TOURNAMENT, ids, buf, limit, &mut |r| each(r.and_then(tournament_of)))
    }

    /// Decodes the first non-empty title of game tag entity `id`.
    pub fn title(&self, id: i64) -> Result<Option<String>> {
        self.title_within(id, usize::MAX)
    }

    /// Decodes the first non-empty title of game tag `id` within `limit` bytes.
    pub fn title_within(&self, id: i64, limit: usize) -> Result<Option<String>> {
        let raw = match self.raw_within(GAME_TAG, id, limit)? {
            Some(r) => Some(r),
            None => self.raw_within(5, id, limit)?,
        };
        Ok(raw.and_then(|r| {
            let mut c = Cursor(&r, 0);
            let count = c.i32()?;
            for _ in 0..count.max(0) {
                let _language = c.i32()?;
                let title = c.string()?;
                if !title.is_empty() {
                    return Some(title);
                }
            }
            None
        }))
    }

    /// Decodes annotator or source entity `id`.
    pub fn source(&self, id: i64) -> Result<Option<String>> {
        self.source_within(id, usize::MAX)
    }

    /// Decodes annotator or source entity `id` within `limit` bytes.
    pub fn source_within(&self, id: i64, limit: usize) -> Result<Option<String>> {
        let raw = match self.raw_within(ANNOTATOR_SOURCE, id, limit)? {
            Some(r) => Some(r),
            None => self.raw_within(2, id, limit)?,
        };
        Ok(raw.and_then(|r| {
            let mut c = Cursor(&r, 0);
            c.string()
        }))
    }
}

fn player_of(r: &[u8]) -> Option<Player> {
    let mut c = Cursor(r, 0);
    Some(Player { last: c.string()?, first: c.string()? })
}

fn tournament_of(r: &[u8]) -> Option<Tournament> {
    let mut c = Cursor(r, 0);
    let place = c.string()?;
    let title = c.string()?;
    let start = Date(c.i32()?);
    let event_type = c.take(1).map_or(0, |b| b[0]);
    Some(Tournament { title, place, start, event_type })
}

struct Cursor<'a>(&'a [u8], usize);

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let v = self.0.get(self.1..self.1.checked_add(n)?)?;
        self.1 += n;
        Some(v)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take(4).map(|v| i32::from_le_bytes(v.try_into().unwrap()))
    }
    fn text(&mut self) -> Option<Cow<'a, str>> {
        let n = usize::try_from(self.i32()?).ok()?;
        self.take(n).map(String::from_utf8_lossy)
    }
    fn string(&mut self) -> Option<String> {
        self.text().map(Cow::into_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_string(s: &str) -> Vec<u8> {
        let mut v = (s.len() as i32).to_le_bytes().to_vec();
        v.extend_from_slice(s.as_bytes());
        v
    }

    #[test]
    fn borrowed_names_preserve_unicode_lossy_and_unmapped_semantics() {
        // Two types of 64-byte interleaved containers, 3 IDs each.
        let mut bytes = vec![0u8; 64 + 128 * 3];
        bytes[..4].copy_from_slice(&64i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2i32.to_le_bytes());
        for i in 0..2 {
            let at = 8 + 20 * i;
            bytes[at..at + 4].copy_from_slice(&64i32.to_le_bytes());
            bytes[at + 4..at + 12].copy_from_slice(&3i64.to_le_bytes());
        }
        fn put(bytes: &mut [u8], offset: usize, payload: &[u8]) {
            bytes[offset..offset + 4].copy_from_slice(&(payload.len() as i32).to_le_bytes());
            bytes[offset + 4..offset + 4 + payload.len()].copy_from_slice(payload);
        }
        let mut player = encode_string("Юрьев");
        player.extend_from_slice(&encode_string("Éric"));
        put(&mut bytes, 64, &player);
        let mut tournament = encode_string("ישראל");
        tournament.extend_from_slice(&encode_string("大会"));
        tournament.extend_from_slice(&20260101i32.to_le_bytes());
        put(&mut bytes, 128, &tournament);
        let mut invalid = vec![1, 0, 0, 0, 0xff];
        invalid.extend_from_slice(&encode_string("A"));
        put(&mut bytes, 192, &invalid);
        // Malformed string length and missing tournament date.
        put(&mut bytes, 320, &[255, 255, 255, 127]);
        put(&mut bytes, 256, &encode_string("bad"));
        let temp = cbvault_fixtures::TempDb::create("native_borrowed_entity_names");
        let path = temp.write(".2lid", &bytes);
        let mapped = Entities::open(&path).unwrap();
        let unmapped = Entities::new(DbFile::open_unmapped(path).unwrap()).unwrap();
        for e in [&mapped, &unmapped] {
            let (last, first) = e.player_name(0).unwrap().unwrap();
            assert_eq!((last.as_ref(), first.as_ref()), ("Юрьев", "Éric"));
            let (title, place) = e.tournament_name(0).unwrap().unwrap();
            assert_eq!((title.as_ref(), place.as_ref()), ("大会", "ישראל"));
            let (last, first) = e.player_name(1).unwrap().unwrap();
            assert_eq!((last.as_ref(), first.as_ref()), ("�", "A"));
            assert!(e.player_name(2).unwrap().is_none());
            assert!(e.player_name(-1).unwrap().is_none());
            assert!(e.player_name(i64::MAX).unwrap().is_none());
            assert!(e.tournament_name(1).unwrap().is_none());
            let old = e.player(0).unwrap().unwrap();
            assert_eq!((old.last.as_str(), old.first.as_str()), ("Юрьев", "Éric"));
        }
        assert!(matches!(unmapped.player_name(0).unwrap().unwrap().0, Cow::Owned(_)));
        if mapped.file.as_slice().is_some() {
            assert!(matches!(mapped.player_name(0).unwrap().unwrap().0, Cow::Borrowed(_)));
            assert!(matches!(mapped.player_name(1).unwrap().unwrap().0, Cow::Owned(_)));
        }
    }

    #[test]
    fn decode_player_and_tournament() {
        let mut player_rec = encode_string("Carlsen");
        player_rec.extend_from_slice(&encode_string("Magnus"));

        let p = player_of(&player_rec).unwrap();
        assert_eq!(p.last, "Carlsen");
        assert_eq!(p.first, "Magnus");
        assert_eq!(p.pgn(), "Carlsen, Magnus");

        let mut tour_rec = encode_string("Wijk aan Zee");
        tour_rec.extend_from_slice(&encode_string("Tata Steel"));
        tour_rec.extend_from_slice(&20230115i32.to_le_bytes());

        let t = tournament_of(&tour_rec).unwrap();
        assert_eq!(t.place, "Wijk aan Zee");
        assert_eq!(t.title, "Tata Steel");
        assert_eq!(t.start, Date(20230115));
    }
}
