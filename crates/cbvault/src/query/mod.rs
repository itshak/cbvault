//! A ChessBase-style game search: every field of the "Game data" page of the
//! search dialog, combined, over a raw classic or 2CBH database.
//!
//! [`GameDataQuery`] is that page as a value: two player names with "ignore
//! colours" and "wins only", tournament, annotator, a date range, an ECO range,
//! a move-count range, an Elo range in each of ChessBase's four modes with a
//! rating difference, the result boxes including mate, stalemate and check,
//! guiding texts, "any text" with "not", and the time control. Every field is
//! optional and every field set must hold: an empty query is every game.
//!
//! [`search`] answers it in two stages:
//!
//! 1. **The header stage** compiles the names to entity-id sets once (ADR-007
//!    §2), then scans the header file in parallel, comparing integers. It
//!    never opens the moves file.
//! 2. **The ending stage** runs only when mate, stalemate or check is asked
//!    for, and only on the games the first stage kept: it replays each main
//!    line with `gigachess` and looks at the final position.
//!
//! The module sits beside [`crate::bridge::search`] and builds only on the
//! bridge's public API, so a caller can mix the two: a resolved
//! [`Filter`](crate::bridge::Filter) can be added as one more criterion with
//! [`GameDataQuery::extra`].

mod ending;
mod header;
mod names;
mod parse;
mod scan;
mod text;

use cbvault_format::error::Error;
use cbvault_format::game::Tournament;

use crate::bridge::{Database, Filter, Range};

pub use ending::Ending;
pub use parse::{ParseError, parse_range};
pub use scan::{Found, SearchOptions, search, search_with};
pub use text::TextMatch;

/// The "Game data" page of ChessBase's search dialog.
///
/// Fields left at their default are not criteria. The defaults of the two
/// matching modes follow the dialog: names match from their start, the other
/// texts anywhere.
#[derive(Clone, Debug, Default)]
pub struct GameDataQuery {
    /// The "White" line: last name and forename.
    pub white: PlayerName,
    /// The "Black" line: last name and forename.
    pub black: PlayerName,
    /// "Ignore colours": the two lines may name either side.
    pub ignore_colours: bool,
    /// "Wins only": the player of the "White" line won the game. With
    /// `ignore_colours`, with whichever colour they had. Needs a "White" line.
    pub wins_only: bool,
    /// How the player names match.
    pub names_match: TextMatch,
    /// "Tournament": the title or the place.
    pub tournament: Option<String>,
    /// "Annotator".
    pub annotator: Option<String>,
    /// How the tournament, annotator and "any text" fields match.
    pub texts_match: TextsMatch,
    /// "Year": the date the game was played, from and to.
    pub date: DateRange,
    /// "ECO": from and to, sub-codes included.
    pub eco: Option<EcoRange>,
    /// "Moves": the number of moves of the main line, as the header stores it
    /// (the classic format caps it at 255).
    pub moves: Option<Range>,
    /// "Elo": the range and the mode it applies in.
    pub elo: Option<EloQuery>,
    /// "Difference": the gap between the two ratings.
    pub elo_difference: Option<EloDifference>,
    /// "Result": the boxes 1-0, 0-1, ½ and 0-0. None ticked is any result.
    pub results: ResultSet,
    /// "Result": the boxes Mate, Stalemate and Check, about the final position.
    /// None ticked is no criterion.
    pub endings: Endings,
    /// "Text": search the guiding texts instead of the games.
    pub text_records: bool,
    /// "Any text", with "Not".
    pub any_text: Option<AnyText>,
    /// "Time control".
    pub time_control: Option<TimeControl>,
    /// A filter of the bridge's own vocabulary, combined with the rest.
    pub extra: Option<Filter>,
}

/// A player line of the dialog: last name, forename, or both.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerName {
    /// The last name.
    pub last: String,
    /// The forename.
    pub first: String,
}

impl PlayerName {
    /// A line from `"Last, First"`, `"Last"` or `", First"`.
    pub fn parse(text: &str) -> PlayerName {
        match text.split_once(',') {
            Some((last, first)) => PlayerName { last: last.trim().to_owned(), first: first.trim().to_owned() },
            None => PlayerName { last: text.trim().to_owned(), first: String::new() },
        }
    }

    /// Whether the line is empty: then it is no criterion.
    pub fn is_blank(&self) -> bool {
        self.last.trim().is_empty() && self.first.trim().is_empty()
    }
}

/// How the free texts match: tournament, annotator and "any text".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextsMatch(pub TextMatch);

impl Default for TextsMatch {
    fn default() -> TextsMatch {
        TextsMatch(TextMatch::Contains)
    }
}

/// A date of the "Year" fields: a year, optionally a month, optionally a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialDate {
    /// The year.
    pub year: u16,
    /// The month, 1-12.
    pub month: Option<u8>,
    /// The day, 1-31.
    pub day: Option<u8>,
}

impl PartialDate {
    /// The comparison key of the date as a lower bound: missing parts are the
    /// earliest.
    fn low(self) -> i32 {
        key(self.year, self.month.unwrap_or(0), self.day.unwrap_or(0))
    }

    /// The comparison key of the date as an upper bound: missing parts are the
    /// latest, so "to 2026.11" keeps all of November.
    fn high(self) -> i32 {
        key(self.year, self.month.unwrap_or(99), self.day.unwrap_or(99))
    }
}

/// `year`, `month` and `day` as one integer that sorts like the date.
fn key(year: u16, month: u8, day: u8) -> i32 {
    i32::from(year) * 10_000 + i32::from(month) * 100 + i32::from(day)
}

/// The "Year" fields: either end may be open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateRange {
    /// The earliest date.
    pub from: Option<PartialDate>,
    /// The latest date.
    pub to: Option<PartialDate>,
}

/// An ECO code of the dialog, with an optional sub-code: `B20` or `B20/03`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EcoCode {
    /// 0-499: A00-E99.
    pub code: u16,
    /// The sub-code; `None` is the first as a lower bound and the last as an
    /// upper bound.
    pub sub: Option<u8>,
}

/// The "ECO" fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EcoRange {
    /// The first code.
    pub from: EcoCode,
    /// The last code.
    pub to: EcoCode,
}

/// The mode of the "Elo" range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EloMode {
    /// At least one of the two ratings is in the range.
    #[default]
    One,
    /// Both ratings are in the range.
    Both,
    /// The average of the two ratings is in the range; both must be known.
    Average,
}

/// The "Elo" range and its mode. ChessBase's "None" mode is no `EloQuery`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EloQuery {
    /// The range.
    pub range: Range,
    /// How it applies to the two ratings.
    pub mode: EloMode,
}

/// The "Difference" field: the absolute gap between the two ratings, both of
/// which must be known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EloDifference {
    /// The gap is at most this.
    AtMost(i32),
    /// The gap is at least this.
    AtLeast(i32),
}

/// The result boxes 1-0, 0-1, ½ and 0-0. A forfeit counts with the result it
/// gave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResultSet {
    /// 1-0.
    pub white_wins: bool,
    /// 0-1.
    pub black_wins: bool,
    /// ½-½.
    pub draw: bool,
    /// 0-0: both players lost.
    pub both_lost: bool,
}

impl ResultSet {
    /// Whether no box is ticked: then any result matches.
    pub fn is_any(&self) -> bool {
        !(self.white_wins || self.black_wins || self.draw || self.both_lost)
    }
}

/// The result boxes about the final position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Endings {
    /// The side to move is checkmated.
    pub mate: bool,
    /// The side to move is stalemated.
    pub stalemate: bool,
    /// The side to move is in check and has a move.
    pub check: bool,
}

impl Endings {
    /// Whether any box is ticked.
    pub fn is_set(&self) -> bool {
        self.mate || self.stalemate || self.check
    }

    /// Whether `ending` is one of the ticked boxes.
    pub fn accepts(&self, ending: Ending) -> bool {
        match ending {
            Ending::Mate => self.mate,
            Ending::Stalemate => self.stalemate,
            Ending::Check => self.check,
            Ending::Other => false,
        }
    }
}

/// The "Any text" field and its "Not" box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnyText {
    /// The text: matched against the players, the tournament (title and
    /// place), the annotator and the source.
    pub text: String,
    /// "Not": keep the games where the text appears nowhere.
    pub negate: bool,
}

/// The "Time control" choice, from the tournament's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeControl {
    /// Neither blitz, rapid nor correspondence.
    Classical,
    /// Rapid.
    Rapid,
    /// Blitz.
    Blitz,
    /// Correspondence.
    Correspondence,
}

impl TimeControl {
    /// Whether `tournament`'s type is this time control.
    pub fn matches(self, tournament: &Tournament) -> bool {
        match self {
            TimeControl::Classical => {
                !(tournament.is_blitz() || tournament.is_rapid() || tournament.is_correspondence())
            }
            TimeControl::Rapid => tournament.is_rapid(),
            TimeControl::Blitz => tournament.is_blitz(),
            TimeControl::Correspondence => tournament.is_correspondence(),
        }
    }
}

/// Why a query could not be answered.
#[derive(Debug)]
pub enum QueryError {
    /// Reading the database failed.
    Database(Error),
    /// The query asks for something this database's format does not tell.
    Unsupported(&'static str),
    /// The query contradicts itself.
    Invalid(&'static str),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::Database(e) => write!(f, "{e}"),
            QueryError::Unsupported(what) => write!(f, "not supported: {what}"),
            QueryError::Invalid(what) => write!(f, "invalid query: {what}"),
        }
    }
}

impl std::error::Error for QueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            QueryError::Database(e) => Some(e),
            _ => None,
        }
    }
}

impl From<Error> for QueryError {
    fn from(e: Error) -> QueryError {
        QueryError::Database(e)
    }
}

/// The result of compiling or running a query.
pub type QueryResult<T> = std::result::Result<T, QueryError>;

/// Compiles a query against `db`: what [`search`] does first, exposed for a
/// caller that wants to know a query is answerable before running it.
pub fn check(db: &Database, query: &GameDataQuery) -> QueryResult<()> {
    header::Compiled::new(db, query).map(|_| ())
}
