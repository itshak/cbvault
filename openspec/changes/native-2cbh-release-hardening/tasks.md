# Implementation checklist — all unverified work remains OPEN

## 0. Record baseline
- [x] 0.1 Create a new active successor rather than overwrite the archived native-2cbh-readonly history.
- [x] 0.2 Preserve measured Mega Database 2026 facts and successful five-job CI for original baseline 07716d49; document one-corpus limitations.
- [ ] 0.3 Strictly validate this OpenSpec and reconcile permanent capability spec after implementation.

## 1. Implement NOW, using GENERATED data ONLY
- [ ] 1.1 Unknown .2cba annotation kinds, truncated data and invalid framing return typed errors from the iterator; never None or silently stopped.
- [ ] 1.2 V2 PgnWriter rejects annotation position/index/comment conversion errors; add regression proving a previously bare-PGN export now fails.
- [ ] 1.3 Per-game, PGN serial/parallel, gold harness and bridge all propagate missing/corrupt annotation .2cba segment/frame instead of .ok() -> None; record game IDs and failures.
- [ ] 1.4 V2 GameSink and Database::game_with with wants_annotations(true) supply correctly borrowed/typed native v2 annotations; inspect native-only payloads without pretending they are classic CBA.
- [ ] 1.5 Explicit .2cbh/.2cbg/.2cba/.2lid paths select v2 despite same-stem classic siblings; ambiguous extensionless stem reports typed ambiguity rather than defaulting to classic.
- [ ] 1.6 Remove v2-reachable panics in classic-only Database helpers; implement equivalent generation-aware public access or typed fallible alternatives.
- [ ] 1.7 Implement correct v2 entity/tag scanning, range scanning, player/event filters, record-kind/deletion classification and non-panicking APIs.
- [ ] 1.8 Implement v2 unindexed position-key search/ranges, Polyglot hashing, FEN/Chess960 starts, early stop, cancellation and ordered hits; compare generated results with classic.
- [ ] 1.9 Implement truly multiworker bounded ordered 2CBH GameSink conversion; prove 1/2/4 workers give same IDs, moves2, keys, annotations and failures, including cancellation.
- [ ] 1.10 Audit all public and CLI info/verify/game/search/convert/export functions for correct v2 dispatch and no silent feature loss.
- [ ] 1.11 Implement a real strict local gold executable: complete tags and streaming PGN tokenization, source ID mapping and EOF, SHA-256, raw and approved-transformation byte equality, nonzero failure exit.
- [ ] 1.12 SAN qualifier and Z0 exception **programmatically legally proved per board**, with explicit per-occurrence counts. Named contextual ChessBase Unicode/PUA, diagram, legacy encoding and clock transformations tested; reject generic text allowances.
- [ ] 1.13 Negative synthetic gold tests for one tag/comment/NAG/variation/result/Unicode changed byte, missing/extra/reordered/duplicate games and non-legal SAN/null exception; assert fail.
- [ ] 1.14 Fuzz malformed/unknown native frames, no-mmap and allocation/throughput regression, preserve CBH/CBV/CBZ floors, MIT provenance and privacy.
- [ ] 1.15 Run final fmt, all-target all-feature Clippy -D warnings, all-feature tests, benchmark compilation, fuzz and five GitHub CI jobs on main. Document final exact SHAs; no release/tag.

## 2. PRIVATE GOLD work ONLY for NEXT AGENT
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
