# cbvault roadmap — November 2026

Read quality, correctness and performance in CBH/2CBH are the primary product. CTG is important but follows the core reader. All implementation remains read-only, MIT, consumer-neutral, and uses gigachess alone.

| Order | OpenSpec change | Gate |
|---|---|---|
| 1 | [consolidate-readers-and-benchmark](../openspec/changes/consolidate-readers-and-benchmark/proposal.md) | Public CI corpus and measured baseline first; then consolidate |
| 2 | [audit-upstream-reader-parity](../openspec/changes/audit-upstream-reader-parity/proposal.md) | Verified upstream facts and read parity |
| 3 | [ctg-native-index-and-fallback](../openspec/changes/ctg-native-index-and-fallback/proposal.md) | Native CTO evidence and fallback correctness |
| 4 | [ctg-opening-tree-streaming-api](../openspec/changes/ctg-opening-tree-streaming-api/proposal.md) | Complete opening-tree and streaming API |
| 5 | [advanced-position-search-and-index-feeds](../openspec/changes/advanced-position-search-and-index-feeds/proposal.md) | Differential search and resource bounds |

## Benchmark prerequisite

See [benchmark corpus](benchmark-corpus.md). CI must actually execute meaningful release-mode workloads, not just compile benches. Public real-game data is allowed only after explicit provenance/redistribution review and checksum pinning. Private Mega, commercial books and proprietary content remain local; the private gold acceptance gate is distinct. Performance claims need paired measurements and correctness gates per ADR-001 through ADR-004.

## Deferred, not an active proposal

[Future ChessBase write support](future-write-support.md) documents the decision and eventual implementation plan. Do not implement or advertise writing now. Reconsider after reference projects establish robust full support and the maintainer explicitly approves a new OpenSpec change.
