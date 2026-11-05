# Native ChessBase 2 (.2cbh): complete read parity with classic CBH, driven by real ChessBase gold

## Why and desired outcome

The goal is **not** merely to read ordinary .2cbh games or to get a synthetic CI run green. The goal is for cbvault's native **read-only** ChessBase 2 reader to match the **feature completeness, legality, metadata fidelity, and full annotation handling of its established classic .cbh reader**, and to support meaningful native 2CBH-only data as well. The empirical release gate is comparison with the **real ChessBase-produced PGN from the same locally held database**. All unexpected differences must drive improvements to the parser and shared PGN writer until canonical PGN is byte-identical, apart from our few proven, intentional, semantically equivalent notation policies.

CBH v2 is not just differently framed games. It carries .2lid entities, typed .2cba annotations, uncommon quoted games with embedded moves, training, multimedia, alternative setup positions, deep RAV and various metadata/record kinds. **Unsupported but valid content, even when the decoder returns a typed error, is an unfinished feature rather than a permissible final outcome.** Invalid/corrupt files must still return typed errors.

A PGN export does not retain everything ChessBase can store (for example some audio/video/images and other native objects). Therefore **two simultaneous acceptance gates** are required: (1) exact complete canonical PGN parity for everything ChessBase renders into PGN; (2) full read/typed lossless access and separately checked coverage for valid native-only data that PGN cannot express. No silent omission is acceptable in either path.

## Real current state — merged, not fully verified (November 5, 2026)

**PR #1** (framing, checksums, metadata offsets, reusable reads) and **PR #2** (MIT-based move word enumeration, .2lid reader, gigachess replay, native PGN writer, annotations, CLI, bounded ordered parallel exporter, shared wrapper and local gold diagnostic) are **merged to main**. The post-merge commit is 7497618983360ca4c0d083506c9b33a8e33be9fd. GitHub Actions CI run 42061315334 successfully completed **five jobs**: Linux, macOS, Windows, nightly fuzz and the one-chess-core check.

Implemented now:
- Verified v2 .2cbh header, .2cbg/.2cba framed record/tag/checksum, byte-level .2lid entity access, basic record classes, non-mmap scratch and mmap paths.
- 16-bit on-disk move-word translation and legality verification using gigachess only; setup and Chess960 paths; PGN output using the shared formatter; sequential and bounded ordered parallel export.
- A separate native cbvault::twocbh::Database with verify, per-game export and complete-range PGN, CLI info/verify/pgn routing; non-breaking bridge::AnyDatabase::open / open_any for shared generation, records and game_pgn; classic bridge::Database remains classic-only.
- Generated synthetic tests for actual legal move replay and stored-order variations, corrupted framing and entity names; a local-only example (crates/cbvault/examples/gold_2cbh.rs) that exports PGN and reports crude raw/canonical differences.

**NOT achieved or demonstrated:** real ChessBase PGN equality, accurate source-ID↔PGN-ordinal mapping, full rare .2cba annotation subtype support, quoted-game embedded move/setup payloads, all 2CBH native-only records and media, independently reviewed later-AGPL/Morphy format-delta clean-room corrections, full classic-style streaming GameSink/position/tag search on v2, and real-data performance/complete hot-path allocation proof. Real database fixtures and matching ChessBase export have NOT been provided to public CI.

**The current gold_2cbh example is a scaffold, not an authoritative gold gate:** it assumes ordinal correspondence, restricts tags to a short whitelist, computes FNV instead of SHA-256, has limited PGN syntax treatment, and does not prove null/SAN intentional differences on a board. Passing it alone is insufficient.

Read [implementation-status.md](implementation-status.md), [golden-parity-runbook.md](golden-parity-runbook.md), and [tasks.md](tasks.md) before further changes.

## Scope and acceptance-driven implementation

### 1 — Preserve and complete the pinned MIT foundation

Continue from the merged baseline. Use only the genuine MIT-tagged oschess-cb-bridge v1.0.3 (commit bffebfc498d52b4e4631aed32b2f8e8194d125ff) for authorized ports, keep source notices and the provenance ledger. Keep **gigachess as the only chess core**, do not add chesscore, shakmaty or a separate SAN engine. Retain classic CBH and CBV/CBZ byte behavior, default read-only access, deterministic ordered parallel output and allocation-aware hot paths.

### 2 — Repair the local real-data gold oracle, then iterate until FULL fidelity

The next agent must follow [golden-parity-runbook.md](golden-parity-runbook.md) **in sequence**, not simply execute existing tests and declare success.

- Obtain an authorized **complete local 2CBH companion set** and ChessBase-exported PGN of exactly the **same frozen snapshot**, same game selection, annotations, ordering and export settings. Use CBVAULT_TEST_DB_2CBH and CBVAULT_2CBH_GOLD_PGN only on the owner machine; keep paths, PGNs and private data offline. Compare whole database when possible; target at least 400,000 games if available; if fewer, compare ALL, plus representative rare-case corpora.
- Fix the comparator: raw SHA-256/byte/first-offset diagnostics, strict streaming canonical UTF-8 PGN parser, full tag/annotation/variation coverage, verified 1-based record IDs to exported-game mapping, zero skipped/ambiguous games, deterministic seq/parallel and mmap/non-mmap equality, explicit counts and negative tests. Preserve real-data privacy; GitHub Actions runs synthetic data only.
- Every unexplained canonical byte difference is a **bug to implement a correction for** in the reader or shared PGN formatter, followed by a generated regression. Never fix a mismatch by dropping a semantic tag, omitting annotations, excluding rare games, broadening string normalizations or labeling an entire annotation class “unsupported”. Iterate the **entire frozen gold corpus** after every group of fixes.
- Deliberately permitted exceptions are **narrow**: ChessBase Z0 vs our -- for the SAME legal null move, and ChessBase redundant file/rank SAN qualifiers vs our minimal SAN **only after gigachess proves the competing piece cannot legally move**. New exceptions require independent demonstrated semantic identity, documented necessity, named counters and review; no general “other” class.
- For native-only multimedia, training, images, game quotations and text/analysis data that ChessBase PGN does not encode, expose typed/lossless content through the read API, cross-check using a local non-PGN oracle/record inventory and keep a separate complete coverage ledger. A PGN comparison alone can never certify them.

### 3 — Close factual gaps legally

The live oschess repository is AGPL-3.0, and Morphy's Java code has no established compatible reuse license. Later source may be analyzed **only in a reviewed, room-isolated facts-only process**, not pasted into our MIT source or used as a direct coding reference by the implementer. Log researchers' source exposure and independent source/spec/test roles. Prefer reproducible ChessBase black-box outputs and original synthetic fixtures where possible. Preserve MIT attribution and obtain expert/license review before concluding legal hygiene.

### 4 — Reliability, performance, full regression and decision

For the exact final implementation, require:
- **100% of selected legitimate games and PGN-visible annotations**, zero unexplained canonical differences, zero dropped/duplicated/ambiguous source records, zero illegal moves and zero unsupported *valid* v2 annotation or native content forms.
- Separate verified typed/lossless native coverage for all binary-only features, with no silent losses. Source stays unchanged; source and gold fingerprints recorded privately.
- Exact byte identity between sequential/parallel and mmap/non-mmap exports for the same frozen game set; recovered malformed files fail safely; classic .cbh gold and .cbv/.cbz archive bytes unchanged.
- Strict fmt, Clippy (-D warnings), all-feature tests, benches, fuzz and green Linux/macOS/Windows GitHub Actions CI on the final head, plus strict OpenSpec validation and measured v2 performance/allocations on the real local dataset. A green synthetic CI alone does NOT pass the gold gate.
- Reviewer sign-off on annotated rare-case completeness, exceptions and provenance; only sanitized aggregate result counts may enter GitHub. **Do not archive this change or claim format parity until all gates pass.**

## Capabilities

### Modified capability: cbvault

Add a fully featured, safe, high-throughput read-only 2CBH generation with complete game/metadata/annotation/variation semantics, plus shared API parity with classic CBH wherever the application interface depends on it. Keep classic/CBV/CBZ behavior and the single gigachess rules engine. See the delta capability requirements in specs/cbvault/spec.md.

## Non-goals and the writing decision

**Writing or editing ChessBase .cbh or .2cbh databases remains explicitly OUT OF SCOPE** for this parity change. The reader must never modify proprietary source files. Our current cbvault contract is permanently read-only unless a separate, explicitly approved architecture/mission change is proposed.

A distinct future feasibility OpenSpec may examine a separate opt-in ChessBase writer or a managed editable destination, starting with **new-file creation and round-trip validation** rather than editing existing databases. Morphy has Java write APIs for CBH and 2CBH, but its source is not verified as MIT-compatible, and its own tests disclose gaps in native quote/training annotation round trips. Do not port it. No writer capability is necessary to make this reader complete.

No AGPL/Java code copying; no ChessBase binary modifications; no distribution of proprietary actual datasets; no second chess core; no change to downstream UI/MCP as part of the file-format gate.

## Sources and related material

- [MIT oschess tag v1.0.3](https://github.com/asavis/oschess-cb-bridge/tree/bffebfc498d52b4e4631aed32b2f8e8194d125ff) — the only permitted port baseline.
- [Current oschess project](https://github.com/asavis/oschess-cb-bridge) — AGPL reader, later facts only through clean room.
- [Morphy](https://github.com/Yarin78/morphy) — Java CBH and 2CBH read/write reference, facts/oracle only unless separately licensed.
- [ChessBase 18 help: 2CBH](https://help.chessbase.com/CBase/18/Eng/new_data_format_2cbh.htm).
- AGENTS.md; ADR-001/002/003/004/005/007; classic gold_pgn.rs; docs/format-spec-2cbh.md; docs/provenance.md; THIRD_PARTY_NOTICES.md.
