//! Searching a raw database without converting it.
//!
//! Two kinds of question, answered two ways.
//!
//! A **tag search** is a linear scan of a 513 MB file of 46-byte records —
//! 28 ms over the reference database on one thread, 8 ms warm on ten. The way
//! to make it cheap is to push the work down: a name resolves to one entity id
//! once, by binary search of the namebase's sorted tree ([`Entities::find_player`]
//! and friends), and the scan then compares that id per record instead of
//! resolving four names per record. [`Filter`] is that resolved form, and
//! [`Database::scan`] runs it in parallel, in ascending game order, without
//! ever opening the moves file.
//!
//! A **position search** is not a scan: 883 million positions. It is answered
//! either by the consumer's own index, fed by the keys a keyed conversion
//! emits, or by replaying the source and testing positions as they go — which
//! is [`Database::for_each_position_key`], with progress and a cancel that
//! takes effect within one chunk.

use cbvault_format::cbh::Entity;
#[cfg(test)]
use cbvault_format::cbh::GameHeader;
use cbvault_format::error::{Error, Result};
use cbvault_format::game::Head;
use cbvault_format::game::{Eco, GameResult, RecordKind};
use rayon::prelude::*;

use super::namebase::Name;
use super::sink::GameHeaderRef;
use super::{Database, walk::Flow};
use cbvault_chess::decode::{GameRef as Walked, MoveSink, TreeStats, start_as_played, walk_from};
use cbvault_chess::start::{Start, StartCache, start_board_cached};
use gigachess::Board;

/// Games where either player is one of `ids`.
///
/// The companion to [`Entities::find_players`](crate::bridge::Entities::find_players):
/// that returns every id a surname names, and this turns the set into one filter,
/// so a common surname gets all its players' games rather than one player's.
///
/// Games where either player is one of `ids`.
///
/// The companion to [`Entities::find_players`](crate::bridge::Entities::find_players):
/// that returns every id a surname names, and this turns the set into one filter,
/// so a common surname gets all its players' games rather than one player's.
///
/// Evaluated via binary search over an [`IdSet`] in $O(\log N)$ time with no
/// heap allocations or recursive stack frames during header matching. An
/// **empty** `ids` matches nothing, not everything.
pub fn any_player(ids: &[u32]) -> Filter {
    Filter::PlayerSet(IdSet::from_ids(ids.iter().copied()))
}

/// A conjunction of criteria, built once and reused.
///
/// This is the piece that makes "white player AND Elo range" one question rather
/// than two, and it is a fold so a caller adds criteria in whatever order its own
/// criteria type happens to carry them:
///
/// ```ignore
/// let filter = AllOf::new()
///     .and(Filter::WhiteEloBetween(Range::at_least(2800)))
///     .and(Filter::YearBetween(Range::new(2000, 2010)))
///     .and(Filter::player(white_id))
///     .filter();
/// ```
///
/// An empty conjunction is `Filter::All`, not a filter that matches nothing —
/// "no criteria" means "every game", and a builder that returned an empty
/// conjunction as unsatisfiable would turn a user clearing the search form into a
/// zero-result query.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllOf(Filter);

impl AllOf {
    /// An empty conjunction.
    pub fn new() -> AllOf {
        AllOf(Filter::All)
    }

    /// Adds one criterion.
    #[must_use]
    pub fn and(mut self, filter: Filter) -> AllOf {
        self.0 = match std::mem::take(&mut self.0) {
            Filter::All => filter,
            existing => Filter::AllOf(Box::new(existing), Box::new(filter)),
        };
        self
    }

    /// Adds a criterion only if it is present, for a consumer whose criteria are
    /// all optional.
    #[must_use]
    pub fn and_opt(self, filter: Option<Filter>) -> AllOf {
        match filter {
            Some(f) => self.and(f),
            None => self,
        }
    }

    /// The conjunction, ready for [`scan`].
    pub fn filter(self) -> Filter {
        self.0
    }
}

impl From<AllOf> for Filter {
    fn from(all: AllOf) -> Filter {
        all.0
    }
}

/// A set of game ids, for "these games and no others".
///
/// A sorted vector with a binary search, so a filter over it costs a
/// `partition_point` per record and no allocation per lookup. Built once from
/// whatever the caller had — another database's answer, a selection, a previous
/// scan — and then reused across every thread of a scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdSet {
    ids: Vec<u32>,
}

impl IdSet {
    /// An empty set.
    pub fn new() -> IdSet {
        IdSet::default()
    }

    /// A set of `ids`, sorted and deduplicated.
    pub fn from_ids(ids: impl IntoIterator<Item = u32>) -> IdSet {
        let mut ids: Vec<u32> = ids.into_iter().collect();
        ids.sort_unstable();
        ids.dedup();
        IdSet { ids }
    }

    /// How many ids the set holds.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Whether `id` is in the set.
    pub fn contains(&self, id: u32) -> bool {
        self.ids.binary_search(&id).is_ok()
    }
}

impl FromIterator<u32> for IdSet {
    fn from_iter<T: IntoIterator<Item = u32>>(ids: T) -> IdSet {
        IdSet::from_ids(ids)
    }
}

/// A closed numeric range, either end of which may be open.
///
/// One type rather than a pair of `Option<i32>`s in every variant, because the
/// semantics are worth naming once: `None` is an **open** end, not a zero bound.
/// A consumer that sends "rating from 2800" sends `Some(2800), None` and must not
/// have it silently become "0 to 2800" — which would return every unrated game in
/// the database.
///
/// This is also what lets a threshold be a special case rather than a second
/// variant: [`Range::at_least`] is `Some(min), None`, so
/// [`Filter::WhiteEloAtLeast`] and [`Filter::WhiteEloBetween`] answer the same
/// question through one comparison and one code path to test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range {
    /// The inclusive lower bound; `None` is unbounded below.
    pub from: Option<i32>,
    /// The inclusive upper bound; `None` is unbounded above.
    pub to: Option<i32>,
}

impl Range {
    /// A range with both ends closed.
    pub fn new(from: i32, to: i32) -> Range {
        Range { from: Some(from), to: Some(to) }
    }

    /// A range unbounded above, which is what "at least `min`" means.
    pub fn at_least(min: i32) -> Range {
        Range { from: Some(min), to: None }
    }

    /// A range unbounded below.
    pub fn at_most(max: i32) -> Range {
        Range { from: None, to: Some(max) }
    }

    /// Whether `value` is within the range.
    ///
    /// An inverted range — `from` above `to` — matches nothing rather than
    /// everything. Both bounds cannot be `None` in practice (that would be
    /// `Filter::All` in a slower form), and an unbounded value is tested against
    /// the bounds that exist.
    #[inline]
    pub fn contains(&self, value: i32) -> bool {
        if let Some(from) = self.from
            && value < from
        {
            return false;
        }
        if let Some(to) = self.to
            && value > to
        {
            return false;
        }
        self.from.is_some() || self.to.is_some()
    }
}

/// The one search-only header field not present in the shared Head trait.
pub trait SearchHeader: Head {
    /// The source entity ID of a game.
    fn source_id(&self) -> i64;
}

impl SearchHeader for cbvault_format::cbh::GameHeader {
    fn source_id(&self) -> i64 {
        i64::from(self.source())
    }
}

impl SearchHeader for GameHeaderRef<'_> {
    fn source_id(&self) -> i64 {
        self.source()
    }
}

/// A predicate over one header record, in its resolved form.
///
/// Every variant is a comparison of a field the record already holds, so the
/// scan costs one pass over `.cbh` and nothing else. The constructors take the
/// *ids* a name resolved to, which is the whole point: [`Filter::player`] is
/// called once per query with the id [`crate::bridge::Entities::find_player`] answered, and
/// then the scan compares one integer per record instead of resolving four
/// names per record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    /// Every game.
    #[default]
    All,
    /// Games whose white player's rating is at least this.
    WhiteEloAtLeast(i32),
    /// Games whose black player's rating is at least this.
    BlackEloAtLeast(i32),
    /// Games where either rating is at least this.
    EloAtLeast(i32),
    /// Games whose white player's rating is in the range. `None` is an open end.
    WhiteEloBetween(Range),
    /// Games whose black player's rating is in the range. `None` is an open end.
    BlackEloBetween(Range),
    /// Games where *either* rating is in the range. `None` is an open end.
    EloBetween(Range),
    /// Games played within the range, by year. `None` is an open end.
    ///
    /// Compared on the year alone. The header's date is a packed day/month/year
    /// and a month/day that a consumer would compare against is almost always a
    /// whole-year range; a caller needing exact dates can read [`crate::bridge::GameHeaderRef`]
    /// itself. A game with no date (year 0) is never in range, because
    /// "unknown" is not "in the range the user asked for".
    YearBetween(Range),
    /// Games with this result.
    Result(GameResult),
    /// Games in this round number, or this sub-round when `sub` is `Some`.
    ///
    /// `sub` of `None` is "any sub-round" rather than an open range — the field
    /// is one byte and is not numeric in the way ratings are, so a half-open
    /// range over it would be a guess.
    Round {
        /// The round number, as the header stores it.
        round: u8,
        /// The sub-round, when the caller distinguishes one.
        sub: Option<u8>,
    },
    /// Games whose ECO code is in `0..=499` (A00–E99) with this sub-code.
    ///
    /// `sub` of `None` matches every sub-code of the opening, which is what
    /// "all of B20" means. A game with no code, a Chess960 start or an
    /// unrecognised value does not match.
    Eco {
        /// The opening code, `0..=499` for A00–E99.
        code: u16,
        /// The sub-code within the opening, when the caller distinguishes one.
        sub: Option<u8>,
    },
    // There is deliberately no "starts from a set-up position" filter: the
    // classic header does not record it — the bit is in the move record's
    // flags — and a tag search over a raw database does not open the moves
    // file, so the question cannot be answered without breaking that. A
    // consumer that needs it has the position keys and can test them.
    /// Games whose white player is this entity id.
    Player(u32),
    /// Games whose black player is this entity id.
    Opponent(u32),
    /// Games where either player is this entity id.
    Either(u32),
    /// Games where either player is one of these entity ids.
    PlayerSet(IdSet),
    /// Games whose white player is one of these entity ids.
    WhiteSet(IdSet),
    /// Games whose black player is one of these entity ids.
    BlackSet(IdSet),
    /// Games in this tournament's entity id.
    Tournament(u32),
    /// Games whose annotator is this entity id.
    Annotator(u32),
    /// Games whose source is this entity id.
    Source(u32),
    /// Games in this set of ids, and no others.
    Ids(IdSet),
    /// Games that satisfy both.
    AllOf(Box<Filter>, Box<Filter>),
    /// Games that satisfy either.
    AnyOf(Box<Filter>, Box<Filter>),
    /// Games that do not satisfy this.
    Not(Box<Filter>),
}

impl Filter {
    /// Games where the named player is white or black. Resolve the name once
    /// with [`crate::bridge::Entities::find_player`] and build the filter from the id.
    pub fn player(id: u32) -> Filter {
        Filter::Either(id)
    }

    /// Games where either player is one of `ids`.
    pub fn player_set(ids: IdSet) -> Filter {
        Filter::PlayerSet(ids)
    }

    /// Games whose white player is one of `ids`.
    pub fn white_set(ids: IdSet) -> Filter {
        Filter::WhiteSet(ids)
    }

    /// Games whose black player is one of `ids`.
    pub fn black_set(ids: IdSet) -> Filter {
        Filter::BlackSet(ids)
    }

    /// Games whose ECO code is the `code` part of a text like `"B20"`, with every
    /// sub-code matching.
    ///
    /// The text form is what every consumer's UI typically holds (e.g. passing `"B20"`)
    /// and turning it into a predicate here means no consumer writes a second conversion
    /// of the same three characters. Case-insensitive, and tolerant of surrounding space. `None` for text that
    /// is not an ECO code, so a caller can report the typo instead of building a
    /// filter that matches nothing and looking like an empty database.
    pub fn eco_text(text: &str) -> Option<Filter> {
        let (code, sub) = parse_eco(text)?;
        Some(Filter::Eco { code, sub })
    }

    /// Both predicates must hold.
    pub fn and(self, other: Filter) -> Filter {
        Filter::AllOf(Box::new(self), Box::new(other))
    }

    /// Either predicate may hold.
    pub fn or(self, other: Filter) -> Filter {
        Filter::AnyOf(Box::new(self), Box::new(other))
    }

    /// The predicate must not hold.
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Filter {
        Filter::Not(Box::new(self))
    }

    /// Applies a resolved header filter uniformly to classic and 2CBH.
    /// IDs are compared as i64 because v2 entity identifiers are 64-bit.
    pub fn matches<H: SearchHeader>(&self, id: u32, header: &H) -> bool {
        let (welo, belo) = header.elo();
        let (round, subround) = header.round();
        let white = header.white();
        let black = header.black();
        let id_eq = |stored: i64, expected: u32| stored == i64::from(expected);
        let id_in = |stored: i64, ids: &IdSet| u32::try_from(stored).is_ok_and(|value| ids.contains(value));
        match self {
            Filter::All => true,
            Filter::WhiteEloAtLeast(min) => welo >= *min,
            Filter::BlackEloAtLeast(min) => belo >= *min,
            Filter::EloAtLeast(min) => welo >= *min || belo >= *min,
            Filter::WhiteEloBetween(range) => range.contains(welo),
            Filter::BlackEloBetween(range) => range.contains(belo),
            Filter::EloBetween(range) => range.contains(welo) || range.contains(belo),
            Filter::YearBetween(range) => range.contains(i32::from(header.played_date().year())),
            Filter::Result(want) => header.result() == *want,
            Filter::Round { round: want, sub } => {
                round == i32::from(*want) && sub.is_none_or(|s| subround == i32::from(s))
            }
            Filter::Eco { code, sub } => match header.eco() {
                Eco::Code { code: c, sub: s } => c == *code && sub.is_none_or(|w| s == w),
                _ => false,
            },
            Filter::Player(want) => id_eq(white, *want),
            Filter::Opponent(want) => id_eq(black, *want),
            Filter::Either(want) => id_eq(white, *want) || id_eq(black, *want),
            Filter::PlayerSet(ids) => id_in(white, ids) || id_in(black, ids),
            Filter::WhiteSet(ids) => id_in(white, ids),
            Filter::BlackSet(ids) => id_in(black, ids),
            Filter::Tournament(want) => id_eq(header.tournament(), *want),
            Filter::Annotator(want) => id_eq(header.annotator(), *want),
            Filter::Source(want) => id_eq(header.source_id(), *want),
            Filter::Ids(ids) => ids.contains(id),
            Filter::AllOf(a, b) => a.matches(id, header) && b.matches(id, header),
            Filter::AnyOf(a, b) => a.matches(id, header) || b.matches(id, header),
            Filter::Not(inner) => !inner.matches(id, header),
        }
    }
}

/// A text ECO code as the header stores it: the `0..=499` code and the sub-code.
///
/// `"B20"` is code 120, sub 0 — the code is the letter's hundred plus the number
/// after it. A bare letter — `"B"` — is code 100 with no sub-code, which is what
/// "all of the B openings" means. Anything else is `None`.
fn parse_eco(text: &str) -> Option<(u16, Option<u8>)> {
    let text = text.trim().as_bytes();
    let letter = u16::from(match text.first()? {
        b'A'..=b'E' => text[0] - b'A',
        b'a'..=b'e' => text[0] - b'a',
        _ => return None,
    });
    let digits = &text[1..];
    // A bare letter is legal, so the empty case is not rejected here; only a
    // fourth digit or a non-digit is.
    if digits.len() > 3 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let number: u16 = std::str::from_utf8(digits).ok()?.parse().unwrap_or(0);
    // The code is the letter's hundred plus the number written after it, read as
    // the digits *are* the code's last two places — "A01" is code 1 and "B20"
    // is code 120. Reading the whole remainder as one number is what makes that
    // work, and it is why this cannot be split on length the way it looks like
    // it should: "A1" and "A01" are the same code, and only one of them is what
    // a PGN writes.
    let code = letter * 100 + number;
    if code > 499 {
        return None;
    }
    match digits.len() {
        // A bare letter is the whole hundred: every sub-code matches.
        0 => Some((code, None)),
        // Two digits is the code with sub-code 0 — the PGN's own form.
        2 => Some((code, Some(0))),
        // Three digits carry the sub-code in the last place.
        3 => Some((code, Some(digits[2] - b'0'))),
        _ => None,
    }
}

/// An entity name from either database generation. Classic names remain
/// borrowed; 2CBH .2lid names are decoded on demand for matched rows only,
/// borrowing valid mapped UTF-8 where possible.
#[derive(Clone, Debug)]
pub enum SearchName<'a> {
    /// Borrowed classic namebase record.
    Classic(Name<'a>),
    /// The two native v2 UTF-8 entity fields.
    TwoCbh { last: String, first: String },
    /// Valid UTF-8 fields borrowed from the mapped native entity file.
    TwoCbhBorrowed { last: &'a str, first: &'a str },
}

impl PartialEq for SearchName<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Classic(a), Self::Classic(b)) => a == b,
            (Self::Classic(_), _) | (_, Self::Classic(_)) => false,
            _ => self.last() == other.last() && self.first() == other.first(),
        }
    }
}

impl Eq for SearchName<'_> {}

impl<'a> SearchName<'a> {
    fn from_native(last: std::borrow::Cow<'a, str>, first: std::borrow::Cow<'a, str>) -> Self {
        match (last, first) {
            (std::borrow::Cow::Borrowed(last), std::borrow::Cow::Borrowed(first)) => {
                Self::TwoCbhBorrowed { last, first }
            }
            (last, first) => Self::TwoCbh { last: last.into_owned(), first: first.into_owned() },
        }
    }

    /// First field, usually a surname or tournament title.
    pub fn last(&self) -> &[u8] {
        match self {
            Self::Classic(name) => name.last(),
            Self::TwoCbh { last, .. } => last.as_bytes(),
            Self::TwoCbhBorrowed { last, .. } => last.as_bytes(),
        }
    }

    /// Second field, usually the player forename.
    pub fn first(&self) -> &[u8] {
        match self {
            Self::Classic(name) => name.first(),
            Self::TwoCbh { first, .. } => first.as_bytes(),
            Self::TwoCbhBorrowed { first, .. } => first.as_bytes(),
        }
    }

    /// Whether both name fields are empty.
    pub fn is_empty(&self) -> bool {
        self.last().is_empty() && self.first().is_empty()
    }

    /// An already UTF-8 single-field name, or None for multi-field names.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Classic(name) => name.as_str(),
            Self::TwoCbh { last, first } if first.is_empty() => Some(last),
            Self::TwoCbhBorrowed { last, first: "" } => Some(last),
            Self::TwoCbh { .. } | Self::TwoCbhBorrowed { .. } => None,
        }
    }

    /// Write the displayed name as Last, First, preserving native Unicode.
    pub fn push_text(&self, out: &mut String) {
        match self {
            Self::Classic(name) => name.push_text(out),
            Self::TwoCbh { last, first } => push_native(out, last, first),
            Self::TwoCbhBorrowed { last, first } => push_native(out, last, first),
        }
    }
}

fn push_native(out: &mut String, last: &str, first: &str) {
    out.push_str(last);
    if !first.is_empty() {
        if !last.is_empty() {
            out.push_str(", ");
        }
        out.push_str(first);
    }
}

#[cfg(test)]
#[test]
fn native_search_names_compare_by_content_across_storage_modes() {
    let a = SearchName::TwoCbh { last: "Юрьев".into(), first: "Éric".into() };
    let b = SearchName::TwoCbhBorrowed { last: "Юрьев", first: "Éric" };
    assert_eq!(a, b);
    let mut text = String::new();
    b.push_text(&mut text);
    assert_eq!(text, "Юрьев, Éric");
}

/// One game a scan matched, with generation-aware header and names.
#[derive(Clone)]
pub struct Match<'a> {
    /// Stable 1-based record ID.
    pub id: u32,
    /// Header from the correct ChessBase generation.
    pub header: GameHeaderRef<'a>,
    /// White player name, classic borrowed or native v2 decoded.
    pub white: SearchName<'a>,
    /// Black player name.
    pub black: SearchName<'a>,
    /// Tournament title.
    pub event: SearchName<'a>,
}

/// Every match of a scan, in ascending game order, with the names each one
/// needs borrowed from the mapped namebase.
///
/// The names are [`Name`]s, not decoded strings: a scan returns slices of the
/// mapped entity data and allocates no per-game string, which is what lets a
/// 100,000-game result cost a few hundred kilobytes rather than a few
/// megabytes. [`Match::player`] joins the two fields of a player into one
/// borrowed string when a caller wants it as text.
#[derive(Debug, Default)]
pub struct Scan<'a> {
    matches: Vec<Match<'a>>,
    records: u64,
    threads: usize,
}

impl<'a> Scan<'a> {
    /// The matches, in ascending game order.
    pub fn matches(&self) -> &[Match<'a>] {
        &self.matches
    }

    /// How many matches there are.
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    /// Whether nothing matched.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Into the matches, without the report.
    pub fn into_matches(self) -> Vec<Match<'a>> {
        self.matches
    }

    /// The matching game numbers, for a caller that already knows the names.
    pub fn ids(&self) -> Vec<u32> {
        self.matches.iter().map(|m| m.id).collect()
    }

    /// What the scan read.
    pub fn stats(&self) -> SearchStats {
        SearchStats { records: self.records, matches: self.matches.len() as u64, threads: self.threads }
    }
}

/// Name printing shared by classic borrowed and v2 decoded search fields.
pub trait DisplaySearchName {
    /// Whether the displayed entity name is blank.
    fn is_empty(&self) -> bool;
    /// Append its correct Last, First display representation.
    fn push_text(&self, out: &mut String);
}

impl DisplaySearchName for Name<'_> {
    fn is_empty(&self) -> bool {
        Name::is_empty(self)
    }
    fn push_text(&self, out: &mut String) {
        Name::push_text(self, out);
    }
}

impl DisplaySearchName for SearchName<'_> {
    fn is_empty(&self) -> bool {
        SearchName::is_empty(self)
    }
    fn push_text(&self, out: &mut String) {
        SearchName::push_text(self, out);
    }
}

impl Match<'_> {
    /// The player's name written into out as Last, First, for either format.
    pub fn player(name: &impl DisplaySearchName, out: &mut String) -> bool {
        if name.is_empty() {
            return false;
        }
        name.push_text(out);
        true
    }
}

/// What a scan or a position search read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchStats {
    /// Records read.
    pub records: u64,
    /// Games that matched.
    pub matches: u64,
    /// Workers the scan ran on; 1 for a sequential scan.
    pub threads: usize,
}

/// How many records one scan step reads: 8,192, the batch every other path uses.
const SCAN_BATCH: u32 = 8192;

/// Reusable, explicitly bounded worker pool for repeated database queries.
/// One context can be shared across databases; it retains no database data.
pub struct SearchContext {
    threads: usize,
    pool: Option<rayon::ThreadPool>,
}

impl SearchContext {
    /// Construct a context once. Zero and one worker execute synchronously.
    pub fn new(threads: usize) -> std::result::Result<Self, rayon::ThreadPoolBuildError> {
        let threads = threads.max(1);
        let pool = if threads > 1 { Some(rayon::ThreadPoolBuilder::new().num_threads(threads).build()?) } else { None };
        Ok(Self { threads, pool })
    }

    /// Worker budget requested at construction.
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// Scan game IDs over the inclusive range, without creating a new pool.
    pub fn scan_ids_range(&self, db: &Database, first: u32, last: u32, filter: &Filter) -> Result<IdScan> {
        scan_ids_range_impl(db, first, last, filter, self.threads, self.pool.as_ref())
    }

    /// Scan all game IDs without resolving names.
    pub fn scan_ids(&self, db: &Database, filter: &Filter) -> Result<IdScan> {
        self.scan_ids_range(db, 1, 0, filter)
    }

    /// Prepare a reusable paginated query using this worker pool.
    pub fn prepare_id_pages<'db>(&self, db: &'db Database, filter: &Filter) -> Result<IdPagePlan<'db>> {
        IdPagePlan::build(db, filter, self.threads, self.pool.as_ref())
    }

    /// Exact count and bounded page, reusing this worker pool.
    pub fn scan_id_page(&self, db: &Database, filter: &Filter, offset: u64, limit: usize) -> Result<IdPage> {
        scan_id_page_impl(db, filter, offset, limit, self.threads, self.pool.as_ref())
    }

    /// Materialized metadata results, over an inclusive range.
    pub fn scan_range<'db>(&self, db: &'db Database, first: u32, last: u32, filter: &Filter) -> Result<Scan<'db>> {
        scan_ranges(db, &ranges_of(db, first, last), filter, self.threads, self.pool.as_ref())
    }

    /// Replay main lines for a position, using the same bounded worker pool.
    pub fn for_each_position_key(
        &self,
        db: &Database,
        query: &PositionQuery<'_>,
        mut on_hit: impl FnMut(Hit) + Send,
    ) -> Result<PositionSearch> {
        replay_ranges(db, &ranges_of(db, 1, 0), query, self.threads, &mut on_hit, self.pool.as_ref())
    }
}

fn collect_chunks<T: Send>(
    db: &Database,
    ranges: &[(u32, u32)],
    threads: usize,
    pool: Option<&rayon::ThreadPool>,
    f: impl Fn(&(u32, u32)) -> Result<T> + Sync + Send,
) -> Result<Vec<T>> {
    if threads <= 1 || ranges.len() <= 1 {
        return ranges.iter().map(f).collect();
    }
    if let Some(pool) = pool {
        return pool.install(|| ranges.par_iter().map(f).collect());
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| Error::corrupt(db.base(), 0, format!("thread pool creation failed: {e}")))?;
    pool.install(|| ranges.par_iter().map(f).collect())
}

/// Scans the header records of the whole database for the games `filter`
/// matches, across `threads` workers, in ascending game order.
///
/// The scan reads `.cbh` and the namebases and nothing else — the moves file is
/// not opened, which is the point of a tag search over a raw database — and it
/// evaluates the predicate per record, so a name predicate costs one integer
/// comparison rather than four name resolutions.
///
/// The result is the same set in the same order at any thread count: each
/// worker collects the ids of the records it was given, and the writer stage
/// concatenates the chunks in id order. `threads` of 0 or 1 scans on the
/// calling thread.
pub fn scan<'db>(db: &'db Database, filter: &Filter, threads: usize) -> Result<Scan<'db>> {
    let ranges = ranges_of(db, 1, 0);
    scan_ranges(db, &ranges, filter, threads, None)
}

/// [`scan`] over the game numbers `first..=last`, so a caller can narrow the
/// work to a range it already knows is interesting. `last` of 0 means the end
/// of the database.
pub fn scan_range<'db>(db: &'db Database, first: u32, last: u32, filter: &Filter, threads: usize) -> Result<Scan<'db>> {
    scan_ranges(db, &ranges_of(db, first, last), filter, threads, None)
}

/// Compact result of a metadata search that needs only matching game IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdScan {
    ids: Vec<u32>,
    stats: SearchStats,
}

impl IdScan {
    /// Matching IDs, in ascending source record order.
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
    /// Take ownership of the IDs without copying them.
    pub fn into_ids(self) -> Vec<u32> {
        self.ids
    }
    /// Number of scanned records, matches and requested worker count.
    pub fn stats(&self) -> SearchStats {
        self.stats
    }
}

/// Scan metadata without resolving entities or retaining full game headers.
/// A corrupt entity not needed by the predicate does not affect an ID scan;
/// header errors are still returned. Moves and annotations are never read.
pub fn scan_ids(db: &Database, filter: &Filter, threads: usize) -> Result<IdScan> {
    scan_ids_range(db, 1, 0, filter, threads)
}

/// ID-only scan over the inclusive range; zero last means end of database.
pub fn scan_ids_range(db: &Database, first: u32, last: u32, filter: &Filter, threads: usize) -> Result<IdScan> {
    scan_ids_range_impl(db, first, last, filter, threads, None)
}

fn scan_ids_range_impl(
    db: &Database,
    first: u32,
    last: u32,
    filter: &Filter,
    threads: usize,
    pool: Option<&rayon::ThreadPool>,
) -> Result<IdScan> {
    let ranges = ranges_of(db, first, last);
    let scan_chunk = |&(first, last): &(u32, u32)| -> Result<Vec<u32>> {
        let mut out = Vec::new();
        visit_matching_ids(db, first, last, filter, |id| out.push(id))?;
        Ok(out)
    };
    let found: Vec<Vec<u32>> = collect_chunks(db, &ranges, threads, pool, scan_chunk)?;
    let records = ranges.iter().map(|(first, last)| u64::from(last - first + 1)).sum();
    let ids = found.into_iter().flatten().collect::<Vec<_>>();
    let stats = SearchStats { records, matches: ids.len() as u64, threads: threads.max(1) };
    Ok(IdScan { ids, stats })
}

// Shared visitor: counting does not allocate a vector of matching IDs.
fn visit_matching_ids(db: &Database, first: u32, last: u32, filter: &Filter, mut visit: impl FnMut(u32)) -> Result<()> {
    match db.header_source() {
        super::HeaderSource::Classic(_) => {
            for header in db.record_batch(first, last)? {
                if header.kind() == RecordKind::Game && !header.is_deleted() && filter.matches(header.id(), &header) {
                    visit(header.id());
                }
            }
        }
        super::HeaderSource::TwoCbh(_) => {
            let mut scratch = Vec::new();
            let window = db.v2_header_window(first, last, &mut scratch)?;
            for id in first..=last {
                let header = GameHeaderRef::TwoCbh(window.get(id).ok_or(Error::NoSuchGame { id })?);
                if header.kind() == RecordKind::Game && !header.is_deleted() && filter.matches(id, &header) {
                    visit(id);
                }
            }
        }
    }
    Ok(())
}

/// A bounded page of matching IDs and the exact total count. No names or
/// moves are resolved. The source must remain immutable during the two passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdPage {
    ids: Vec<u32>,
    offset: u64,
    stats: SearchStats,
}

impl IdPage {
    /// IDs in source order, starting at the zero-based matching-result offset.
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
    /// Offset requested, even if it is beyond the end of the results.
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// Statistics for the complete filter, not merely this page.
    pub fn stats(&self) -> SearchStats {
        self.stats
    }
}

/// Scan the entire database for the exact match count and one page of IDs.
/// The first pass counts matches per bounded header chunk; only the chunks
/// overlapping the page are scanned a second time. Memory is proportional
/// to the page size and the number of chunks, not the total match count.
pub fn scan_id_page(db: &Database, filter: &Filter, offset: u64, limit: usize, threads: usize) -> Result<IdPage> {
    scan_id_page_impl(db, filter, offset, limit, threads, None)
}

fn scan_id_page_impl(
    db: &Database,
    filter: &Filter,
    offset: u64,
    limit: usize,
    threads: usize,
    pool: Option<&rayon::ThreadPool>,
) -> Result<IdPage> {
    IdPagePlan::build(db, filter, threads, pool)?.page(offset, limit)
}

/// A reusable exact-count query over an immutable database. Retains one
/// cumulative count per header chunk, not one ID per match. Subsequent pages
/// seek directly to their first matching chunk. The source must not change
/// for the lifetime of this plan; reopen and rebuild after database changes.
pub struct IdPagePlan<'db> {
    db: &'db Database,
    filter: Filter,
    ranges: Vec<(u32, u32)>,
    ends: Vec<u64>,
    stats: SearchStats,
}

impl<'db> IdPagePlan<'db> {
    /// Prepare once; the plan can outlive the temporary worker pool.
    pub fn new(db: &'db Database, filter: &Filter, threads: usize) -> Result<Self> {
        Self::build(db, filter, threads, None)
    }

    fn build(db: &'db Database, filter: &Filter, threads: usize, pool: Option<&rayon::ThreadPool>) -> Result<Self> {
        let ranges = ranges_of(db, 1, 0);
        let mut ends = collect_chunks(db, &ranges, threads, pool, |&(first, last)| {
            let mut count = 0u64;
            visit_matching_ids(db, first, last, filter, |_| count += 1)?;
            Ok(count)
        })?;
        let mut matches = 0;
        for count in &mut ends {
            matches += *count;
            *count = matches;
        }
        let stats = SearchStats {
            records: ranges.iter().map(|(a, b)| u64::from(b - a + 1)).sum(),
            matches,
            threads: threads.max(1),
        };
        Ok(Self { db, filter: filter.clone(), ranges, ends, stats })
    }

    /// Exact counts from the preparation scan.
    pub fn stats(&self) -> SearchStats {
        self.stats
    }

    /// Return a page without recounting the database.
    pub fn page(&self, offset: u64, limit: usize) -> Result<IdPage> {
        let end = offset.saturating_add(limit as u64).min(self.stats.matches);
        let expected = end.saturating_sub(offset);
        let mut ids = Vec::with_capacity(expected as usize);
        if expected > 0 {
            let first_chunk = self.ends.partition_point(|&n| n <= offset);
            let mut prefix = if first_chunk == 0 { 0 } else { self.ends[first_chunk - 1] };
            for (&(first, last), &chunk_end) in self.ranges[first_chunk..].iter().zip(&self.ends[first_chunk..]) {
                if prefix >= end {
                    break;
                }
                if chunk_end > prefix {
                    let mut rank = prefix;
                    visit_matching_ids(self.db, first, last, &self.filter, |id| {
                        if rank >= offset && rank < end {
                            ids.push(id);
                        }
                        rank += 1;
                    })?;
                    if rank != chunk_end {
                        return Err(Error::corrupt(
                            self.db.base(),
                            0,
                            "header results changed since page plan preparation",
                        ));
                    }
                }
                prefix = chunk_end;
            }
        }
        if ids.len() as u64 != expected {
            return Err(Error::corrupt(self.db.base(), 0, "incomplete ID page"));
        }
        Ok(IdPage { ids, offset, stats: self.stats })
    }
}

/// The chunk boundaries of `first..=last`, or of the whole database.
fn ranges_of(db: &Database, first: u32, last: u32) -> Vec<(u32, u32)> {
    let last = if last == 0 { db.records() } else { last.min(db.records()) };
    let mut out = Vec::new();
    let mut at = first.max(1);
    while at <= last {
        let end = at.saturating_add(SCAN_BATCH - 1).min(last);
        out.push((at, end));
        if end == last {
            break;
        }
        at = end + 1;
    }
    out
}

/// The scan itself, over `ranges`.
fn scan_ranges<'db>(
    db: &'db Database,
    ranges: &[(u32, u32)],
    filter: &Filter,
    threads: usize,
    pool: Option<&rayon::ThreadPool>,
) -> Result<Scan<'db>> {
    let classic_entities = if db.generation() == super::Generation::Classic { Some(db.entities()) } else { None };
    let v2_entities = db.twocbh_entities();
    let scan_chunk = |&(first, last): &(u32, u32)| -> Result<Vec<Match<'db>>> {
        let mut out = Vec::new();
        if let Some(entities) = v2_entities {
            // Native 2CBH headers are 192 bytes and .2lid entities are
            // interleaved. Never reinterpret them as classic 46-byte records.
            let mut scratch = Vec::new();
            let window = db.v2_header_window(first, last, &mut scratch)?;
            for id in first..=last {
                let header = GameHeaderRef::TwoCbh(window.get(id).ok_or(Error::NoSuchGame { id })?);
                if !matches!(header.kind(), RecordKind::Game) || header.is_deleted() {
                    continue;
                }
                if !filter.matches(id, &header) {
                    continue;
                }
                let (wl, wf) = entities.player_name(header.white())?.unwrap_or_default();
                let (bl, bf) = entities.player_name(header.black())?.unwrap_or_default();
                let (event, _) = entities.tournament_name(header.tournament())?.unwrap_or_default();
                out.push(Match {
                    id,
                    header,
                    white: SearchName::from_native(wl, wf),
                    black: SearchName::from_native(bl, bf),
                    event: SearchName::from_native(event, std::borrow::Cow::Borrowed("")),
                });
            }
        } else if let Some(entities) = classic_entities {
            let mut batch = db.record_batch(first, last)?;
            for header in batch.by_ref() {
                if !matches!(header.kind(), RecordKind::Game) || header.is_deleted() {
                    continue;
                }
                if !filter.matches(header.id(), &header) {
                    continue;
                }
                out.push(Match {
                    id: header.id(),
                    header: GameHeaderRef::ClassicOwned(header),
                    white: SearchName::Classic(entities.name(Entity::Player, header.white())?.unwrap_or(BLANK)),
                    black: SearchName::Classic(entities.name(Entity::Player, header.black())?.unwrap_or(BLANK)),
                    event: SearchName::Classic(Name::of(
                        entities.name(Entity::Tournament, header.tournament())?.unwrap_or(BLANK).last(),
                    )),
                });
            }
        }
        Ok(out)
    };

    let found: Vec<Vec<Match<'db>>> = collect_chunks(db, ranges, threads, pool, scan_chunk)?;

    let records: u64 = ranges.iter().map(|(first, last)| u64::from(last - first + 1)).sum();
    let matches = found.into_iter().flatten().collect::<Vec<_>>();
    Ok(Scan { matches, records, threads: if threads <= 1 { 1 } else { threads } })
}

/// The name of an entity the database does not have: empty, never `None`.
const BLANK: Name<'static> = Name::blank();

impl std::fmt::Debug for Match<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Match")
            .field("id", &self.id)
            .field("white", &String::from_utf8_lossy(self.white.last()))
            .field("black", &String::from_utf8_lossy(self.black.last()))
            .field("event", &String::from_utf8_lossy(self.event.last()))
            .finish()
    }
}

/// One position a replay found, in the game it was found in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The game's number in `.cbh`.
    pub game: u32,
    /// The ply, counted from 0 at the start position.
    pub ply: u32,
    /// The Polyglot key of the position.
    pub key: u64,
}

/// What an unindexed replay read, and whether it finished.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PositionSearch {
    /// Games replayed, including speculative work in the final bounded wave.
    pub games: u64,
    /// Main-line plies played.
    pub plies: u64,
    /// Positions reported, one per hit.
    pub hits: u64,
    /// Games whose walk failed.
    pub failures: u64,
    /// Whether the replay ran to the end. `false` after a cancel: the hits
    /// gathered so far are a prefix of the answer, never a complete one, and
    /// this is what says so.
    pub complete: bool,
}

/// What a caller wants from an unindexed position replay.
#[derive(Clone, Copy)]
pub struct PositionQuery<'p> {
    /// The position to look for, as a Polyglot key. One key is one position;
    /// run the replay once per key, or test several keys with repeated replays.
    pub key: u64,
    /// How often the replay reports progress, in games; 0 for never.
    pub every: u64,
    /// Polled before and during replay batches and before hit delivery. A true
    /// answer stops delivery and prevents another wave.
    pub cancelled: Option<&'p (dyn Fn() -> bool + Sync)>,
    /// Early-stop predicate evaluated per move during replay.
    pub early_stop: Option<&'p (dyn Fn(&gigachess::Board) -> super::walk::Flow + Sync)>,
}

impl std::fmt::Debug for PositionQuery<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PositionQuery")
            .field("key", &format_args!("{:#018x}", self.key))
            .field("every", &self.every)
            .field("cancelled", &self.cancelled.is_some())
            .field("early_stop", &self.early_stop.is_some())
            .finish()
    }
}

impl Default for PositionQuery<'_> {
    /// A query for a key of 0, with no progress and no cancel: a fresh query
    /// the caller is expected to fill in.
    fn default() -> PositionQuery<'static> {
        PositionQuery { key: 0, every: 0, cancelled: None, early_stop: None }
    }
}

impl<'p> PositionQuery<'p> {
    /// A query for one key, with no progress and no way to cancel.
    pub fn of(key: u64) -> PositionQuery<'static> {
        PositionQuery { key, every: 0, cancelled: None, early_stop: None }
    }

    /// Reports progress every `every` games.
    pub fn every(mut self, every: u64) -> PositionQuery<'p> {
        self.every = every;
        self
    }

    /// Stops replay and ordered delivery when the predicate answers `true`.
    pub fn cancel_with(mut self, cancelled: &'p (dyn Fn() -> bool + Sync)) -> PositionQuery<'p> {
        self.cancelled = Some(cancelled);
        self
    }

    /// Early-stop predicate evaluated per move during replay.
    pub fn early_stop_with(
        mut self,
        early_stop: &'p (dyn Fn(&gigachess::Board) -> super::walk::Flow + Sync),
    ) -> PositionQuery<'p> {
        self.early_stop = Some(early_stop);
        self
    }
}

/// One worker's replay: the games it read, the positions it found, and whether
/// it got through its chunk.
#[derive(Debug, Default)]
struct Replay {
    stats: PositionSearch,
    hits: Vec<Hit>,
    /// The chunk stopped early because the search was cancelled.
    cancelled: bool,
}

// Position searches only need matching keys and a main-line ply count, not
// the entire decoded line or FEN text. Roll back provisional hits when any
// part of the game fails, exactly as the materialized path did.
struct PositionSink<'a> {
    query: &'a PositionQuery<'a>,
    hits: &'a mut Vec<Hit>,
    id: u32,
    ply: u32,
    main: bool,
    stopped: bool,
    stop_all: bool,
}

impl PositionSink<'_> {
    #[inline]
    fn position(&mut self, board: &Board) {
        let key = board.zobrist();
        if key == self.query.key {
            self.hits.push(Hit { game: self.id, ply: self.ply, key });
        }
        if let Some(pred) = self.query.early_stop {
            match pred(board) {
                Flow::Continue => {}
                Flow::SkipGame => self.stopped = true,
                Flow::Stop => {
                    self.stopped = true;
                    self.stop_all = true;
                }
            }
        }
    }
}

impl MoveSink for PositionSink<'_> {
    #[inline]
    fn play(&mut self, _: &Board, _: u16, main: bool) {
        self.main = main;
    }
    #[inline]
    fn played(&mut self, board: &Board) {
        if self.main {
            self.ply += 1;
            self.position(board);
        }
    }
    fn branch(&mut self) {}
    fn resume(&mut self) {}
    fn stopped(&self) -> bool {
        self.stopped
    }
    fn wants_zobrist(&self) -> bool {
        true
    }
}

impl Replay {
    fn one(
        &mut self,
        id: u32,
        query: &PositionQuery<'_>,
        cache: &mut StartCache,
        start: Result<Start>,
        walk: impl FnOnce(&Start, &mut PositionSink<'_>) -> Result<TreeStats>,
    ) -> bool {
        let Ok(start) = start else {
            self.stats.failures += 1;
            return true;
        };
        let Ok(board) = start_board_cached(&start, cache) else {
            self.stats.failures += 1;
            return true;
        };
        let prior = self.hits.len();
        let mut sink =
            PositionSink { query, hits: &mut self.hits, id, ply: 0, main: true, stopped: false, stop_all: false };
        sink.position(&board);
        if !sink.stopped && walk(&start, &mut sink).is_err() {
            self.hits.truncate(prior);
            self.stats.failures += 1;
            return true;
        }
        self.stats.games += 1;
        self.stats.plies += u64::from(sink.ply);
        !sink.stop_all
    }
}

/// Games a position-search worker replays between two cancel polls: a few
/// milliseconds of work, so a superseded explorer query is released at once.
const CANCEL_POLL_GAMES: u32 = 256;

/// Replays the database's main lines looking for one position, in parallel, and
/// reports where it is found.
///
/// This is the position search with no index: it reads the source and tests
/// every position it reaches, at the replay rate `docs/bridge.md` records. It
/// exists for a set that has no sidecar yet, and for a position rare enough
/// that building one is not worth it. The other answer to the same question is
/// the consumer's own index, fed by the keys a keyed conversion emits.
///
/// Bounded waves of 2048-game chunks deliver hits in caller range order.
/// Workers poll cancellation every 256 games and delivery polls before each
/// hit. Cancellation from a callback prevents subsequent hits and waves:
/// [`PositionSearch::complete`] is `false` and its hits are a prefix of the
/// answer, never the whole of it.
pub fn for_each_position_key(
    db: &Database,
    query: &PositionQuery<'_>,
    threads: usize,
    mut on_hit: impl FnMut(Hit) + Send,
) -> Result<PositionSearch> {
    let ranges = ranges_of(db, 1, 0);
    for_each_position_key_range(db, &ranges, query, threads, &mut on_hit)
}

/// [`for_each_position_key`] over the game numbers `first..=last`.
pub fn for_each_position_key_range(
    db: &Database,
    ranges: &[(u32, u32)],
    query: &PositionQuery<'_>,
    threads: usize,
    on_hit: &mut (impl FnMut(Hit) + Send),
) -> Result<PositionSearch> {
    replay_ranges(db, ranges, query, threads, on_hit, None)
}

// Each wave bounds speculative replay and retained hits.
const REPLAY_BATCH: u32 = 2048;

fn deliver_replay_waves(
    db: &Database,
    ranges: &[(u32, u32)],
    threads: usize,
    pool: Option<&rayon::ThreadPool>,
    cancelled: impl Fn() -> bool,
    replay: impl Fn(&(u32, u32)) -> Result<Replay> + Sync + Send,
    on_hit: &mut (impl FnMut(Hit) + Send),
) -> Result<PositionSearch> {
    let mut total = PositionSearch { complete: true, ..PositionSearch::default() };
    if cancelled() {
        total.complete = false;
        return Ok(total);
    }
    let owned = if threads > 1 && pool.is_none() {
        Some(
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .map_err(|e| Error::corrupt(db.base(), 0, format!("thread pool creation failed: {e}")))?,
        )
    } else {
        None
    };
    let pool = pool.or(owned.as_ref());
    let mut chunks = ranges.iter().flat_map(|&(first, last)| {
        (first..=last)
            .step_by(REPLAY_BATCH as usize)
            .map(move |start| (start, start.saturating_add(REPLAY_BATCH - 1).min(last)))
    });
    let width = threads.max(1);
    loop {
        if cancelled() {
            total.complete = false;
            break;
        }
        let wave: Vec<_> = chunks.by_ref().take(width).collect();
        if wave.is_empty() {
            break;
        }
        let replays = collect_chunks(db, &wave, threads, pool, &replay)?;
        // Work counters include speculative work in the final wave.
        for part in &replays {
            total.games += part.stats.games;
            total.plies += part.stats.plies;
            total.failures += part.stats.failures;
        }
        for part in replays {
            for hit in part.hits {
                if cancelled() {
                    total.complete = false;
                    return Ok(total);
                }
                on_hit(hit);
                total.hits += 1;
            }
            if part.cancelled || cancelled() {
                total.complete = false;
                return Ok(total);
            }
        }
    }
    Ok(total)
}

fn replay_ranges(
    db: &Database,
    ranges: &[(u32, u32)],
    query: &PositionQuery<'_>,
    threads: usize,
    on_hit: &mut (impl FnMut(Hit) + Send),
    pool: Option<&rayon::ThreadPool>,
) -> Result<PositionSearch> {
    if db.generation() == super::Generation::TwoCbh {
        return for_each_position_key_range_v2(db, ranges, query, threads, on_hit, pool);
    }
    let is_cancelled = || query.cancelled.is_some_and(|cancel| cancel());
    let moves = db.moves()?;
    let path = db.members().moves.clone();
    let replay_chunk = |&(first, last): &(u32, u32)| -> Result<Replay> {
        let mut out = Replay::default();
        let mut cache = StartCache::new();
        let mut record = Vec::new();
        // The cancel is asked before a chunk starts and every
        // `CANCEL_POLL_GAMES` games inside it, so a cancelled search stops
        // within a few hundred games per worker instead of finishing the set.
        if is_cancelled() {
            out.cancelled = true;
            return Ok(out);
        }
        let mut headers = db.record_batch(first, last)?;
        let mut since_poll = 0u32;
        for header in headers.by_ref() {
            since_poll += 1;
            if since_poll >= CANCEL_POLL_GAMES {
                since_poll = 0;
                if is_cancelled() {
                    out.cancelled = true;
                    break;
                }
            }
            if !matches!(header.kind(), RecordKind::Game) || header.is_deleted() {
                continue;
            }
            let Some(at) = db.offset_of(header.id(), header.moves_offset(), header.annotations_offset()).ok() else {
                out.stats.failures += 1;
                continue;
            };
            let Ok(bytes) = super::convert::move_record(moves, at, &mut record) else {
                out.stats.failures += 1;
                continue;
            };
            let Ok(game) = cbvault_format::cbh::moves::GameMoves::parse(&path, bytes) else {
                out.stats.failures += 1;
                continue;
            };
            let what = Walked::at(header.id(), at);
            let keep_going = out.one(header.id(), query, &mut cache, start_as_played(what, &game), |start, sink| {
                walk_from(what, &game, start, sink)
            });
            if !keep_going {
                out.cancelled = true;
                break;
            }
        }
        Ok(out)
    };

    deliver_replay_waves(db, ranges, threads, pool, is_cancelled, replay_chunk, on_hit)
}

/// The v2 version of the unindexed position replay. It must never parse a
/// 2CBG frame using the classic 4-byte CBG header, or enumerate 46-byte CBH
/// records where v2 has 192-byte records.
fn for_each_position_key_range_v2(
    db: &Database,
    ranges: &[(u32, u32)],
    query: &PositionQuery<'_>,
    threads: usize,
    on_hit: &mut (impl FnMut(Hit) + Send),
    pool: Option<&rayon::ThreadPool>,
) -> Result<PositionSearch> {
    let is_cancelled = || query.cancelled.is_some_and(|cancel| cancel());
    let moves = db.moves_segment()?;
    let replay_chunk = |&(first, last): &(u32, u32)| -> Result<Replay> {
        let mut out = Replay::default();
        let mut cache = StartCache::new();
        if is_cancelled() {
            out.cancelled = true;
            return Ok(out);
        }
        let mut scratch = Vec::new();
        // Preserve per-record recovery on a failed bulk read.
        let window = db.v2_header_window(first, last, &mut scratch).ok();
        let mut since_poll = 0u32;
        for id in first..=last {
            since_poll += 1;
            if since_poll >= CANCEL_POLL_GAMES {
                since_poll = 0;
                if is_cancelled() {
                    out.cancelled = true;
                    break;
                }
            }
            let header = match window
                .as_ref()
                .and_then(|w| w.get(id))
                .map(GameHeaderRef::TwoCbh)
                .map_or_else(|| db.header_ref(id), Ok)
            {
                Ok(h) => h,
                Err(_) => {
                    out.stats.failures += 1;
                    continue;
                }
            };
            if !matches!(header.kind(), RecordKind::Game | RecordKind::Analysis) || header.is_deleted() {
                continue;
            }
            let at = header.moves_offset();
            let seg_rec = match moves.record_at(at) {
                Ok(r) => r,
                Err(_) => {
                    out.stats.failures += 1;
                    continue;
                }
            };
            let game = match cbvault_format::twocbh::moves::GameMoves::from_record(moves.path(), seg_rec.record()) {
                Ok(g) => g,
                Err(_) => {
                    out.stats.failures += 1;
                    continue;
                }
            };
            let what = Walked::at(id, at);
            let keep_going =
                out.one(id, query, &mut cache, cbvault_chess::v2::decode_v2_start(&game), |start, sink| {
                    cbvault_chess::v2::walk_v2_from(what, &game, start, sink)
                });
            if !keep_going {
                out.cancelled = true;
                break;
            }
        }
        Ok(out)
    };

    deliver_replay_waves(db, ranges, threads, pool, is_cancelled, replay_chunk, on_hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ECO text parser must be the exact inverse of `Eco::code_text`, or a
    /// search for "B20" silently returns the wrong games.
    ///
    /// Every code in every hundred is checked, not a sample: the mapping is
    /// arithmetic in three places and one of them being wrong costs a user the
    /// difference between "500 wrong results" and "every B opening".
    #[test]
    fn eco_text_round_trips_every_code() {
        for code in 0..=499u16 {
            let text = Eco::Code { code, sub: 0 }.code_text().unwrap();
            let text = std::str::from_utf8(&text).unwrap();
            assert_eq!(parse_eco(text), Some((code, Some(0))), "{text} must parse back to code {code}");
            // And with a non-zero sub-code, which is how most games are stored.
            for sub in [1u8, 7, 42, 127] {
                let field = Eco::Code { code, sub }.field();
                let back = Eco::from_field(field);
                assert_eq!(back, Eco::Code { code, sub }, "code {code} sub {sub} must survive the field round trip");
            }
        }
    }

    #[test]
    fn eco_text_rejects_what_is_not_a_code() {
        for bad in ["", " ", "F20", "B20X", "20", "B999", "B2O", "b2O", "Z"] {
            assert_eq!(parse_eco(bad), None, "{bad:?} is not an ECO code");
        }
    }

    #[test]
    fn a_bare_letter_covers_the_whole_hundred() {
        assert_eq!(parse_eco("B"), Some((100, None)));
        assert_eq!(parse_eco("E"), Some((400, None)));
        assert_eq!(parse_eco("b"), Some((100, None)));
        // Out of the five-letter range.
        assert_eq!(parse_eco("F"), None);
    }

    #[test]
    fn range_treats_none_as_open_not_zero() {
        // The bug this guards: "from 2800" becoming 0..2800 and returning every
        // unrated game in the database.
        assert!(Range::at_least(2800).contains(2800));
        assert!(Range::at_least(2800).contains(4000));
        assert!(!Range::at_least(2800).contains(0));
        assert!(!Range::at_least(2800).contains(2799));

        assert!(Range::new(2000, 2010).contains(2000));
        assert!(Range::new(2000, 2010).contains(2010));
        assert!(!Range::new(2000, 2010).contains(2011));
        assert!(!Range::new(2000, 2010).contains(1999));

        assert!(Range::at_most(1500).contains(0));
        assert!(!Range::at_most(1500).contains(1501));

        // An unbounded range matches nothing rather than everything: it is not
        // Filter::All in disguise, it is a query with no constraint.
        assert!(!Range { from: None, to: None }.contains(0));
        // Inverted ranges match nothing.
        assert!(!Range::new(2010, 2000).contains(2005));
    }

    fn mock_header(white: u32, black: u32) -> GameHeader {
        let mut b = [0u8; cbvault_format::cbh::RECORD_SIZE];
        b[0] = 1; // Game
        b[0x09] = (white >> 16) as u8;
        b[0x0a] = (white >> 8) as u8;
        b[0x0b] = white as u8;
        b[0x0c] = (black >> 16) as u8;
        b[0x0d] = (black >> 8) as u8;
        b[0x0e] = black as u8;
        GameHeader::from_bytes(1, &b)
    }

    #[test]
    fn any_player_empty_matches_nothing() {
        let f = any_player(&[]);
        assert!(!f.matches(1, &mock_header(1, 2)));
        assert!(!f.matches(1, &mock_header(0, 0)));
    }

    #[test]
    fn any_player_matches_identically_to_fold_and_handles_large_sets() {
        let legacy_fold = |ids: &[u32]| -> Filter {
            if ids.is_empty() {
                return Filter::Not(Box::new(Filter::All));
            }
            let mut iter = ids.iter();
            let first = Filter::Either(*iter.next().expect("non-empty"));
            iter.fold(first, |acc, id| Filter::AnyOf(Box::new(acc), Box::new(Filter::Either(*id))))
        };

        // 1 player
        let ids_1 = [42];
        let f1 = any_player(&ids_1);
        let fold1 = legacy_fold(&ids_1);
        for (w, b) in [(42, 1), (1, 42), (42, 42), (99, 100)] {
            let h = mock_header(w, b);
            assert_eq!(f1.matches(1, &h), fold1.matches(1, &h));
        }

        // 2 players
        let ids_2 = [10, 20];
        let f2 = any_player(&ids_2);
        let fold2 = legacy_fold(&ids_2);
        for (w, b) in [(10, 1), (1, 20), (20, 10), (30, 40)] {
            let h = mock_header(w, b);
            assert_eq!(f2.matches(1, &h), fold2.matches(1, &h));
        }

        // 100 players: executes without stack overflow or recursion
        let ids_100: Vec<u32> = (1..=100).collect();
        let f100 = any_player(&ids_100);
        let fold100 = legacy_fold(&ids_100);
        for id in [1, 50, 100, 101, 500] {
            let h_white = mock_header(id, 9999);
            let h_black = mock_header(9999, id);
            assert_eq!(f100.matches(1, &h_white), fold100.matches(1, &h_white));
            assert_eq!(f100.matches(1, &h_black), fold100.matches(1, &h_black));
        }

        // Directional sets
        let white_set = Filter::white_set(IdSet::from_ids([10, 20]));
        let black_set = Filter::black_set(IdSet::from_ids([10, 20]));
        assert!(white_set.matches(1, &mock_header(10, 99)));
        assert!(!white_set.matches(1, &mock_header(99, 10)));
        assert!(black_set.matches(1, &mock_header(99, 20)));
        assert!(!black_set.matches(1, &mock_header(20, 99)));
    }
}
