# Handoff to the NEXT agent — private Mega 2026 golden run

The current agent owns **implementation, synthetic tests and main-branch CI**. The next agent owns the **real private-data run, discrepancy fixes and report**. Do not treat unchecked tasks as complete; consult tasks.md and actual Git tree first.

## 1. Inputs, immutability, privacy
On the license holder's local computer, acquire the same licensed Mega Database 2026 complete .2cbh/.2cbg/.2lid/.2cba (plus optional relevant companions) and its *matching* ChessBase 18 PGN of 3,410,497 games. Preserve source unchanged; hash every companion before and after with SHA-256 and record size/mtime/ChessBase version, export options, charset, annotations and sorting in an UNTRACKED private manifest. Never upload source files, raw PGN, snippets, filenames/paths, detailed per-game diff or private manifest to GitHub, chat or remote services.

## 2. Run exact strict comparison
Set CBVAULT_TEST_DB_2CBH and CBVAULT_2CBH_GOLD_PGN to local paths and follow the **new strict executable's documented CLI**, not the old non-failing gold_pgn diagnostic. Run all eligible source slots, mapping IDs to PGN and asserting full 3,410,497 matching games, no unknown valid annotation, no missing/extra/reordered/duplicate games, no malformed Unicode, no untallied record errors and full EOF. Keep two distinct verdicts: raw PGN byte length/SHA-256/first differing offset, and strict byte equality after only approved, proof-backed transforms. Verify each SAN over-disambiguation at the actual legal board using gigachess; verify each null token at its actual position. All other deviations, including Unicode/diagram/GUI clock/encoding, must be *contextually* transformed with per-byte occurrence accounting and exact test.

Check serial vs real multiworker export in 1/2/4 workers and mmap/no-mmap; compare complete bytes and counters. Check source hashes unchanged after each run. If no strict executable has been implemented or any counters are unverified, report BLOCKED rather than falling back to the old 99.895% statistic.

## 3. For every residual difference
Group by source-grounded specific cause, record first differing game/offset **only in private local logs**; reproduce using ORIGINAL generated fixture, fix parser/formatter or add an independently reviewed narrow transform, rerun whole frozen corpus. Never edit the golden PGN, hide tags/comments/variations, skip a game, allow “OTHER”/“unimportant annotation difference”, or pass on percentages. Invalid/corrupted records remain typed failures; valid unknown native features require implementation or are explicitly unsupported/unverified.

## 4. Return a decision-ready, privacy-safe report
Provide total source slots, eligible/matched/raw-identical/transformed-identical counts, SHA-256 (safe only if not identifying private filenames), every named exception count and proof method, illegality/unknown/unsupported/unreadable/native-only coverage counts, 1/2/4 thread byte hashes, peak RSS and throughput, classic golden regression, Git commit SHA, five CI results and acknowledged limitations. Do not claim 100% of all possible 2CBH files. Explicitly state **Mega Database 2026 is the only real corpus** and that Chess960/rare annotation/media/training/embedded quotes may be absent and not fully field-verified.

Stop for owner review. The next release/tag/BlindBase integration is an independent approved action.
