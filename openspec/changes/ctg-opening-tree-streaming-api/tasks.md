# Tasks

## 1. Implementation and verification

- [x] 1.1 Define stable typed tree/stats API and examples with no application dependency; document perspectives and move currency.
- [x] 1.2 Implement stats-only child probe and one-call tree; move CLI-derived orientation logic into library.
- [ ] 1.3 Benchmark and implement allocation-free key generation, one legal generation, piece ordinal tables and validated board construction if supported. (Partial: allocation-free key generation and shared `legal_moves` generation completed; `experiment/ctg-bitboard-ordinals` audited with 0 parity diffs on 228.7M positions but not adopted after CI `46162027386` failure and 10-12% parallel `verify --children` regression; direct board construction deferred.)
- [x] 1.4 Add raw and decoded streaming without eager index; progress, cancellation, budgets and strict/recoverable error handling.
- [x] 1.5 Test annotations and stats against independent real-book oracle including unknowns, repeated probes and both orientations.
- [x] 1.6 Run strict validation, fmt, clippy, tests, fuzz and paired benchmarks; document public API and provenance.

Evidence: full native-book differential run 45851071642, hot-path run 45849147120, integration CI 46161926424, native benchmarks 46161926531, and combined routing/stream acceptance 46161926430 (85.8M-entry scale gate); opt-in promotion child probing (`DiscoveredPromotion` / `probe_promotion_children`) added on `Book`, `NativeBook`, and `AdaptiveBook` without polluting stored parent continuations.
