# Native 2CBH release hardening — strict gold and complete reader integration

## Background and baseline

The earlier 2CBH implementation change was archived on 2026-11-06. Preserve it as historical work; **do not treat its checked tasks as proof that complete 2CBH support is achieved**.

The local Mega Database 2026 / ChessBase 18 run examined 3,410,497 games, reported zero move decode and annotation read errors, and yielded 3,312,796 existing-comparator matches (97.135%), 3,406,934 matches including *heuristically classified* SAN qualifier differences (99.895%), and 3,563 residual differences. This is strong **one-corpus** evidence, but the existing gold example does not establish byte-accurate full-content equality. Main baseline commit 07716d49ba380768aa65673475fa4b6acb605bd7 passed five GitHub CI jobs.

Inspection of the code identifies concrete gaps: a v2 PGN annotation-validation failure can silently switch to bare PGN; unknown 2CBA types terminate iteration without an error; annotation-file read errors are sometimes converted to absence; common bridge search APIs invoke classic-only paths; v2 annotations are not delivered to GameSink; v2 ordered sink conversion currently falls back to sequential; explicit v2 paths can be misdetected when classic files share the same stem.

## Goal

Bring the **read-only** v2 reader to reliable classic-CBH-equivalent behavior through every public Database, game listing, metadata, annotation, PGN, search, position-search, sink, ordered parallel conversion and CLI API, without silently ignoring valid information. Preserve the gigachess-only rules engine and all classic CBH/CBV/CBZ correctness/performance contracts.

**Strict gold release gate:** Compare the frozen real source game records to ChessBase-exported PGN using verified source ID -> PGN mapping, every header tag, every Unicode byte, every RAV/comment/NAG, SAN, result and relevant directive. Record both (a) **raw byte-for-byte PGN SHA-256/byte counts/first difference** and (b) **byte-for-byte equality after narrowly authorized nonsemantic output transformations**. No similarity percentages, 13-tag whitelist, lossy Unicode repair, silently dropped items or generic "ChessBase quirk" waivers may count as success. Every unauthorized byte must be diagnosed and corrected, or the gate fails with nonzero exit. Raw differences due deliberate standard notation and ChessBase wrapping must remain separately visible; never claim transformed equality is raw equality.

**Proof for exceptions:** ChessBase Z0 may match our -- ONLY at a proven legal null-move node. ChessBase redundant SAN qualifiers may match minimal gigachess SAN ONLY after legal-board replay proves both strings denote the exact same move/next board, and that rival same-piece candidates are illegal. Other deliberately improved ChessBase-specific representations (PUA chess font glyphs, diagram control bytes, codepage/mojibake, Unicode, localized GUI clock boilerplate) require separate contextual transformations, automated positive and negative fixtures, per-class byte counts and reviewer sign-off. No text-level pattern-only SAN approval.

**No silent annotation loss:** Every unknown/corrupted/unreadable annotation, invalid content/position, and native-only feature must be explicitly detected and preserved through an appropriate typed/borrowed representation or reported as a failure. A valid unknown native format is not "fully supported" just because the parser reports Unsupported.

## Execution

Implement **on main** all work possible with public generated data now, including the strict comparator implementation, independent synthetic negative tests, complete v2-aware bridge API and deterministic parallel conversion, then run cross-platform CI and stop. **Do NOT run private real gold now**. The next agent, on the owner's machine, must only have to run the gold procedure, analyze remaining differences, implement missing decoding rules with legal/provenance review, rerun and present a sanitized review report. See design.md, tasks.md, specs/cbvault/spec.md, gold-runbook.md.

## Real-data limitation (must appear in release notes)

Only one real second-generation database is currently available: **Mega Database 2026**, with its ChessBase 18 PGN. Do **not** demand other licensed files. Absent or very rare structures in that particular corpus **cannot be assumed verified**: Chess960 may be missing; special annotations, training/media, embedded quoted-game moves, uncommon setups and other unusual v2 types may still have issues despite generated tests. State explicitly "Validated on Mega Database 2026 only" and list each feature as real-tested, synthetic-only, unverified or unsupported. Do not claim universal 100% compatibility.

## Non-goals and acceptance

No editing/writing native CBH/2CBH, no second chess core, no copying post-MIT AGPL or unlicensed Morphy code, no uploading private ChessBase databases/PGNs/game excerpts, no premature version bump/tag/publish/BlindBase changes. Version bump and downstream integration need a separate review after passing the real oracle.

The current active OpenSpec stays open until **zero unexplained byte differences**, zero unreported valid-data annotation losses, correct v2 functionality across public APIs, deterministic single/multiworker output, validated classic regressions, synthetic strict negative tests and five green CI jobs. Private gold stage is explicitly deferred for the next agent, never falsely checked by public CI.
