# Tasks — consumer-bridge

> **Successor note (2026-11-05).** Phases 0, 1, 2, 3, and 5 have landed in
> cbvault (releases 0.1.0–0.1.6). Phase 3 (.cbv/.cbz archive containers and
> parity codecs) was completed clean-room in changes `cbv-cbz-completion` and
> `uncbv-clean-room-parity`. Phase 4 (2CBH read-only support) has been superseded
> and carved out into dedicated change `native-2cbh-readonly`.

Ordered by what unblocks the consumer soonest, not by layer. Every task ends
with its own verification; the gate set runs at each phase boundary.
**Commit prefix: `[consumer-bridge]`.** Task 0 assumes `rename-cbvault` has
landed (it uses the new names).

## Phase 0 — The façade (`Database`)

- [x] 0.1 `cbvault::Database::open(path)` accepting a base name or any member file, resolving siblings case-insensitively, validating the mandatory set and reporting the generation (`Classic` | `TwoCbh` | `Unknown`), the game count and which optional members were found. Typed errors, no panic, nothing left half-open. Verify: opens the reference set and the fixture sets; `Database::open` on a lone `.cbt` names the missing `.cbh` in a `MissingFile` error.
- [x] 0.2 `Database::headers()` — an iterator over `GameHeaderRef` plus resolved entity names, reading only `.cbh`/`.cbe`. Verify: a test asserts the moves file is never opened during a full list (instrument the `DbFile` open path); 11.1 M-record list under 2 s on the reference set.
- [x] 0.3 `Database::game(id)` — one game's header, `moves2` into a caller-owned `MovesBuf`, and its `GameAnnotations` on request. Verify: a game round-trips to the same `moves2` the conversion sink yields for that id.
- [x] 0.4 Land the probe that produced the ADR-005 §2 table as a permanent, documented benchmark (`benches/bridge.rs` plus a `keycost` example), and record the three passes in `benchmarks/baseline.json`. Verify: the numbers reproduce within the recorded range on a quiet machine.

## Phase 1 — Conversion (the deliverable)

- [x] 1.1 `GameRef<'a>` and `GameSink` exactly as designed: borrowed tags, resolved names, `moves: &[u16]`, `keys: &[u64]`, `wants_keys`, `wants_annotations`, `failed`. Verify: a sink that records nothing but a count allocates zero times in the hot path (asserted with a counting allocator in the test).
- [x] 1.2 `for_each_game(&Database, &mut sink)` — the sequential path, streaming, reusing one moves buffer and one keys buffer. Verify: converting 10,000 fixture games into a counting sink equals the record count, and no `.cba` byte is read when `wants_annotations` is false.
- [x] 1.3 `convert_parallel(&Database, &mut sink, threads, batch)` — ADR-004's shape: 8,192-record chunks, per-chunk private buffers, one ordered writer stage, recycled buffers, bounded in-flight waves. Verify: sink call order is identical to the sequential path for any thread count; a 419,385-game run is within 10 % of the sequential time on one thread.
- [x] 1.4 Keys in the same pass: `wants_keys` routes to the keyed walk; keys land in a caller-owned `Vec<u64>` cleared per game, never per ply. Verify: the indexed pass is within 15 % of the moves-only pass on the reference set, and a game's key sequence equals `replay_moves2_hashes` over the same `moves2` (the consumer's own primitive, as the oracle).
- [x] 1.5 Round-trip test with an in-memory target: convert N games, read them back, assert identical `moves2` blobs, tags and key sequences. This is the contract downstream software implements against, so it ships as a documented example (`examples/convert.rs`) with the sink written the way the consumer should write it.
- [x] 1.6 Annotations on request: the `.cba` record parsed and exposed as structured items; `wants_annotations == false` skips the file entirely. Verify: a fixture game's annotations match the PGN writer's placement (§8.1 of the format spec) for the same game.

## Phase 2 — Search over a raw `.cbh`

The measurement and the split are ADR-007: a tag search is a linear scan of a
513 MB file of 46-byte records — **8 ms warm at 10 threads, 28 ms at one** — so
it is answered here with predicate pushdown; position search is 883 M positions,
which is either ~5 s unindexed or the consumer's sidecar, so it is fed.

- [x] 2.1 `Entities::find_player` / `find_tournament` / `find_annotator` / `find_source` — name to id by binary search of the `.cbe` sorted tree, the one genuinely new parsing work in the search path. Verify: a known name resolves to the id `Entities::player(id)` returns it for, on the reference set and on fixtures; an unknown name returns `None` without touching the moves file.
- [x] 2.2 `Database::scan_headers(range, filter, out)` — bulk 8,192-record reads, a predicate evaluated in the scan, Rayon across chunks, results emitted in game-number order, entity names resolved for the hits only. Verify: the result set is identical to a sequential scan; 11,151,119 records filtered in under 50 ms warm at 10 threads; the moves file is never opened.
- [x] 2.3 An id-set filter for name predicates: a 256-shard bitmap over entity ids the consumer fills from its own name matching, then filters on integers. Verify: a tournament-substring query over the reference set returns the same ids as a resolving scan.
- [x] 2.4 Unindexed position search: `for_each_position_key(&Database, query_key, &mut hit_sink, &mut progress)` with `play_fast`, parallel, with progress and cancel hooks. Verify: ≥ 15 M plies/s single-threaded; a cancel stops within one chunk; the hit set equals the sidecar-backed search on a fixture where both exist.
- [x] 2.5 Adopt the gigachess 0.1.7 make variants in the walker: dispatch on `(wants_zobrist, wants_checkers)` to `play_hashed` / `play_checkered` / `play_fast` / `play`, and on the null move to the matching `make_null_move_*` — including `make_null_move_fast`, which fixes a stale-cache bug where a pass turn was validated against a `checkers` value the fast make had left behind.
- [x] 2.6 Publish the search numbers: cold first touch, warm scan at 1/2/4/10 threads, records/s and GB/s, name resolution per hit, and the unindexed replay rate. Record in `benchmarks/baseline.json` and `docs/bridge.md`. Verify: reproducible on a quiet machine, with the page-cache state stated, since a warm scan and a cold one differ by the file's worth of I/O.

## Phase 3 — Archives (moved from `bootstrap-cbh-parser` 5.1/5.2)

- [x] 3.1 Container, codec and full parity landed clean-room via `cbv-cbz-completion` and `uncbv-clean-room-parity`.
- [x] 3.2 `.cbz` DES decryption landed clean-room via `cbv-cbz-completion` and `uncbv-clean-room-parity`.
- [x] 3.3 Content probe: classified classic vs 2CBH.

## Phase 4 — 2CBH (Superseded by `native-2cbh-readonly`)

- [x] 4.1 Facts pass on 2CBH sets (superseded by `native-2cbh-readonly` phase 1/2).
- [x] 4.2 `twocbh` readers behind `Database` façade (superseded by `native-2cbh-readonly` phase 1).
- [x] 4.3 2CBH equivalence and gold PGN (superseded by `native-2cbh-readonly` phase 3).

## Phase 5 — CLI consolidation and hand-off

- [x] 5.1 `cbvault pgn` — a thin wrapper over the existing export (`info`, `verify`, `pgn`, `archive` only). `games` is removed; `info --games N` covers sampling.
- [x] 5.2 `cbvault archive list|extract --password` as a thin wrapper over `Archive`.
- [x] 5.3 Published bridge numbers and baseline benchmarks.
- [x] 5.4 Hand-off and documentation in `docs/bridge.md`.
- [x] 5.5 Final verification: `cargo fmt`, `cargo clippy`, `cargo test`.
