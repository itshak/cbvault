//! The namebases as the conversion and search paths need them: entity names by
//! id, borrowed from the mapped file, and name to id by binary search of the
//! sorted tree every namebase's records form.
//!
//! Every name here is a **borrowed** slice of the mapped namebase, which is
//! what makes a list of eleven million games allocation-free; a build without
//! the `mmap` feature therefore cannot resolve names and says so with a typed
//! error rather than quietly copying.
//!
//! The tree is a property of the *file*, and the reader in `cbvault-format`
//! addresses records by id only: a record's first nine bytes are its place in
//! the tree — a left child, a right child and one byte this crate skips — and
//! are not exposed. Searching by name needs those bytes, so this module opens
//! the namebases itself over [`cbvault_format::file::DbFile`] and re-reads the
//! layout `SPEC.md` § 2.4 records. It is deliberately the only place in
//! `cbvault` that knows it; the rest of the library goes through [`Entities`].
//!
//! **What the trees of the reference database are worth.** On the local Mega
//! Database 2025 the in-order walk of each tree is sorted by the name field's
//! raw bytes, so a plain descent finds
//!
//! | namebase | names a descent finds | names it misses |
//! |----------|-----------------------|-----------------|
//! | `.cbp` players | 463,259 of 463,261 | 2 |
//! | `.cbc` annotators | 2,478 of 2,479 | 1 |
//! | `.cbs` sources | 479 of 479 | 0 |
//! | `.cbe` teams | 67,563 of 67,563 | 0 |
//! | `.cbt` tournaments | 9,663 of 105,349 | 95,686 |
//!
//! The tournament tree is not a valid binary search tree: its in-order walk has
//! 223 inversions, and 223 nodes that break the ordering sit high enough to
//! orphan their whole subtree, which is why a descent there finds under a
//! tenth of the names. A name lookup happens once per query and not once per
//! record, so a descent that misses falls back to a verified scan of the file
//! — 10 MB of the reference set's `.cbt`, a few milliseconds — and the answer
//! is then always an id that resolves back to the name asked for.
//! [`Found::via`] says which of the two paths answered.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use cbvault_format::cbh::Entity;
use cbvault_format::cbh::bytes::NameBuf;
use cbvault_format::error::{Error, Result};
use cbvault_format::file::DbFile;

/// The fixed value at 0x08 of every namebase header.
const MAGIC: i32 = 1_234_567_890;
/// The header before the first record, plus whatever extra bytes a file has.
const HEADER: u64 = 28;
/// The largest record data accepted; the real ones are at most 1,608 bytes.
const MAX_DATA: i32 = 64 << 10;
/// The left child of a deleted record.
const DELETED: i32 = -999;
/// A record's own head: two child indexes and one byte this crate skips.
const NODE_HEAD: usize = 9;

/// An entry in the namebase sorted key table.
#[derive(Clone, Copy, Debug)]
struct KeyEntry {
    /// Offset into the `keys` byte buffer where this key starts.
    offset: u32,
    /// The record id.
    id: u32,
}

/// A flat in-memory table of all valid non-deleted record keys, sorted
/// lexicographically by key bytes, with ties broken by ascending record id.
///
/// Built on first lookup in under 300 ms for ~1 M records (~8 MB memory index + keys).
/// Allows O(log N) binary search for exact and prefix matches, regardless of whether
/// the file's binary search tree links are intact or damaged.
struct KeyTable {
    keys: Vec<u8>,
    index: Vec<KeyEntry>,
}

#[inline]
fn get_key(keys: &[u8], offset: u32) -> &[u8] {
    let o = offset as usize;
    let len = keys[o] as usize;
    &keys[o + 1..o + 1 + len]
}

impl KeyTable {
    /// Every id whose key is exactly `want`, strictly in ascending id order.
    fn find_all(&self, want: &[u8]) -> Vec<u32> {
        if want.is_empty() {
            return Vec::new();
        }
        let start = self.index.partition_point(|e| get_key(&self.keys, e.offset) < want);
        let end = self.index[start..].partition_point(|e| get_key(&self.keys, e.offset) == want) + start;
        self.index[start..end].iter().map(|e| e.id).collect()
    }

    /// Every id whose key begins with `prefix`, ordered by key then by id.
    fn find_prefix(&self, prefix: &[u8]) -> Vec<u32> {
        if prefix.is_empty() {
            return Vec::new();
        }
        let start = self.index.partition_point(|e| get_key(&self.keys, e.offset) < prefix);
        let end = self.index[start..].partition_point(|e| get_key(&self.keys, e.offset).starts_with(prefix)) + start;
        self.index[start..end].iter().map(|e| e.id).collect()
    }
}

/// One record of a namebase as the search path reads it.
struct Node<'a> {
    /// The left child: -1 for none, -999 for a deleted record.
    #[allow(dead_code)]
    left: i32,
    /// The right child, -1 for none.
    #[allow(dead_code)]
    right: i32,
    /// The record's data, after its nine-byte tree head.
    data: &'a [u8],
}

/// Which namebase a file is, and so which fields its records hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// `.cbp`: a last name then a forename.
    Player,
    /// `.cbt`: a title then a place.
    Tournament,
    /// `.cbc`: one name.
    Annotator,
    /// `.cbs`: one name.
    Source,
    /// `.cbe`: one name.
    Team,
}

impl Kind {
    /// The extension the file is written with.
    const fn ext(self) -> &'static str {
        match self {
            Kind::Player => ".cbp",
            Kind::Tournament => ".cbt",
            Kind::Annotator => ".cbc",
            Kind::Source => ".cbs",
            Kind::Team => ".cbe",
        }
    }

    /// The narrowest record data the file may hold.
    const fn min_data(self) -> i32 {
        match self {
            Kind::Player => 50,
            Kind::Tournament => 0x4a,
            Kind::Annotator => 45,
            Kind::Source => 25,
            Kind::Team => 50,
        }
    }

    /// The width of the field the tree is sorted by, and the whole name of a
    /// single-field record.
    const fn name_width(self) -> usize {
        match self {
            Kind::Player => 30,
            Kind::Tournament => 40,
            Kind::Annotator => 45,
            Kind::Source => 25,
            Kind::Team => 50,
        }
    }

    /// The width of the field after the first: the player's forename, the
    /// tournament's place. 0 for a record that holds one name.
    const fn second_width(self) -> usize {
        match self {
            Kind::Player => 20,
            Kind::Tournament => 30,
            _ => 0,
        }
    }
}

/// One namebase file: a header, then fixed-size records, each placed in a
/// sorted tree by its first name field.
struct Namebase {
    file: DbFile,
    /// The offset of the first record.
    header: u64,
    /// One record's size, tree head included.
    record: u64,
    /// Records in the file, deleted ones included.
    count: u32,
    /// The tree's root record, -1 in a file with no records.
    #[allow(dead_code)]
    root: i32,
    /// Which file this is, and so which fields its records hold.
    kind: Kind,
    /// Sorted (key, id) table built lazily on first lookup.
    table: OnceLock<KeyTable>,
}

impl Namebase {
    /// Opens `path`, whose records must carry at least the fields `kind` needs.
    fn open(path: PathBuf, kind: Kind) -> Result<Self> {
        let file = DbFile::open(path.clone())?;
        let len = file.size()?;
        if len < HEADER {
            return Err(Error::corrupt(&path, 0, format!("{len}-byte file is shorter than its header")));
        }
        let head = file.read(0, HEADER as usize)?;
        let int = |o: usize| i32::from_le_bytes(head[o..o + 4].try_into().unwrap_or_default());
        if int(0x08) != MAGIC {
            return Err(Error::corrupt(&path, 0x08, "bad magic"));
        }
        let data = int(0x0c);
        if !(kind.min_data()..=MAX_DATA).contains(&data) {
            return Err(Error::corrupt(&path, 0x0c, format!("record data size {data}")));
        }
        let extra = int(0x18);
        if extra != 0 && extra != 4 {
            return Err(Error::corrupt(&path, 0x18, format!("{extra} extra header bytes")));
        }
        let header = HEADER + extra as u64;
        let record = NODE_HEAD as u64 + data as u64;
        Ok(Namebase {
            file,
            header,
            record,
            count: u32::try_from(len.saturating_sub(header) / record).unwrap_or(u32::MAX),
            root: int(0x04),
            kind,
            table: OnceLock::new(),
        })
    }

    /// Records in the file, deleted ones included.
    fn count(&self) -> u32 {
        self.count
    }

    /// The record `id` as the search path reads it, or `None` for an id past
    /// the file and a deleted record.
    fn node(&self, id: i32) -> Result<Option<Node<'_>>> {
        if id < 0 || u64::from(id as u32) >= u64::from(self.count) {
            return Ok(None);
        }
        let at = self.header + u64::from(id as u32) * self.record;
        let Some(slice) = self.file.slice_at(at, self.record as usize) else {
            return Err(Error::corrupt(
                self.file.path(),
                at,
                "an entity name is borrowed from the memory map; build with the `mmap` feature",
            ));
        };
        Ok(node_of(slice))
    }

    /// The name of record `id`, or `None` for an id past the file, a deleted
    /// record and a blank one.
    fn name(&self, id: u32) -> Result<Option<Name<'_>>> {
        let Some(node) = self.node(id as i32)? else { return Ok(None) };
        let last = self.key(&node);
        if last.is_empty() {
            return Ok(None);
        }
        Ok(Some(Name { last, first: self.second(&node) }))
    }

    /// The sorted name field of a record, up to its first zero byte.
    fn key<'a>(&self, node: &Node<'a>) -> &'a [u8] {
        field(node.data, 0, self.kind.name_width())
    }

    /// The second name field — the player's forename, the tournament's place —
    /// or an empty slice for a single-field record.
    fn second<'a>(&self, node: &Node<'a>) -> &'a [u8] {
        let width = self.kind.second_width();
        if width == 0 {
            return &[];
        }
        field(node.data, self.kind.name_width(), width)
    }

    /// The id whose sorted name field is `name`, by descending the tree.
    ///
    /// Bounded by the record count, so a damaged file that makes the tree
    /// circular stops instead of looping. `None` means both "no such name" and
    /// "the tree does not lead to it", and on a real database the second is
    /// common enough — see the module docs — which is why
    /// [`Namebase::find_with`] follows up with a scan.
    #[allow(dead_code)]
    fn descend(&self, name: &str) -> Result<Option<u32>> {
        let want = name.as_bytes();
        let mut at = self.root;
        for _ in 0..=self.count {
            let Some(node) = self.node(at)? else { return Ok(None) };
            match self.key(&node).cmp(want) {
                std::cmp::Ordering::Equal => return Ok(Some(at as u32)),
                std::cmp::Ordering::Less => at = node.right,
                std::cmp::Ordering::Greater => at = node.left,
            }
        }
        Ok(None)
    }

    /// The first id whose sorted name field is `name`, scanning the file.
    #[allow(dead_code)]
    fn scan(&self, name: &str) -> Result<Option<u32>> {
        let want = name.as_bytes();
        for id in 0..self.count {
            let Some(node) = self.node(id as i32)? else { continue };
            if self.key(&node) == want {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Every id whose sorted name field is `name`, in ascending id order.
    ///
    /// [`Namebase::descend`] answers "an id", and that is not the same question
    /// as "which ids". A namebase is keyed by **last name** for a player and by
    /// **title** for a tournament, so one key has many records: the reference
    /// database's `.cbp` holds 463,261 players and a search for a common surname
    /// matches several of them. Returning one id would silently drop the others'
    /// games, which reads to a user as "that player has fewer games than I
    /// thought" rather than as a bug.
    ///
    /// The records sharing a key are not guaranteed to hang off one subtree, so
    /// this walks the tree once and collects every match rather than assuming a
    /// range. It is bounded by the record count exactly as `descend` is, so a
    /// damaged file stops instead of looping.
    /// Returns the sorted key table, building it from the mapped file on first lookup.
    fn table(&self) -> Result<&KeyTable> {
        if let Some(t) = self.table.get() {
            return Ok(t);
        }
        let built = self.build_table()?;
        let _ = self.table.set(built);
        Ok(self.table.get().expect("just set"))
    }

    /// Builds the sorted key table by scanning all records in the mapped file.
    fn build_table(&self) -> Result<KeyTable> {
        let Some(file_bytes) = self.file.as_slice() else {
            return Err(Error::corrupt(
                self.file.path(),
                0,
                "an entity name is borrowed from the memory map; build with the `mmap` feature",
            ));
        };
        let mut keys = Vec::with_capacity((self.count as usize).saturating_mul(12));
        let mut index = Vec::with_capacity(self.count as usize);
        let rec_size = self.record as usize;
        let name_w = self.kind.name_width();
        let header = self.header as usize;

        for id in 0..self.count {
            let at = header + (id as usize) * rec_size;
            let Some(slice) = file_bytes.get(at..at + rec_size) else {
                continue;
            };
            if slice.len() < NODE_HEAD {
                continue;
            }
            let left = i32::from_le_bytes(slice[0..4].try_into().unwrap_or_default());
            if left == DELETED {
                continue;
            }
            let key = field(&slice[NODE_HEAD..], 0, name_w);
            if key.is_empty() {
                continue;
            }
            let offset = keys.len() as u32;
            keys.push(key.len() as u8);
            keys.extend_from_slice(key);
            index.push(KeyEntry { offset, id });
        }

        use rayon::prelude::*;
        index.par_sort_unstable_by(|a, b| {
            let k_a = get_key(&keys, a.offset);
            let k_b = get_key(&keys, b.offset);
            k_a.cmp(k_b).then_with(|| a.id.cmp(&b.id))
        });

        Ok(KeyTable { keys, index })
    }

    /// Every id whose sorted name field is `name`, in ascending id order.
    ///
    /// Built on first lookup via [`KeyTable`] in O(log N) binary search.
    fn find_all(&self, name: &str) -> Result<Vec<u32>> {
        if name.is_empty() {
            return Ok(Vec::new());
        }
        let table = self.table()?;
        Ok(table.find_all(name.as_bytes()))
    }

    /// Every id whose sorted name field begins with `prefix`.
    fn find_prefix(&self, prefix: &str) -> Result<Vec<u32>> {
        if prefix.is_empty() {
            return Ok(Vec::new());
        }
        let table = self.table()?;
        Ok(table.find_prefix(prefix.as_bytes()))
    }

    /// The first id the entity `name` names, and which path resolved it.
    fn find_with(&self, name: &str) -> Result<Option<Found>> {
        if name.is_empty() {
            return Ok(None);
        }
        let ids = self.find_all(name)?;
        if let Some(&id) = ids.first() {
            return Ok(Some(Found { id, via: Via::Tree }));
        }
        Ok(None)
    }

    #[cfg(test)]
    pub(crate) fn scan_all(&self, name: &str) -> Result<Vec<u32>> {
        let want = name.as_bytes();
        let mut ids = Vec::new();
        for id in 0..self.count {
            let Some(node) = self.node(id as i32)? else { continue };
            if self.key(&node) == want {
                ids.push(id);
            }
        }
        Ok(ids)
    }
}

/// A record's tree head and data, given its bytes; a deleted record is `None`.
fn node_of(bytes: &[u8]) -> Option<Node<'_>> {
    if bytes.len() < NODE_HEAD {
        return None;
    }
    let int = |o: usize| i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap_or_default());
    let left = int(0);
    if left == DELETED {
        return None;
    }
    Some(Node { left, right: int(4), data: &bytes[NODE_HEAD..] })
}

/// The name field at `at`, cut at its width and at its first zero byte.
fn field(data: &[u8], at: usize, width: usize) -> &[u8] {
    if at >= data.len() {
        return &[];
    }
    let end = at.saturating_add(width).min(data.len());
    let field = &data[at..end];
    let stop = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    &field[..stop]
}

/// How a name resolved to an id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// The tree descent found it: at most a record count deep, and the answer
    /// on the reference database for every player, source, annotator and team
    /// name.
    Tree,
    /// The verified scan found it, because the descent did not lead there.
    Scan,
}

/// An entity id a name resolved to, and the path that resolved it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    /// The entity's id, which is its record's 0-based position.
    pub id: u32,
    /// Which of the two paths answered.
    pub via: Via,
}

/// One entity's name, borrowed from the mapped namebase as one or two slices.
///
/// The format stores a player's last name and forename in one record, and a
/// tournament's title and place in another, so a name is one or two borrowed
/// field slices: nothing is copied to hand one out and nothing is allocated to
/// hold one. The bytes are the database's own code page — UTF-8 where a field
/// holds it, Windows-1252 above — so [`Name::as_str`] answers for the names
/// that already are UTF-8 (all but 356 of the reference database's 463,262
/// players) and [`Name::push_text`] decodes the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name<'a> {
    /// The first field: a last name, a title, or the whole name.
    last: &'a [u8],
    /// The second field: a forename, a place, or empty.
    first: &'a [u8],
}

impl<'a> Name<'a> {
    /// The empty name, for an entity the database does not have: a caller then
    /// never has to decide what a missing name means.
    pub const fn blank() -> Name<'a> {
        Name { last: &[], first: &[] }
    }

    /// A name of one field, for a record that holds one.
    pub const fn of(last: &'a [u8]) -> Name<'a> {
        Name { last, first: &[] }
    }

    /// A name of two fields: a player's last name and forename, or a
    /// tournament's title and place.
    pub const fn pair(last: &'a [u8], first: &'a [u8]) -> Name<'a> {
        Name { last, first }
    }

    /// The first field's bytes, as stored.
    #[inline]
    pub fn last(&self) -> &'a [u8] {
        self.last
    }

    /// The second field's bytes, as stored; empty for a single-field name.
    #[inline]
    pub fn first(&self) -> &'a [u8] {
        self.first
    }

    /// Whether the name is blank.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.last.is_empty() && self.first.is_empty()
    }

    /// The name as UTF-8 when its bytes already are: the whole name for a
    /// single-field record, and `None` for a two-field one, whose name is the
    /// two fields joined.
    #[inline]
    pub fn as_str(&self) -> Option<&'a str> {
        if !self.first.is_empty() {
            return None;
        }
        std::str::from_utf8(self.last).ok()
    }

    /// The first field decoded into `buf`, whose borrow the result keeps.
    #[inline]
    pub fn decode_last<'b>(&self, buf: &'b mut NameBuf) -> &'b str {
        buf.set(self.last);
        buf.as_str()
    }

    /// The second field decoded into `buf`, whose borrow the result keeps; the
    /// empty string for a single-field record.
    #[inline]
    pub fn decode_first<'b>(&self, buf: &'b mut NameBuf) -> &'b str {
        if self.first.is_empty() {
            return "";
        }
        buf.set(self.first);
        buf.as_str()
    }

    /// The whole name appended to `out`: `Last, First` for a player, the title
    /// for everything else, decoded by the rules the PGN tags use.
    pub fn push_text(&self, out: &mut String) {
        let mut buf = NameBuf::new();
        if self.first.is_empty() {
            out.push_str(self.decode_last(&mut buf));
            return;
        }
        if !self.last.is_empty() {
            out.push_str(self.decode_last(&mut buf));
            out.push_str(", ");
        }
        out.push_str(self.decode_first(&mut buf));
    }
}

/// The namebases of a classic database: the four the format requires, and
/// `.cbe` teams when the set has the file.
///
/// This is the bridge's own reader of the namebases (see the module docs). It
/// answers both directions a consumer needs from the same mapped bytes — id to
/// name for the conversion and the export, name to id for the search — and
/// holds no copy of any of them.
pub struct Entities {
    players: Namebase,
    tournaments: Namebase,
    annotators: Namebase,
    sources: Namebase,
    teams: Option<Namebase>,
}

impl std::fmt::Debug for Entities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entities")
            .field("players", &self.players.count())
            .field("tournaments", &self.tournaments.count())
            .field("annotators", &self.annotators.count())
            .field("sources", &self.sources.count())
            .field("teams", &self.teams.as_ref().map(Namebase::count))
            .finish()
    }
}

impl Entities {
    /// Opens the namebases that share `stem`'s path — the extension appended,
    /// resolved case-insensitively. The four the format requires must be
    /// there; `.cbe` is optional.
    pub fn open(stem: &Path) -> Result<Self> {
        let file = |kind: Kind| Namebase::open(super::sibling(stem, kind.ext()), kind);
        let teams = match file(Kind::Team) {
            Ok(teams) => Some(teams),
            Err(Error::Io { .. }) => None,
            Err(e) => return Err(e),
        };
        Ok(Entities {
            players: file(Kind::Player)?,
            tournaments: file(Kind::Tournament)?,
            annotators: file(Kind::Annotator)?,
            sources: file(Kind::Source)?,
            teams,
        })
    }

    /// Records in each of the four namebases, deleted ones included: players,
    /// tournaments, annotators, sources.
    pub fn counts(&self) -> [u64; 4] {
        [
            u64::from(self.players.count()),
            u64::from(self.tournaments.count()),
            u64::from(self.annotators.count()),
            u64::from(self.sources.count()),
        ]
    }

    /// Records in `.cbe` (teams), 0 when the set has no such file.
    pub fn team_count(&self) -> u64 {
        self.teams.as_ref().map_or(0, |teams| u64::from(teams.count()))
    }

    /// The namebase that holds `entity`, or `None` for teams in a set that has
    /// no `.cbe`.
    fn of(&self, entity: Entity) -> Option<&Namebase> {
        Some(match entity {
            Entity::Player => &self.players,
            Entity::Tournament => &self.tournaments,
            Entity::Annotator => &self.annotators,
            Entity::Source => &self.sources,
            Entity::Team => self.teams.as_ref()?,
        })
    }

    /// The name of entity `id` of `entity`'s namebase, or `None` for an id past
    /// the file, a deleted record and a blank one. The bytes are borrowed from
    /// the mapped file: nothing is allocated to look a name up.
    pub fn name(&self, entity: Entity, id: u32) -> Result<Option<Name<'_>>> {
        match self.of(entity) {
            Some(file) => file.name(id),
            None => Ok(None),
        }
    }

    /// Every id of `entity`'s namebase with its name, in ascending id order —
    /// the bulk export a consumer interns its player, event, site, annotator
    /// and source tables from. A deleted or blank record is skipped.
    pub fn for_each(&self, entity: Entity, mut f: impl FnMut(u32, Name<'_>)) -> Result<()> {
        let Some(file) = self.of(entity) else { return Ok(()) };
        for id in 0..file.count() {
            if let Some(name) = file.name(id)? {
                f(id, name);
            }
        }
        Ok(())
    }

    /// The id of the player `name` names and the path that resolved it. `name`
    /// is the last name as stored; the forename is not part of the key, since
    /// the tree is sorted by the last name alone.
    pub fn find_player_with(&self, name: &str) -> Result<Option<Found>> {
        self.players.find_with(name)
    }

    /// The id of the player `name` names, or `None`. A last name is not unique,
    /// so the answer is *an* id that resolves to `name`; read the name back
    /// with [`Entities::player_text`] to see which one.
    pub fn find_player(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.find_player_with(name)?.map(|found| found.id))
    }

    /// **Every** id the player last name `name` names, ascending.
    ///
    /// [`Entities::find_player`] answers "an id" and that is the wrong question
    /// for a search: the `.cbp` is keyed by **last name**, so a surname shared by
    /// several players — and a common one is shared by thousands — has one key and
    /// many records. A filter built from one of them silently omits the others'
    /// games, which a user reads as "this player has fewer games than I thought".
    ///
    /// This is the whole set, so a caller builds one filter and gets every game
    /// that name appears in. Empty for a name the file does not hold.
    ///
    /// `name` is the last name as stored. Matching is exact and case-sensitive,
    /// as the tree's own comparison is — see [`Namebase::find_all`].
    pub fn find_players(&self, name: &str) -> Result<Vec<u32>> {
        self.players.find_all(name)
    }

    /// Every id whose player last name begins with `prefix`.
    pub fn find_players_prefix(&self, prefix: &str) -> Result<Vec<u32>> {
        self.players.find_prefix(prefix)
    }

    /// Every id whose tournament title is `name`, ascending.
    ///
    /// The same reason as [`Entities::find_players`]: `.cbt` is keyed by title and
    /// ChessBase reuses a title across sites and years, so one title is many
    /// records. A tournament search that returned one of them would under-report
    /// every event sharing a name.
    pub fn find_tournaments(&self, name: &str) -> Result<Vec<u32>> {
        self.tournaments.find_all(name)
    }

    /// Every id whose tournament title begins with `prefix`.
    pub fn find_tournaments_prefix(&self, prefix: &str) -> Result<Vec<u32>> {
        self.tournaments.find_prefix(prefix)
    }

    /// Every id whose annotator name is `name`, ascending.
    pub fn find_annotators(&self, name: &str) -> Result<Vec<u32>> {
        self.annotators.find_all(name)
    }

    /// Every id whose annotator name begins with `prefix`.
    pub fn find_annotators_prefix(&self, prefix: &str) -> Result<Vec<u32>> {
        self.annotators.find_prefix(prefix)
    }

    /// Every id whose source title is `name`, ascending.
    pub fn find_sources(&self, name: &str) -> Result<Vec<u32>> {
        self.sources.find_all(name)
    }

    /// Every id whose source title begins with `prefix`.
    pub fn find_sources_prefix(&self, prefix: &str) -> Result<Vec<u32>> {
        self.sources.find_prefix(prefix)
    }

    /// Every id whose team name is `name`, ascending; always empty in a set
    /// without `.cbe`.
    pub fn find_teams(&self, name: &str) -> Result<Vec<u32>> {
        Ok(self.teams.as_ref().map(|t| t.find_all(name)).transpose()?.unwrap_or_default())
    }

    /// Every id whose team name begins with `prefix`; always empty in a set
    /// without `.cbe`.
    pub fn find_teams_prefix(&self, prefix: &str) -> Result<Vec<u32>> {
        Ok(self.teams.as_ref().map(|t| t.find_prefix(prefix)).transpose()?.unwrap_or_default())
    }

    #[cfg(test)]
    pub(crate) fn scan_all_players(&self, name: &str) -> Result<Vec<u32>> {
        self.players.scan_all(name)
    }

    /// [`Entities::find_player`] for a tournament's title, with the path.
    pub fn find_tournament_with(&self, name: &str) -> Result<Option<Found>> {
        self.tournaments.find_with(name)
    }

    /// The id of the tournament `name` names, or `None`.
    pub fn find_tournament(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.find_tournament_with(name)?.map(|found| found.id))
    }

    /// [`Entities::find_player`] for an annotator's name, with the path.
    pub fn find_annotator_with(&self, name: &str) -> Result<Option<Found>> {
        self.annotators.find_with(name)
    }

    /// The id of the annotator `name` names, or `None`.
    pub fn find_annotator(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.find_annotator_with(name)?.map(|found| found.id))
    }

    /// [`Entities::find_player`] for a source's title, with the path.
    pub fn find_source_with(&self, name: &str) -> Result<Option<Found>> {
        self.sources.find_with(name)
    }

    /// The id of the source `name` names, or `None`.
    pub fn find_source(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.find_source_with(name)?.map(|found| found.id))
    }

    /// [`Entities::find_player`] for a team's name; always `None` in a set
    /// without `.cbe`.
    pub fn find_team(&self, name: &str) -> Result<Option<u32>> {
        let Some(teams) = &self.teams else { return Ok(None) };
        Ok(teams.find_with(name)?.map(|found| found.id))
    }

    /// The name of the player `id` written into `buf` as `Last, First`, so a
    /// lookup can be checked against what the database holds. `false` for an id
    /// the file does not have.
    pub fn player_text(&self, id: u32, buf: &mut String) -> Result<bool> {
        let Some(name) = self.players.name(id)? else { return Ok(false) };
        name.push_text(buf);
        Ok(true)
    }

    /// The name of the entity `id` written into `buf`, the way the PGN tags
    /// spell it: `Last, First` for a player, `title, place` for a tournament,
    /// and the name alone for an annotator, a source and a team.
    pub fn entity_text(&self, entity: Entity, id: u32, buf: &mut String) -> Result<bool> {
        let Some(name) = self.name(entity, id)? else { return Ok(false) };
        name.push_text(buf);
        Ok(true)
    }
}
