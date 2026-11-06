# Agent execution runbook: bring 2CBH to full classic CBH parity

Status (2026-11-05): foundational reader/PGN implementation merged to main through PR #1 and PR #2. The post-merge CI at commit 7497618983360ca4c0d083506c9b33a8e33be9fd passed all five jobs (run 42061315334). **Real ChessBase parity, rare annotation coverage, and clean-room review are NOT yet complete.**

## Non-negotiable goal

Every valid game and every feature that classic CBH supports must work in 2CBH, including rare games, nested RAV, Chess960, unusual setups, multimedia, quotes containing games and moves, training annotations and all metadata. Cover v2-only features too. For PGN-representable data, the exact canonical UTF-8 PGN bytes of cbvault must match the corresponding real ChessBase PGN, except proven, precisely enumerated formatting policies already deliberately chosen for classic CBH. Every unexpected difference is a parser/formatter bug to investigate and FIX, not a new exclusion. There must be **zero silent omissions, zero unhandled valid annotation records and zero unexplained canonical differences**.

ChessBase PGN cannot preserve every native binary feature. Non-PGN content needs a separate **typed, lossless native read/inspection gate**, with source byte references and retained raw payload where format semantics are unknown; never drop native media or training content simply because PGN cannot represent it. The original files remain read-only. Writing ChessBase binaries is a separate possible future change.

## Stage 0 — inspect the actual code (do this first)

1. Read AGENTS.md, this change's proposal.md, design.md, specs/cbvault/spec.md, tasks.md, implementation-status.md, docs/provenance.md, docs/format-spec-2cbh.md, the classic gold_pgn.rs and gold_2cbh.rs, plus ADR-001/002/003/004/005/007. Check main HEAD rather than assuming a previous feature branch is current.
2. Inventory, and classify as implemented / synthetic-tested / real-file-proven / unsupported-valid / unknown: format twocbh header/record/segment/word/moves/entities/annotation_items, chess v2 replay, cbvault::twocbh::Database, pgn writer, ordered parallel pipeline, bridge::AnyDatabase, CLI, fixtures and all 2CBA item kinds.
3. Construct a **feature × on-disk type × oracle coverage** table covering normal moves, variation depth, Chess960/underpromotions/castling/null/en-passant, arbitrary FEN/setups, moves after result/end marker, ratings/Elo kind, source/team/tournament/date/round/board/beauty/flags, all 2LID entity variants, .2lgd/.2lcd where needed, deleted/guiding-text/analysis/moveless records, language/Unicode/escaping, NAGs, icons, arrows/colored squares, evaluations, clocks/elapsed time, multiple text annotation types, media, games quoted inside annotations, training questions and nested quotes. Absence of a case in the real database does not prove support: add synthetic coverage.
4. Baseline strict fmt, all-target/all-feature Clippy with -D warnings, all-feature tests, bench compilation, no-mmap allocation tests, fuzzing, and strict OpenSpec validation (if the project OpenSpec CLI is missing, install the supported CLI or document NOT RUN). Preserve classic CBH gold and CBV/CBZ hash baselines.
5. Any behavioral research using post-MIT AGPL oschess or unlicensed Morphy must use a genuine room-isolated facts-only process with independent review, documented source exposure and an oracle evaluator. Pinned oschess MIT v1.0.3 can be ported with required attribution. **Never copy AGPL/Java code, pseudocode, tables or tests.** If the necessary facts cannot be established legally, ask for compatible permission rather than guessing.

## Stage 1 — freeze REAL local source and ChessBase PGN

1. Use the authorized complete companion set: .2cbh + .2cbg + .2lid, plus .2cba and any other relevant optional members/assets. Keep ALL private database files, generated PGNs, detailed diff logs and local paths on the owner's machine; never upload to public GitHub, CI, issues, transcripts or cloud LLMs.
2. Make a read-only snapshot. Compute SHA-256 and file sizes for each companion before/after ChessBase export and after cbvault parsing; reject concurrent modifications. Record ChessBase exact version/build, PGN export settings, encoding, locale, annotation options, source order and selected game IDs in an **untracked private manifest**. Do not run destructive experiments on the only original.
3. Export all games with all available PGN tags, RAV, comments, NAGs and ChessBase-supported annotations. Prefer all games; if larger, target at least 400,000 contiguous game records plus first/middle/end and rare-case strata. For smaller sets, compare **every** game and state the limitation; require a second corpus when necessary for unrepresented features.
4. Establish **exact source record ID ↔ PGN ordinal mapping**. Deleted, text and analysis record slots do not necessarily produce game PGN. Duplicated identical games may make simplistic player/date keys ambiguous. Use the ChessBase game-list sequence, verified full-game signatures and explicit mapping; reject ambiguous, missing, extra and reordered games.
5. Existing local-only diagnostic command:
       export CBVAULT_TEST_DB_2CBH="/private/path/database-stem"
       export CBVAULT_2CBH_GOLD_PGN="/private/path/chessbase-export.pgn"
       cargo run -p cbvault --release --example gold_2cbh -- --threads 1
       cargo run -p cbvault --release --example gold_2cbh -- --threads 4
   The example currently accepts --threads and --batch-size. **It is NOT YET the final oracle**. If no real files exist, say "PRIVATE GOLD NOT RUN" and continue building fixtures and parser coverage. Never claim the fixture suite proves golden equality.

## Stage 2 — harden the comparator BEFORE treating a green result as success

1. Replace ordinal assumptions with validated ID mapping and a complete denominator. Detect dropped, extra, reordered, multiply exported, decoded-with-error and skipped games; account separately for valid non-game records.
2. Replace noncryptographic FNV-1a with raw-output SHA-256, sizes and first raw mismatching byte offset; separately SHA-256 the frozen source companions. Correct stream short-read/EOF handling. Keep raw mismatch counts even when canonical serialization passes.
3. Build a streaming PGN lexer/parser that preserves complete games regardless of line wrapping, multibyte UTF-8, BOM, quoted/escaped tag values, line endings, multiline/nested commentary, NAGs, RAV brackets, move numbers, SAN including check/mate, nulls, terminal results and all PGN extensions. Invalid encoding must fail, never silently substitute U+FFFD. Include property tests at every possible chunk boundary.
4. Canonicalize **only proven nonsemantic rendering**: CRLF vs LF, line wrapping, inter-token whitespace, header ordering, and an **explicit, reviewed list** of ChessBase-only tags for which a meaningful equivalent is genuinely unavailable. The existing 13-name KEEP whitelist is insufficient by itself: audit ALL source and gold tags. Keep every semantic tag, comment, directive, variation topology, NAG, annotation node, result and Unicode character. Never discard a comment or tag to improve the match.
5. First compare complete canonical UTF-8 **bytes WITHOUT exception rewrites**, record the first differing canonical byte and game ID in local diagnostics, then apply narrowly adjudicated intentional exceptions:
   * NULL_SPELLING: ChessBase Z0 vs our -- only at the same null-move node; prove identical board continuation and legal context.
   * CHESSBASE_PINNED_PIECE_SAN: ChessBase redundant SAN file/rank qualifier vs our minimal legal SAN only after gigachess verifies the competing same-type move is illegal at that exact node. Never blindly strip an extra SAN character.
   * Any other pre-existing classic policy must be separately documented, legally/semantically proved, independently reviewed, explicitly named and regression-tested; generic "OTHER", annotation or tag waivers are forbidden.
6. Make synthetic **negative** tests for one changed comment byte, NAG, Unicode scalar, full tag, clock value, result, missing RAV branch, wrong annotation ply, extra/dropped game, ambiguous duplicate identity, altered SAN with a second LEGAL competing piece and a non-null Z0. Each must fail; merely normalizing text or count to equality is not allowed.
7. Output safe aggregate status by default: source record/game count, mapped/missing/extra, raw sizes/SHA-256 and mismatch offset, exact canonical equal, each justified exception count, each unexplained diff category, annotation coverage and illegal-move count, first differing *ordinal/offset* only. Private per-game text, paths and identifying data stay offline in untracked restricted files.

## Stage 3 — use differences to COMPLETE the parser

For each discrepancy class: isolate one **local-only fact**, create an **original synthetic fixture** from verified format facts, reproduce with a failing test, implement the missing code via gigachess/cbvault format abstractions, add both positive and negative tests, rerun full gold comparison, and document provenance. Repeat until the FULL corpus matches. Do not let the comparator "fix" output by hiding source data.

Priority families:
- Fully decode every valid 2CBA annotation ID and record structure, all language and Unicode fields, move/branch attachment, clocks, evaluations, arrows, graphics, sound/video/image references, training/correspondence content, rich comments and long records.
- Support quoted games with **embedded move streams and setup positions**, quotations attached to variation nodes, all nested structures. The current quoted-game parser deliberately rejects some: this is a **must-fix feature gap**, not an allowed endpoint.
- Cover Chess960, alternate starts/FEN, promotion/underpromotion, null, castling, en-passant, arbitrary-depth variations, end-of-game markers and trailing moves with board legality at every ply and correct SAN/check/mate.
- Decode real 2LID entity variants, all meaningful metadata, dates, Elo type, source and team, board/round/beauty flags, text/analysis records; use 2LGD/2LCD only where real features require them. Do not invent unknown fields.
- For valid non-PGN annotations retain typed source and bytes, expose to the caller and verify through independent local reference and structural expectations; add native-content coverage counts **separately from PGN equality**. An annotation becoming "unsupported" is a failure to finish this proposal.
- After each batch: full frozen corpus again, classic CBH gold unchanged, CBV/CBZ byte tests, source SHA-256 still unchanged, serial/parallel and mmap/no-mmap comparisons.

## Stage 4 — final gates

**ALL conditions required:** no unexplained canonical PGN differences; no missing/extra/ambiguous source-to-PGN mappings; no illegal moves; no silently omitted or unsupported VALID game/annotation/metadata payload; deliberate style exceptions proven by board/format semantics with separately reported counts; native-only content represented losslessly with its own checks. A percentage score is NOT acceptance.

Exact sequential vs ordered parallel output across 1, 2, 4 and available N workers; same bytes with and without mmap. Bounded memory/zero-alloc hot-path and paired Criterion/whole-export performance evidence on the private workload per ADR-001/002; classic and archive baselines preserved. Full strict fmt, Clippy, tests, benchmarks, nightly fuzz and Linux/macOS/Windows CI green on the final changes. Full format-source/provenance review.

**Only sanitized aggregate results/feature inventory** may be committed. Never commit private chess game text, file fragments, paths, screenshots or detailed diffs. Update proposal.md, tasks.md, implementation-status.md, design.md, the delta capability spec, provenance and format documentation. Archive OpenSpec ONLY after all gates. If gold files are absent, explicitly report **BLOCKED: REAL CHESSBASE GOLD NOT RUN** and leave tasks open.
