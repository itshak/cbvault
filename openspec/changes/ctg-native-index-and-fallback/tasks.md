# Tasks

## 1. Implementation and verification

- [x] 1.1 Freeze CTO/CTB format facts and independent oracle plan; verify licenses and collect user-created public CTG.
- [x] 1.2 Implement native lookup only after proving collision and negative semantics; validate against scanning and damaged fixtures.
- [ ] 1.3 Implement optional versioned persistent fallback with identity, corruption detection, atomic publication and cancellation. (Deferred: verified native `.cto`/`.ctb` covers all tested books with `0` missed routes and bounded scan stays in-memory.)
- [x] 1.4 Reduce fallback peak memory via bounded runs or partitioning, benchmark sort and candidate deduplication.
- [x] 1.5 Add strict/recoverable damage semantics, native/fallback diagnostics, missing companion handling and safe immutable-file policy.
- [x] 1.6 Run strict validation, fmt, clippy, tests, fuzz and paired real-book benchmarks; publish coverage and provenance.
