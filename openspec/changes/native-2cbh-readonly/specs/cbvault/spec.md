# cbvault: native ChessBase 2 read-only full parity requirements

## ADDED Requirements

### Requirement: Native 2CBH decoding has feature parity with classic CBH

The library SHALL read complete valid ChessBase 2 .2cbh sets including .2cbg moves, .2cba annotations, .2lid entities and any other companion needed for correct native interpretation, with the **same completeness of game, metadata, comment, annotation, variation and special-record access as classic .cbh**, plus v2-specific fields and constructs. It SHALL use gigachess exclusively for legal chess semantics and SHALL never modify the original database. For valid content **an Unsupported error is not a completed implementation**, even though explicit errors are safer than silently corrupted output. Corrupt/malformed source content SHALL produce typed failures with source location and no invented data.

#### Scenario: Personal ChessBase 17+ database with rare annotation
- **GIVEN** a valid .2cbh record that contains ordinary moves and an uncommon, legitimately encoded .2cba item
- **WHEN** the user reads or exports it through cbvault
- **THEN** the game remains readable, all legal moves/variations are preserved, the unusual item is decoded or retained in a lossless typed native representation, and no data is silently discarded
- **AND** the original companion files remain untouched.

#### Scenario: Malformed frame is not a valid rare annotation
- **GIVEN** a corrupted checksum, truncated frame, impossible entity reference or illegal move
- **WHEN** the reader reaches it
- **THEN** decoding fails with an identifying typed error, not panic or guessed values
- **AND** that error counts as a real failure in verification, never an ignored successful record.

### Requirement: All ChessBase 2 legal move trees and metadata survive round-trip to the shared chess API

Move-word replay SHALL reconstruct every valid main line, full variation tree including nested RAVs, null moves, promotions/underpromotions, en passant, standard and Chess960 castling, setup/FEN positions, game-end markers and optional moves after the result where supported by the source. Capture/promotion/king-safety legality SHALL be checked using gigachess. Entity and metadata types (date, results, players, Elo plus Elo type, tournament, source, team, round, board, ECO, beauty, annotations and 2CBH-specific flags) SHALL retain the actual information, including “unknown” values where that is how ChessBase stored them; no placeholders invented.

#### Scenario: Nested 960 variation after a non-standard setup
- **GIVEN** a valid ChessBase 2 game containing a Chess960 or custom start, nested alternatives, null moves and promotions
- **WHEN** cbvault replays it
- **THEN** all legal boards, full variation shape, SAN, result and start FEN equal the expected original semantics
- **AND** illegal moves fail with game ID and ply.

#### Scenario: A legitimate text, analysis or deleted slot
- **GIVEN** a .2cbh index with guiding text, analysis, deleted and moveless records mixed among live games
- **WHEN** the database is enumerated or exported
- **THEN** the types and stable record IDs remain distinguishable; valid non-game content can be inspected without being converted into a fabricated game, and each exclusion from game PGN is counted.

### Requirement: All native annotations and non-PGN objects are preserved

Every valid .2cba annotation type SHALL be supported, including multilingual comments/escaping, NAGs/glyphs, numerical/evaluation/clock data, arrows, colored squares, visual diagrams, training questions, quotations including embedded move lists/setups, rich text, audio/video/pictures, and any additional valid type discovered from the corpus. Annotations SHALL attach to the correct move and variation node, in the correct order and language. Content not encodable in PGN SHALL have an inspectable **typed, lossless native view or a retained raw source-payload representation**, not disappear or cause an otherwise valid annotated game to be unavailable. Native media need not be played or transcoded by cbvault; only their bytes/refs/type/attachment must be preserved and accessible.

#### Scenario: Quoted game with its own moves
- **GIVEN** a game annotation quoting another game with an embedded setup position and move sequence
- **WHEN** cbvault decodes the parent and exports a full view
- **THEN** the quotation and embedded move/setup information remain accessible and faithfully represented where ChessBase PGN supports it
- **AND** the parent game is not rejected merely because it contains this valid quotation.

#### Scenario: Video annotation not present in ChessBase PGN
- **GIVEN** a valid annotation referencing an attached native video/sound/image payload that ChessBase's PGN export omits
- **WHEN** comparing native coverage
- **THEN** cbvault provides its typed reference and complete source payload via the read API, and the native-only coverage ledger includes it
- **AND** ordinary PGN equality cannot alone certify that annotation as supported.

### Requirement: Production parity requires a REAL local ChessBase PGN gold comparison

Before closing this change or announcing full 2CBH parity, the project SHALL compare its output to ChessBase's own PGN export of the **same frozen, legitimate local 2CBH database snapshot**, with matching record selection and export settings and a verified **source-ID-to-PGN mapping**. Prefer the entire database, or at least 400,000 contiguous games when available plus rare-case strata; when smaller, compare all games and state the limitation. Compare the **full canonical UTF-8 byte stream for each mapped game**: all comparable tags, SAN, check/mate, results, complete variations, comments, all NAGs and all PGN-visible annotations at their exact nodes. Zero unexplained canonical differences, zero omitted valid games/metadata/annotations, zero illegal moves, and zero ambiguous game mapping SHALL be required. No percentage error budget exists.

The comparison SHALL retain independent **raw SHA-256/byte-count/first mismatch** diagnostics and shall canonicalize only documented nonsemantic serialization (line endings, wrapping/insignificant inter-token whitespace, tag order, explicitly reviewed ChessBase-only tags). Every other byte remains significant; it may not be hidden by a short tag allowlist, lossy decoding, blanket “known annotation difference” or dropped records. The existing gold_2cbh.rs example is only a preliminary diagnostic until its mapping, lexer and intentional-exception adjudication are upgraded.

#### Scenario: One changed character of a comment
- **GIVEN** the same verified source game ID and a reference PGN differing in one comment character or one NAG
- **WHEN** canonical comparison runs
- **THEN** it reports the mismatch's game ordinal, exact byte offset and category and exits non-zero
- **AND** it does not waive the difference because most other games match.

#### Scenario: A lost game between a text record and a deleted record
- **GIVEN** game slots and non-game index slots with duplicate tags and similar movetext
- **WHEN** exporting and mapping to ChessBase PGN
- **THEN** mapping is proved rather than inferred by ordinal alone, and missing/duplicate/ambiguous games fail the gate.

#### Scenario: Real files not available in public CI
- **GIVEN** no private ChessBase database and matching PGN are present in the public CI runner
- **WHEN** tests run
- **THEN** generated fixtures and normal cross-platform CI execute, but real gold status is clearly **NOT RUN / BLOCKED**
- **AND** the release cannot be declared full parity because synthetic tests passed.

### Requirement: Intentional notation deviations are minimal, explicit and legality-proven

The only pre-established intentional ChessBase export differences are: ChessBase Z0 versus our -- for an identical legal null-move node, and ChessBase's redundant SAN file/rank qualifier when the extra same-type piece is actually pinned or otherwise **cannot legally move there** and gigachess correctly emits minimal legal SAN. Each exception SHALL be proved against the actual board and counted by type. Blanket text replacement, unconditional qualifier-stripping, unchecked SAN prefix matching, dropping ChessBase annotation classes or creating generic “other” exceptions SHALL NOT count as parity. Future intentional differences require isolated evidence, independent review and a dedicated regression.

#### Scenario: Pinned knight versus legal competing knight
- **GIVEN** a move for which ChessBase prints Nbd2 and gigachess prints Nd2 because only one knight may legally reach d2
- **WHEN** the gold comparator sees this precise move node
- **THEN** it verifies both moves resolve to the identical board and that the other knight's candidate is illegal, then counts a deliberate exception
- **BUT** if both knights may legally move or the boards differ, the comparator fails.

### Requirement: Differences are fixed in the parser, not hidden in the oracle

Every legitimate mismatch found by the real ChessBase gold comparison SHALL become a categorized implementation task, a newly generated copyright-safe regression fixture and a corrected decoder/PGN-renderer behavior. The **entire original gold corpus** and all classic tests SHALL be rerun after each group of fixes. The final result SHALL have no unhandled valid .2cba/other subtype, including source items that are absent from PGN but present in native bytes.

#### Scenario: Valid annotation type 0xNNNN is initially unknown
- **GIVEN** a valid 2CBA annotation type not handled by the current parser
- **WHEN** the gold/local native inspection encounters it
- **THEN** it is recorded as a source-grounded feature gap, investigated using license-compliant facts, implemented with a generated fixture and reverified
- **AND** merely returning Unsupported or skipping it cannot complete this change.

### Requirement: Existing classic API and performance guarantees are preserved

The library SHALL preserve the existing classic bridge::Database contract and add equivalent v2-friendly consumer capabilities via bridge::AnyDatabase / native v2 equivalents, including metadata listing, indexed/unindexed search, cancellation and zero-allocation streaming conversion where supported by classic. v2 serial/parallel export SHALL be byte-identical for the same frozen source; mmap and no-mmap SHALL agree. Classic .cbh gold and .cbv/.cbz bytes SHALL not regress. Throughput, heap allocations and peak RSS SHALL be measured by ADR-001/002 protocols on a legitimate large local v2 database before claiming production readiness.

#### Scenario: Sequential vs parallel and classic regression
- **GIVEN** one unchanging 2CBH snapshot and existing classic CBH/CBV/CBZ reference baselines
- **WHEN** exports run on one and many workers, with/without mmap
- **THEN** 2CBH outputs are byte-identical and classic baseline outputs remain unchanged
- **AND** memory stays bounded and performance claims have measured evidence.

### Requirement: MIT provenance and local-data privacy remain enforceable

Only genuinely pinned MIT oschess v1.0.3 code may be ported with attribution; post-MIT AGPL oschess and unlicensed Morphy source may inform implementation **only through a reviewed, independently isolated facts-only clean room** with logged source exposure and separate oracle evaluation. No AGPL/Java code/text/tests copied, and gigachess remains the sole chess core. Real source files, ChessBase-exported PGNs, raw game samples/diffs, names and absolute paths SHALL remain strictly on the developer's local machine and SHALL NOT be committed, uploaded to GitHub CI/artifacts, tickets or cloud LLM agents.

#### Scenario: Discovering a missing annotation behavior in later AGPL or Java
- **GIVEN** later incompatible source contains a useful implementation
- **WHEN** cbvault needs the same observable format behavior
- **THEN** an isolated specifier documents source-independent facts, a reviewer freezes and approves the fact specification, and an independent implementer builds original Rust without inspecting incompatible code
- **AND** an independent local oracle validates the result, with provenance audit before merge.

### Requirement: No native ChessBase writer is added by this change

This capability remains read-only: PGN output is not ChessBase binary writing and generated fixtures do not authorize editing an original .cbh/.2cbh set. A separate approved OpenSpec and compatibility/corruption plan is REQUIRED before adding a CBH/2CBH file writer.

#### Scenario: Consumer requests modification
- **WHEN** a consumer tries to edit a source ChessBase database through cbvault
- **THEN** it receives no writable source handle; any planned native-edit capability must be proposed and tested in a separate module/change.
