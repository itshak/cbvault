# ctg-opening-tree-streaming-api

## ADDED Requirements

### Requirement: Opening-tree queries expose typed child statistics

A one-level query SHALL return legal moves in moves2 currency with child statistics, explicit perspective, recommendations and typed diagnostics. Missing statistics SHALL be distinguishable from zero and corrupt data. A stats-only query SHALL NOT decode child continuations unnecessarily.

#### Scenario: A Black move is displayed
- **WHEN** the normalized child has opponent-to-move statistics
- **THEN** White, Black and mover results are correctly oriented
- **AND** unknown annotations are preserved without guesses

### Requirement: Whole-book traversal is bounded and cancellable

The library SHALL support sequential borrowed raw traversal and optional decoded traversal, progress and cooperative cancellation, without requiring a full lookup index for operations that do not probe children.

#### Scenario: A large book is streamed
- **WHEN** a consumer requests raw entries under a memory budget
- **THEN** records are delivered incrementally and cancellation stops promptly
- **AND** damaged entries are reported explicitly
