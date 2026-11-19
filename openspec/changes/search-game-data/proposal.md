## Why

The bridge's `Filter` answers the tag questions a consumer's own search form
sends (an Elo range, a year, a result, an ECO code, resolved player ids), but a
ChessBase user searches with ChessBase's dialog, and its **Game data** page asks
more: names by last name *and* forename, matched from their start, "ignore
colours", "wins only", the tournament's place as well as its title, date ranges
to the month or day, ECO *ranges* with sub-codes, the number of moves, the Elo
range in four modes with a rating difference, several result boxes at once,
mate, stalemate and check, guiding texts, "any text" with "not", and the time
control. Several of these are integers the header already holds; the rest are
one pass over a namebase or, for the final position, a replay of the games the
header scan kept. There was also no name-to-id lookup for 2CBH at all, so the
largest database a user holds today (Mega Database 2026 is 2CBH) could not be
searched by name.

## What Changes

- A new module, `cbvault::query`, beside `bridge::search` and built on the
  bridge's public API only:
  - `GameDataQuery`, one value per field of the page, every field optional;
  - `search` / `search_with` (threads, a game range, cancellation), returning
    the matching ids in ascending order at any thread count;
  - name matching that folds case and Latin accents, from the start of the name
    by default;
  - a name-to-id pass for both generations, shared by the White line, the
    Black line and "any text";
  - an ending stage for mate, stalemate and check that replays only the games
    the header scan kept;
  - `QueryError::Unsupported` for what the format does not tell (the classic
    time control), and `QueryError::Invalid` for a query that contradicts
    itself ("wins only" without a White line).
- `cbvault search <db> [criteria]`, a thin shell over `query::search_with`.
- A benchmark, `benches/query.rs`, with fixture gates and a whole-database
  report.

**Not changed:** `bridge::search` and every other existing module. The only
edits outside new files register the module, the subcommand and the bench.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `cbvault`: a ChessBase-style "Game data" search, combined, over both
  generations.

## Impact

- New: `crates/cbvault/src/query/`, `crates/cbvault-cli/src/search.rs`,
  `crates/cbvault/tests/query_game_data.rs`,
  `crates/cbvault-cli/tests/cli_search.rs`, `crates/cbvault/benches/query.rs`,
  `docs/search.md`.
- Touched: `crates/cbvault/src/lib.rs` (`pub mod query`),
  `crates/cbvault-cli/src/main.rs` (the subcommand), `crates/cbvault/Cargo.toml`
  (the bench), README, CHANGELOG, `docs/provenance.md`.
- Additive API only. No new dependency; `gigachess` remains the only chess core.
