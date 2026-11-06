# Implementation checklist — synthetic implementation versus private release acceptance

**Public synthetic implementation: COMPLETE and cross-platform CI green.** Only stage 2 requires real licensed Mega Database data; no such corpus or private PGN was used or uploaded in this change. The active OpenSpec remains open until stage 2 strict parity and owner approval.

Checked implementation items mean merged code and generated-data regression coverage, **not** a passed private Mega Database golden test. A green public CI build never authorizes a claim of full 2CBH parity.

## 0. Record baseline
- [x] 0.1 Create a new active successor rather than overwrite the archived native-2cbh-readonly history.
- [x] 0.2 Preserve measured Mega Database 2026 facts and successful five-job CI for original baseline 07716d49; document one-corpus limitations.
- [x] 0.3 Strictly validate the active OpenSpec in Linux CI; retain its deltas as an active change until the private golden gate and owner approval, when they will be merged into the permanent capability spec during archive.

## 1. Implement NOW, using GENERATED data ONLY
- [x] 1.1 Unknown .2cba annotation kinds, truncated data and invalid framing return typed errors from the iterator; never None or silently stopped.
- [x] 1.2 V2 PgnWriter rejects annotation position/index/comment conversion errors; add regression proving a previously bare-PGN export now fails.
- [x] 1.3 Per-game, PGN serial/parallel, gold harness and bridge all propagate missing/corrupt annotation .2cba segment/frame instead of .ok() -> None; record game IDs and failures.
- [x] 1.4 V2 GameSink and Database::game_with with wants_annotations(true) supply correctly borrowed/typed native v2 annotations; inspect native-only payloads without pretending they are classic CBA.
- [x] 1.5 Explicit .2cbh/.2cbg/.2cba/.2lid paths select v2 despite same-stem classic siblings; ambiguous extensionless stem reports typed ambiguity rather than defaulting to classic.
- [x] 1.6 Remove v2-reachable panics in classic-only Database helpers; implement equivalent generation-aware public access or typed fallible alternatives.
- [x] 1.7 Implement correct v2 entity/tag scanning, range scanning, player/event filters, record-kind/deletion classification and non-panicking APIs.
- [x] 1.8 Implement v2 unindexed position-key search/ranges, Polyglot hashing, FEN/Chess960 starts, early stop, cancellation and ordered hits; compare generated results with classic.
- [x] 1.9 Implement truly multiworker bounded ordered 2CBH GameSink conversion; prove 1/2/4 workers give same IDs, moves2, keys, annotations and failures, including cancellation.
- [x] 1.10 Audit all public and CLI info/verify/game/search/convert/export functions for correct v2 dispatch and no silent feature loss.
- [x] 1.11 Implement fail-closed local strict gold executable with full tags, streaming PGN, source slot/order and EOF accounting, SHA-256, raw versus canonical byte equality and nonzero failure exit. **An independent one-to-one identity proof against the private ChessBase export belongs to 2.1–2.2; ordinal pairing alone is not sufficient proof.**
- [x] 1.12 Implement board-verified SAN overqualification and Z0/null exception adjudication with per-occurrence counts. For ChessBase-specific Unicode/PUA, diagrams, encoding and GUI clock output, **do not enable blanket allowances**: add only narrow, contextual, proof-backed and independently tested transforms in 2.3–2.4 when the private oracle demonstrates a genuine difference.
- [x] 1.13 Synthetic strict gold positives and negatives cover changes to tags, comments (including literal CRLF), NAGs, variations, results, Unicode, missing/extra/reordered/duplicate games, illegal SAN, SAN ambiguity and fabricated null; all unauthorized differences fail.
- [x] 1.14 Run generated malformed/unknown-frame fuzz and no-mmap regressions, build performance/benchmark targets, maintain classic CBH/CBV/CBZ tests and MIT provenance/privacy. **Paired full-corpus throughput, RSS and zero-allocation acceptance remain in private stage 2.5.**
- [x] 1.15 Complete fmt, all-target all-feature Clippy -D warnings, all-feature and strict-oracle example tests, synthetic 2CBH no-mmap integration tests, benchmark compilation, nightly fuzz, and all five CI jobs on main. Verified code commit `1bbdff9219503784add4bb39674c4f013da2b25f`, [five-green run 42183997583](https://github.com/itshak/cbvault/actions/runs/42183997583). No release/tag.

## 2. PRIVATE GOLD work ONLY for NEXT AGENT

The strict gold executable is intentionally expected to fail at first on unknown ChessBase-specific differences. Keep the real licensed corpus and private discrepancy logs off GitHub. **Stage 2—not an invented matching percentage—is the release blocker.**
- [ ] 2.1 Locally freeze Mega Database 2026 companion set and matching ChessBase 18 export; record source SHA-256, export settings and stable record IDs privately. No files/snippets/paths to GitHub.
- [ ] 2.2 Execute strict full 3,410,497-game comparator, raw and authorized transforms, complete source-ID matching, no unexpected bytes/failures/omissions; repeat for serial, multiworker and no-mmap modes as feasible.
- [ ] 2.3 Review and board-prove 94,138 previously *heuristic* SAN cases; classify each of 3,563 prior residual games by actual byte cause. No blanket “quirk” exemptions or 99.9% acceptance.
- [ ] 2.4 Where mismatch means real decoding/annotation bug, implement correct parser and synthetic regression, rerun the ENTIRE gold corpus; iterate to zero unapproved byte differences or report BLOCKED with exact class counts.
- [ ] 2.5 Run paired performance/RSS/zero-alloc and classic CBH/CBV/CBZ regression and final CI, produce sanitized aggregate proof and hand to owner for approval.
- [ ] 2.6 State **"Validated on Mega Database 2026 only"**, with Chess960 and rare native annotation/media/training/quotation real coverage clearly labeled as unverified when absent. Another real database is NOT a prerequisite.
- [ ] 2.7 Do not archive or publish until strict golden release gate passes and owner reviews report.

## 3. Explicit non-goals
- Native CBH/.2cbh file writing/editing.
- Requiring another licensed real v2 database.
- Running private Mega gold on a public runner.
- Version bump, crate publish, or downstream integration during this change.
