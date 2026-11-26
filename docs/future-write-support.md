# Future work: ChessBase write support (deferred)

**Decision: no write support now.** cbvault remains read-only; no active writer OpenSpec proposal, feature flag, partial writer or public promise. Revisit only with explicit maintainer approval after the upstream format/writer ecosystem matures and core reader quality is secured.

## Why defer

Morphy Java currently demonstrates CBH/2CBH transaction architecture; its v2 README (https://github.com/Yarin78/morphy/blob/6759f7157c/morphy-cb2/README.md) documents missing delete, analysis writing, limited annotations, uncomputed classifications and collation differences. oschess is actively developed (https://github.com/asavis/oschess-cb-bridge); its PGN editing does not establish ChessBase write parity. Wait for robust independently demonstrated support, reducing reverse-engineering burden rather than ship incomplete writes. Morphy source is treated as unlicensed; later oschess AGPL code not imported into MIT. Use two-room facts-only workflow and black-box differential tests; retain MIT-era notices.

## Entry criteria

- Core reader benchmarks and correctness gates stable; maintainer explicitly authorizes changing read-only policy and opens a new spec.
- Upstream evidence for create, append, replace, delete, annotations, entity/index consistency and real ChessBase interoperability, with remaining gaps mapped. Do not confuse a successful transaction with crash durability.
- Independently established exact record encoding, cross-file references, entity collation, derived/optional companions, format bounds, unknown data preservation and failure behavior. Freeze facts and hygiene review before implementation.
- Test with ChessBase on Windows, disposable databases, public licensed benchmark corpus and original annotated fixtures. Decide exact release scope only after complete supported-feature coverage; no silent losses.

## Eventual implementation sequence

1. Common typed game and metadata model, moves2/gigachess only, separate optional writer crate/feature with zero reader overhead. Validate inputs and unsupported features before any mutation. Explicit format capability matrix; no silent drop, truncation or annotation conversion.
2. Classic CBH creation and bulk export to a new staged directory, entity/namebase and metadata indexes, annotations, variations, nonstandard starts; independently read and verify, open/import/export in ChessBase. Publish a complete new file family atomically via directory-level publication where supported, never pretend multiple independent file renames are atomic.
3. 2CBH creation and bulk export: framed move/annotation records, entity tables, game-list/sort indexes, checksums/offsets, class fields, unknown values; test full consistency and round trips. No new public API until supported scope is complete and tested.
4. Existing database append/replace only after exclusive/coordinated access and generation preconditions, stable reader snapshots/unmapped I/O, preservation of unknown records/companions, transaction journaling or copy-on-write recovery, fsync/ordering and crash injection at every write boundary. Do not write a database open by ChessBase; refuse when safe coordination cannot be established.
5. Delete, compact and rebuild derived indexes only after reliable semantics, ID/reference handling and rollback/recovery. Test interrupted updates, full disk, permissions, concurrent readers, malformed inputs, unsupported companions and external changes.
6. Release readiness: complete supported-feature matrix, ChessBase round-trip field-level parity, corruption-free crash recovery, all reader gates unchanged, performance and provenance ledger; independently reviewed docs and versioned API. If incomplete, keep development experimental and unpublished, or defer further.

CTG writing is separate and even later; native index and read/statistics parity must be established first. A consumer can keep personal recommendations in its own overlay without modifying books.
