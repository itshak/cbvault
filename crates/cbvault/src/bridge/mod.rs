//! The consumer-facing façade: a read-only [`Database`] over a validated
//! classic `.cbh` file set, and the sink-based conversion API a consumer
//! writes its target database against.
//!
//! # What this module is for
//!
//! A consumer's job with a `.cbh` set is two jobs. *Convert* it — stream every
//! game into a target reference base or user database, as fast as
//! the source can be read. And *serve* it read-only: list games, show one,
//! search by tag, search by position. Both are here, and both are read-only:
//! cbvault never opens a source file for writing, never creates anything in a
//! database's directory, and writes no target database of its own.
//!
//! The conversion is a callback, not a format. [`GameSink`] is told once per run
//! what it needs — position keys, annotations — and is then handed one
//! [`GameRef`] per game, entirely borrowed, in ascending game-number order.
//! Nothing is allocated per game across that boundary, and nothing is
//! formatted: a sink that only counts games touches the allocator zero times.
//!
//! # The shape of the API
//!
//! - [`Database::open`] takes a base name or any member file, resolves its
//!   siblings case-insensitively, checks the mandatory set, and reports the
//!   generation, the record count and which optional members it found.
//! - [`Database::headers`] walks the header records and nothing else: it never
//!   opens the moves file, so listing eleven million games is a scan of a
//!   513 MB file and a handful of name lookups each.
//! - [`Database::game`] hands over one game, and [`for_each_game`] /
//!   [`convert_parallel`] hand over all of them, sequentially or across
//!   threads, in the same order either way.
//! - [`Entities`] answers name to id for a tag search, and the scan in
//!   [`scan`] pushes a resolved id down to a per-record comparison.
//!
//! # Provenance
//!
//! Original to cbvault. It consumes
//! `cbvault-format` for bytes and `cbvault-chess` for chess, and re-reads the
//! namebase tree itself because the format crate's reader addresses entities by
//! id only — see the private `namebase` module.

mod convert;
mod namebase;
mod search;
mod sink;
mod walk;

#[cfg(test)]
mod alloc;
#[cfg(test)]
mod real;
#[cfg(test)]
mod tests;

pub use convert::{ConvertStats, DEFAULT_BATCH, convert_parallel, for_each_game, for_each_range};
pub use namebase::{Entities, Found, Name, Via};
pub use search::{
    AllOf, Filter, Hit, IdSet, Match, PositionQuery, PositionSearch, Range, Scan, SearchStats, any_player,
    for_each_position_key, scan, scan_range,
};
pub use sink::{GameHeaderRef, GameRef, GameSink};
pub use walk::{Flow, GameBuf, MovesBuf, Names};

pub use cbvault_format::error::{Error, Result};

use std::path::{Path, PathBuf};

use cbvault_format::cbh::Headers as CbhHeaders;
use cbvault_format::cbh::moves::GameMoves;
use cbvault_format::cbh::{Annotations, Entity, Flags, GameHeader, Wide};
use cbvault_format::error::Role;
use cbvault_format::file::DbFile;
use cbvault_format::game::RecordKind;

/// Which generation of the format a file set is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generation {
    /// The classic `.cbh` family, which is what this module reads.
    Classic,
    /// The 2CBH `.2cbh` family, which ChessBase 17 and later writes for a
    /// working database. Recognised so a caller can be told before it tries to
    /// open one; reading it is a later change, behind this same façade.
    TwoCbh,
    /// Neither: the directory holds no member of a known generation.
    Unknown,
}

impl Generation {
    /// The generation's name, as [`Generation::as_str`] spells it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Generation::Classic => "classic",
            Generation::TwoCbh => "2CBH",
            Generation::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Generation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The extensions that make a path a member of a classic set, and the ones
/// that make it a member of a 2CBH set. A path whose extension is one of these
/// is a member file; anything else is taken as a base name, so a base name that
/// happens to contain a dot still works.
const CLASSIC_MEMBERS: [&str; 19] = [
    ".cbh", ".cbg", ".cba", ".cbp", ".cbt", ".cbc", ".cbs", ".cbe", ".cbj", ".cbl", ".cbtt", ".cbm", ".flags", ".cit",
    ".cib", ".cit2", ".cib2", ".cbb", ".cbgi",
];
const TWOCBH_MEMBERS: [&str; 6] = [".2cbh", ".2cbg", ".2cba", ".2lid", ".2lgd", ".2lcd"];

/// The members a classic set must have, with the role each plays, in the order
/// they are checked — headers first, so a directory that holds a `.cbt` and
/// nothing else is told about the `.cbh` it is missing.
const MANDATORY: [(&str, Role); 7] = [
    (".cbh", Role::Headers),
    (".cbg", Role::Moves),
    (".cba", Role::Annotations),
    (".cbp", Role::Entities),
    (".cbt", Role::Entities),
    (".cbc", Role::Entities),
    (".cbs", Role::Entities),
];

const MANDATORY_2CBH: [(&str, Role); 3] = [(".2cbh", Role::Headers), (".2cbg", Role::Moves), (".2lid", Role::Entities)];

/// The path `ext` appended to `stem`, resolved case-insensitively from the same
/// directory when the exact name is not there.
///
/// ChessBase on Windows writes a set with whatever case the file names carry,
/// and a set copied between systems keeps it, so `Mega Database 2025.cbh` and
/// `MEGA DATABASE 2025.CBH` are the same file. The exact name is returned when
/// nothing matches, so a [`Error::MissingFile`] names the path that was looked
/// for.
pub(crate) fn sibling(stem: &Path, ext: &str) -> PathBuf {
    let mut name = stem.as_os_str().to_owned();
    name.push(ext);
    let exact = PathBuf::from(name);
    if exact.symlink_metadata().is_ok() {
        return exact;
    }
    let (Some(dir), Some(want)) = (exact.parent(), exact.file_name()) else { return exact };
    let want = want.to_string_lossy().to_ascii_lowercase();
    let Ok(entries) = std::fs::read_dir(dir) else { return exact };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().to_ascii_lowercase() == want {
            return entry.path();
        }
    }
    exact
}

/// The base path of the set `path` names: `path` itself when it carries no
/// member extension, and `path` without that extension when it does.
pub fn stem_of(path: &Path) -> PathBuf {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return path.to_owned() };
    let ext = format!(".{}", ext.to_ascii_lowercase());
    let member = CLASSIC_MEMBERS.contains(&ext.as_str()) || TWOCBH_MEMBERS.contains(&ext.as_str());
    if member { path.with_extension("") } else { path.to_owned() }
}

/// The generation of the set whose files share `path`'s base name, without
/// opening any of them: a `.cbh` or `.cbg` makes the set classic, a `.2cbh` or
/// `.2cbg` makes it 2CBH, and a directory with neither is [`Generation::Unknown`].
///
/// Cheap enough to call before every open, and the honest way for a consumer to
/// find out that a set is 2CBH — which [`Database::open`] cannot read yet —
/// rather than reading a [`Error::MissingFile`] for a `.cbh` that will never be
/// there.
pub fn generation_of(path: impl AsRef<Path>) -> Generation {
    let path = path.as_ref();
    // The caller's explicit extension wins even when both generations share
    // one basename. Avoid silently interpreting a 2CBH database as classic.
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "2cbh" | "2cbg" | "2cba" | "2lid" | "2lgd" | "2lcd") {
        return Generation::TwoCbh;
    }
    if matches!(ext.as_str(), "cbh" | "cbg" | "cba" | "cbp" | "cbt" | "cbc" | "cbs") {
        return Generation::Classic;
    }
    let stem = stem_of(path);
    let classic = [".cbh", ".cbg"].iter().any(|e| sibling(&stem, e).symlink_metadata().is_ok());
    let v2 = [".2cbh", ".2cbg"].iter().any(|e| sibling(&stem, e).symlink_metadata().is_ok());
    match (classic, v2) {
        (true, false) => Generation::Classic,
        (false, true) => Generation::TwoCbh,
        // Ambiguous bare stems must be explicitly selected by extension.
        _ => Generation::Unknown,
    }
}

/// The files of an opened set: the seven every classic set has, and the
/// optional ones that were found beside them.
///
/// The mandatory paths are reported because a consumer shows them when a set is
/// incomplete, and the optional ones because their presence is what a caller
/// checks before asking for something only a complete set has — `.flags` for
/// the Top Games bits, `.cbj` for the 64-bit offsets of a set over 4 GiB, `.cbe`
/// for teams, `.cbtt` for the tournament tree.
#[derive(Clone, Debug, Default)]
pub struct Members {
    /// `.cbh`: the 46-byte header records.
    pub headers: PathBuf,
    /// `.cbg`: the move records.
    pub moves: PathBuf,
    /// `.cba`: the annotation records.
    pub annotations: PathBuf,
    /// `.cbp`: players.
    pub players: PathBuf,
    /// `.cbt`: tournaments.
    pub tournaments: PathBuf,
    /// `.cbc`: annotators.
    pub annotators: PathBuf,
    /// `.cbs`: sources.
    pub sources: PathBuf,
    /// `.cbj`: 64-bit offsets, for a set whose moves or annotations pass 4 GiB.
    pub wide: Option<PathBuf>,
    /// `.cbe`: teams and clubs.
    pub teams: Option<PathBuf>,
    /// `.flags`: the Top Games bits, two per record.
    pub flags: Option<PathBuf>,
    /// `.cbtt`: the tournament tree, one record per tournament.
    pub tournament_tree: Option<PathBuf>,
    /// `.cbl`: the cross-table settings.
    pub cross_table: Option<PathBuf>,
}

impl Members {
    /// The paths of the optional members that were found, with the extension
    /// each carries — what an error message about a missing optional member
    /// needs, and what a caller checks for.
    pub fn optional(&self) -> Vec<(&'static str, &Path)> {
        let found = [
            (".cbj", &self.wide),
            (".cbe", &self.teams),
            (".flags", &self.flags),
            (".cbtt", &self.tournament_tree),
            (".cbl", &self.cross_table),
        ];
        found.into_iter().filter_map(|(ext, path)| path.as_deref().map(|p| (ext, p))).collect()
    }

    /// The names of the optional members that were found.
    pub fn optional_names(&self) -> Vec<&'static str> {
        self.optional().into_iter().map(|(ext, _)| ext).collect()
    }
}

/// A generation-aware borrowed header index. A caller can select the proper
/// wire layout without ever interpreting 192-byte 2CBH records as 46-byte CBH.
pub enum HeaderSource<'a> {
    /// Classic CBH header index.
    Classic(&'a CbhHeaders),
    /// Native ChessBase 2 header index.
    TwoCbh(&'a cbvault_format::twocbh::Headers),
}

impl HeaderSource<'_> {
    /// Total record slots, including deleted and guiding text entries.
    pub fn records(&self) -> u32 {
        match self {
            Self::Classic(h) => h.records(),
            Self::TwoCbh(h) => h.records(),
        }
    }
}

/// Generation-aware borrowed entity store. Use this instead of the legacy
/// classic-only `Database::entities()` on a database of unknown generation.
pub enum EntitySource<'a> {
    /// Classic .cbp/.cbt/.cbc/.cbs namebases.
    Classic(&'a Entities),
    /// Native .2lid table.
    TwoCbh(&'a cbvault_format::twocbh::Entities),
}

/// A read-only view of one ChessBase database.
///
/// A `Database` holds no writable handle on anything and creates, modifies and
/// deletes nothing in the set's directory: every file is opened for reading
/// only, and the moves and annotation files are not opened at all until
/// something asks for a game. Two operations therefore stay honest by
/// construction — a game list never touches `.cbg`, and a conversion that does
/// not want annotations never touches `.cba`, which is 209 MB on the reference
/// set. Both are asserted by tests rather than merely intended.
///
/// ```no_run
/// use cbvault::bridge::{Database, GameRef, GameSink};
///
/// struct Counted(u64);
/// impl GameSink for Counted {
///     fn game(&mut self, _: GameRef<'_>) { self.0 += 1; }
/// }
///
/// let db = Database::open("Mega Database 2025/Mega Database 2025")?;
/// let mut sink = Counted(0);
/// cbvault::bridge::for_each_game(&db, &mut sink)?;
/// # Ok::<(), cbvault::bridge::Error>(())
/// ```
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum Backend {
    Classic(ClassicBackend),
    TwoCbh(TwoCbhBackend),
}

#[derive(Debug)]
struct ClassicBackend {
    headers: CbhHeaders,
    entities: Entities,
    wide: Option<Wide>,
    flags: Option<Flags>,
    moves: std::sync::OnceLock<DbFile>,
    annotations: std::sync::OnceLock<Annotations>,
    format_entities: std::sync::OnceLock<cbvault_format::cbh::Entities>,
}

#[derive(Debug)]
struct TwoCbhBackend {
    headers: cbvault_format::twocbh::Headers,
    entities: cbvault_format::twocbh::Entities,
    moves: std::sync::OnceLock<cbvault_format::twocbh::Segment>,
    #[allow(dead_code)]
    annotations: std::sync::OnceLock<Option<cbvault_format::twocbh::Segment>>,
}

/// A read-only view of one ChessBase database.
#[derive(Debug)]
pub struct Database {
    /// The base path every member of the set shares, extension included.
    base: PathBuf,
    /// Which generation the set is.
    generation: Generation,
    /// The files that were found.
    members: Members,
    backend: Backend,
}

impl Database {
    /// Opens the set `path` names — its base name with no extension, or any one
    /// of its member files, in any case.
    pub fn open(path: impl AsRef<Path>) -> Result<Database> {
        let path = path.as_ref();
        let stem = stem_of(path);
        match generation_of(path) {
            Generation::Classic => Self::open_classic(stem),
            Generation::TwoCbh => Self::open_twocbh(stem),
            Generation::Unknown => {
                if sibling(&stem, ".cbh").exists() && sibling(&stem, ".2cbh").exists() {
                    Err(Error::corrupt(&stem, 0, "both classic and 2CBH exist: pass an explicit .cbh or .2cbh path"))
                } else {
                    Err(Error::MissingFile { path: sibling(&stem, ".cbh"), role: Role::Headers })
                }
            }
        }
    }

    fn open_classic(stem: PathBuf) -> Result<Database> {
        for (ext, role) in MANDATORY {
            let path = sibling(&stem, ext);
            if path.symlink_metadata().is_err() {
                return Err(Error::MissingFile { path, role });
            }
        }

        let headers = CbhHeaders::open(&stem)?;
        let entities = Entities::open(&stem)?;
        let wide = Wide::open_auto(&stem);
        let flags = match Flags::open(&stem) {
            Ok(flags) => Some(flags),
            Err(Error::MissingFile { .. }) => None,
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let members = Members {
            headers: sibling(&stem, ".cbh"),
            moves: sibling(&stem, ".cbg"),
            annotations: sibling(&stem, ".cba"),
            players: sibling(&stem, ".cbp"),
            tournaments: sibling(&stem, ".cbt"),
            annotators: sibling(&stem, ".cbc"),
            sources: sibling(&stem, ".cbs"),
            wide: present(&stem, ".cbj"),
            teams: entities.team_count().gt(&0).then(|| sibling(&stem, ".cbe")),
            flags: flags.as_ref().map(|_| sibling(&stem, ".flags")),
            tournament_tree: present(&stem, ".cbtt"),
            cross_table: present(&stem, ".cbl"),
        };
        Ok(Database {
            base: stem,
            generation: Generation::Classic,
            members,
            backend: Backend::Classic(ClassicBackend {
                headers,
                entities,
                wide,
                flags,
                moves: std::sync::OnceLock::new(),
                annotations: std::sync::OnceLock::new(),
                format_entities: std::sync::OnceLock::new(),
            }),
        })
    }

    fn open_twocbh(stem: PathBuf) -> Result<Database> {
        for (ext, role) in MANDATORY_2CBH {
            let path = sibling(&stem, ext);
            if path.symlink_metadata().is_err() {
                return Err(Error::MissingFile { path, role });
            }
        }

        let headers = cbvault_format::twocbh::Headers::open(&stem)?;
        let entities = cbvault_format::twocbh::Entities::open(sibling(&stem, ".2lid"))?;
        let members = Members {
            headers: sibling(&stem, ".2cbh"),
            moves: sibling(&stem, ".2cbg"),
            annotations: sibling(&stem, ".2cba"),
            players: sibling(&stem, ".2lid"),
            tournaments: sibling(&stem, ".2lid"),
            annotators: sibling(&stem, ".2lid"),
            sources: sibling(&stem, ".2lid"),
            wide: None,
            teams: present(&stem, ".2lid"),
            flags: None,
            tournament_tree: present(&stem, ".2lgd"),
            cross_table: present(&stem, ".2lcd"),
        };
        Ok(Database {
            base: stem,
            generation: Generation::TwoCbh,
            members,
            backend: Backend::TwoCbh(TwoCbhBackend {
                headers,
                entities,
                moves: std::sync::OnceLock::new(),
                annotations: std::sync::OnceLock::new(),
            }),
        })
    }

    /// The base path the set's members share, without an extension.
    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Which generation the set is written in.
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// The files of the set, and which optional members were found.
    pub fn members(&self) -> &Members {
        &self.members
    }

    /// Records in the header file, guiding texts and deleted games included.
    pub fn records(&self) -> u32 {
        match &self.backend {
            Backend::Classic(c) => c.headers.records(),
            Backend::TwoCbh(t) => t.headers.games(),
        }
    }

    /// Header index of the actual generation; unlike `headers_file`,
    /// this never panics when the database is 2CBH.
    pub fn header_source(&self) -> HeaderSource<'_> {
        match &self.backend {
            Backend::Classic(c) => HeaderSource::Classic(&c.headers),
            Backend::TwoCbh(t) => HeaderSource::TwoCbh(&t.headers),
        }
    }

    /// Entity store of the actual generation; unlike `entities`, this
    /// never interprets the interleaved .2lid table as classic namebases.
    pub fn entity_source(&self) -> EntitySource<'_> {
        match &self.backend {
            Backend::Classic(c) => EntitySource::Classic(&c.entities),
            Backend::TwoCbh(t) => EntitySource::TwoCbh(&t.entities),
        }
    }

    /// Fallible classic-only header accessor for callers that need classic
    /// wire bytes, instead of panicking on a v2 source.
    pub fn try_classic_headers(&self) -> Result<&CbhHeaders> {
        match &self.backend {
            Backend::Classic(c) => Ok(&c.headers),
            Backend::TwoCbh(_) => Err(Error::corrupt(&self.base, 0, "classic headers are unavailable on 2CBH; use header_source()")),
        }
    }

    /// Fallible classic-only entity accessor for legacy consumers.
    pub fn try_classic_entities(&self) -> Result<&Entities> {
        match &self.backend {
            Backend::Classic(c) => Ok(&c.entities),
            Backend::TwoCbh(_) => Err(Error::corrupt(&self.base, 0, "classic entities are unavailable on 2CBH; use entity_source()")),
        }
    }

    /// The `.cbh` header file, for callers certain the database is classic.
    /// On v2, prefer the non-panicking `header_source` or `try_classic_headers`.
    pub fn headers_file(&self) -> &CbhHeaders {
        match &self.backend {
            Backend::Classic(c) => &c.headers,
            Backend::TwoCbh(_) => panic!("headers_file is only available on classic databases"),
        }
    }

    /// Classic namebases; only call after checking generation is Classic.
    /// For a safe v2-aware alternative, use `entity_source`.
    pub fn entities(&self) -> &Entities {
        match &self.backend {
            Backend::Classic(c) => &c.entities,
            Backend::TwoCbh(_) => {
                panic!("entities() is only available on classic databases; use twocbh_entities() for 2CBH")
            }
        }
    }

    /// The 2CBH headers, when the database is 2CBH.
    pub fn twocbh_headers(&self) -> Option<&cbvault_format::twocbh::Headers> {
        match &self.backend {
            Backend::Classic(_) => None,
            Backend::TwoCbh(t) => Some(&t.headers),
        }
    }

    /// The 2CBH entities, when the database is 2CBH.
    pub fn twocbh_entities(&self) -> Option<&cbvault_format::twocbh::Entities> {
        match &self.backend {
            Backend::Classic(_) => None,
            Backend::TwoCbh(t) => Some(&t.entities),
        }
    }

    /// The 64-bit offsets of a set whose moves or annotations pass 4 GiB, when
    /// it has a `.cbj` and the index was opened.
    pub fn wide(&self) -> Option<&Wide> {
        match &self.backend {
            Backend::Classic(c) => c.wide.as_ref(),
            Backend::TwoCbh(_) => None,
        }
    }

    /// The Top Games bits, when the set has a `.flags` file.
    pub fn flags(&self) -> Option<&Flags> {
        match &self.backend {
            Backend::Classic(c) => c.flags.as_ref(),
            Backend::TwoCbh(_) => None,
        }
    }

    /// The moves file, opened on the first request for a game and shared by
    /// every later one.
    pub(crate) fn moves(&self) -> Result<&DbFile> {
        match &self.backend {
            Backend::Classic(c) => {
                if let Some(file) = c.moves.get() {
                    return Ok(file);
                }
                let file = DbFile::open(self.members.moves.clone())?;
                Ok(c.moves.get_or_init(|| file))
            }
            Backend::TwoCbh(_) => {
                let seg = self.moves_segment()?;
                Ok(seg.db_file())
            }
        }
    }

    /// The 2CBH moves segment.
    pub(crate) fn moves_segment(&self) -> Result<&cbvault_format::twocbh::Segment> {
        match &self.backend {
            Backend::Classic(_) => Err(Error::corrupt(&self.base, 0, "not a 2CBH database")),
            Backend::TwoCbh(t) => {
                if let Some(s) = t.moves.get() {
                    return Ok(s);
                }
                let s = cbvault_format::twocbh::Segment::open(self.members.moves.clone())?;
                Ok(t.moves.get_or_init(|| s))
            }
        }
    }

    /// The annotation file, opened on the first request for annotations.
    pub(crate) fn annotations(&self) -> Result<&Annotations> {
        match &self.backend {
            Backend::Classic(c) => {
                if let Some(file) = c.annotations.get() {
                    return Ok(file);
                }
                let file = Annotations::open(&self.base)?;
                Ok(c.annotations.get_or_init(|| file))
            }
            Backend::TwoCbh(_) => Err(Error::corrupt(&self.base, 0, "classic annotations not supported on 2CBH")),
        }
    }

    /// The 2CBH annotations segment, if present.
    #[allow(dead_code)]
    pub(crate) fn annotations_segment(&self) -> Result<Option<&cbvault_format::twocbh::Segment>> {
        match &self.backend {
            Backend::Classic(_) => Ok(None),
            Backend::TwoCbh(t) => {
                if let Some(opt) = t.annotations.get() {
                    return Ok(opt.as_ref());
                }
                let path = self.members.annotations.clone();
                let seg = if path.exists() { Some(cbvault_format::twocbh::Segment::open(path)?) } else { None };
                let _ = t.annotations.set(seg);
                Ok(t.annotations.get().and_then(|o| o.as_ref()))
            }
        }
    }

    /// The format crate's entity tables, opened on the first request for PGN formatting.
    pub(crate) fn format_entities(&self) -> Result<&cbvault_format::cbh::Entities> {
        match &self.backend {
            Backend::Classic(c) => {
                if let Some(entities) = c.format_entities.get() {
                    return Ok(entities);
                }
                let entities = cbvault_format::cbh::Entities::open(&self.base)?;
                Ok(c.format_entities.get_or_init(|| entities))
            }
            Backend::TwoCbh(_) => Err(Error::corrupt(&self.base, 0, "classic format_entities not supported on 2CBH")),
        }
    }
}

/// The path `ext` appended to `stem` when the set has that member.
fn present(stem: &Path, ext: &str) -> Option<PathBuf> {
    let path = sibling(stem, ext);
    path.symlink_metadata().is_ok().then_some(path)
}

impl std::fmt::Debug for HeaderRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderRef")
            .field("id", &self.id)
            .field("white", &String::from_utf8_lossy(self.white.last()))
            .field("black", &String::from_utf8_lossy(self.black.last()))
            .field("event", &String::from_utf8_lossy(self.event.last()))
            .finish()
    }
}

/// What one game of a list looks like: its header record and the names its
/// entity ids resolve to, all borrowed.
///
/// The names are [`Name`]s — slices of the mapped namebase, not decoded
/// strings — so a list of eleven million games allocates nothing and copies
/// nothing: a caller that wants `&str` takes [`Name::as_str`], which answers
/// for every name the database stores as UTF-8, and decodes the rest through
/// [`Name::push_text`].
#[derive(Clone, Copy)]
pub struct HeaderRef<'a> {
    /// The record's id in `.cbh`.
    pub id: u32,
    /// The header record as stored.
    pub header: GameHeaderRef<'a>,
    /// The white player: a last name and a forename, in one record.
    pub white: Name<'a>,
    /// The black player.
    pub black: Name<'a>,
    /// The tournament: a title, and a place in [`HeaderRef::site`].
    pub event: Name<'a>,
    /// The tournament's place.
    pub site: Name<'a>,
    /// The game's annotator, or a blank name.
    pub annotator: Name<'a>,
    /// The game's source, or a blank name.
    pub source: Name<'a>,
}

/// What a walk over the headers of a database met.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderStats {
    /// Game records handed to the caller.
    pub games: u64,
    /// Guiding texts skipped: records that hold text, not a game.
    pub texts: u64,
    /// Records marked deleted.
    pub deleted: u64,
    /// Records past the last game, which the format has no kind for.
    pub unknown: u64,
}

impl Default for Records<'_> {
    fn default() -> Records<'static> {
        Records::Mapped(&[])
    }
}

/// The records of a batch, borrowed from the memory map where there is one.
enum Records<'db> {
    Mapped(&'db [u8]),
    Owned(Vec<u8>),
}

impl Records<'_> {
    /// The `index`th record of the batch, which starts at id `first`.
    fn get(&self, first: u32, index: u32) -> Option<GameHeader> {
        let at = index as usize * 46;
        let bytes: &[u8; 46] = self.as_slice().get(at..at + 46)?.try_into().ok()?;
        Some(GameHeader::from_bytes(first + index, bytes))
    }

    /// The batch's bytes.
    fn as_slice(&self) -> &[u8] {
        match self {
            Records::Mapped(slice) => slice,
            Records::Owned(bytes) => bytes,
        }
    }
}

/// How many records one list step reads: 8,192 records are 376 KB, the batch
/// the replay and export paths read at too.
const LIST_BATCH: u32 = 8192;

/// The name of one entity kind, remembered from the record before, so a list
/// resolves a name once per *run* of games rather than once per game.
///
/// On the reference database consecutive records repeat the same tournament in
/// 99.0 % of steps, the same annotator in 98.5 % and the same source in 98.2 %;
/// the two players repeat in 4.2 % and 0.25 % of steps, so those two are looked
/// up every time. One remembered id per field is six words of state and turns
/// three of the five lookups per record into a comparison.
#[derive(Debug)]
struct Memo<'db> {
    white: Option<(u32, Name<'db>)>,
    black: Option<(u32, Name<'db>)>,
    event: Option<(u32, Name<'db>)>,
    annotator: Option<(u32, Name<'db>)>,
    source: Option<(u32, Name<'db>)>,
}

impl<'db> Memo<'db> {
    /// A memo with nothing remembered yet.
    fn new() -> Memo<'db> {
        Memo { white: None, black: None, event: None, annotator: None, source: None }
    }

    /// The name of `entity` at `id`, remembered from the last call.
    ///
    /// The remembered name borrows the same mapped namebase, so it stays valid
    /// for as long as the database does; nothing is re-read and nothing is
    /// allocated on a hit.
    fn resolve(&mut self, entities: &'db Entities, entity: Entity, id: u32) -> Result<Name<'db>> {
        let slot = match (entity, id) {
            (Entity::Player, _) if self.white.as_ref().is_some_and(|(seen, _)| *seen == id) => &mut self.white,
            (Entity::Player, _) if self.black.as_ref().is_some_and(|(seen, _)| *seen == id) => &mut self.black,
            (Entity::Player, _) => &mut self.white,
            (Entity::Tournament, _) => &mut self.event,
            (Entity::Annotator, _) => &mut self.annotator,
            _ => &mut self.source,
        };
        if let Some((seen, name)) = *slot
            && seen == id
        {
            return Ok(name);
        }
        let name = entities.name(entity, id)?.unwrap_or(BLANK);
        *slot = Some((id, name));
        Ok(name)
    }
}

/// The name of an entity the database does not have: empty, never `None`, so a
/// caller never has to decide what a missing name means.
const BLANK: Name<'static> = Name::blank();

/// An iterator over the game headers of a database, with the entity names each
/// record's ids resolve to.
///
/// It reads the header file and the namebases and nothing else: the moves file
/// is never opened, and neither are the annotations. Records that are not games
/// — guiding texts, records marked deleted, records past the last game — are
/// skipped, and counted in the [`HeaderStats`] the bulk walk reports.
///
/// The iterator **lends**: its items borrow both the memory map and the
/// iterator's own remembered names, so `next` takes and returns borrows of
/// `self` rather than being a `std::iter::Iterator`. That is what makes the
/// names free: an item that owned five decoded names would copy 700 bytes per
/// record, and eleven million times that is not a game list. Use
/// [`Headers::for_each`] for the bulk form, which is what the eleven-million
/// record measurement is taken on.
pub struct Headers<'db> {
    db: &'db Database,
    /// The id of the next record to read.
    at: u32,
    /// The last id of the set.
    last: u32,
    /// The records of the current batch.
    batch: Records<'db>,
    /// The first id in `batch`.
    batch_first: u32,
    /// How many records `batch` holds.
    batch_len: u32,
    /// The next index within `batch`.
    index: u32,
    memo: Memo<'db>,
    stats: HeaderStats,
}

impl<'db> Headers<'db> {
    /// The iterator over `db`'s games, starting at the first record.
    pub fn new(db: &'db Database) -> Headers<'db> {
        Headers {
            db,
            at: 1,
            last: db.records(),
            batch: Records::Owned(Vec::new()),
            batch_first: 1,
            batch_len: 0,
            index: 0,
            memo: Memo::new(),
            stats: HeaderStats::default(),
        }
    }

    /// What the walk has met so far.
    pub fn stats(&self) -> HeaderStats {
        self.stats
    }

    /// How many games the walk has handed over so far.
    pub fn count(&self) -> u64 {
        self.stats.games
    }

    /// The whole list, in order, through `f`, which may fail: the fallible
    /// bulk form, for a caller that wants the typed error rather than
    /// [`Iterator::for_each`]'s silence.
    pub fn try_for_each(&mut self, mut f: impl FnMut(HeaderRef<'db>) -> Result<()>) -> Result<HeaderStats> {
        for header in self.by_ref() {
            f(header?)?;
        }
        Ok(self.stats)
    }

    /// Reads the next batch of records, from the map where there is one.
    fn fill(&mut self) -> Result<()> {
        let Backend::Classic(c) = &self.db.backend else {
            return Ok(());
        };
        let count = LIST_BATCH.min(self.last - self.at + 1);
        let bytes = count as usize * 46;
        let at = u64::from(self.at) * 46;
        let file = c.headers.db_file();
        self.batch_first = self.at;
        self.batch_len = count;
        self.index = 0;
        self.at += count;
        self.batch = match file.slice_at(at, bytes) {
            Some(slice) => Records::Mapped(slice),
            None => {
                let mut owned = match std::mem::replace(&mut self.batch, Records::Mapped(&[])) {
                    Records::Owned(bytes) => bytes,
                    Records::Mapped(_) => Vec::new(),
                };
                owned.clear();
                owned.resize(bytes, 0);
                c.headers.read_records(self.batch_first, count, &mut owned)?;
                Records::Owned(owned)
            }
        };
        Ok(())
    }
}

impl<'db> Iterator for Headers<'db> {
    type Item = Result<HeaderRef<'db>>;

    /// The next game, or `None` at the end of the set.
    ///
    /// Nothing that is not a game is yielded: a guiding text, a record marked
    /// deleted and a record past the last game are passed over and tallied in
    /// [`Headers::stats`], which is what makes the difference between
    /// [`Database::records`] and [`Database::game_count`] the number a caller
    /// reports.
    fn next(&mut self) -> Option<Result<HeaderRef<'db>>> {
        let db: &'db Database = self.db;
        if let Backend::TwoCbh(t) = &db.backend {
            while self.at <= self.last {
                let id = self.at;
                self.at += 1;
                let header = match t.headers.record(id) {
                    Ok(h) => h,
                    Err(e) => return Some(Err(e)),
                };
                if header.is_deleted() {
                    self.stats.deleted += 1;
                    continue;
                }
                match header.kind() {
                    RecordKind::Game => {}
                    RecordKind::Text => {
                        self.stats.texts += 1;
                        continue;
                    }
                    RecordKind::Analysis | RecordKind::Unknown(_) => {
                        self.stats.unknown += 1;
                        continue;
                    }
                }
                self.stats.games += 1;
                return Some(Ok(HeaderRef {
                    id,
                    header: GameHeaderRef::TwoCbh(header),
                    white: BLANK,
                    black: BLANK,
                    event: BLANK,
                    site: BLANK,
                    annotator: BLANK,
                    source: BLANK,
                }));
            }
            return None;
        }

        loop {
            if self.index >= self.batch_len {
                if self.at > self.last {
                    return None;
                }
                if let Err(e) = self.fill() {
                    return Some(Err(e));
                }
            }
            let index = self.index;
            self.index += 1;
            let Some(header) = self.batch.get(self.batch_first, index) else { continue };
            match header.kind() {
                RecordKind::Game => {}
                RecordKind::Text => {
                    self.stats.texts += 1;
                    continue;
                }
                RecordKind::Analysis | RecordKind::Unknown(_) => {
                    self.stats.unknown += 1;
                    continue;
                }
            }
            if header.is_deleted() {
                self.stats.deleted += 1;
                continue;
            }
            let entities = db.entities();
            let resolved = (|| -> Result<HeaderRef<'db>> {
                let white = self.memo.resolve(entities, Entity::Player, header.white())?;
                let black = self.memo.resolve(entities, Entity::Player, header.black())?;
                let tournament = self.memo.resolve(entities, Entity::Tournament, header.tournament())?;
                let event = Name::of(tournament.last());
                let site = Name::of(tournament.first());
                let annotator = self.memo.resolve(entities, Entity::Annotator, header.annotator())?;
                let source = self.memo.resolve(entities, Entity::Source, header.source())?;
                Ok(HeaderRef {
                    id: header.id(),
                    header: GameHeaderRef::ClassicOwned(header),
                    white,
                    black,
                    event,
                    site,
                    annotator,
                    source,
                })
            })();
            match resolved {
                Ok(item) => {
                    self.stats.games += 1;
                    return Some(Ok(item));
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

impl Database {
    /// Every game of the database, as a list of headers with their resolved names.
    ///
    /// Nothing but the header file and the namebases is read. The moves file is
    /// never opened — asserted by a test that instruments this module's open path
    /// and, independently, by one that renames `.cbg` away before listing — so the
    /// cost of a list is a sequential pass over `.cbh` and a few name lookups per
    /// record. Over the reference database (11,149,379 games) that is a 513 MB scan.
    pub fn headers(&self) -> Headers<'_> {
        Headers::new(self)
    }

    /// The number of records that are games — not guiding texts, not marked
    /// deleted, not past the last game.
    ///
    /// This is a scan of the header file, so it costs a pass over `.cbh` rather
    /// than a field read; the count the file states for free is
    /// [`Database::records`]. Only a caller that needs the exact number for a
    /// report should pay for it.
    pub fn game_count(&self) -> Result<u64> {
        let mut headers = self.headers();
        let stats = {
            for _ in headers.by_ref() {}
            headers.stats()
        };
        Ok(stats.games)
    }

    /// The header record of game `id`, borrowed from the mapped `.cbh` with no copy
    /// and no allocation.
    pub fn header_ref(&self, id: u32) -> Result<GameHeaderRef<'_>> {
        if id == 0 || id > self.records() {
            return Err(Error::NoSuchGame { id });
        }
        match &self.backend {
            Backend::Classic(c) => {
                let at = u64::from(id) * 46;
                let bytes = c
                    .headers
                    .db_file()
                    .slice_at(at, 46)
                    .and_then(|slice| <&[u8; 46]>::try_from(slice).ok())
                    .ok_or_else(|| Error::corrupt(c.headers.path(), at, "header record out of range"))?;
                Ok(GameHeaderRef::Classic(cbvault_format::cbh::GameHeaderRef::from_bytes(id, bytes)))
            }
            Backend::TwoCbh(t) => {
                let header = t.headers.record(id)?;
                Ok(GameHeaderRef::TwoCbh(header))
            }
        }
    }

    /// The byte offset of `header`'s move record, in `.cbg`.
    pub fn move_offset(&self, header: &GameHeaderRef<'_>) -> Result<u64> {
        match header {
            GameHeaderRef::Classic(h) => self.offset_of(h.id(), h.moves_offset(), h.annotations_offset()),
            GameHeaderRef::ClassicOwned(h) => self.offset_of(h.id(), h.moves_offset(), h.annotations_offset()),
            GameHeaderRef::TwoCbh(h) => Ok(h.moves_offset()),
        }
    }

    /// The byte offset of a game whose record holds the 32-bit offsets `short`.
    ///
    /// A set whose `.cbg` passes 4 GiB keeps the real offsets in `.cbj`, and a
    /// record whose `.cbj` entry disagrees with the 32-bit ones is a corrupt record
    /// rather than a guess — the same rule the batch and the PGN export follow.
    pub fn offset_of(&self, id: u32, moves: u32, annotations: u32) -> Result<u64> {
        let short = (moves, annotations);
        Ok(match self.wide() {
            Some(wide) => wide.offsets(id, short)?.0,
            None => u64::from(short.0),
        })
    }

    /// One game: its header, its `moves2` main line in `buf`, its position keys if
    /// `buf` was told to keep them, its resolved names, and — on request — its
    /// annotations.
    ///
    /// The `GameRef` borrows both the database and `buf`, so it is valid until the
    /// next call with the same buffer, and a caller that wants to keep a game copies
    /// what it needs out of it. The moves are exactly the ones
    /// [`for_each_game`] hands a sink for the same id, so this is a read of one
    /// game and not a second decoder.
    ///
    /// ```no_run
    /// use cbvault::bridge::{Database, GameBuf};
    ///
    /// let db = Database::open("Mega Database 2025/Mega Database 2025")?;
    /// let mut buf = GameBuf::new();
    /// let game = db.game(1, &mut buf)?;
    /// println!("{} moves for {}", game.moves.len(), game.white);
    /// # Ok::<(), cbvault::bridge::Error>(())
    /// ```
    pub fn game<'a>(&'a self, id: u32, buf: &'a mut GameBuf) -> Result<GameRef<'a>> {
        self.game_with(id, buf, buf.wants_annotations())
    }

    /// [`Database::game`], with the annotations asked for or not by the call rather
    /// than by the buffer. A single-game read is what a game view uses, so this is
    /// the shape that answers "show me this game with its comments".
    pub fn game_with<'a>(&'a self, id: u32, buf: &'a mut GameBuf, want_annotations: bool) -> Result<GameRef<'a>> {
        let header = self.header_ref(id)?;
        let at = self.move_offset(&header)?;
        match &self.backend {
            Backend::Classic(_) => {
                let mut record = Vec::new();
                let bytes = convert::move_record(self.moves()?, at, &mut record)?;
                let game = GameMoves::parse(&self.members.moves, bytes)?;
                buf.walk(id, at, &game)?;
                walk::resolve_names(&header, self.entities(), buf)?;
                buf.view(id, header, self, want_annotations)
            }
            Backend::TwoCbh(t) => {
                let seg = self.moves_segment()?;
                let seg_rec = seg.record_at(at)?;
                let game =
                    cbvault_format::twocbh::moves::GameMoves::from_record(&self.members.moves, seg_rec.record())?;
                buf.walk_v2(id, at, &game)?;
                walk::resolve_names_v2(&header, &t.entities, buf)?;
                buf.view(id, header, self, want_annotations)
            }
        }
    }

    /// Exports game `id` as a PGN string, using `buf`'s annotation preference.
    pub fn game_pgn(&self, id: u32, buf: &mut GameBuf) -> Result<String> {
        self.game_pgn_with(id, buf, buf.wants_annotations())
    }

    /// Exports game `id` as a complete PGN string: tags, movetext, and annotations
    /// when `want_annotations` is true.
    ///
    /// Formats both standard games with moves and historical move-less (score-only)
    /// games cleanly (emitting `[PlyCount "0"]` and the terminal result).
    pub fn game_pgn_with(&self, id: u32, buf: &mut GameBuf, want_annotations: bool) -> Result<String> {
        let header = self.header_ref(id)?;
        if !matches!(header.kind(), RecordKind::Game | RecordKind::Analysis) {
            return Err(Error::corrupt(self.base(), u64::from(id), format!("record {id} is not a game or analysis")));
        }
        let at = self.move_offset(&header)?;
        match &self.backend {
            Backend::Classic(_) => {
                let mut record = Vec::new();
                let bytes = convert::move_record(self.moves()?, at, &mut record)?;
                let game = GameMoves::parse(&self.members.moves, bytes)?;
                let anns = if want_annotations && header.annotations_offset() != 0 {
                    let h_ref = header.as_classic().expect("classic header");
                    Some(self.annotations()?.of_ref(&h_ref, self.wide(), buf.annotation_scratch())?)
                } else {
                    None
                };
                let entities = self.format_entities()?;
                let mut writer = crate::pgn::PgnWriter::new();
                let mut out = Vec::new();
                let h_ref = header.as_classic().expect("classic header");
                writer
                    .write_game(&mut out, &h_ref, entities, &game, anns.as_ref())
                    .map_err(|e| Error::corrupt(self.base(), u64::from(id), e.to_string()))?;
                String::from_utf8(out).map_err(|e| Error::corrupt(self.base(), u64::from(id), e.to_string()))
            }
            Backend::TwoCbh(t) => {
                let seg = self.moves_segment()?;
                let seg_rec = seg.record_at(at)?;
                let game =
                    cbvault_format::twocbh::moves::GameMoves::from_record(&self.members.moves, seg_rec.record())?;
                let h = header.as_twocbh().copied().expect("2cbh header");
                let anns_rec = if want_annotations && h.annotations_offset() != 0 {
                    let seg = self.annotations_segment()?.ok_or_else(|| {
                        Error::corrupt(
                            &self.members.annotations,
                            h.annotations_offset(),
                            "2CBA annotation frame referenced but file is missing",
                        )
                    })?;
                    Some(seg.record_at(h.annotations_offset())?)
                } else {
                    None
                };
                let anns = anns_rec.as_ref().map(|r| cbvault_format::twocbh::Annotations::from_record(r.record()));
                let mut writer = crate::pgn::PgnWriter::new();
                let mut out = Vec::new();
                writer
                    .write_game_v2(&mut out, &h, &t.entities, &game, anns.as_ref())
                    .map_err(|e| Error::corrupt(self.base(), u64::from(id), e.to_string()))?;
                String::from_utf8(out).map_err(|e| Error::corrupt(self.base(), u64::from(id), e.to_string()))
            }
        }
    }

    /// The records `first..=last` as one batch, borrowed from the map where the file
    /// is mapped.
    ///
    /// The whole point of the batch is that a list or a scan reads `.cbh` in
    /// 8,192-record steps instead of one 46-byte read per record, which is what
    /// keeps eleven million headers inside two seconds. Without the `mmap` feature
    /// the batch is read into a buffer of its own, once per batch.
    pub fn record_batch<'a>(&'a self, first: u32, last: u32) -> Result<RecordBatch<'a>> {
        RecordBatch::open(self, first, last)
    }
}

/// One batch of header records, in id order.
///
/// Borrows the mapped `.cbh` where the file is mapped, and a caller's scratch
/// where it is not, so a scan over eleven million records is a few hundred
/// reads rather than eleven million.
pub struct RecordBatch<'a> {
    records: Records<'a>,
    first: u32,
    index: u32,
    len: u32,
}

impl<'a> RecordBatch<'a> {
    /// Reads the records `first..=last`, bounded by what the file holds.
    fn open(db: &'a Database, first: u32, last: u32) -> Result<RecordBatch<'a>> {
        let Backend::Classic(c) = &db.backend else {
            return Err(Error::corrupt(db.base(), 0, "RecordBatch not supported on 2CBH"));
        };
        let total = db.records();
        let first = first.max(1);
        let last = last.min(total);
        if first > last {
            return Ok(RecordBatch { records: Records::Mapped(&[]), first, index: 0, len: 0 });
        }
        let count = last - first + 1;
        let bytes = count as usize * 46;
        let at = u64::from(first) * 46;
        let file = c.headers.db_file();
        let records = match file.slice_at(at, bytes) {
            Some(slice) => Records::Mapped(slice),
            None => {
                let mut owned = vec![0u8; bytes];
                c.headers.read_records(first, count, &mut owned)?;
                Records::Owned(owned)
            }
        };
        Ok(RecordBatch { records, first, index: 0, len: count })
    }

    /// The ids the batch covers.
    pub fn ids(&self) -> std::ops::RangeInclusive<u32> {
        self.first..=self.first + self.len.saturating_sub(1)
    }

    /// How many records the batch holds.
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Iterator for RecordBatch<'_> {
    type Item = GameHeader;

    fn next(&mut self) -> Option<GameHeader> {
        if self.index >= self.len {
            return None;
        }
        let index = self.index;
        self.index += 1;
        self.records.get(self.first, index)
    }
}
