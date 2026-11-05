# cbvault: native 2CBH read-only compatibility

## ADDED Requirements

### Requirement: Native versioned 2CBH read-only decoding

The library SHALL open supported `.2cbh` database sets with `.2cbg`, `.2cba` and required `.2lid` entities using verified little-endian fields and record tags, SHALL reconstruct complete legal chess moves and main/side variations using **gigachess only**, and SHALL never write to any original ChessBase companion file. Unsupported/corrupt/changed records SHALL return explicit typed errors, not fabricated chess data.

#### Scenario: User opens a current personal 2CBH file
- **GIVEN** a complete supported 2CBH set with known games and variations, opened read-only
- **WHEN** the caller invokes the existing `Database` facade to list, retrieve, verify and render PGN
- **THEN** the game count, player/event identifiers and legal move trees match independently verified facts
- **AND** no file in the original database directory is modified.

#### Scenario: Damaged record
- **GIVEN** a 2CBH record whose checksum, tag, entity reference or move word is invalid
- **WHEN** the reader reaches the record
- **THEN** a typed error identifies the failing game and reason without panicking, inventing moves or silently dropping a record.

### Requirement: MIT source baseline and clean-room provenance are enforced

Any ported 2CBH implementation SHALL originate only from the verified MIT oschess `v1.0.3` commit `bffebfc498d52b4e4631aed32b2f8e8194d125ff`, retain MIT attribution, and be mapped to existing gigachess and cbvault primitives. Work informed by later AGPL oschess or unlicensed Morphy Java implementation SHALL use the reviewed two-room, facts-only clean-room process with documented provenance. No AGPL/Morphy code, comments, tests or structures SHALL be copied into the distributed MIT implementation.

#### Scenario: A later AGPL bug fix is wanted
- **GIVEN** a behavior present only in a later AGPL oschess release
- **WHEN** cbvault needs to support it
- **THEN** a separate specifier documents independently verifiable format facts and an isolated implementer writes original Rust from the reviewed specification
- **AND** a license/provenance audit and independent oracle comparison are completed before merging.

### Requirement: PGN parity is proved against real local ChessBase export

Before claiming production-ready 2CBH support, the change MUST compare the same explicitly mapped game IDs from a real, user-owned local 2CBH database against **ChessBase-generated PGN**, preferably a complete database or at least a substantial 400,000-game contiguous sample where available plus rare-case sampling. The canonical PGN representation used by the existing classic comparator SHALL be compared **byte-for-byte** across comparable tags, full game trees, variations, comments and NAGs, with raw byte diagnostics retained. Every difference SHALL be explained and classified; **zero unexplained canonical bytes, zero illegal moves and zero silently omitted supported records** are required.

#### Scenario: Golden PGNs disagree by one move letter
- **GIVEN** a locally exported ChessBase PGN and the corresponding cbvault PGN
- **WHEN** one normalized SAN move differs beyond a verified legal-SAN qualifier exception
- **THEN** the comparator reports the exact source game ID, category and first differing byte position and fails
- **AND** the discrepancy cannot be dismissed as a percentage tolerance.

#### Scenario: Null move and pinned-knight disambiguation
- **GIVEN** a source game with a null move and a position where two knights attack d2 but one is pinned and legally unable to move there
- **WHEN** ChessBase prints `Z0` for the null or `Nbd2` for the chosen knight
- **THEN** cbvault preserves `--` for the null and **`Nd2`** when only one knight can legally make the move, using gigachess's actual legal-move and king-safety check
- **AND** the comparator counts those discrepancies under separately justified exceptions rather than marking them unexplained failures.

### Requirement: Source privacy and local-only large database acceptance

Real `.2cbh` companions, ChessBase PGN exports, user metadata, per-game diffs and test output SHALL stay on the developer's local machine. Public CI MUST use synthetic fixtures only; env-gated local tests SHALL accept explicitly configured paths and skip visibly when absent **without** satisfying the release gold gate.

#### Scenario: CI runs without a Mega Database
- **GIVEN** public CI has no local ChessBase files or `CBVAULT_TEST_DB_2CBH` and `CBVAULT_2CBH_GOLD_PGN`
- **WHEN** the normal test suite runs
- **THEN** generated synthetic 2CBH tests run and the local gold test is reported as NOT RUN
- **AND** no private chess database is uploaded, referenced by absolute path or synthesized as an apparent gold result.

### Requirement: Existing classic output and performance guarantees remain intact

The 2CBH implementation SHALL reuse `gigachess` SAN/legality and the existing PGN writer, source reader and ordered export interfaces. Classic .cbh, .cbv and .cbz correctness/byte parity SHALL not regress; a 2CBH sequential/parallel export of the same source revision SHALL be byte-identical. Performance claims SHALL be supported by ADR-001/002's paired benchmarks and whole-database profile.

#### Scenario: Parallel 2CBH export
- **GIVEN** one stable 2CBH source with 400,000 or more games
- **WHEN** the library exports the same exact games sequentially and with multiple workers
- **THEN** their output bytes and game counts are identical
- **AND** existing classic gold output and archive extraction hashes are unchanged.
