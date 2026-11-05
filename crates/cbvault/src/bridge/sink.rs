//! What a consumer's sink is handed, and what it says it needs.
//!
//! [`GameRef`] is entirely borrowed. Every field is a slice of bytes the walk
//! already holds — the record from the memory map, the names out of the
//! mapped namebases, the `moves2` and the keys in the walker's reused buffers —
//! so a sink that only counts games allocates nothing, and a sink that writes a
//! row copies into its own target and never back.
//!
//! Two of the design's fields are additions to the shape this file was given:
//! [`GameRef::site`] and [`GameRef::annotations`]. The first is a resolved name
//! the consumer's `Games` row needs (`SiteID`) and costs one reused buffer; the
//! second is what [`GameSink::wants_annotations`] promises to deliver, which the
//! design's field list had no slot for. Everything else is as specified.

use cbvault_format::cbh::GameAnnotations;
use cbvault_format::error::Error;
use cbvault_format::game::{Date, Eco, GameResult, Head, RecordKind};

/// A header record borrowed from either a classic `.cbh` or 2CBH `.2cbh` database.
/// A borrowed or owned game header across supported database generations.
#[derive(Clone, Copy)]
pub enum GameHeaderRef<'a> {
    /// A borrowed classic 46-byte `.cbh` header record.
    Classic(cbvault_format::cbh::GameHeaderRef<'a>),
    /// An owned classic 46-byte `.cbh` header record.
    ClassicOwned(cbvault_format::cbh::GameHeader),
    /// An owned 2CBH 192-byte `.2cbh` header record.
    TwoCbh(cbvault_format::twocbh::GameHeader),
}

impl std::fmt::Debug for GameHeaderRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameHeaderRef").field("id", &self.id()).finish()
    }
}

impl<'a> GameHeaderRef<'a> {
    /// Borrows the classic header if this is a classic record.
    pub fn as_classic(&self) -> Option<cbvault_format::cbh::GameHeaderRef<'_>> {
        match self {
            Self::Classic(h) => Some(*h),
            Self::ClassicOwned(h) => {
                Some(cbvault_format::cbh::GameHeaderRef::from_bytes(h.id(), h.bytes().try_into().unwrap()))
            }
            Self::TwoCbh(_) => None,
        }
    }

    /// Borrows the 2CBH header if this is a 2CBH record.
    pub fn as_twocbh(&self) -> Option<&cbvault_format::twocbh::GameHeader> {
        match self {
            Self::TwoCbh(h) => Some(h),
            _ => None,
        }
    }

    /// Raw bytes of the header record.
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Classic(h) => h.bytes(),
            Self::ClassicOwned(h) => h.bytes(),
            Self::TwoCbh(h) => h.bytes(),
        }
    }

    /// Game or record identifier (1-based).
    #[inline]
    pub fn id(&self) -> u32 {
        match self {
            Self::Classic(h) => h.id(),
            Self::ClassicOwned(h) => h.id(),
            Self::TwoCbh(h) => h.id(),
        }
    }

    /// Whether this record is marked as deleted.
    #[inline]
    pub fn is_deleted(&self) -> bool {
        match self {
            Self::Classic(h) => h.is_deleted(),
            Self::ClassicOwned(h) => h.is_deleted(),
            Self::TwoCbh(h) => h.is_deleted(),
        }
    }

    /// The kind of record (game, text, analysis, unknown).
    #[inline]
    pub fn kind(&self) -> RecordKind {
        match self {
            Self::Classic(h) => h.kind(),
            Self::ClassicOwned(h) => h.kind(),
            Self::TwoCbh(h) => h.kind(),
        }
    }

    /// Moves record byte offset in moves container.
    #[inline]
    pub fn moves_offset(&self) -> u64 {
        match self {
            Self::Classic(h) => u64::from(h.moves_offset()),
            Self::ClassicOwned(h) => u64::from(h.moves_offset()),
            Self::TwoCbh(h) => h.moves_offset(),
        }
    }

    /// Annotations record byte offset in annotations container.
    #[inline]
    pub fn annotations_offset(&self) -> u64 {
        match self {
            Self::Classic(h) => u64::from(h.annotations_offset()),
            Self::ClassicOwned(h) => u64::from(h.annotations_offset()),
            Self::TwoCbh(h) => h.annotations_offset(),
        }
    }

    /// White player entity key or ID.
    #[inline]
    pub fn white(&self) -> i64 {
        match self {
            Self::Classic(h) => i64::from(h.white()),
            Self::ClassicOwned(h) => i64::from(h.white()),
            Self::TwoCbh(h) => h.white(),
        }
    }

    /// Black player entity key or ID.
    #[inline]
    pub fn black(&self) -> i64 {
        match self {
            Self::Classic(h) => i64::from(h.black()),
            Self::ClassicOwned(h) => i64::from(h.black()),
            Self::TwoCbh(h) => h.black(),
        }
    }

    /// Tournament entity key or ID.
    #[inline]
    pub fn tournament(&self) -> i64 {
        match self {
            Self::Classic(h) => i64::from(h.tournament()),
            Self::ClassicOwned(h) => i64::from(h.tournament()),
            Self::TwoCbh(h) => h.tournament(),
        }
    }

    /// Annotator entity key or ID.
    #[inline]
    pub fn annotator(&self) -> i64 {
        match self {
            Self::Classic(h) => i64::from(h.annotator()),
            Self::ClassicOwned(h) => i64::from(h.annotator()),
            Self::TwoCbh(h) => h.annotator(),
        }
    }

    /// Source entity key or ID.
    #[inline]
    pub fn source(&self) -> i64 {
        match self {
            Self::Classic(h) => i64::from(h.source()),
            Self::ClassicOwned(h) => i64::from(h.source()),
            Self::TwoCbh(h) => h.source(),
        }
    }

    /// Played date as recorded in the header.
    #[inline]
    pub fn played_date(&self) -> Date {
        match self {
            Self::Classic(h) => h.played_date(),
            Self::ClassicOwned(h) => h.played_date(),
            Self::TwoCbh(h) => h.played_date(),
        }
    }

    /// Game result.
    #[inline]
    pub fn result(&self) -> GameResult {
        match self {
            Self::Classic(h) => h.result(),
            Self::ClassicOwned(h) => h.result(),
            Self::TwoCbh(h) => h.result(),
        }
    }

    /// Round and subround numbers.
    #[inline]
    pub fn round(&self) -> (i32, i32) {
        match self {
            Self::Classic(h) => (i32::from(h.round()), i32::from(h.subround())),
            Self::ClassicOwned(h) => (i32::from(h.round()), i32::from(h.subround())),
            Self::TwoCbh(h) => (i32::from(h.round()), i32::from(h.subround())),
        }
    }

    /// White and black Elo ratings.
    #[inline]
    pub fn elo(&self) -> (i32, i32) {
        match self {
            Self::Classic(h) => (i32::from(h.white_elo()), i32::from(h.black_elo())),
            Self::ClassicOwned(h) => (i32::from(h.white_elo()), i32::from(h.black_elo())),
            Self::TwoCbh(h) => (i32::from(h.white_elo()), i32::from(h.black_elo())),
        }
    }

    /// White player's Elo rating.
    #[inline]
    pub fn white_elo(&self) -> u16 {
        match self {
            Self::Classic(h) => h.white_elo(),
            Self::ClassicOwned(h) => h.white_elo(),
            Self::TwoCbh(h) => h.white_elo().max(0) as u16,
        }
    }

    /// Black player's Elo rating.
    #[inline]
    pub fn black_elo(&self) -> u16 {
        match self {
            Self::Classic(h) => h.black_elo(),
            Self::ClassicOwned(h) => h.black_elo(),
            Self::TwoCbh(h) => h.black_elo().max(0) as u16,
        }
    }

    /// ECO opening code.
    #[inline]
    pub fn eco(&self) -> Eco {
        match self {
            Self::Classic(h) => h.eco(),
            Self::ClassicOwned(h) => h.eco(),
            Self::TwoCbh(h) => h.eco(),
        }
    }

    /// Move count (plies or fullmoves depending on stored format).
    #[inline]
    pub fn move_count(&self) -> i32 {
        match self {
            Self::Classic(h) => i32::from(h.move_count()),
            Self::ClassicOwned(h) => i32::from(h.move_count()),
            Self::TwoCbh(h) => i32::from(h.move_count()),
        }
    }
}

impl<'a> Head for GameHeaderRef<'a> {
    fn id(&self) -> u32 {
        self.id()
    }
    fn kind(&self) -> RecordKind {
        self.kind()
    }
    fn is_deleted(&self) -> bool {
        self.is_deleted()
    }
    fn white(&self) -> i64 {
        self.white()
    }
    fn black(&self) -> i64 {
        self.black()
    }
    fn tournament(&self) -> i64 {
        self.tournament()
    }
    fn annotator(&self) -> i64 {
        self.annotator()
    }
    fn other(&self) -> Option<(i64, i64)> {
        match self {
            Self::Classic(h) => h.other(),
            Self::ClassicOwned(h) => h.other(),
            Self::TwoCbh(h) => h.other(),
        }
    }
    fn result(&self) -> GameResult {
        self.result()
    }
    fn eco(&self) -> Eco {
        self.eco()
    }
    fn played_date(&self) -> Date {
        self.played_date()
    }
    fn round(&self) -> (i32, i32) {
        self.round()
    }
    fn elo(&self) -> (i32, i32) {
        self.elo()
    }
    fn move_count(&self) -> i32 {
        self.move_count()
    }
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Classic(h) => h.bytes(),
            Self::ClassicOwned(h) => h.bytes(),
            Self::TwoCbh(h) => h.bytes(),
        }
    }
}

impl<'a> From<cbvault_format::cbh::GameHeaderRef<'a>> for GameHeaderRef<'a> {
    fn from(h: cbvault_format::cbh::GameHeaderRef<'a>) -> Self {
        GameHeaderRef::Classic(h)
    }
}

impl<'a> From<cbvault_format::cbh::GameHeader> for GameHeaderRef<'a> {
    fn from(h: cbvault_format::cbh::GameHeader) -> Self {
        GameHeaderRef::ClassicOwned(h)
    }
}

impl<'a> From<cbvault_format::twocbh::GameHeader> for GameHeaderRef<'a> {
    fn from(h: cbvault_format::twocbh::GameHeader) -> Self {
        GameHeaderRef::TwoCbh(h)
    }
}

/// One game, as a conversion or a read-only view hands it over.
///
/// Every field borrows: nothing here owns memory, and a sink that keeps a
/// `GameRef` past the call has to copy what it wants. The `moves` slice is the
/// **main line** only — the format's variations are walked, and their keys
/// computed, but the line the target stores is the one handed over (see
/// `docs/bridge.md`).
///
/// `keys` is empty unless the sink asked for keys with
/// [`GameSink::wants_keys`], and then holds one Polyglot key per position of
/// the main line, **the start position first**: `keys.len() == moves.len() + 1`,
/// and the sequence equals what `gigachess::database::replay_moves2_hashes`
/// returns for the same `moves` and `start_fen`, which is the contract the
/// consumer's own sidecar is built on.
pub struct GameRef<'a> {
    /// The game's number in `.cbh`: stable, 1-based, and ascending across a
    /// conversion whatever the thread count.
    pub id: u32,
    /// The header record as stored: date, result, round, ratings, ECO, medals,
    /// flags, the move count, and the entity ids the names below came from.
    pub header: GameHeaderRef<'a>,
    /// The white player, as `Last, First`.
    pub white: &'a str,
    /// The black player, as `Last, First`.
    pub black: &'a str,
    /// The tournament's title.
    pub event: &'a str,
    /// The tournament's place — an addition to the design's list, and the
    /// `SiteID` of a converted row.
    pub site: &'a str,
    /// The game's annotator, empty when it has none.
    pub annotator: &'a str,
    /// The game's source, empty when it has none.
    pub source: &'a str,
    /// The position the game starts from, as FEN; `None` for the standard
    /// start, which is all but about 1,500 games of the reference database and
    /// is therefore never rendered.
    pub start_fen: Option<&'a str>,
    /// The main line as 16-bit `moves2`, in the order the moves were played.
    pub moves: &'a [u16],
    /// One Polyglot key per position of the main line, the start position
    /// first; empty unless the sink asked for keys.
    pub keys: &'a [u64],
    /// The game's annotations, borrowed from the `.cba` record, when the sink
    /// asked for them; `None` otherwise, and an empty record for a game that
    /// has none.
    pub annotations: Option<GameAnnotations<'a>>,
}

/// The consumer's side of the conversion: one call per game, in game-number
/// order, from one thread at a time.
///
/// A sink is asked once per run what it needs ([`GameSink::wants_keys`],
/// [`GameSink::wants_annotations`]) rather than once per ply, so the walk can
/// choose its make — `Board::play_fast` with neither, the hash-maintaining
/// `play_hashed` with keys — once for the whole conversion rather than
/// per move. That decision is the same contract ADR-003 made one level down,
/// moved up to where it belongs.
pub trait GameSink {
    /// One game, in ascending game-number order. Called from one thread at a
    /// time, so a sink that owns a writer does not need a lock.
    fn game(&mut self, game: GameRef<'_>);

    /// Whether the walk should maintain the incremental Polyglot key, and hand
    /// one per position in [`GameRef::keys`]. `false` (the default) keeps the
    /// fast make, measured at 48.3 ns/ply over the reference database against
    /// 53.4 for the keyed walk; asking for keys costs about 11 % of the pass and
    /// saves a second pass over the source entirely.
    fn wants_keys(&self) -> bool {
        false
    }

    /// Whether the walk should stop early. Polled, not pushed: the sequential
    /// walk asks before each record, the parallel walk before each wave, and a
    /// `true` answer ends the run with the stats gathered so far and
    /// `complete: false`. `false` (the default) walks to the last record,
    /// which is today's behaviour for every existing sink.
    fn cancelled(&self) -> bool {
        false
    }

    /// Whether the `.cba` record of each game should be read and parsed into
    /// [`GameRef::annotations`]. `false` (the default) does not open the file
    /// at all — 209 MB on the reference database.
    fn wants_annotations(&self) -> bool {
        false
    }

    /// A game that could not be decoded. The walk continues with the next one;
    /// the error is typed, and a damaged record is never a panic.
    fn failed(&mut self, _id: u32, _error: &Error) {}
}

impl GameRef<'_> {
    /// The game's exact ply count — one per move played.
    ///
    /// `moves.len()` already answers this, and a caller may well write that.
    /// This exists because the number is easy to get subtly wrong: the header's
    /// `move_count()` is a `u8` capped at 255 counting **moves**, not plies, so
    /// `2 * header.move_count()` is the right answer only for a game from the
    /// standard start with no castling bookkeeping — it is wrong for a game with
    /// a set-up start, and silently so.
    ///
    /// A consumer filtering `ply_count_from`/`ply_count_to` wants this, and wants
    /// it from the move stream the walk has already read, not from the header.
    /// Free: the walk knows the length.
    #[inline]
    pub fn plies(&self) -> usize {
        self.moves.len()
    }

    /// Whether the walk was asked for this game's keys.
    ///
    /// A game with no keys is not a game with no positions — it is a game whose
    /// sink did not ask, so `plies` and the key count cannot be derived from each
    /// other without this.
    #[inline]
    pub fn has_keys(&self) -> bool {
        !self.keys.is_empty()
    }
}

impl std::fmt::Debug for GameRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameRef")
            .field("id", &self.id)
            .field("white", &self.white)
            .field("black", &self.black)
            .field("event", &self.event)
            .field("site", &self.site)
            .field("annotator", &self.annotator)
            .field("source", &self.source)
            .field("start_fen", &self.start_fen)
            .field("moves", &self.moves.len())
            .field("keys", &self.keys.len())
            .field("annotations", &self.annotations.as_ref().map(|a| a.count()))
            .finish()
    }
}
