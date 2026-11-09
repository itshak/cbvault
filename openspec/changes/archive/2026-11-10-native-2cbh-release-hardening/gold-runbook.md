# Archived private Mega Database 2026 sample gold runbook

The previous private agent reported a PASS on the frozen 3,410,497-game ChessBase 18 PGN export against cbvault `70b61c8`, with zero unexplained canonical-byte differences. The later 8,330,763 eligible source games were only header-classified and are not claimed to match ChessBase. This remains a reproducible **local-only** acceptance procedure; rerun the sample after material parser or oracle changes. See `closeout.md`.

## 1. Inputs, immutability, privacy
On the license holder's local computer, acquire the same licensed Mega Database 2026 complete .2cbh/.2cbg/.2lid/.2cba (plus optional relevant companions) and its *matching* ChessBase 18 PGN of 3,410,497 games. Preserve source unchanged; hash every companion before and after with SHA-256 and record size/mtime/ChessBase version, export options, charset, annotations and sorting in an UNTRACKED private manifest. Never upload source files, raw PGN, snippets, filenames/paths, detailed per-game diff or private manifest to GitHub, chat or remote services.

## Executable starting point (PRIVATE local machine only)

The prepared strict comparator is `crates/cbvault/examples/strict_gold_2cbh.rs`. It is deliberately stricter than the historical, non-failing `gold_pgn.rs`. To run:

```sh
export CBVAULT_TEST_DB_2CBH="/private/local/Mega Database 2026"
export CBVAULT_2CBH_GOLD_PGN="/private/local/chessbase-18-mega-2026.pgn"
export CBVAULT_STRICT_GOLD_EXPECTED_GAMES=3410497
cargo test -p cbvault --example strict_gold_2cbh
cargo run -p cbvault --release --example strict_gold_2cbh
```

**The prior frozen sample passed at `70b61c8`** with `raw_equal=false`. The comparator reports both raw hashes and counted permitted canonical transformations. ChessBase-only GUI tags and proprietary training directives are separately accounted for as metadata not present in cbvault's comparable PGN, not asserted to be byte-identical. This runbook is retained for repeat verification.

The default denominator is exactly **3,410,497**; a partial export must fail. An explicit variable may be used for generated fixtures or legitimately different local corpus sizes, but every production acceptance report must keep the 3,410,497 Mega denominator. Raw SHA-256 equality and canonical token-byte equality are independently reported; raw differences are not silently normalized.

## Additional serial/parallel/no-mmap byte replay (PRIVATE local machine)

The strict comparator above does not accept a worker-count flag. Independently
verify that the public CLI emits exactly the same full PGN bytes with 1, 2 and 4
workers and with memory mapping disabled. The explicit `.2cbh` extension is
intentional: it prevents a same-stem legacy `.cbh` from changing the selected
generation. Choose an untracked **private** output directory; these commands
must never run on GitHub Actions or commit/upload PGN.

```sh
export PRIVATE_OUT="/private/local/gold-check-output"
mkdir -p "$PRIVATE_OUT"
cargo run --release -p cbvault-cli -- pgn "$CBVAULT_TEST_DB_2CBH.2cbh" "$PRIVATE_OUT/v2-1.pgn" --threads 1 --json
cargo run --release -p cbvault-cli -- pgn "$CBVAULT_TEST_DB_2CBH.2cbh" "$PRIVATE_OUT/v2-2.pgn" --threads 2 --json
cargo run --release -p cbvault-cli -- pgn "$CBVAULT_TEST_DB_2CBH.2cbh" "$PRIVATE_OUT/v2-4.pgn" --threads 4 --json
cargo run --release -p cbvault-cli -- pgn "$CBVAULT_TEST_DB_2CBH.2cbh" "$PRIVATE_OUT/v2-no-mmap.pgn" --threads 2 --no-mmap --json
cmp "$PRIVATE_OUT/v2-1.pgn" "$PRIVATE_OUT/v2-2.pgn"
cmp "$PRIVATE_OUT/v2-1.pgn" "$PRIVATE_OUT/v2-4.pgn"
cmp "$PRIVATE_OUT/v2-1.pgn" "$PRIVATE_OUT/v2-no-mmap.pgn"
shasum -a 256 "$PRIVATE_OUT"/v2-*.pgn
```

A nonzero CLI exit, nonzero comparison or unexpected error counter blocks release.
The CLI's PGN stream is separate from the strict comparator's ChessBase
canonical comparison; passing these `cmp` commands alone is never proof of
ChessBase gold parity.

## 2. Run exact strict comparison
Set CBVAULT_TEST_DB_2CBH and CBVAULT_2CBH_GOLD_PGN to local paths and follow the **new strict executable's documented CLI**, not the old non-failing gold_pgn diagnostic. Compare all 3,410,497 eligible sample games, classify remaining source headers, map the sampled IDs to PGN and assert 3,410,497 matched games, no unknown valid annotation, no missing/extra/reordered/duplicate games, no malformed Unicode, no untallied record errors and full EOF. Keep two distinct verdicts: raw PGN byte length/SHA-256/first differing offset, and strict byte equality after only approved, proof-backed transforms. Verify each SAN over-disambiguation at the actual legal board using gigachess; verify each null token at its actual position. All other deviations, including Unicode/diagram/GUI clock/encoding, must be *contextually* transformed with per-byte occurrence accounting and exact test.

Check serial vs real multiworker export in 1/2/4 workers and mmap/no-mmap; compare complete bytes and counters. Check source hashes unchanged after each run. If no strict executable has been implemented or any counters are unverified, report BLOCKED rather than falling back to the old 99.895% statistic.

## 3. For every residual difference
Group by source-grounded specific cause, record first differing game/offset **only in private local logs**; reproduce using ORIGINAL generated fixture, fix parser/formatter or add an independently reviewed narrow transform, rerun whole frozen corpus. Never edit the golden PGN, hide tags/comments/variations, skip a game, allow “OTHER”/“unimportant annotation difference”, or pass on percentages. Invalid/corrupted records remain typed failures; valid unknown native features require implementation or are explicitly unsupported/unverified.

## 4. Return a decision-ready, privacy-safe report
Provide total source slots, eligible/matched/raw-identical/transformed-identical counts, SHA-256 (safe only if not identifying private filenames), every named exception count and proof method, illegality/unknown/unsupported/unreadable/native-only coverage counts, 1/2/4 thread byte hashes, peak RSS and throughput, classic golden regression, Git commit SHA, five CI results and acknowledged limitations. Do not claim 100% of all possible 2CBH files. Explicitly state **Mega Database 2026 is the only real corpus** and that Chess960/rare annotation/media/training/embedded quotes may be absent and not fully field-verified.

Stop for owner review. The next release/tag/BlindBase integration is an independent approved action.

## Independent revalidation after commit 97de8c3 (2026-11-09)

The former strict oracle produced a reported `unexplained_games=0` result, but subsequent code review identified additional gate problems. Its full run stopped as soon as the expected game count was reached without scanning the remaining source slots, and comment-order equality was determined by sorting individual characters (which can hide changed words). Review changes now fail closed on those cases, duplicate tags, unsafe FEN castling equivalence, 2CBA trailing bytes and suppressed native text decode errors.

**Do not carry the previous PASS forward** to a new SHA without a fresh, complete private run. The new complete-run acceptance must show `source_slots == games + non_game_slots`, no extra eligible records after the expected ordinal, `unexplained_games=0` under hardened equivalences, and the same full annotated PGN across one/few workers and with mmap disabled. Do not interpret the source-header slot count as a game count.

Before archiving, separately document whether the 51,415,248 counted ChessBase-only GUI tags are truly unrepresented metadata or missing source fields, and account for 2,084 training-question exceptions and 92 previously accepted free-character comment permutations. Compare tag names and source semantics; no blanket tag waiver or anagram equality is a strict byte-parity proof. Keep the previous reported pass as historical evidence, not release certification of the revised checker.
