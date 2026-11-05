# Native 2CBH parity: work checklist and release blocker

**Source of truth:** [proposal.md](proposal.md) sets **complete read-feature parity with classic CBH v1** as the goal. [golden-parity-runbook.md](golden-parity-runbook.md) contains the exact local-data agent procedure. [implementation-status.md](implementation-status.md) distinguishes current implementation from unverified features. A checkbox is complete only with code/tests/evidence; no valid rare annotation may remain unsupported when this change is closed.

**Merged baseline:** PR #1 and PR #2 are merged into main at 7497618983360ca4c0d083506c9b33a8e33be9fd; GitHub Actions run 42061315334 has all five jobs green (Linux, macOS, Windows, fuzz, one chess core). These are **synthetic test results, not a ChessBase gold pass**.

## 1. Foundation completed (merged main)

- [x] 1.1 Correct .2cbg/.2cba frame/checksum, real 2-byte tag and content offsets, spare padding B, malformed-input checks and synthetic framing tests.
- [x] 1.2 Decode 192-byte .2cbh header layout, game/text/analysis/deleted flags and 2LID entity identifiers; remove false inline-player interpretation.
- [x] 1.3 Implement .2lid interleaved registry and names, MIT v1.0.3-pinned v2 move-word enumeration, gigachess-only legal replay, setups and variation decoding paths.
- [x] 1.4 Implement typed .2cba annotation items with fail-closed unknown variants and shared PGN writer, native cbvault::twocbh::Database API, CLI info/verify/pgn and bounded ordered parallel export.
- [x] 1.5 Add bridge::AnyDatabase::open / bridge::open_any for common generation, record count and per-game PGN without altering the established classic-only bridge::Database.
- [x] 1.6 Add synthetic fixtures, legal non-empty game replay and stored-order variation tests, frame corruption tests, no-mmap frame allocation tests and classic regression CI.
- [x] 1.7 Add a local-only gold_2cbh.rs diagnostic example and an initial OpenSpec implementation checkpoint; keep all private real files out of source control.
- [x] 1.8 Preserve upstream MIT attribution and document provenance of the implemented v2 foundation; verify CI rejects a second chess rules engine.
- [x] 1.9 Verify post-merge CI on main (run 42061315334, five green jobs).
- [ ] 1.10 Independent final legal/source-exposure review of all reused format knowledge and complete dependency licenses (source audit is **not** established by green CI).
- [ ] 1.11 Measure complete **end-to-end** v2 heap allocations/throughput/peak RSS on an actual large database and compare with classic CBH floor; synthetic framing tests alone are insufficient.

## 2. First things the NEXT AGENT must do (before trusting gold)

- [ ] 2.1 Read the runbook, AGENTS.md, design/specs, ADRs, gold_pgn.rs and gold_2cbh.rs; verify actual main head, toolchain and code paths before any edits.
- [ ] 2.2 Create a complete item-by-item **classic CBH v1 vs 2CBH v2 feature matrix**, with status implemented / synthetic-covered / real-verified / unsupported valid / unexplored. Cover every annotation type, setup, variation, media, metadata, source sidecar and non-game record.
- [ ] 2.3 Fix existing gold_2cbh.rs: source-ID↔ChessBase-export order **mapping** with explicit deleted/text/analysis handling, duplicate disambiguation and full denominator; never assume selected game ID == PGN ordinal.
- [ ] 2.4 Upgrade the gold output to **raw SHA-256** of both PGNs and source companion fingerprints, raw byte counts/first mismatch, stable canonical UTF-8 bytes and first canonical mismatch. Current FNV-1a is not sufficient.
- [ ] 2.5 Replace limited 13-tag KEEP whitelist / simple splitting with **complete streaming PGN parsing and documented, reviewed canonicalization** (Unicode/charset, escaping, comments, PGN move numbers, RAV, NAGs, all meaningful tags, results and text directives). Detect missing/extra games and bad exports; never silently drop content.
- [ ] 2.6 Add strict adversarial synthetic tests: a one-byte comment or NAG difference, changed annotation attachment, Unicode/date/rating/tag, variant branch/result, missing/reordered or duplicate games, and an invalid SAN/null exception **must fail**.
- [ ] 2.7 Encode the only existing intentional formatting rules with game-position proof: ChessBase Z0 versus -- null moves and pinned-piece redundant SAN versus minimal legal gigachess SAN; log both exception counts. No generic “tag difference/annotation difference/other” waivers.
- [ ] 2.8 Ensure detailed gold logs contain **no real PGN, player info, source paths or private snippets in GitHub/CI/tool transcripts**. Public output is only sanitized counts/status; detailed per-game diffs remain local/untracked.

## 3. Local authorized **real database + ChessBase-produced PGN** (critical release gate)

- [ ] 3.1 Obtain authorized complete .2cbh/.2cbg/.2lid/.2cba and relevant other companion files locally, with a snapshot frozen read-only. Record private SHA-256 of each source and export settings/version.
- [ ] 3.2 Export exactly the same record selection from actual ChessBase as full PGN, with all available tags/comments/RAV/NAGs/languages and a recorded stable sort/order. Re-hash source after export to detect mutation. Set CBVAULT_TEST_DB_2CBH and CBVAULT_2CBH_GOLD_PGN locally.
- [ ] 3.3 Verify game-ID mapping one-to-one, including deleted/text/analysis/moveless/non-game slots. Fail if any game missing, duplicated, reordered or ambiguously identified.
- [ ] 3.4 Compare preferably **100%** of the selected database; when >=400,000 games exist cover at least a contiguous 400,000 plus first/middle/end and targeted rare-case strata. When fewer exist, compare **all** and disclose total. Add a separate corpus as required for rare features.
- [ ] 3.5 Run current diagnostic initially with the actual env vars; subsequently run the **upgraded strict comparator**. Record raw and canonical byte difference counts/categories, verified legal moves, every annotation type, complete denominator, errors and proven exception counts.
- [ ] 3.6 Verify complete coverage of every annotation item, nested variation, Chess960/setup, metadata type, game end/trailing move marker, text/analysis and unusual records; supplement missing real cases with generated synthetic/regression fixtures.
- [ ] 3.7 Audit native-only content that **does not appear in ChessBase PGN**: media/pictures/audio/video, rich text/training/questions, quoted games and any unsupported item; preserve typed/borrowed/raw lossless information and check via an independent local oracle/byte-level inventory. PGN equality by itself MUST NOT mark these complete.
- [ ] 3.8 Re-run entire gold corpus after each cluster of changes until **zero unexplained canonical bytes, zero valid-data unsupported items, zero illegal moves, zero silent omissions, zero ambiguous mapping**. Any reported unsupported *valid* variant blocks closure, even if all other games match.

## 4. Implement every missing valid-data feature found by stage 3

- [ ] 4.1 Decode **all valid .2cba annotation layouts**, multilingual comments, glyphs/NAGs, diagram/numeric/time/eval/clock/training/graphics/media representations with exact move/variation attachment and escaping; preserve unknown-but-valid raw representation pending full decoder.
- [ ] 4.2 Support **quotes with embedded full move lists and setup positions**, quote chains and unusual placements. Existing partial game/annotations/quote.rs rejects these: eliminate this as a valid-content limitation and add generated fixtures.
- [ ] 4.3 Cover every real Chess960/start/setup/underpromotion/null/en passant/castling/deep RAV and v2 end-of-game/trailing-move case, validated on gigachess board and exported with correct SAN and PGN topology.
- [ ] 4.4 Decode all meaningful 2LID/2LGD/2LCD variants and metadata where needed: Unicode names, teams, events, site/source, dates, Elo rating *type*, rounds/board, beauty/flags, text/analysis records and all other observed fields; do not invent missing data.
- [ ] 4.5 Bring v2 metadata/header streaming, read-only browsing, search, position-query, GameSink/cancellation and per-game views to the **same consumer-facing capability** as classic CBH, using common API or clearly documented native v2 equivalents without regressing classic performance. Any scope tradeoff requires explicit review and cannot be called “CBH parity.”
- [ ] 4.6 For each behavior: produce failing original synthetic fixture + provenance record, implement decoder/PGN fix, add positive/negative/regression tests, rerun full local oracle and classic gold. Do not relax tests or source coverage.
- [ ] 4.7 If gaps require post-MIT AGPL oschess or Morphy Java **source** facts, execute separate Room 1 research, independent hygiene/legal audit, frozen original factual spec, independent Room 2 implementation and Room 3 black-box validation. Log source exposure and artifact checksums; no source copying.

## 5. Final verification (not checked until genuinely performed)

- [ ] 5.1 Compare same-source sequential vs bounded parallel output byte-for-byte for 1, 2, 4 and available N threads, mmap on/off, all selected ranges and full database; source hashes unchanged and output deterministic.
- [ ] 5.2 Run classic CBH real gold (ADR-002 floor), CBV/CBZ extraction hash/oracle tests, zero-alloc hot path benchmarks, fuzz for all v2 boundaries and Linux/macOS/Windows integration tests. No classic regression or unmeasured v2 performance degradation.
- [ ] 5.3 Run exact final-tree cargo fmt --all --check, cargo clippy --workspace --all-targets --all-features -- -D warnings, cargo test --workspace --all-features, cargo check --benches --all-features, no-mmap allocation tests, openspec validate native-2cbh-readonly --strict and all five GitHub Actions jobs.
- [ ] 5.4 Produce a sanitized aggregate proof: total source records and games, per-feature/annotation counts, exact matches, each proven deliberate difference, **zero** unexplained mismatches/unsupported valid data/illegal moves/silent loss, source snapshot and privacy audit, real-workload throughput/memory and legal sign-off.
- [ ] 5.5 Review every remaining unchecked item; any uncompleted **valid-content support** item blocks parity. Update capability spec/docs, classify native-only objects, acquire reviewer sign-off, **then** archive OpenSpec. No “99.9%” pass.

## Separate future decision: writing CBH / 2CBH

- [ ] W.1 If native editing becomes a product priority, create a **separate** OpenSpec for a writer feasibility spike: independent read/write module, new-file creation first, ChessBase-readability verification, crash-safe persistence, entity/index updates and every annotation byte. Do **not** make the read-only v2 parity gate depend on writing or use Morphy's unlicensed Java code as directly reusable code.
