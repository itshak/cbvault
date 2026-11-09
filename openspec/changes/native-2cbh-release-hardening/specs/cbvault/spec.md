# cbvault strict v2 reliability — requirement delta

## MODIFIED Requirements

### Requirement: Native versioned 2CBH read-only decoding

The library SHALL preserve all valid 2CBH move and annotation data through a generation-aware public reader and SHALL never silently omit requested, valid annotations or reinterpret a malformed record as a successful unannotated game. When a valid type is not yet supported, it SHALL report a typed unsupported-feature error with record ID, offset and kind; that error SHALL fail the complete parity gate. Corruption SHALL also fail with a distinct typed error and SHALL not panic.

#### Scenario: Unknown typed annotation
- **GIVEN** a v2 annotated game containing an unknown annotation type or invalid length
- **WHEN** an item iterator, PGN writer, verification, public game API or conversion sink processes the game
- **THEN** the error propagates, source ID is counted as failed, and complete export cannot be reported as successful
- **AND** the iterator does NOT silently terminate and the PGN writer does NOT emit the bare game instead.

#### Scenario: Native text helper must not conceal annotation decode errors
- **GIVEN** a 2CBA record with text followed by an unknown or malformed annotation kind
- **WHEN** a caller invokes strict text extraction (`try_texts`)
- **THEN** extraction returns the typed decode error instead of silently flattening and dropping the damaged item
- **AND** the legacy best-effort diagnostic `texts` method is not used as proof of full annotation decoding.

#### Scenario: Missing optional .2cba with actual annotation references
- **GIVEN** a valid v2 header references an annotation record but the .2cba companion is absent or corrupt
- **WHEN** the user requests annotations, exports complete annotated PGN or verifies annotations
- **THEN** a typed error reports missing/corrupt source data instead of treating the game as intentionally unannotated
- **AND** all success/failure counters represent the actual result.

#### Scenario: User opens a current personal 2CBH file
- **GIVEN** a complete supported 2CBH set with known games and variations, opened read-only
- **WHEN** the caller invokes the existing `Database` facade to list, retrieve, verify and render PGN
- **THEN** the game count, player/event identifiers and legal move trees match independently verified facts
- **AND** no file in the original database directory is modified.

#### Scenario: Damaged record
- **GIVEN** a 2CBH record whose checksum, tag, entity reference or move word is invalid
- **WHEN** the reader reaches the record
- **THEN** a typed error identifies the failing game and reason without panicking, inventing moves or silently dropping a record.

### Requirement: Database sets are opened and validated

Database::open and generation_of SHALL respect an explicitly supplied classic or 2CBH member extension, including .2cbh, .2cbg, .2cba and .2lid; they SHALL NOT select the wrong generation merely because another generation's companion has the same basename. An extensionless ambiguous stem SHALL report a typed ambiguity or require explicit selection.

#### Scenario: Both generations share a stem
- **GIVEN** the same directory contains Mega.cbh and Mega.2cbh with complete companions
- **WHEN** the user opens Mega.2cbh
- **THEN** generation and parsed files are TwoCbh, not Classic
- **AND** opening Mega.cbh remains Classic and an ambiguous bare Mega path does not silently pick one.

#### Scenario: Classic database

- **WHEN** `Database::open("Mega.cbh")` is called on a valid classic set
- **THEN** the database reports its generation and game count (`(size − 46) / 46`)
- **AND** sibling files are resolved case-insensitively from the same directory.

#### Scenario: Missing mandatory file

- **WHEN** the mandatory `.cbg` file is absent
- **THEN** opening fails with a typed error naming the missing file
- **AND** no partial state is left open.

#### Scenario: Damaged header

- **WHEN** the `.cbh` header magic does not match the documented values
- **THEN** opening fails with `Corrupt`, naming the file and offset, without panicking.

#### Scenario: Derived files present or absent

- **WHEN** a classic set is opened with or without derived files (`.cbgi`, `.cbb`, `.cko`, `.cpo`, `.patterns/`, `.accelerators/`)
- **THEN** the reported game count and the decoded games are identical in both cases
- **AND** derived files appear as present or absent in `info`, never as errors.

### Requirement: PGN parity is proved against real local ChessBase export

The strict local gold gate SHALL enforce **byte-for-byte equality** for all compared PGN bytes after ONLY documented, individually justified and counted nonsemantic formatting/ChessBase-specific transformations. It SHALL also independently report raw SHA-256, raw byte length and first differing raw byte, without falsely claiming raw equality when transformations were needed. It SHALL compare all tags, SAN, entire nested move tree, comments/literal Unicode, NAGs/results, and any PGN directives; no 13-tag allowlist, dropped annotations or generic similarity threshold is permitted. It SHALL prove exact source-record-to-PGN mapping, counts, duplicates, EOF and absence of illegal moves or unreported annotation failures. Any unapproved difference or unknown valid item exits nonzero.

#### Scenario: One unexpected byte
- **GIVEN** matched games differing by one nonapproved tag, comment, NAG, variation, result or Unicode byte
- **WHEN** strict gold compares them
- **THEN** it exits with failure and reports the first differing byte/category privately and aggregate safely
- **AND** a 99.9999% match rate never overrides that failure.

#### Scenario: ChessBase SAN qualification difference
- **GIVEN** ChessBase emits an extra SAN file/rank qualifier absent from standard gigachess SAN
- **WHEN** strict gold classifies a deliberate exception
- **THEN** it replays the actual board and source move, proves both notation forms denote the SAME legal move and next board and proves alternative same-type candidate moves illegal
- **AND** text-prefix differences alone cannot be accepted.

#### Scenario: Null, encoding, diagram and graphical differences
- **GIVEN** an actual null-move node and ChessBase-only output glyph, control byte, codepage or GUI clock text
- **WHEN** an exception is applied
- **THEN** it uses a separately tested context-specific transformation with exact input/output bytes and a named count, and the final transformed PGN is byte-identical
- **AND** arbitrary comment text or unrelated tokens remain unchanged.

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

### Requirement: Existing classic output and performance guarantees remain intact

The library SHALL keep all classic CBH and CBV/CBZ golden behavior, one gigachess chess core, strictly read-only source access, correct no-mmap/regular path, deterministic parallel output, bounded memory and established hot-path allocation budgets. Private ChessBase files, PGN, game excerpts and full per-game diffs SHALL stay on the owner's local machine.

#### Scenario: No real Mega present in GitHub Actions
- **WHEN** normal public CI runs without the private Mega corpus
- **THEN** comprehensive generated synthetic and adversarial tests, Clippy/fmt/fuzz/bench compilation execute
- **AND** the private gold gate reports NOT RUN and cannot mark real-data acceptance complete.

#### Scenario: Parallel 2CBH export
- **GIVEN** one stable 2CBH source with 400,000 or more games
- **WHEN** the library exports the same exact games sequentially and with multiple workers
- **THEN** their output bytes and game counts are identical
- **AND** existing classic gold output and archive extraction hashes are unchanged.

## ADDED Requirements

### Requirement: Native 2CBH annotations are accessible in consumer APIs

A caller requesting game annotations SHALL receive a typed, borrowed and lossless representation of v2-native items with exact ply/variation association, raw payload where applicable and source provenance. GameSink, Database::game_with, single-game PGN, and serial/parallel conversion SHALL produce equivalent annotation availability; no classic-only type punning or silent None. A genuinely unrepresentable PGN-native object SHALL remain accessible through the structured reader.

#### Scenario: Annotation-requesting sink over v2
- **GIVEN** a v2 game with comments, NAGs and native-only typed objects
- **WHEN** GameSink::wants_annotations returns true
- **THEN** the delivered view carries the actual requested v2 annotations with same bytes, positions and types as the source
- **AND** disabled annotation access may skip reading for performance, with no incorrect loss claim.

### Requirement: All public reader and search operations are generation-aware

The same public Database capabilities SHALL work correctly on v2: metadata/headers, entity/name lookup, tag filters/scan and scan_range, legal move walk, position search/range/early-stop/cancellation, per-game PGN, annotations, streaming GameSink, deterministic bounded parallel GameSink conversion, and CLI information/verify/export. A v2 operation SHALL NOT call a panicking classic-only accessor or decode a 2CBG record using classic CBG framing.

#### Scenario: v2 tag and position search
- **GIVEN** a generated v2 database with known player, event, date, ECO and a known target Polyglot key in a Chess960/custom-start or ordinary game
- **WHEN** a user performs tag scan or unindexed position-key search, serial or parallel
- **THEN** the same game IDs and legal position hits are returned as the independent expected oracle, including correct record-kind exclusion
- **AND** cancellation and early-stop semantics are correct without panics.

#### Scenario: Parallel v2 sink conversion
- **GIVEN** a database with game/text/deleted/analysis and corrupt-record fixtures
- **WHEN** conversion uses 1, 2 or 4 worker threads
- **THEN** decoded games, positions, keys, requested annotations, failures and record order are exactly the same
- **AND** work actually occurs across workers when more than one is requested, with memory bounded by worker count.

### Requirement: Real-data scope is accurately reported

Mega Database 2026 with ChessBase 18 PGN is the only available real 2CBH corpus; no second licensed database is required. A full gold pass SHALL be reported as **validated on Mega Database 2026 only**, never universal compatibility. Rare or absent features (Chess960, unusual annotations, embedded quotations, native-only content omitted from PGN) SHALL each be labeled real-verified, synthetic-only, unverified, or unsupported.

#### Scenario: Mega has zero Chess960 games
- **GIVEN** the Mega source corpus does not contain a particular rare valid 2CBH feature
- **WHEN** full-corpus gold passes
- **THEN** the documentation explicitly reports that the feature was not covered by real data and may still have defects
- **AND** original synthetic fixtures exercise the known legal shape without claiming universal field validation.
