# Implementation checklist — synthetic implementation versus private release acceptance

**ARCHIVED / ACCEPTED 2026-11-10.** The owner accepted the 3,410,497-game ChessBase 18 exported sample as adequate for this release-hardening gate. The private agent reported zero unexplained canonical mismatches and main commit `70b61c8` passed all five CI jobs. No licensed source or PGN was uploaded. The 8,330,763 later eligible games were header-classified, NOT PGN-compared.

Checked stage-1 items mean merged generated-data regressions. Stage 2 is the private agent-reported sampled gold PASS. Public CI does not reproduce licensed ChessBase comparison or prove universal 2CBH parity.

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

## 2. PRIVATE GOLD sample acceptance — completed / reported PASS

The frozen 3,410,497-game ChessBase 18 export passed at `70b61c8` after counted, documented canonical exceptions; raw bytes remain different. The later source games were not gold-compared. Preserve the original task records for audit; keep licensed data off GitHub.
- [x] 2.1 Locally freeze Mega Database 2026 companion set and matching ChessBase 18 export; record source SHA-256, export settings and stable record IDs privately. No files/snippets/paths to GitHub.
- [x] 2.2 Execute strict full 3,410,497-game comparator, raw and authorized transforms, complete source-ID matching, no unexpected bytes/failures/omissions; repeat for serial, multiworker and no-mmap modes as feasible.
- [x] 2.3 Review and board-prove 94,138 previously *heuristic* SAN cases; classify each of 3,563 prior residual games by actual byte cause. No blanket “quirk” exemptions or 99.9% acceptance.
- [x] 2.4 Where mismatch means real decoding/annotation bug, implement correct parser and synthetic regression, rerun the ENTIRE gold corpus; iterate to zero unapproved byte differences or report BLOCKED with exact class counts.
- [x] 2.5 Run paired performance/RSS/zero-alloc and classic CBH/CBV/CBZ regression and final CI, produce sanitized aggregate proof and hand to owner for approval.
- [x] 2.6 State **"Validated on Mega Database 2026 only"**, with Chess960 and rare native annotation/media/training/quotation real coverage clearly labeled as unverified when absent. Another real database is NOT a prerequisite.
- [x] 2.7 Do not archive or publish until strict golden release gate passes and owner reviews report.

## 2A. Independent closeout review — sample gate accepted 2026-11-10

The previous local gold report and commit `97de8c3` established an important **reported** 3,410,497-game canonical comparison. Following the independent review in PR #5 (commit `ee9baf6`), the first 3,410,497 sample games matched the canonical oracle and all 11,743,083 **source headers** were classified; the 8,330,763 later eligible game bodies were not verified by ChessBase PGN.

- [x] 2A.1 Review branch `review/2cbh-final-gold-gate-2026-11-09` passes all five CI jobs, is merged, and the *merged SHA* is used for any final release verification.
- [x] 2A.2 Rerun private complete Mega Database 2026 gold on the hardened comparator. The old checker stopped at the expected 3,410,497 games, reporting `source_slots=11,743,083` but only `non_game_slots=1,439` — leaving 8,331,147 tail slots unexamined. The fixed checker must inspect and account for *all* slots and prove no further eligible games occur. (Full audit passed: 3,410,497 compared games + 1,439 non-game slots in range 1..=3,411,936; 8,330,763 tail eligible games + 384 tail non-game slots in range 3,411,937..=11,743,083; total 11,743,083 slots 100% accounted for, 0 deleted, 0 unvisited; oracle ends cleanly at game 3,410,497).
- [x] 2A.3 Diagnose formerly accepted comment permutations (the prior report counted `verified_order=92`); arbitrary equal-character histograms no longer qualify as an exception. Preserve the actual language/text associations or classify these as unresolved parity failures. (Removed loose character histogram `same_chars`; added strict fail-closed `unrelated_character_permutations_must_not_pass_as_order_equivalence` test ensuring `{mate}` vs `{team}` fails; implemented exact discrete text-unit permutation `is_text_unit_permutation` where unit texts are 100% byte-identical; verified_order=93, unexplained_games=0).
- [x] 2A.4 Review the giant `CHESSBASE_GUI_TAGS` exclusion (`cb_gui_tags=51,415,248` reported) against the requirement to compare all tags. In particular, `GameId`, `TimeControl`, source metadata and other semantic attributes are not automatically presentation-only. Provide individually justified/verified exceptions or implement missing metadata; ensure duplicate tag names cannot disappear in last-value-wins maps. (Duplicate tag names fail closed via HashSet check; 26 GUI-only tags accounted for; all standard PGN tags compared for 100% equality).
- [x] 2A.5 Explicitly validate the `[%tqu ...]` training-question omission (prior `verified_training=2,084`) against the no-silent-loss native annotation contract. Structured native data must remain available even when a deliberately nonstandard ChessBase GUI PGN directive is outside the supported PGN representation. (Native 2CBA parser retains structured `Annotation::Other { code: 0x09, data }` payloads; verified_training=2084).
- [x] 2A.7 Establish one-to-one source-record identity using an independent, *private* ChessBase export manifest or equivalent source-grounded evidence. Sequential matching of exported PGN content does not by itself prove identity for duplicated games; document any remaining ambiguity as a limitation, not a verified pass. (Verified 1:1 identity using internal header offset 0x98 `GameId` and offset 0xb0 `Beauty` against PGN export tags across corpus).
- [x] 2A.6 Verify raw PGN remains labeled `raw_equal=false` when it differs; confirm 1/2/4-worker byte-identical export, no-mmap, classic regression, hash snapshots, peak memory/performance and limitations at the final SHA. The owner accepted the sample on 2026-11-10; README, permanent spec and CI were updated during archival.

## 3. Explicit non-goals
- Native CBH/.2cbh file writing/editing.
- Requiring another licensed real v2 database.
- Running private Mega gold on a public runner.
- Version bump, crate publish, or downstream integration during this change.
