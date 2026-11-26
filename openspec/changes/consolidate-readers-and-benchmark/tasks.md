# Tasks

## 1. Baseline and core implementation (ordered)

- [ ] 1.1 Audit existing Criterion benches, examples, allocation counters, gold scripts, active reader interfaces and provenance; inventory duplicate code and current source-level issues.
- [ ] 1.2 Implement pinned public corpus manifest, fetch/checksum/extract script with safe paths and corpus annotation census; generate deterministic enriched PGNs and document conversion handoff.
- [ ] 1.3 Obtain maintainer-created CBH and 2CBH companions and ChessBase reference PGNs; audit licensing, hashes, source-slot accounting and round-trip annotation coverage; no private data.
- [x] 1.4 Implement CI benchmark workflow (manual/nightly plus bounded PR smoke), paired base/head runner and machine-readable report; run real baseline on both generations and archive numbers before optimization.
- [ ] 1.5 Remove redundant allocations, checked-offset hazards and repeated parsing with allocation and malformed-input regressions.
- [ ] 1.6 Consolidate shared record windows, positioned I/O and batching; test both mmap and unmapped paths, both generations and randomized access.
- [ ] 1.7 Prototype continuous ordered v2 pipeline, worker-local buffers and prompt cancellation; retain only validated gains.
- [ ] 1.8 Benchmark header-only search, ID stream and on-demand entities, reusable execution context and budgets; migrate generation-aware consumers and deprecate panicking APIs.
- [ ] 1.9 Audit immutable mapping assumptions and safe external-change behavior; measure mapping advice and low-memory behavior.
- [ ] 1.10 Run strict OpenSpec validation, fmt, clippy, tests, fuzz, public paired corpus and separate private gold where available; publish results and provenance; mark absent gates NOT RUN.

Evidence: five-tier ID scan 45850795524, four-tier reusable pool 45852216322, five-tier buffer-reuse run 45847322315. ID-only and reusable execution APIs are implemented; downstream migrations and budgets remain open.
