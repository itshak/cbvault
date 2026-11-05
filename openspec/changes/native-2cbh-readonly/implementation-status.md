# Native 2CBH implementation checkpoint (2026-11-05)

**Integration status:** read-only native .2cbh reader, legality-checked replay and PGN output implemented on the 2CBH feature branch, with synthetic cross-platform CI. This is **not** a claim of complete ChessBase format equivalence. Do not close/archive this OpenSpec or advertise guaranteed byte parity before the user's private local gold gate.

## Implemented and unit/integration-tested

- Header record/offset parsing, framed .2cbg and .2cba checksum/tag verification, sidecar size checks, record classification, deleted/text/analysis filtering.
- Interleaved .2lid entity registry and player/tournament lookups, borrowed mmap reads and capacity-reusing non-mmap buffers.
- MIT-pinned 2CBH move-word enumeration; checked translation to gigachess, not another chess rules engine. Setup, Chess960 start, promotion, castling, en passant, null markers and replay/variation stack have code paths; synthetic fixture coverage varies by feature.
- Borrowed .2cba typed annotation index; shared comment formatting. Unknown annotation layouts and embedded quoted games whose moves cannot be represented produce errors instead of silently omitted PGN.
- `cbvault::twocbh::Database` public native read-only API: `open`, `record`, `verify`, `write_game`, `write_game_reuse`, `export`, `export_parallel`. The 2CBH CLI `info`, `verify`, `pgn` dispatch through this API. Parallel output uses bounded worker-owned PGN buffers and deterministic ordered emission.
- Generated legal-move and variation PGN integration tests, corruption tests, check/Clippy/test/fmt/bench/fuzz platform CI, no-mmap frame allocation tests. The source code remains MIT-only and does not link chesscore/shakmaty.

## Remaining non-private engineering/coverage risks

1. **The established `bridge::Database` remains classic-only for API compatibility.** A new `bridge::AnyDatabase::open` / `bridge::open_any` handles both generations and provides common record-count / `game_pgn` access; the native v2 facade remains `cbvault::twocbh::Database`. Classic-style `GameSink`, positional search and full tag-search semantics for native 2CBH are not yet equivalent. The CLI dispatches explicit v2 input.
2. **Known unsupported content:** some embedded quoted-game move lists and unknown 2CBA annotation layouts are rejected. Rare setup/Chess960 and annotation variants need the complete generated coverage and/or real-file evidence mandated by tasks 2.4/2.6. No claim of silent-loss-free *all record types* before the real gold gate.
3. **Clean-room later-AGPL/Morphy delta** remains a separate review protocol (Phase 2); neither an independent audit nor code derivations from the later AGPL/unlicensed sources have occurred in this implementation branch. If differential evidence is required, run this with separate implementer/oracle roles and record source-exposure review. Do not assert a legally validated clean room without evidence.
4. **Performance:** warm v2 decoding and bounded export are implemented, not measured against a large actual v2 database for throughput or true end-to-end allocations. The protected real-file performance gate still awaits a private local dataset.

## Prepared local-only gold test

The repository contains `crates/cbvault/examples/gold_2cbh.rs`, a synthetic-tested **local-only** comparator. It reports raw-byte sizes, FNV-1a fingerprints and first raw mismatch offset separately from game-by-game canonical comparisons with permitted formatting normalization. It does **not** waive SAN/null differences or unrecognized annotation discrepancies. No real database or PGN is shipped.

```sh
export CBVAULT_TEST_DB_2CBH="/private/path/chess-database-stem"
export CBVAULT_2CBH_GOLD_PGN="/private/path/export-from-chessbase.pgn"
cargo run -p cbvault --release --example gold_2cbh -- --threads 4
```

Both inputs must represent the **same unchanged database snapshot**, in matching record order, including all selected games and with full annotations/variations in the ChessBase export. The example exports games into a temporary local PGN and deletes it on exit. Its summary prints counts and differing game ordinals only, not real player names or raw PGN fragments. A cryptographic source fingerprint and the detailed known-exception adjudication are further work for the actual acceptance run.

The release criteria are **zero missing games**, **zero unexplained canonical mismatches**, **no silently discarded content**, full source-to-PGN record mapping, classic CBH/CBV regression preservation, and measured v2 throughput/allocations. Failing gold comparison is a valid result; do not merge/ship as *complete and verified* merely because CI is green.
