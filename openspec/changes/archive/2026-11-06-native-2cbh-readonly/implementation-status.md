# Native ChessBase 2 / 2CBH: current code vs intended full parity

**Audit date: 2026-11-05.** Active OpenSpec change, **NOT complete**. The target is **100% canonical ChessBase PGN parity except rigorously proven deliberate formatting policies, plus full typed/lossless coverage of native-only annotations and all classic CBH reading capabilities**. See [proposal.md](proposal.md), [tasks.md](tasks.md), [golden-parity-runbook.md](golden-parity-runbook.md) and [specs/cbvault/spec.md](specs/cbvault/spec.md).

## Merged implementation — confirmed in cbvault/main

- PR #1 corrected v2 frame/checksum/tags/content boundaries and metadata, implemented reusable zero-allocation framing tests.
- PR #2 added the .2lid name/metadata registry, v2 move word enum and conversion to gigachess (no second chess core), legality-checked replay, setup/Chess960/variation paths, typed .2cba items, shared PGN formatting and native per-game reader.
- The native cbvault::twocbh::Database offers open, records, record, verify, write_game, write_game_reuse, export and export_parallel. Native CLI info, verify and pgn dispatch by generation; bounded ordered worker buffers preserve output order.
- The **classic** bridge::Database stays classic-only for API stability. **bridge::AnyDatabase / bridge::open_any** now selects classic or v2 and offers generation, record count and one-game PGN. This is not yet the full classic GameSink/search/position/query API for v2.
- Synthetic tests cover nonempty standard openings and one stored-order variation, basic entities and PGN, malformed frames and no-mmap framing. Existing .cbh/.cbv/.cbz tests remain in CI.
- The local example crates/cbvault/examples/gold_2cbh.rs exports a full database to a temporary PGN and compares basic canonical tokens and selected metadata against a local ChessBase PGN. It is a **diagnostic scaffold**, not a proof of complete game, annotation or ChessBase PGN parity.

**GitHub evidence:** merged PRs #1 and #2; final integration commit 7497618983360ca4c0d083506c9b33a8e33be9fd; run 42061315334 completed successfully across **Linux, macOS, Windows, nightly fuzzing, one-core dependency check**. That run used **generated synthetic data only**, so **real ChessBase parity and large-workload performance were not tested**.

## Known incomplete or unverified behavior — MUST be closed, not waived

| Area | Present behavior | Work required for complete parity |
|---|---|---|
| Real ChessBase equality | No actual matched v2 DB + ChessBase PGN run | Freeze source, source-ID mapping, upgraded byte-accurate canonical oracle, complete corpus zero unexplained diffs |
| Gold comparator | FNV-1a, limited 13-tag keep-list, simple PGN lexer and ordinal pairing; no semantic SAN adjudication | SHA-256, complete tags/comments/RAV/NAGs/charset, strict identity/order and mapping, legal null/SAN exception proofs, robust negative tests |
| Ordinary game moves | Replays through gigachess; synthetic opening coverage | Full-source legal replay, edge-case board correctness, promotions/castling/Chess960/variations under real corpus |
| Typed .2cba annotations | Known typed frames and shared output; unknown layouts fail with explicit errors | Every valid subtype, localization, comments, clocks, eval, graphics, training, media, correct ply and variation attachment |
| Quoted game annotations | Quotation parser has partial v2 support and rejects embedded moves/setups | Full structured, legal embedded quoted games and all nested/linked formats |
| Native-only ChessBase content | Some PGN-inexpressible data/sidecar semantics unverified | Expose and separately verify lossless typed/raw native views; no drop even when PGN cannot carry them |
| Full metadata / record shapes | Basic headers, entity lookups, game/text/analysis/deleted flags | Verify v2-only end markers, rating type/beauty/round/teams, sidecars and true text/analysis handling |
| Shared API | AnyDatabase records+PGN works; classic bridge stays classic-only | Full v2 read/search/metadata/sink/cancellation/position parity or reviewed native equivalents |
| Performance | Framing zero-alloc synthetic, bounded PGN workers | Warm complete-path allocation count, peak memory, throughput and paired benchmarks on real v2 DB |
| Provenance | MIT pin credited, no second chess core in CI | Independent later-AGPL/Morphy facts-only clean room and final license review if informed by that source |

**Important:** “Unknown annotation type” fail-closed is safer than silently deleting it but **is NOT equivalent to implementing that valid annotation**. The goal demands that a valid ChessBase game stays readable even when it includes rare content. Corruption is still a typed error. No permanent valid-content exclusions are allowed at completion.

## Exact next-agent sequence

1. Read [golden-parity-runbook.md](golden-parity-runbook.md) and complete its Stage 0 feature/coverage matrix.
2. Harden the gold comparator before interpreting a result: validated source-ID mapping, SHA-256 and raw byte offsets, complete tokenization and semantic tag/annotation comparison, exact deliberate exception adjudication.
3. Run a genuine **local-only** frozen real database + matching ChessBase full PGN, **all games preferably or at least 400k when available**, plus rare-case strata and a separate native-only object inventory.
4. For every discrepancy/unsupported legitimate feature, implement the decoder/formatter, create generated regression fixtures and iterate full oracle until **zero unclassified differences and zero valid-record omissions**.
5. Cross-check classic CBH/CBV/CBZ, deterministic serial/parallel and mmap/no-mmap bytes, performance and final five-platform CI. Keep the source private. Document aggregate proof and provenance; only then close the change.

Current diagnostic command (not final gate):

    export CBVAULT_TEST_DB_2CBH="/private/path/database-stem"
    export CBVAULT_2CBH_GOLD_PGN="/private/path/chessbase-export.pgn"
    cargo run -p cbvault --release --example gold_2cbh -- --threads 4

Both paths are local only. No private ChessBase files, exports, derived game fragments, local paths or per-game differences enter the GitHub repository, public CI, issues or remote agent context.

## Writing CBH / 2CBH

**cbvault intentionally does not write ChessBase native databases**. Both classic CBH and native v2 remain read-only; the PGN writer and fixture builders are not ChessBase file writers. Morphy implements Java writing for both generations but has no verified MIT reuse permission and documents v2 quotation/training limitations. Native writing/editing is **not part of this gold/parity change**. If needed for the product, research in a separate independent writer OpenSpec with a dedicated storage architecture and compatibility/corruption tests, never weakening the read-only reader contract.
