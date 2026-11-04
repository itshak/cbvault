# Design: native 2CBH reader and byte-grounded acceptance

## Decisions, source provenance, license barrier

| Source | Status | Permitted use | Hard boundary |
|---|---|---|---|
| `oschess-cb-bridge` v1.0.3 @ `bffebfc498d52b4e4631aed32b2f8e8194d125ff` | **MIT** at pinned commit; verify every ported file and dependency | Port under MIT with attribution, independently adapt to cbvault + gigachess | Pin files by commit, no unreviewed updates to main |
| Current `oschess-cb-bridge` | **AGPL-3.0-only** workspace (2026-11-05) | Factual gap research in Room 1; separate-process output oracle where locally allowed | No source/text/algorithm-code/structure pasted into Room 2 |
| `Yarin78/morphy` `format/v2`, `morphy-cb2` | No verified reuse license | Source of format *facts* and behavioral oracle, reviewed spec only | No copied Java code, comments, test arrays or substantial documentation |
| ChessBase's own PGN export | Local user-generated reference output | Strongest gold oracle for exact user's database | Never uploaded or committed; no ChessBase code reverse-engineering required |
| cbvault `docs/format-spec-2cbh.md` | Original measured observations, some *wrong field interpretations* | Retain reproduced evidence and reconcile disputed claims | Corrections require reproducible failing/agreeing records and changelog |

Legal review is required before asserting a strict clean-room defense or shipping a copy-pasted MIT subsystem with uncertain provenance. License of the later AGPL project **does not revoke** the already-granted MIT permission for code genuinely included in the pinned version.

## Phase 1: port architecture, not a second chess framework

The partial `twocbh` module already has `header`, `record`, `moves`, `annotations`, `segment`, `testdata`. Preserve its public neutral types where possible, but correct incorrect decoding rules. Use relevant MIT v1.0.3 components (`v2::{frame,record,moves,entities,annotations,window}`, `movetable/**`, `replay/**` only insofar as facts/port permitted) to build the 2CBH reader; port API-level logic while replacing `chesscore` entirely with `gigachess`. Already existing `cbvault` `GameSink`/borrowed `GameRef`, `moves2`, board caches and `PgnWriter` remain authoritative.

Verified correction checklist (compare read bytes with pinned MIT and Morphy specification; do not assert until unit fixtures pass):

- `.2cbh`: 192-byte header + 192-byte game/analysis/text records; offsets `0x08` move frame and `0x10` annotation frame; game references at `0x18` and `0x20` point to **player entities in .2lid**, not inline player names. `0x68` and `0x78` are part of rating-type fields, not player text.
- `.2cbg`/`.2cba`: framing magic `88 77 66 55 44 33 22 11`; 4-byte `A` content length, 4-byte `B` spare capacity; checksum **big-endian** at `0x10`; tag **2 bytes** at `0x18`; content begins at **`0x1a`**; trailer length = `A+B+34`. Existing local parser has interpreted tag as opaque 8 bytes and included tag in content; fix.
- `.2cbg` payload is 16-bit **little-endian move word stream** with markers `fffa` null, `fffb` setup, `fffc` moves, `fffd` alternatives, `ffff` line end; reconstruct tree and enforce legal move and capture correctness using gigachess.
- `.2lid` is the entity registry; `.2lgd`/ `.2lcd` are search/sort indexes. For a first read-only reader, prefer dependable full/filtered scans and build cbvault/BlindBase-owned caches rather than modifying external indexes. Preserve 1-based game IDs; classify guiding texts, analyses, deleted placeholders and mover-less games.
- Stage Chess960, promotions, en passant, null moves, positions with setup FEN, annotation attachment, Unicode entity names, locale labels and nontrivial variation trees in contract fixtures.

**Exit gate:** standalone library opens generated and real 2CBH read-only, `verify` checks all moves for legality, GUI-independent `Database` reports games/headers and emits PGN in the same sink interface, no memory leak/panic on malformed input, classic CBH/CBV tests and gold floor unchanged. Upstream `chesscore` is absent from runtime dependency tree. Ported files retain MIT notices and `docs/provenance.md` rows.

## Phase 2: formal two-room clean-room factual improvements

1. **Baseline:** inventory the precise source tree at pinned MIT tag; compare committed trees (not just RELEASE notes) against later AGPL releases and Morphy's Java writer/reader. Enumerate behavioral gaps: larger header variants, malformed records, entity completeness, move/capture metadata, commentary languages, time controls, engine evaluations, historical Chess960/setups, deleted records, giant DB behavior, correctness/performance fixes.
2. **Room 1, researcher/specifier:** may inspect AGPL/Morphy code *only in a separate isolated work context*. Writes an original factual delta document `docs/research/2cbh-after-mit-facts.md` referencing tagged versions and reproducible black-box experiments, field width/offset/value tables, fixture-generation instructions, expected observable outcomes and uncertainties. No GPL/Java code, pseudocode mirroring original expression, copied tables, original identifiers or unique literal prose goes into that spec. Redact real game records and paths.
3. **Hygiene/legal review:** independently validate each spec claim against synthetic files or local ChessBase/oracle outputs. Freeze reviewed facts doc and record hash/version, who/what saw which sources. A two-agent workflow is **not** automatically independent; enforce separate directories, contexts, task assignments and CI permission boundaries.
4. **Room 2, implementer:** only sees cbvault's MIT baseline, approved fact-specification, and synthetic fixtures. Cannot read later AGPL/Morphy source or access research session logs. Implements minimal independent Rust changes with gigachess and cbvault patterns.
5. **Room 3, tester/reviewer:** executes later AGPL/Morphy *only as separate oracles*, compares resulting outputs to ChessBase/our reader. Any discovered mismatch returns to Room 1 as a *factual requirement*, not a source patch.
6. **Provenance:** update original/ported/clean-room module ledger and notices; don't retroactively label older MIT-port code "clean-room". If separation/provenance cannot be demonstrated, defer the feature or obtain a compatible license/permission.

**No AGPL-to-MIT relicensing, guessed encoding or copied Java snippets.** If a behavior cannot be implemented from verified facts alone, maintain a typed Unsupported result or loss report. Only testable deltas can be merged.

## Phase 3: LOCAL-ONLY ChessBase PGN differential test — release blocker

### Fixture/rights constraints

- Use a real user-owned complete 2CBH **set of companion files** on the developer's computer. Set an explicit local `CBVAULT_2CBH_TEST_DB` path and `CBVAULT_2CBH_GOLD_PGN` to ChessBase's own PGN export; optionally a machine-private `CBVAULT_2CBH_GOLD_MANIFEST` recording selected game IDs, export settings, database fingerprint, versions and ordering.
- Test contiguous first 400,000 or more games when available, **plus** stratified samples across the full database (first/middle/end; annotated, variations, Chess960, setup FEN, promotions, underpromotions, null moves, corrupted/edited/deleted entries). Prefer the full database if feasible. If fewer records exist, compare 100% and state sample limit; do not extrapolate counts from a small fixture.
- Export ChessBase and cbvault **from the same snapshot/revision**, choosing fields/annotations and language consistently. The PGN ordering is not assumed to map 1:1 with raw game ID because guiding texts, analyses, deletions and filtered games exist: create verified ID mapping and fail on ambiguous or missing mapping.
- Keep the database and resulting PGNs, per-game diffs, content hashes, disassembly/oracle files, absolute local paths, usernames and personal metadata out of the repo, CI, GitHub Actions artifacts, issues and external LLM tool contexts. Public CI generates synthetic chess records with dummy names from an in-repo fixture builder only. Local comparison tooling must avoid printing names/PGN/paths in shareable summary output; use IDs, counts, hashes and discrepancy classification. No real chess content is uploaded to API-hosted AI models.

### Three levels of equality

**Level 0, raw bytes:** record full raw SHA-256, total byte count and first mismatching byte offset for paired outputs with identical export settings, but do not assert raw-file parity when exporter line wrapping, tag order or ChessBase-only tags differ.

**Level 1, canonical-byte parity (primary gate):** reuse/refactor `crates/cbvault/examples/gold_pgn.rs`: parse PGNs per game, preserve all comparable standard tags (Seven Tag Roster + ECO/WhiteElo/BlackElo/SetUp/FEN/Variant as in classic), move sequence, variation parentheses, NAGs, comments, positions, results; normalize **only** documented nonsemantic serialization differences (line endings, wrapping/whitespace, tag ordering, ChessBase-only tags explicitly omitted under classic rule). Re-serialize the canonical records to stable UTF-8 bytes and compare **byte-for-byte**. Every unmatched byte is reported by game ID, category and localized offset, not ignored by similarity scoring.

**Level 2, justified exceptions:** category `NULL_SPELLING` maps ChessBase `Z0` to cbvault `--`, preserving exact move/side context and no invalid check/mate suffix; category `CHESSBASE_OVER_DISAMBIGUATION` allows a redundant SAN file/rank qualifier **only where gigachess confirms the other same-type piece's move is illegal** (e.g., pinned knight, only legal `Nd2`, ChessBase prints `Nbd2`). Do not simply accept every string shape `Nbd2` vs `Nd2`; verify actual board and candidate legality. Classic comparator's other *documented intentional* tag/comment differences may be separately allowlisted by exact rule and evidence, not blanket "annotation discrepancies". Semantic PGN tags, side-to-move, ratings, moves, comments, ECO and annotation attachment remain strict. Check/mate suffix normalization is acceptable only if verified as a formatting policy; never conceal incorrect board legality or check state.

Release requires:
- **0 unclassified canonical mismatches**; each known exception counted by kind with reproducible evidence and threshold regression tracked; no “we matched 97% therefore passed”.
- **0 illegal moves, 0 silently skipped games or annotations, 0 unreported source changes** in the declared supported subset. Unsupported special record types are explicitly excluded with a count and rationale before comparison.
- Sequential and ordered-parallel export produce identical bytes, full game counts and parseable PGNs; metadata and annotations are compared, not only mainline SAN.
- Classic `.cbh` gold baseline **must not regress** (ADR-002: at earlier measurement 407,350 exact / 11,977 deliberate / 50 annotation / 3 other / 5 tags, 0 read errors / 0 decode errors). That historic floor is **not** permission to treat new unexplained 2CBH differences as acceptable.
- Produce a local summary with denominator/coverage, exact count, deliberate-validated count, illegal/decode failures, annotations/metadata mismatch buckets, exit status; only aggregate redacted results may be copied to the repo manually after rights review.

### Performance

Use ADR-001/002: zero per-game/per-move allocations in hot path where practical, borrowed headers/buffer reuse, worker-local gigachess board/cache state, ordered streaming batches, bounded memory. Benchmark with paired old/new Criterion, whole-database export throughput, fresh profile, cold/hot indexed reads, heap and mmap memory, parallel equality and realistic local I/O limits. **No unmeasured speed promises**; define acceptable regression against classic path and exact new 2CBH workload before the performance gate, with sample-size and system load recorded.

## Additional release safeguards

- No change to classic CBH/CBV/CBZ behavior (including archive extraction byte hashes), CLI flags or PGN formatting except feature detection and format selection.
- Explicit typed errors for unsupported v2 versions, damaged checksums, short/truncated/overlapping frames, invalid entity IDs, illegal move/capture, malformed variations and source changed during read; no guessing or null fallback.
- Read-only file permissions: never create .2cbh family indexes or write to ChessBase folders. Any derived position index goes in the consumer-controlled cache, not beside source files.
- Validate with `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `openspec validate native-2cbh-readonly --strict`; public CI only synthetic; local gold runs recorded separately. Manual application consumption is later BlindBase work.
