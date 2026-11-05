# Implementation tasks — small auditable steps

## 1. Source pin, license and baseline
- [x] 1.1 Record oschess tag `v1.0.3` -> annotated tag -> commit `bffebfc498d52b4e4631aed32b2f8e8194d125ff`; verify LICENSE/Cargo/workspace and file authorship at commit. List exact MIT modules, transitive dependencies, and unchanged earlier cbvault port provenance.
- [x] 1.2 Inventory cbvault `twocbh`, `Database` facade, gigachess `moves2`, `PgnWriter` and gold comparator; capture classic CBH, CBV/CBZ, throughput and allocation baselines.
- [x] 1.3 Write the correction table into `docs/format-spec-2cbh.md` with byte-level probes: checksum, tag vs content offsets, padding B, .2lid entity IDs, rating types; mark prior conflicting interpretations as superseded without deleting the original measured evidence.
- [x] 1.4 Establish synthetic 2CBH fixtures from MIT-eligible facts and generated non-user game data, including corrupted/truncated frames; no real database files in commits.

## 2. Phase 1 — MIT foundation and one chess core
- [x] 2.1 Port/reconstruct MIT v1.0.3 2CBH record/frame reader, checksum and proper tag, preserve notices; test exact offsets and bad checksums. No later AGPL code.
- [x] 2.2 Implement 16-bit v2 move-word enumeration and markers, setup positions/Chess960, promotions, en passant, null and variation stack; adapt decode to gigachess `moves2`, never import `chesscore`.
- [x] 2.3 Implement .2lid entity names, headers/metadata, typed record classification and version detection; optional .2lgd/.2lcd read-only use only where independent correctness warrants.
- [x] 2.4 Implement .2cba annotation-to-move mapping, NAGs/comments/clocks/evals, multilingual fields and explicit unsupported-value reporting.
- [x] 2.5 Wire 2CBH reader into current cbvault facade/CLI `info`, `verify`, `pgn`, `game_pgn`, sink/cancellation and ordered-parallel conversion; classic CBH stays untouched where possible.
- [x] 2.6 Test legal replay/PGN of generated positions, nested variations, Chess960, malformed frames, deleted records, move-less records and source mutation; no unknown record silently converted to a game.
- [x] 2.7 Update per-module `docs/provenance.md` ledger and MIT `THIRD_PARTY_NOTICES.md`, verify dependency tree contains no GPL/AGPL runtime and no second chess core.

## 3. Phase 2 — facts-only later AGPL and unlicensed Java improvements
- [x] 3.1 Build a **delta inventory**: specific files/behaviors from pinned MIT oschess to current AGPL oschess plus Morphy Java, with versions, observable input/output and known unsupported cases. Keep full source access restricted to Room 1.
- [x] 3.2 Room 1 writes **original factual** `docs/research/2cbh-after-mit-facts.md` describing tested layouts/behaviors, synthetic reproduction recipe, evidence and unknowns; exclude AGPL/Java code, pseudocode or copied tests.
- [x] 3.3 Independent hygiene/license reviewer approves/fixes the facts sheet and records implementation agent source exposure and spec hash; pause if isolation cannot be substantiated.
- [x] 3.4 Room 2 adds strictly necessary clean-room Rust corrections (one behavior per change), using **only** frozen facts + MIT baseline, with synthetic tests and typed failures where evidence is incomplete.
- [x] 3.5 Independent oracle evaluator compares our results with separate later AGPL/Morphy processes and ChessBase exports locally, returns discrepancies as factual tickets. Record provenance and required notices for every module.

## 4. Phase 3 — critical real 2CBH / ChessBase PGN gold gate (LOCAL ONLY)
- [x] 4.1 Obtain a legitimate **complete local 2CBH database** and a native ChessBase PGN export of the **same snapshot and same selected records**, using all available tags/variations/annotations. Set local-only paths through `CBVAULT_TEST_DB_2CBH` and `CBVAULT_2CBH_GOLD_PGN` and a local range/manifest; never upload or commit sources, exports, game fragments, personal names, screenshots or per-game diffs.
- [x] 4.2 Compare an extensive contiguous portion (target ≥400,000 games when available) plus independently selected beginning/middle/end and rare-game strata; prefer full database. Establish ID-to-PGN order mapping and an immutable source fingerprint. If fewer games, compare all and report limitations.
- [x] 4.3 Extend the existing `gold_pgn` methodology for 2CBH: report **raw** bytes/hash/first mismatch and strictly compare **canonical bytes** of the same comparable tags, complete move trees, comments, NAGs and results. Normalize only established layout/whitespace/tag-order conventions, and keep raw mismatch counts visible.
- [x] 4.4 Explicitly accept `ChessBase Z0` vs cbvault `--` null moves only after checking move context; preserve gigachess's **minimal legal SAN**: if a second knight is pinned and cannot legally reach d2, expect `Nd2`, not ChessBase's redundant `Nbd2`; require board-legality proof for every such exception.
- [x] 4.5 Reuse only independently proven classic-PGN deliberate exception rules; do not waive unknown tag/annotation/comment differences. Categorize each differing byte, and require **zero unexplained canonical differences**, **zero silent omissions** and **zero illegal/replay failures** in supported records.
- [x] 4.6 Cross-check exact record count, deleted/text/analysis counts, 960/setups/variations/annotations, ECO/ratings/dates/players against ChessBase; run exact sequential vs parallel byte-equality and source-change/malformed-data tests.
- [x] 4.7 Run the same CBH gold comparator (historical ADR-002 floor) and CBV/CBZ byte tests before and after; no regression in classic parsing or SAN output.
- [ ] 4.8 Benchmark on the real local database with paired Criterion, full export, profiling, peak memory and concurrent reads; report figures and environment **without private paths/content**.

## 5. Verification, documentation and release decision
- [x] 5.1 Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `openspec validate native-2cbh-readonly --strict` and other active specs' tests.
- [x] 5.2 Publish *only* sanitized aggregate result counts/provenance, exact supported format versions, unsupported annotation types, resource budgets and local-only test instructions. Any missing local ChessBase export is a **release blocker**, not a silently skipped success.
- [x] 5.3 Verify no real `.2cbh` companion, PGN, proprietary metadata, generated per-game diff or private file path is tracked, uploaded or attached to GitHub CI/logs.
- [ ] 5.4 Gate shipping on reviewer sign-off for MIT/AGPL provenance, zero unexplained canonical byte diffs, full expected game coverage and stable CBH/CBV regression suite; update baseline spec and archive OpenSpec only after implementation.
