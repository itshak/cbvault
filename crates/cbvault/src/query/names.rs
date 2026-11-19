//! Typed names to entity ids, once per query.
//!
//! ADR-007 §2: a free-text predicate walks the namebase once and marks the ids
//! whose name matches, and the scan then compares integers. This module is
//! that walk, for both generations, through the public readers only: the
//! classic `Entities::for_each` and the `.2lid` batch readers.

use std::borrow::Cow;

use cbvault_format::cbh::Entity;
use cbvault_format::cbh::bytes::NameBuf;
use cbvault_format::error::{Error, Result};
use cbvault_format::game::Tournament;
use cbvault_format::twocbh;

use super::text::fold_into;
use crate::bridge::{Database, IdSet};

/// The namebase a name is looked up in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Table {
    /// Players: last name and forename.
    Player,
    /// Tournaments: title and place.
    Tournament,
    /// Annotators: one field.
    Annotator,
    /// Sources: one field.
    Source,
}

/// Bytes read per batch from `.2lid`.
const BATCH_BYTES: usize = 256 << 10;

/// The ids of `table` whose two folded fields satisfy `keep`. A one-field
/// table hands an empty second field.
pub(crate) fn ids_where(db: &Database, table: Table, mut keep: impl FnMut(&str, &str) -> bool) -> Result<IdSet> {
    let [ids] = ids_where_each(db, table, |a, b| [keep(a, b)])?;
    Ok(ids)
}

/// [`ids_where`] for `N` criteria at once, in one pass over the namebase: a
/// `.2lid` interleaves every table, so a pass over its players reads the
/// whole file, and the White line, the Black line and "any text" share it.
pub(crate) fn ids_where_each<const N: usize>(
    db: &Database,
    table: Table,
    mut keep: impl FnMut(&str, &str) -> [bool; N],
) -> Result<[IdSet; N]> {
    let mut ids: [Vec<u32>; N] = std::array::from_fn(|_| Vec::new());
    let mut last = String::new();
    let mut first = String::new();
    let mut test = |id: u32, a: &str, b: &str| {
        last.clear();
        first.clear();
        fold_into(a, &mut last);
        fold_into(b, &mut first);
        for (set, hit) in ids.iter_mut().zip(keep(&last, &first)) {
            if hit {
                set.push(id);
            }
        }
    };
    match db.twocbh_entities() {
        Some(entities) => each_twocbh(entities, table, &mut test)?,
        None => {
            let entity = match table {
                Table::Player => Entity::Player,
                Table::Tournament => Entity::Tournament,
                Table::Annotator => Entity::Annotator,
                Table::Source => Entity::Source,
            };
            let (mut a, mut b) = (NameBuf::new(), NameBuf::new());
            db.entities().for_each(entity, |id, name| {
                let last = name.decode_last(&mut a);
                let first = name.decode_first(&mut b);
                test(id, last, first);
            })?;
        }
    }
    Ok(ids.map(IdSet::from_ids))
}

/// Every entity of `table` in a `.2lid`, with its two fields.
fn each_twocbh(entities: &twocbh::Entities, table: Table, f: &mut impl FnMut(u32, &str, &str)) -> Result<()> {
    match table {
        // Players and tournaments are read raw, in batches, and their two
        // length-prefixed strings borrowed in place: nothing is allocated per
        // entity, which is what keeps a pass over 482,129 players short.
        // Players and tournaments are read raw, in batches, and their two
        // length-prefixed strings borrowed in place: nothing is allocated per
        // entity, which is what keeps a pass over 482,129 players short.
        //
        // A 2CBH annotator is a player: the header's annotator id indexes the
        // player table (the reference database's annotator ids name players
        // such as "ChessBase"), so annotators are read from there too.
        Table::Player | Table::Annotator | Table::Tournament => {
            let (typ, swap) = match table {
                Table::Tournament => (twocbh::entities::TOURNAMENT, true),
                _ => (twocbh::entities::PLAYER, false),
            };
            batched(entities.count(typ), |ids, buf, at| {
                entities.read_within(typ, ids, buf, usize::MAX, &mut |record: Option<&[u8]>| {
                    // A tournament stores its place first, then its title.
                    if let (Some((one, two)), Ok(id)) = (record.and_then(two_strings), u32::try_from(*at)) {
                        if swap { f(id, &two, &one) } else { f(id, &one, &two) }
                    }
                    *at += 1;
                    Ok::<(), Error>(())
                })
            })
        }
        // Sources are read the way the bridge resolves a game's source
        // (`twocbh::Entities::source`), which falls back from type 3 to type 2;
        // the table is small, so one read per id is enough.
        Table::Source => {
            let count = entities.count(twocbh::entities::SOURCE).max(entities.count(twocbh::entities::GAME_TAG));
            for id in 0..count {
                if let (Some(name), Ok(id)) = (entities.source(id)?, u32::try_from(id)) {
                    f(id, &name, "");
                }
            }
            Ok(())
        }
    }
}

/// The first two length-prefixed strings of a `.2lid` record, borrowed when
/// they are UTF-8 (as the reader's own decoder takes them, lossily otherwise).
fn two_strings(record: &[u8]) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
    let mut at = 0usize;
    let mut next = || -> Option<Cow<'_, str>> {
        let len = usize::try_from(i32::from_le_bytes(record.get(at..at + 4)?.try_into().ok()?)).ok()?;
        let bytes = record.get(at + 4..at + 4 + len)?;
        at += 4 + len;
        Some(String::from_utf8_lossy(bytes))
    };
    let one = next()?;
    let two = next()?;
    Some((one, two))
}

/// Runs `read` over `0..count` in batches until it stops advancing. `read`
/// gets the ids still wanted, a scratch buffer, and the id of the next record
/// it hands its callback, which the callback advances.
fn batched(count: i64, mut read: impl FnMut(std::ops::Range<i64>, &mut [u8], &mut i64) -> Result<u64>) -> Result<()> {
    let mut buf = vec![0u8; BATCH_BYTES];
    let mut at = 0i64;
    while at < count {
        let mut next = at;
        let n = read(at..count, &mut buf, &mut next)?;
        if n == 0 {
            break;
        }
        at = at.saturating_add(i64::try_from(n).unwrap_or(i64::MAX));
    }
    Ok(())
}

/// The ids of the tournaments whose time control is `want`, from the event
/// type byte. Only 2CBH decodes that byte; the classic `.cbt` keeps a type
/// byte this reader does not yet know, so a classic database answers `None`.
pub(crate) fn tournaments_with(db: &Database, want: super::TimeControl) -> Result<Option<IdSet>> {
    let Some(entities) = db.twocbh_entities() else { return Ok(None) };
    let count = entities.count(twocbh::entities::TOURNAMENT);
    let mut ids = Vec::new();
    batched(count, |range, buf, at| {
        entities.read_tournaments_within(range, buf, usize::MAX, &mut |t: Option<Tournament>| {
            if let (Some(t), Ok(id)) = (t, u32::try_from(*at))
                && want.matches(&t)
            {
                ids.push(id);
            }
            *at += 1;
            Ok::<(), Error>(())
        })
    })?;
    Ok(Some(IdSet::from_ids(ids)))
}
