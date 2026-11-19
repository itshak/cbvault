//! A [`GameDataQuery`] compiled for one database: every name resolved to an
//! entity-id set, every range to integers, so that testing a header record is
//! a handful of integer comparisons and no allocation.

use cbvault_format::game::{GameResult, Head, RecordKind};

use super::names::{self, Table};
use super::text::Needle;
use super::{
    EcoCode, EloDifference, EloMode, EloQuery, Endings, GameDataQuery, PlayerName, QueryError, QueryResult, ResultSet,
};
use crate::bridge::{Database, Filter, IdSet, Range};

/// A header record of either generation, with its source id, which the
/// shared [`Head`] trait does not carry.
pub(crate) trait Record: Head {
    /// The source's entity id.
    fn source_id(&self) -> i64;
}

impl Record for cbvault_format::cbh::GameHeader {
    fn source_id(&self) -> i64 {
        i64::from(self.source())
    }
}

impl Record for crate::bridge::GameHeaderRef<'_> {
    fn source_id(&self) -> i64 {
        self.source()
    }
}

/// The entity-id sets "any text" matched.
#[derive(Debug)]
struct AnyIds {
    players: IdSet,
    tournaments: IdSet,
    annotators: IdSet,
    sources: IdSet,
    negate: bool,
}

/// A query ready to test header records.
#[derive(Debug)]
pub(crate) struct Compiled {
    white: Option<IdSet>,
    black: Option<IdSet>,
    ignore_colours: bool,
    wins_only: bool,
    tournament: Option<IdSet>,
    annotator: Option<IdSet>,
    date: Option<(i32, i32)>,
    eco: Option<(u16, u16)>,
    moves: Option<Range>,
    elo: Option<EloQuery>,
    elo_difference: Option<EloDifference>,
    results: ResultSet,
    pub(crate) endings: Endings,
    pub(crate) text_records: bool,
    any_text: Option<AnyIds>,
    time_control: Option<IdSet>,
    pub(crate) extra: Option<Filter>,
}

impl Compiled {
    /// Resolves `query`'s names in `db` and checks it can be answered.
    pub(crate) fn new(db: &Database, query: &GameDataQuery) -> QueryResult<Compiled> {
        let how = query.texts_match.0;
        let any_needle = query.any_text.as_ref().and_then(|any| Needle::new(&any.text, how));
        let white_line = Line::new(&query.white, query);
        let black_line = Line::new(&query.black, query);
        let annotator_needle = query.annotator.as_deref().and_then(|t| Needle::new(t, how));
        // In 2CBH an annotator is a player, so the annotator is looked up in
        // the same pass (see `names::each_twocbh`).
        let twocbh = db.twocbh_entities().is_some();
        let annotator_in_players = twocbh && annotator_needle.is_some();
        // The White line, the Black line, the players of "any text" and a 2CBH
        // annotator share one pass over the player namebase.
        let (white, black, any_players, annotator_players) = if white_line.is_some()
            || black_line.is_some()
            || any_needle.is_some()
            || annotator_in_players
        {
            let mut joined = String::new();
            let [w, b, a, n] = names::ids_where_each(db, Table::Player, |last, first| {
                let any = any_needle.as_ref().is_some_and(|needle| {
                    joined.clear();
                    joined.push_str(last);
                    if !first.is_empty() {
                        joined.push_str(", ");
                        joined.push_str(first);
                    }
                    needle.matches_folded(&joined)
                });
                let annotator = annotator_in_players
                    && annotator_needle.as_ref().is_some_and(|n| n.matches_folded(last) || n.matches_folded(first));
                [
                    white_line.as_ref().is_some_and(|l| l.matches(last, first)),
                    black_line.as_ref().is_some_and(|l| l.matches(last, first)),
                    any,
                    annotator,
                ]
            })?;
            (white_line.is_some().then_some(w), black_line.is_some().then_some(b), a, annotator_in_players.then_some(n))
        } else {
            (None, None, IdSet::new(), None)
        };
        if query.wins_only && white.is_none() {
            return Err(QueryError::Invalid("\"wins only\" needs a player in the White line"));
        }
        if query.text_records && query.endings.is_set() {
            return Err(QueryError::Invalid("a guiding text has no final position"));
        }
        let titled = |table: Table, text: &Option<String>| -> QueryResult<Option<IdSet>> {
            let Some(needle) = text.as_deref().and_then(|t| Needle::new(t, how)) else { return Ok(None) };
            Ok(Some(names::ids_where(db, table, |a, b| needle.matches_folded(a) || needle.matches_folded(b))?))
        };
        let tournament = titled(Table::Tournament, &query.tournament)?;
        let annotator = match annotator_players {
            Some(ids) => Some(ids),
            None => titled(Table::Annotator, &query.annotator)?,
        };
        let any_text = match (&query.any_text, &any_needle) {
            (Some(any), Some(needle)) => {
                let either = |a: &str, b: &str| needle.matches_folded(a) || needle.matches_folded(b);
                let annotators =
                    if twocbh { any_players.clone() } else { names::ids_where(db, Table::Annotator, either)? };
                Some(AnyIds {
                    players: any_players,
                    tournaments: names::ids_where(db, Table::Tournament, either)?,
                    annotators,
                    sources: names::ids_where(db, Table::Source, either)?,
                    negate: any.negate,
                })
            }
            _ => None,
        };
        let time_control = match query.time_control {
            Some(want) => Some(names::tournaments_with(db, want)?.ok_or(QueryError::Unsupported(
                "the time control of a classic database: its tournament type byte is not decoded yet",
            ))?),
            None => None,
        };
        let date = match (query.date.from, query.date.to) {
            (None, None) => None,
            (from, to) => Some((from.map_or(i32::MIN, |d| d.low()), to.map_or(i32::MAX, |d| d.high()))),
        };
        let eco = query.eco.map(|r| (eco_field(r.from, 0), eco_field(r.to, 127)));
        Ok(Compiled {
            white,
            black,
            ignore_colours: query.ignore_colours,
            wins_only: query.wins_only,
            tournament,
            annotator,
            date,
            eco,
            moves: query.moves,
            elo: query.elo,
            elo_difference: query.elo_difference,
            results: query.results,
            endings: query.endings,
            text_records: query.text_records,
            any_text,
            time_control,
            extra: query.extra.clone(),
        })
    }

    /// Whether records of `kind` are searched at all: games, or guiding texts
    /// when the "Text" box is ticked.
    #[inline]
    pub(crate) fn wants(&self, kind: RecordKind) -> bool {
        if self.text_records { kind == RecordKind::Text } else { kind == RecordKind::Game }
    }

    /// Whether the record passes every criterion the header can answer.
    /// [`Compiled::extra`] is tested by the caller, which knows the record's
    /// concrete type.
    pub(crate) fn matches<H: Record>(&self, header: &H) -> bool {
        if self.text_records {
            return self.date_ok(header);
        }
        self.players_ok(header)
            && in_set(&self.tournament, header.tournament())
            && in_set(&self.annotator, header.annotator())
            && in_set(&self.time_control, header.tournament())
            && self.date_ok(header)
            && self.eco_ok(header)
            && self.moves.is_none_or(|r| r.contains(header.move_count()))
            && self.elo_ok(header)
            && self.result_ok(header.result())
            && self.any_text_ok(header)
    }

    fn players_ok<H: Record>(&self, header: &H) -> bool {
        if self.white.is_none() && self.black.is_none() {
            return true;
        }
        let (w, b) = (header.white(), header.black());
        let straight = in_set(&self.white, w) && in_set(&self.black, b);
        let swapped = self.ignore_colours && in_set(&self.white, b) && in_set(&self.black, w);
        if !self.wins_only {
            return straight || swapped;
        }
        // "Wins only" is about the player of the White line: they won with
        // white when the lines matched straight, with black when swapped.
        let result = header.result();
        (straight && white_won(result)) || (swapped && black_won(result))
    }

    fn date_ok<H: Record>(&self, header: &H) -> bool {
        let Some((low, high)) = self.date else { return true };
        let date = header.played_date();
        let key = super::key(date.year(), date.month(), date.day());
        date.year() != 0 && (low..=high).contains(&key)
    }

    fn eco_ok<H: Record>(&self, header: &H) -> bool {
        let Some((low, high)) = self.eco else { return true };
        match header.eco() {
            eco @ cbvault_format::game::Eco::Code { .. } => (low..=high).contains(&eco.field()),
            _ => false,
        }
    }

    fn elo_ok<H: Record>(&self, header: &H) -> bool {
        let (w, b) = header.elo();
        let range_ok = match self.elo {
            None => true,
            Some(EloQuery { range, mode: EloMode::One }) => range.contains(w) || range.contains(b),
            Some(EloQuery { range, mode: EloMode::Both }) => range.contains(w) && range.contains(b),
            Some(EloQuery { range, mode: EloMode::Average }) => w > 0 && b > 0 && range.contains((w + b) / 2),
        };
        let difference_ok = match self.elo_difference {
            None => true,
            Some(EloDifference::AtMost(gap)) => w > 0 && b > 0 && (w - b).abs() <= gap,
            Some(EloDifference::AtLeast(gap)) => w > 0 && b > 0 && (w - b).abs() >= gap,
        };
        range_ok && difference_ok
    }

    fn result_ok(&self, result: GameResult) -> bool {
        let r = &self.results;
        r.is_any()
            || (r.white_wins && white_won(result))
            || (r.black_wins && black_won(result))
            || (r.draw && matches!(result, GameResult::Draw | GameResult::DrawForfeit))
            || (r.both_lost && result == GameResult::BothLost)
    }

    fn any_text_ok<H: Record>(&self, header: &H) -> bool {
        let Some(any) = &self.any_text else { return true };
        let hit = contains(&any.players, header.white())
            || contains(&any.players, header.black())
            || contains(&any.tournaments, header.tournament())
            || contains(&any.annotators, header.annotator())
            || contains(&any.sources, header.source_id());
        hit != any.negate
    }
}

/// A player line, folded: the last name and the forename it must match.
struct Line {
    last: Option<Needle>,
    first: Option<Needle>,
}

impl Line {
    /// The line, or `None` when it is blank and so no criterion.
    fn new(line: &PlayerName, query: &GameDataQuery) -> Option<Line> {
        let last = Needle::new(&line.last, query.names_match);
        let first = Needle::new(&line.first, query.names_match);
        (last.is_some() || first.is_some()).then_some(Line { last, first })
    }

    /// Whether a player's folded names satisfy the line.
    fn matches(&self, last: &str, first: &str) -> bool {
        self.last.as_ref().is_none_or(|n| n.matches_folded(last))
            && self.first.as_ref().is_none_or(|n| n.matches_folded(first))
    }
}

/// The ECO field value of `code`, with `missing` standing for an absent
/// sub-code. The field sorts like (code, sub), so a range of fields is a
/// range of codes.
fn eco_field(code: EcoCode, missing: u8) -> u16 {
    cbvault_format::game::Eco::Code { code: code.code.min(499), sub: code.sub.unwrap_or(missing).min(127) }.field()
}

#[inline]
fn white_won(result: GameResult) -> bool {
    matches!(result, GameResult::WhiteWins | GameResult::WhiteWinsForfeit)
}

#[inline]
fn black_won(result: GameResult) -> bool {
    matches!(result, GameResult::BlackWins | GameResult::BlackWinsForfeit)
}

/// Whether `id` is in `set`; an absent set is no criterion.
#[inline]
fn in_set(set: &Option<IdSet>, id: i64) -> bool {
    set.as_ref().is_none_or(|s| contains(s, id))
}

#[inline]
fn contains(set: &IdSet, id: i64) -> bool {
    u32::try_from(id).is_ok_and(|id| set.contains(id))
}
