# Tasks

## 1. Implementation and verification

- [x] 1.1 Define stable typed tree/stats API and examples with no application dependency; document perspectives and move currency.
- [x] 1.2 Implement stats-only child probe and one-call tree; move CLI-derived orientation logic into library.
- [ ] 1.3 Benchmark and implement allocation-free key generation, one legal generation, piece ordinal tables and validated board construction if supported. (Partial: allocation-free key generation and shared `legal_moves` generation completed; piece ordinal tables and direct board construction deferred.)
- [ ] 1.4 Add raw and decoded streaming without eager index; progress, cancellation, budgets and strict/recoverable error handling. (Deferred beyond current CTO/adaptive integration.)
- [x] 1.5 Test annotations and stats against independent real-book oracle including unknowns, repeated probes and both orientations.
- [x] 1.6 Run strict validation, fmt, clippy, tests, fuzz and paired benchmarks; document public API and provenance.

Evidence: full native-book differential run 45851071642 and hot-path run 45849147120. Combined integration CI pending. Piece ordinal tables and direct board construction are not yet implemented, so 1.3 remains open.
