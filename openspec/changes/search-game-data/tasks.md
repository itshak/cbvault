# Tasks

## 1. Implement

- [x] 1.1 `GameDataQuery` and its field types; text forms (`2020-2026.11`, `A00-E99/99`, `2500-3500`)
- [x] 1.2 Name folding and matching (prefix, contains, exact); name-to-id passes for classic and 2CBH, shared per namebase
- [x] 1.3 The compiled header predicate and the parallel header scan, ascending at any thread count
- [x] 1.4 The ending stage (mate, stalemate, check), Chess960 starts included
- [x] 1.5 `cbvault search`

## 2. Tests

- [x] 2.1 Every field alone and combined, on a classic and a 2CBH fixture; thread-count invariance
- [x] 2.2 A header-only query with `.cbg` hidden
- [x] 2.3 Refusals: wins only without a White line, endings on guiding texts, classic time control
- [x] 2.4 CLI: options, JSON, PGN output, bad options
- [ ] 2.5 Matching rules confirmed against ChessBase's counts on a test database (names, wins only, Elo modes with a missing rating, the difference, check vs mate)

## 3. Measurements

- [x] 3.1 Fixture gates against the bridge's scan (`benches/query.rs`)
- [x] 3.2 Whole-database report on Mega Database 2026 (2CBH), 1 and 10 threads

## 4. Documentation

- [x] 4.1 `docs/search.md`, README, CHANGELOG, provenance row
