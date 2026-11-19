# Searching a database: the "Game data" page

`cbvault::query` answers every field of the **Game data** page of ChessBase's
search dialog, combined, over a raw classic (`.cbh`) or 2CBH (`.2cbh`)
database. `cbvault search` is the same query from the command line.

```rust,no_run
use cbvault::bridge::{Database, Range};
use cbvault::query::{EloMode, EloQuery, GameDataQuery, PlayerName, search};

let db = Database::open("Mega Database 2026.2cbh")?;
let mut query = GameDataQuery {
    white: PlayerName::parse("Carlsen, Magnus"),
    black: PlayerName::parse("Caruana"),
    ignore_colours: true,
    elo: Some(EloQuery { range: Range::at_least(2700), mode: EloMode::Both }),
    ..GameDataQuery::default()
};
query.results.draw = true;
let found = search(&db, &query, 8)?;
println!("{} games", found.ids.len());
# Ok::<(), Box<dyn std::error::Error>>(())
```

```bash
cbvault search "Mega Database 2026.2cbh" --white "Carlsen, Magnus" --black Caruana \
  --ignore-colours --elo 2700- --elo-mode both --result 1/2 --pgn found.pgn
```

## The fields

Every field is optional and every field given must hold; an empty query is
every game.

| Dialog | `GameDataQuery` | `cbvault search` | Meaning |
|---|---|---|---|
| White / Black | `white`, `black` (`PlayerName { last, first }`) | `--white "Last, First"`, `--black` | The last name and/or forename, by default **from their start** (`names_match`: prefix, contains, exact) |
| Ignore colours | `ignore_colours` | `--ignore-colours` | The two lines may name either side |
| Wins only | `wins_only` | `--wins-only` | The player of the White line won, with the colour they had; needs a White line |
| Tournament | `tournament` | `--tournament` | The title **or the place** contains the text (`texts_match`) |
| Annotator | `annotator` | `--annotator` | The annotator contains the text |
| Year | `date` (`DateRange`) | `--year 2020-2026.11` | `year`, `year.month` or `year.month.day` at either end; an upper bound without a month keeps the whole year. A game without a date is outside every range |
| ECO | `eco` (`EcoRange`) | `--eco A00-E99/99` | Codes with optional sub-codes; an upper bound without a sub-code keeps them all. A game without an ECO code (or a Chess960 game) is outside every range |
| Moves | `moves` | `--moves 0-22` | The header's move count; the classic format caps it at 255 |
| Elo + mode | `elo` (`EloQuery { range, mode }`) | `--elo 2500-3500 --elo-mode one\|both\|avg` | One rating, both, or their average in the range ("None" is no `elo`). The average needs both ratings |
| Difference | `elo_difference` | `--elo-diff-max N`, `--elo-diff-min N` | The gap between the ratings, both of which must be known |
| 1-0, 0-1, ½, 0-0 | `results` | `--result 1-0,0-1,1/2,0-0` | Any ticked result; a forfeit counts with the result it gave |
| Mate, Stalemate, Check | `endings` | `--result mate,stalemate,check` | The final position of the main line (see below) |
| Text | `text_records` | `--text` | Guiding texts instead of games |
| Any text + Not | `any_text` (`AnyText { text, negate }`) | `--any-text T [--not]` | The text in a player, the tournament (title or place), the annotator or the source; "Not" keeps the games where it appears nowhere |
| Time control | `time_control` | `--time-control classical\|rapid\|blitz\|correspondence` | From the tournament's type byte: **2CBH only**. A classic database answers `QueryError::Unsupported` (its `.cbt` type byte is not decoded) |

Texts are compared case-insensitively and with the accents of Latin letters
removed, so `gonzalez` finds `González`.

A bridge `Filter` can be added as one more criterion with
`GameDataQuery::extra`.

## How it runs

1. **Names become id sets once.** Each text is matched against its namebase in
   one pass (ADR-007 §2), and the White line, the Black line and "any text"
   share the pass over the players. The scan then compares integers.
2. **The header scan** runs in parallel over chunks of 8,192 records and returns
   the ids in ascending order at any thread count. It never opens the moves
   file (`tests/query_game_data.rs` hides `.cbg` to prove it).
3. **The ending stage** runs only when mate, stalemate or check is ticked, and
   only on the games the scan kept: each main line is replayed with
   `Board::play`, which keeps the checkers cache, and the final position is
   classified. A Chess960 game starts from the board its header's start number
   builds. A game that cannot be replayed is counted in `Found::failed`, never
   matched.

## Measured

Mega Database 2026 (2CBH, 11,740,862 records), warm,
`CBVAULT_QUERY_REPORT=1 cargo bench -p cbvault --bench query`:

| Query | Matches | 1 thread | 10 threads |
|---|---:|---:|---:|
| bridge `Filter::EloAtLeast(2700)` through `extra` | 140,185 | 0.415 s | 0.093 s |
| Elo ≥ 2700, one | 140,185 | 0.363 s | 0.067 s |
| Elo ≥ 2700, both | 35,561 | 0.362 s | 0.077 s |
| White Carlsen, Black Caruana, ignore colours | 212 | 1.092 s | 0.187 s |
| Tournament contains "Wijk" | 22,843 | 0.545 s | 0.112 s |
| ECO B20–B99, moves ≤ 22 | 259,667 | 0.434 s | 0.072 s |
| Mate, games from 2025 on (replays 487,297 games) | 58,391 | 6.482 s | 0.904 s |

The name queries pay one pass over the `.2lid`, which interleaves every
entity table (2.0 GB on this database).

On the 20,000-game fixture (`cargo bench -p cbvault --bench query`), one
thread: the bridge's `scan` with `EloAtLeast(1)` 6.34 ms, the same criterion as
a query 0.44 ms, and a query setting every header field 0.48 ms. The two scans
do not do the same work: `scan` resolves the names of every match it returns,
the query returns ids.

## Not yet established

- The **classic time control**: the `.cbt` type byte is not decoded, so the
  query refuses it rather than guessing.
- **Good games / Beautiful games**: which header, flag or medal bits ChessBase
  uses is not established; they are not offered.
- **In repertoire** needs a position search, which this page does not cover.
- The matching rules above are the ones a test database must confirm against
  ChessBase's own counts: name matching (start vs anywhere, accents), "wins
  only" with ignored colours, the Elo modes with a missing rating, whether
  "Difference" is an upper or a lower bound, and whether "Check" includes mate.
