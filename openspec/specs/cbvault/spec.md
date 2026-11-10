# cbvault

## Purpose

Define what `cbvault` must do: read ChessBase databases (classic `.cbh` family, 2CBH `.2cbh` family, `.cbv`/`.cbz` archives) safely and fast, decode games into 16-bit `moves2` streams through `gigachess`, and expose metadata and annotations — read-only, MIT-licensed, zero-allocation in hot paths.

## Requirements

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

### Requirement: Game headers decode to a stable record type

The library SHALL decode each `.cbh` record into a `GameHeader` covering at least: game id, flags (game / guiding text / deleted), move and annotation offsets, white/black player refs, tournament, annotator, source refs, date (year/month/day with unknowns preserved), result, line evaluation, round/subround, Elo pair, ECO, medals, flags word, and mainline move count.

#### Scenario: Guiding text records

- **WHEN** record flags mark a guiding text
- **THEN** it is exposed as a guiding text, not a game, sharing the game id space.

#### Scenario: Date with unknown parts

- **WHEN** the packed date has a zero month
- **THEN** the decoded date represents "year only" without inventing values.

### Requirement: Namebase entities resolve by id

The library SHALL resolve player, tournament, annotator, source and team references from the namebase files, decoding ISO 8859-1 text and honouring the per-file byte order.

#### Scenario: Placeholder entity

- **WHEN** a field refers to an empty entity (blank user field)
- **THEN** an empty string is returned, not an error.

### Requirement: Namebase exact and prefix lookup via key table

The library SHALL build an in-memory sorted key table on first lookup from the mapped file, providing $O(\log N)$ binary search for exact and prefix queries across players, tournaments, annotators, sources, and teams. The lookup SHALL return all matching records in ascending ID order even if the file's binary search tree links are corrupt or severed.

#### Scenario: Damaged namebase tree

- **WHEN** a namebase's root link or child pointers are severed or corrupt
- **THEN** `find_players` and `find_players_prefix` return every matching record in the file.

### Requirement: Early-stop walk and position search predicate

The position walk and search API SHALL accept an early-stop predicate returning `Flow::Continue`, `Flow::SkipGame`, or `Flow::Stop`. When `SkipGame` is returned, the move walk for the current game SHALL terminate immediately and proceed to the next game without decoding remaining plies.

#### Scenario: Pruned search returns the same hits

- **WHEN** a position search runs with a predicate that answers `SkipGame` only for positions from which the target is unreachable
- **THEN** it returns exactly the hits of the same search without a predicate

### Requirement: Unindexed position search stops promptly on cancel

`for_each_position_key` and `for_each_position_key_range` SHALL ask the query's cancel before each chunk and at least every 256 games within one, and SHALL stop replaying once it answers `true`. A cancelled search SHALL report `complete = false`, and its hits SHALL be a prefix of the answer per chunk, never presented as the whole of it.

#### Scenario: Cancelled before the search starts

- **WHEN** a position search starts with its cancel already raised
- **THEN** it replays no game and reports `complete = false`

#### Scenario: Cancelled mid-search

- **WHEN** the cancel is raised while a worker is inside a chunk
- **THEN** the worker stops within 256 games and the search reports `complete = false`

### Requirement: Pipelined parallel conversion with ordered reorder buffer

The parallel conversion walk SHALL decode chunks concurrently on worker threads and stream them to the sink in strict ascending chunk order through a bounded reorder buffer, recycling chunk allocations and stopping cooperatively on cancellation.

#### Scenario: Parallel output equals serial output

- **WHEN** a set is converted with the parallel walk and with the sequential walk
- **THEN** the sink receives the same games, in the same order, with byte-identical payloads

### Requirement: Games decode to moves2 via gigachess

Game decoding SHALL produce, for each game, a stream of 16-bit `moves2` moves validated on a `gigachess` board — the only chess implementation in the tree. Castling MUST use gigachess king→rook encoding (`e1h1`, `e1a1`, Chess960 included). The variation tree SHALL be exposed with push/pop structure preserved for callers that need the full tree.

#### Scenario: Exact round-trip with the source

- **WHEN** a game is decoded and re-encoded to SAN through `gigachess`
- **THEN** the SAN stream matches the original game movetext identically.

#### Scenario: Illegal move in source

- **WHEN** the source stream encodes a move that `gigachess` rejects as illegal
- **THEN** decoding fails with a typed `Move` error naming the game and ply
- **AND** it never silently produces a corrupt board.

### Requirement: Annotations attach to moves and variations

The library SHALL decode `.cba` annotation records into per-move comments: text before/after a move, NAG symbols and evaluations, arrows, squares and colours — associated with the correct node of the variation tree.

#### Scenario: Text and symbols

- **WHEN** an annotation record contains text and NAG records
- **THEN** they are attached to the corresponding moves in order.

#### Scenario: Multimedia-only annotations

- **WHEN** an annotation contains sound/video/picture records
- **THEN** decoding succeeds and exposes the record kinds without failing the game.

### Requirement: PGN output SHALL use the standard notation, not ChessBase's export quirks

The writer SHALL spell a move the way the PGN/SAN standard does wherever
ChessBase's own export departs from it, while reading the stored form exactly as
ChessBase wrote it:

- SAN disambiguation SHALL be **minimal** — a file qualifier only when no other
  legal candidate of the same type shares that file, otherwise a rank, otherwise
  both — and SHALL NOT copy ChessBase's unconditional hint for a same-type twin;
- a null move SHALL be written `--`, never ChessBase's `Z0`;
- a backslash inside a tag value SHALL be doubled, and a `FEN` tag SHALL be the
  start position the moves were played from;
- stored text SHALL be decoded by its code page rather than turned into U+FFFD.

The rationale is that each of these is ChessBase's own export convention rather
than the standard: `Z0` is not PGN (the standard reserves `--` for a move that
changes nothing), and a disambiguation hint is by definition redundant, so
writing the minimal form loses no information and stays parseable. Reading stays
liberal — both null-move spellings and both hinted and unhinted SAN are accepted
on input — so the difference is one of style, not meaning. `docs/format-spec.md`
§10 carries the full table with the per-item evidence and cost.

#### Scenario: A twin piece does not earn a hint

- **WHEN** two pieces of the same type can legally reach one square and the
  move is already identified
- **THEN** the SAN carries no file or rank qualifier
- **AND** the gold comparison counts that game in its deliberate
  over-disambiguation class rather than as a regression (11,977 of 419,385).

#### Scenario: A null move is written as the standard spelling

- **WHEN** a record's move is the null-move word
- **THEN** the movetext contains `--` and not `Z0`
- **AND** the gold comparison maps `Z0` to `--` before comparing, so the
  deviation costs no differences.

#### Scenario: A tag value stays valid PGN

- **WHEN** a player's name contains a backslash
- **THEN** the tag writes it doubled, which is what the standard requires.

### Requirement: PGN export streams without intermediate text

The library SHALL export a database or a selected game range to PGN as a streaming writer, generating SAN through `gigachess` only at the output boundary. With memory mapping disabled, peak resident memory of a full-database export SHALL stay in the tens of megabytes (buffers only), independent of database size.

#### Scenario: Large export

- **WHEN** exporting a database of millions of games
- **THEN** memory stays bounded (streaming; no whole-database materialization).

#### Scenario: Large export without memory mapping

- **WHEN** exporting Mega Database 2025 (11,151,119 records) with mapping disabled
- **THEN** the PGN bytes are identical to the mapped export
- **AND** peak resident memory stays below 200 MB at any thread count.

### Requirement: `.cbv` and `.cbz` archives are readable

The library SHALL list and extract `.cbv` archives (header, member table, and
all four block modes: stored, LZ, Huffman, and Huffman-then-LZ) and SHALL decrypt
`.cbz` archives given the user password (legacy DES scheme), implementing the
container **clean-room** from verified facts.

Extraction SHALL cover **every member** of a conforming archive, and the decoded
bytes SHALL equal those of the reference implementation run as a separate
process, on **every member of every test corpus**. A member the reader cannot
decode SHALL be reported as a typed error and never written; it SHALL NOT be
counted as extracted, and the library's own reporting SHALL state the extractable
share in **bytes** as well as in member count, because a large member count of
small members is not progress toward reading a database.

The block mode SHALL be read **per block**, not per member, because a member's
stream may switch mode between its blocks.

#### Scenario: List members

- **WHEN** `Archive::open("Mega.cbv")` is called
- **THEN** member names and sizes are reported without extracting.

#### Scenario: Extract to a directory

- **WHEN** extraction is requested
- **THEN** member files are written with correct sizes and a checksum
- **AND** the original archive is untouched.

#### Scenario: Wrong password for `.cbz`

- **WHEN** the password does not decrypt the archive
- **THEN** the operation fails with a typed `WrongPassword` error.

#### Scenario: A member's blocks mix modes

- **WHEN** a member's stream switches mode between its blocks
- **THEN** each block is decoded with the mode its **own** payload names, since
  the mode belongs to a block rather than to a member.

#### Scenario: A block names a mode the format does not define

- **WHEN** a block's mode byte falls outside `0x00`–`0x03`
- **THEN** the member is refused with a typed corruption error naming the offset,
  because every defined mode is decoded and an unknown one is damage rather than
  a missing feature.

#### Scenario: A password that is not exactly eight bytes long

- **WHEN** a `.cbz` is opened with a password shorter or longer than eight bytes
- **THEN** the key is derived by **repeat**ing the password when it is shorter and
  by **fold**ing it when it is longer, as the format specifies
- **AND** the archive opens, verified against the reference's own samples for
  each of the three rules.

#### Scenario: A member name that would escape the destination

- **WHEN** a member's name is absolute, or contains a `..` component
- **THEN** the archive is refused **before any file is written**, so a refused
  archive leaves nothing behind.

#### Scenario: Reporting what the reader can do

- **WHEN** a caller asks what an archive yields
- **THEN** the share of members and the share of bytes are both reported
- **AND** a member that cannot be decoded is named rather than counted

- **AND** the parity comparison SHALL be performed separately, by the lead

### Requirement: CLI covers the workflows

The `cbh` CLI SHALL provide `info` (database summary), `verify` (full decode with an error report), `pgn` (export with ranges), `games` (metadata listing) and `archive` (list/extract), with stable machine-readable output via `--json`. The `verify` and `pgn` commands SHALL accept `--no-mmap` (plain reads instead of memory mapping) and `--no-wide` (never open `.cbj`).

#### Scenario: Verify a database

- **WHEN** `cbvault verify Mega.cbh` runs
- **THEN** it reports games decoded, games failed (with ids) and positions checked
- **AND** it exits non-zero when failures exceed the configured threshold.

#### Scenario: Low-memory export flags

- **WHEN** `cbvault pgn Mega.cbh out.pgn --no-mmap --no-wide` runs
- **THEN** the PGN bytes equal the default export over the same range
- **AND** the export report still goes to stderr so stdout stays pure PGN.

#### Scenario: Failed games are named

- **WHEN** a `pgn` export has failures
- **THEN** the report lists every failed game id with its typed error
- **AND** a game whose moves will not write is counted as a failure, never as an exported game.

### Requirement: Zero-allocation hot paths

Decoding SHALL reuse caller-provided buffers and MUST NOT allocate per move or
per game in the hot path; game headers accessed during batch iterations SHALL
borrow slices directly from mapped or buffered headers without cloning owned
structures; entity names read on the export path SHALL be decoded into
caller-owned reusable buffers rather than fresh strings, and PGN tags,
annotation tokens and comment text SHALL be written into the writer's own
reused buffers; strings (SAN/FEN/PGN) are generated only at output boundaries.
Benchmarks SHALL demonstrate no regression against the recorded baseline, and
performance work SHALL be verified with before/after measurements recorded in
`benchmarks/baseline.json` and `docs/format-spec.md`.

#### Scenario: Streaming decode
- **WHEN** decoding all games of a fixture with a reused buffer
- **THEN** allocations after warm-up are limited to documented, amortized cases (for example growing output buffers).

#### Scenario: Zero-Copy Header Access
- **WHEN** inspecting headers within a batch
- **THEN** the returned header view borrows the underlying 46-byte slice without heap allocation or copying.

#### Scenario: Zero-Allocation Export of a Game
- **WHEN** a game with tags, a main line, variations and annotations is written with a warmed writer
- **THEN** no heap allocation occurs for the game, and the writer's own buffers grow only when a game is larger than any seen before.

### Requirement: Damaged input never panics

Every parser entry point SHALL return typed errors; `unsafe` code requires a justification comment; fuzz targets SHALL exercise every file-type reader and the archive container.

#### Scenario: Truncated file

- **WHEN** a file is truncated mid-record
- **THEN** the reader reports the truncation with file and offset
- **AND** it never panics.

### Requirement: Move Tree Decode Throughput

The decoder SHALL verify and walk stored classic games without allocating heap buffers per game, achieving single-threaded sequential move-decode throughput greater than or equal to 250,000 records per second on Apple Silicon M-series processors.

#### Scenario: Single-Threaded Verification Budget
- **GIVEN** a classic ChessBase database on local disk (such as Mega Database 2025)
- **WHEN** decoded sequentially on a single worker thread using `gigachess`
- **THEN** decoding throughput SHALL exceed 250,000 records per second on Apple Silicon M-series processors, and peak resident memory SHALL not exceed 10 MiB.

### Requirement: Batched Move Record Reading

The database layer SHALL provide batched span reads for game move streams (`.cbg`), allowing sequential readers to fetch thousands of contiguous game payloads in bounded multi-megabyte I/O chunks.

#### Scenario: Contiguous Span Reading
- **GIVEN** an open `.cbh` and `.cbg` pair
- **WHEN** reading a batch of consecutive records up to `MAX_BATCH_RECORDS`
- **THEN** the reader SHALL issue at most two large reads (one for headers, one for the move record span) when move offsets are contiguous, falling back to individual record reads only when offset span exceeds `MAX_BATCH_SPAN`.

### Requirement: Parallel Database Replay and Verification

The database processing layer SHALL support multi-threaded verification and replay utilizing Rayon work-stealing parallelism, matching the concurrency stack used in `gigachess` (ADR-002) and downstream consumer applications.

#### Scenario: Rayon-Parallel Processing
- **GIVEN** an open database and a Rayon thread pool
- **WHEN** iterating over games in parallel across multiple worker threads
- **THEN** all games SHALL be decoded and verified independently with zero thread contention, scaling near-linearly with available CPU cores.

### Requirement: PGN export throughput budget

The library SHALL export PGN at a documented single-threaded throughput on a
classic database of millions of games, and the budget SHALL be verified by a
Criterion benchmark recorded in `benchmarks/baseline.json`. The rendered output
SHALL be byte-for-byte identical to the output of the same writer before this
change, and the gold-standard comparison over the reference database SHALL
report no more differing games than before this change.

#### Scenario: Single-threaded whole-database export

- **GIVEN** a classic ChessBase database of at least 10,000,000 game records
  (such as Mega Database 2025)
- **WHEN** the whole database is written as PGN on one worker thread
- **THEN** throughput SHALL be at least 65,000 records per second (at most
  171.9 s for 11,149,379 records, the measured pre-change baseline)
- **AND** peak resident memory SHALL not exceed 2 GiB.

#### Scenario: Gold-standard parity is preserved

- **GIVEN** the reference database and its ChessBase PGN export
- **WHEN** the exporter's output is compared game by game with the export
- **THEN** the number of byte-identical games SHALL be at least 407,350 of
  419,385 and the number of differing annotation comments at most 50
  (the measured pre-change baseline).

### Requirement: Parallel PGN export

The library SHALL provide a multi-threaded PGN export that uses Rayon
work-stealing parallelism — the same concurrency stack as
`verify_parallel` (ADR-004 records the design) — and SHALL write the output in
record-id order.
Parallel output SHALL be byte-for-byte identical to the sequential writer's
output for the same database, and per-game failures SHALL be collected and
reported as typed errors exactly as the sequential path does.

#### Scenario: Rayon-parallel export

- **GIVEN** an open database and a Rayon thread pool
- **WHEN** a database is exported to PGN across N worker threads
- **THEN** each worker SHALL own private writer state (tree, tag, movetext,
  annotation-index and comment-part buffers), rendering whole batches into
  pooled buffers
- **AND** a single writer stage SHALL emit the batches in record-id order, so
  the byte stream is identical to the sequential export.

#### Scenario: Byte-identical output

- **WHEN** the same database is exported sequentially and in parallel
- **THEN** both output files are byte-for-byte equal
- **AND** a damaged record produces the same typed error in both modes.

#### Scenario: Scaling budget

- **GIVEN** a classic database of at least 10,000,000 game records on an
  Apple Silicon machine with at least 8 cores
- **WHEN** the database is exported with at least 8 worker threads
- **THEN** the wall clock SHALL be at most 25 s (at least 6x faster than the
  single-threaded export)
- **AND** peak resident memory SHALL stay below 8 GiB, i.e. bounded per worker
  and not per database.

### Requirement: Entity names are read without allocation

Entity records (players, tournaments, annotators, sources) SHALL be readable
into caller-owned reusable buffers, borrowing the mapped file where the
database files are memory-mapped, and the decoding SHALL NOT allocate a `String`
or a record-sized `Vec` per lookup. The owned accessors SHALL remain available
and SHALL return the same text as the borrowed ones.

#### Scenario: Tag path over a large database

- **WHEN** PGN tags are written for every record of a database with millions of
  records
- **THEN** each player and tournament name is decoded into a buffer owned by
  the writer and reused for the next record
- **AND** no allocation occurs per record after warm-up.

#### Scenario: Owned and borrowed access agree

- **WHEN** a name is read through the borrowed and through the owned accessor
- **THEN** both return the same string, including for a field holding
  Windows-1252 bytes and for a blank record.

### Requirement: Movetext formatting avoids the formatting machinery

Movetext and tag formatting SHALL NOT use the `core::fmt` machinery on the
per-move and per-tag paths: move numbers, Elos, dates, round/sub-round text and
annotation tokens SHALL be written into reused string buffers by dedicated
`#[inline]` writers, and the output SHALL be identical to the `core::fmt`
rendering it replaces.

#### Scenario: Whole-database movetext

- **WHEN** a database of millions of games is exported to PGN
- **THEN** the movetext equals the output produced by the formatting-machinery
  writer
- **AND** the per-move path performs no formatting-machinery calls.

### Requirement: SAN rendering reuses the position the walk already made

A sink that renders SAN SHALL take the body of a move's notation from the
position the move is played from and the check/mate suffix from the position it
leaves behind, and a move sink SHALL declare whether it reads the chess core's
cached checkers bitboard, so the walk makes moves with a checkers-maintaining
primitive whenever it does.

The cached checkers SHALL NOT be read after a make that leaves it stale: a
caller that trades cache maintenance for speed and then asks the chess core a
question that reads the cache gets a wrong answer, not a fast one. Every such
fast path SHALL be pinned by a test that would fail if the cache were stale.

#### Scenario: The suffix costs no second make

- **GIVEN** an exporter whose walk makes every move to reach the next ply
- **WHEN** it renders a move
- **THEN** the body is rendered from the pre-move position and the suffix from
  the post-move position the walk already holds
- **AND** no second board copy, make or unmake happens for the notation
- **AND** the rendered token equals what the single-call renderer produces for
  the same move.

#### Scenario: Disambiguation is never answered from a stale cache

- **GIVEN** a walk that makes moves with a primitive that does not maintain the
  cached checkers
- **WHEN** the chess core generates moves for that board — SAN disambiguation
  among them
- **THEN** either the walk maintains the cache, or no generation happens on that
  board
- **AND** a property test over the reference database asserts the rendered
  notation equals the single-call renderer's for every ply of every game.

### Requirement: A performance change is measured on the real workload and gated on bytes

A change to a hot path SHALL be measured three ways and recorded in
`benchmarks/baseline.json`: an in-process benchmark paired against its saved
baseline (so a busy machine affects both sides equally), a whole-database run of
the reference database, and a fresh profile of where the time now goes. The
numbers SHALL also be written into the format specification.

A change that alters output SHALL be gated on byte-level evidence before it is
believed faster: the gold-standard comparison over the reference database (and
the annotation-read and move-decode error counts), plus a byte-equality check
between the sequential and the parallel writer. A difference found by such a
gate SHALL be explained, not absorbed — a differing byte count is a finding to
chase down, and chasing it may reveal a correctness bug that the performance
work uncovered.

#### Scenario: Paired measurement on a loaded machine

- **GIVEN** a machine that is not idle
- **WHEN** a hot-path change is measured
- **THEN** the in-process benchmark runs both the old and the new implementation
  in the same process against the same saved baseline
- **AND** the whole-database numbers are reported together with the machine's
  load, and a whole-database run that repeats within a few per cent is reported
  as a range rather than a single figure.

#### Scenario: A byte-level gate finds a bug

- **GIVEN** a hot-path change that is expected to be output-neutral
- **WHEN** the whole-database output differs from the pre-change output
- **THEN** the difference is localised to the games concerned and explained
  before the change is accepted
- **AND** where the new output is the correct one, the finding is recorded and
  the earlier behaviour is documented as the bug.

### Requirement: The library compiles on every platform a consumer builds on

Every crate in this workspace SHALL compile on Linux, macOS and Windows with
default features and with `--all-features`, and every `#[cfg]`-gated body SHALL
be compiled by CI on the target it is gated for. Code that names an
OS-specific API of a dependency SHALL be gated on the operating system that
provides it, and not only on the feature that pulls the dependency in. A
performance hint, a scheduling hint or any other advisory call MAY be omitted on
a target where the dependency has no equivalent, provided the omission is
unobservable to a caller.

#### Scenario: A default-on feature does not imply a Unix-only body

- **WHEN** a default-on feature pulls in a dependency whose API is `#[cfg(unix)]`
- **THEN** every use of that API is additionally gated on the operating system
- **AND** the crate compiles on Windows with default features and with
  `--all-features`.

#### Scenario: A platform-gated body is built by CI

- **WHEN** a function is gated `#[cfg(windows)]`
- **THEN** a CI job runs on `windows-latest` and compiles all targets, so the body
  is type-checked, linted under `-D warnings` and exercised by the suite
- **AND** a `#[cfg(unix)]` body is compiled by the Linux and macOS jobs.

#### Scenario: An advisory call is dropped where it does not exist

- **WHEN** a target has no equivalent of an advisory call
- **THEN** the call is omitted
- **AND** no decoding result, public API, error type or output byte differs from
  a target where the call is made.

#### Scenario: A CI job does not fail for a shell difference

- **WHEN** a job runs steps that use shell features absent from the platform's
  default shell
- **THEN** the job pins the shell its steps need
- **AND** a step fails only for a reason in the code, never for the shell it runs
  under.

### Requirement: First-class PGN export on Database

The `Database` façade SHALL provide high-level methods `game_pgn` and `game_pgn_with`
that return the complete, valid PGN string for a given game ID, including tags,
movetext, and annotations (when requested).

#### Scenario: Exporting a standard game with moves
- **WHEN** `db.game_pgn(id, &mut buf)` is called on a game with moves
- **THEN** it returns a PGN string containing all mandatory PGN tags and the complete movetext.

#### Scenario: Exporting a move-less score-only game
- **WHEN** `db.game_pgn(id, &mut buf)` is called on a valid historical game with 0 moves
- **THEN** it returns a valid PGN string with `[PlyCount "0"]` and the terminal game result
- **AND** no error or panic is raised.

### Requirement: One name for the project, and honest names for formats

The project, its crates, its binary, its capability and its environment
variables SHALL be named `cbvault` or `cbvault-*`, and no current-facing
artifact SHALL name the project by a retired project name. Names that refer to a file
format — a file extension, a format-specific reader module, a role, a format
fact or a documented byte layout — SHALL keep their `cbh`/`2cbh`/`cbv`/`cbz`
spelling. Historical records SHALL NOT be rewritten: the archived change
directory, the git history, vendored upstream sources and third-party
attribution entries keep the names they were written with.

#### Scenario: The reader module keeps its format name

- **WHEN** a consumer reads a classic database's headers
- **THEN** the path is `cbvault_format::cbh::Headers`
- **AND** the crate that provides it is named `cbvault-format`.

#### Scenario: A stale name is caught

- **WHEN** the tracked tree is searched for the old crate names, crate paths or
  environment variables
- **THEN** no hit is found outside the explicit allowlist of historical
  records.

#### Scenario: Attribution is untouched

- **WHEN** the rename is applied
- **THEN** third-party and upstream project names in the provenance ledger and
  the notices file are byte-identical to before.

### Requirement: A `.cbz` archive opens with a password

The library SHALL open a password-protected `.cbz` container from a path and a
password. The scheme SHALL be DES in ECB over the whole file with the key taken
as the password's first eight bytes, zero-padded when the password is shorter,
and the container's magic SHALL be enciphered along with the rest of the file.

Opening SHALL verify the password by deciphering the first block and checking it
against the container magic, and SHALL NOT read the remainder of the file to do
so.

#### Scenario: A protected archive opens with the right password

- **WHEN** a `.cbz` is opened with the password it was written under
- **THEN** its member table is parsed and its members are listed
- **AND** a member decodes to the same bytes the equivalent `.cbv` yields

#### Scenario: Opening reads one block, not the archive

- **WHEN** a protected archive is opened
- **THEN** the number of bytes deciphered is the eight of the header check
- **AND** the data pool is not read

### Requirement: A wrong password is reported as a wrong password

When the deciphered first block is not a container magic the library SHALL
report `Error::WrongPassword`, naming the archive. It SHALL NOT report a corrupt
archive, a truncated file or a missing codec, because the file is none of those:
it is a well-formed container under a key the caller did not supply.

#### Scenario: A wrong password is named as such

- **WHEN** a `.cbz` is opened with a password that is not the right one
- **THEN** the error is `WrongPassword` and names the archive
- **AND** the error is not a corruption, a truncation or a missing codec

#### Scenario: A password of another length is still a wrong password

- **WHEN** a `.cbz` is opened with a password shorter or longer than the right one
- **THEN** the result is `WrongPassword`
- **AND** the reader does not panic

### Requirement: A protected archive deciphers on demand

The reader SHALL hold the key and SHALL decipher each member stream as it is
read, rather than deciphering the archive into memory when it is opened. Listing
the members of a protected archive SHALL read only the member table, and SHALL
NOT decipher the data pool.

#### Scenario: Listing a protected archive reads only the table

- **WHEN** the members of a protected archive are listed
- **THEN** no member stream is deciphered
- **AND** the cost does not grow with the size of the data pool

#### Scenario: A member at an unaligned offset deciphers correctly

- **WHEN** a member's stream does not begin on an eight-byte boundary
- **THEN** the read is aligned down, deciphered, and sliced
- **AND** the member decodes to the same bytes an unencrypted archive yields

### Requirement: Extraction refuses to guess

A member whose compression mode this build cannot decode SHALL be reported as
`Error::CodecUnavailable`, naming the member and its mode. The library SHALL NOT
write partially decoded or guessed bytes to disk, and SHALL NOT report a
partial extraction as a complete one.

#### Scenario: An undecodable member stops the extraction

- **WHEN** extraction reaches a member in a mode no codec handles
- **THEN** it reports `CodecUnavailable` naming the member and its mode
- **AND** it does not report a partial extraction as a complete one

#### Scenario: A stored member whose head lies is rejected

- **WHEN** a stored member's stream body is not the length the table states
- **THEN** it is reported as corrupt rather than written out as the wrong bytes

### Requirement: Compression modes are named, not assumed

The codec table SHALL name every mode observed in the reference archive and
SHALL state, per mode, whether it is decoded. A mode marked undecoded SHALL
have the differential work that was attempted and the reason it was not closed
recorded next to it, so the next reader inherits the evidence rather than the
dead end.

#### Scenario: Every observed mode is named in the codec table

- **WHEN** the codec table is read
- **THEN** each mode observed in the reference archive is listed
- **AND** each is marked decoded or not, with the evidence for the claim

#### Scenario: An unclosed mode keeps its differential evidence

- **WHEN** a mode remains undecoded
- **THEN** the attempts made and the reason they did not close it are recorded
    next to it

### Requirement: A clean-room specification records only non-copyrightable facts

The project SHALL maintain a specification of the reference `.cbv`/`.cbz`
behaviour that contains only material that is not copyrightable: byte layouts,
field semantics, algorithms expressed as procedures, constants, observable
control flow, error conditions, and measured observations.

It SHALL NOT contain any Rust, C or pseudocode mirroring the reference's
structure; any identifier, module, file or type name from the reference; any
comment, doc comment or error-message string; the order in which the reference's
functions run; or anything else that would let a reader reconstruct the
reference's source.

Every claim in the specification SHALL carry an evidence note, and every citation
SHALL be to an observation — a byte range, a run of the reference binary, or a
published description — and never to a line of source.

#### Scenario: A paragraph is tested against the source

- **WHEN** any paragraph of the specification is reviewed
- **THEN** the reviewer asks whether it could have been written by someone who
  never opened the reference
- **AND** a paragraph that could not is removed

#### Scenario: A behaviour is stated

- **WHEN** the specification states what the reference does under some input
- **THEN** it is stated as a rule with a condition and an outcome
- **AND** it is not accompanied by code that mirrors how the source expresses it

### Requirement: The implementer is separated from the reference source

Code that implements the reference's behaviour SHALL be written by an agent that
has no access to the reference source, the reference's tests, or the reference
binary. The separation SHALL be enforced by a mechanism, and SHALL be recorded,
not merely asserted.

The specification SHALL be reviewed and frozen before the implementer begins, so
that the specification cannot be altered after the barrier is in force.

#### Scenario: The implementer begins work

- **WHEN** the implementer agent is started
- **THEN** it has been given the frozen specification and the existing crate
- **AND** it has no access to the reference source, tests or binary
- **AND** the fact is recorded before it starts, not afterwards

#### Scenario: The specification is still being edited

- **WHEN** the specification changes after the implementer has started
- **THEN** the change is refused, because it would defeat the barrier
- **AND** the change is deferred until the implementer has finished

### Requirement: Parity is demonstrated against the reference process

The project SHALL demonstrate parity by comparing its own output with the
reference implementation run as a separate process, for every member of the
reference corpus, and SHALL require byte equality on every member.

A member known to differ for a reason outside the codec — a file the reference
rewrites locally, or a special case — SHALL be named and accounted for rather
than excluded silently.

#### Scenario: The corpus is compared

- **WHEN** the differential test runs
- **THEN** every member of the corpus is compared by content, not by size
- **AND** any member that differs is reported by name

#### Scenario: The implementer cannot check its own work

- **WHEN** the implementer completes
- **THEN** it SHALL NOT have run the reference binary

### Requirement: Header filter vocabulary

The library SHALL provide header predicates for rating ranges, played-year
ranges, results, rounds and ECO codes, in addition to the existing name and
rating-threshold predicates. A rating predicate SHALL express an **open** range
where either bound is absent, and an absent bound SHALL NOT be treated as zero.

Every predicate SHALL be a comparison of a field the 46-byte header record
already holds, so a tag search remains one pass over `.cbh` and SHALL NOT open the
moves file.

#### Scenario: A rating range with one open end
- **WHEN** a filter carries a rating range with a lower bound and no upper bound
- **THEN** games at or above the lower bound match
- **AND** a game below it does not, including an unrated game whose rating is zero.

#### Scenario: A rating threshold is a special case of a range
- **WHEN** a threshold predicate and the equivalent open-ended range are evaluated against the same header
- **THEN** they select the same games.

#### Scenario: An inverted range matches nothing
- **WHEN** a range's lower bound is above its upper bound
- **THEN** no game matches, rather than every game.

#### Scenario: A date range is compared on the year
- **WHEN** a played-year range is given
- **THEN** games whose packed date falls within it match
- **AND** a game with no date does not match, because "unknown" is not "in the range the user asked for".

#### Scenario: Result, round and ECO predicates
- **WHEN** a filter carries a result, a round (optionally a sub-round) or an ECO code
- **THEN** games with that exact value match
- **AND** an ECO code with no sub-code matches every sub-code of that opening
- **AND** a game with no ECO, a Chess960 start, or an unrecognised value does not match an ECO predicate.

#### Scenario: A text ECO code is parsed as a filter
- **WHEN** text such as `"B20"` or `"b"` is given
- **THEN** it resolves to the header's ECO code, case-insensitively and ignoring surrounding space
- **AND** text that is not an ECO code yields no filter, so a caller can report the typo rather than build a predicate that silently matches nothing.

#### Scenario: No criterion forces a read of the moves file
- **WHEN** a filter built from these predicates runs
- **THEN** only the header file and the namebases are read.

### Requirement: Entity name resolution returns every matching id

The library SHALL resolve a name to **every** entity id whose sorted name field
equals it, not to a single one. A namebase is keyed by last name for a player and
by title for a tournament, so one key may have many records.

An absent name SHALL resolve to an empty set. An empty set SHALL produce a
predicate that matches nothing, never one that matches everything.

#### Scenario: A shared last name resolves to all its players
- **WHEN** several players share a last name and that name is resolved
- **THEN** every one of their ids is returned, in ascending order.

#### Scenario: A longer name is not a prefix match
- **WHEN** a name that is a strict prefix of a stored name is resolved
- **THEN** the longer stored name is not returned.

#### Scenario: An absent name resolves to nothing
- **WHEN** a name the file does not hold is resolved
- **THEN** the result is empty, and a predicate built from it matches no game.

#### Scenario: Every namebase resolves its whole match set
- **WHEN** a name is resolved in the player, tournament, annotator, source or team namebase
- **THEN** every matching id in that namebase is returned.

### Requirement: Criteria compose into one query

The library SHALL provide a conjunction builder so that several criteria are
expressed as one filter. An empty conjunction SHALL match every game. Several
criteria SHALL narrow each other, so a query carrying two criteria returns fewer
games than either criterion alone.

#### Scenario: Two criteria narrow each other
- **WHEN** a filter carries a player and a rating range
- **THEN** games matching both are returned
- **AND** the result is smaller than the result of either criterion alone.

#### Scenario: An empty conjunction matches everything
- **WHEN** no criteria are supplied
- **THEN** every game matches, so clearing a consumer's search form yields the whole database rather than nothing.

#### Scenario: The result does not depend on the worker count
- **WHEN** the same filter is scanned at one thread and at many
- **THEN** the returned games and their order are identical.

### Requirement: A game's exact ply count is available to a sink

The library SHALL expose a game's exact ply count — one per move played — to a
consumer's sink, derived from the move stream the walk has already read. The
header's move-count field SHALL NOT be used for this, and the documentation SHALL
state that it counts moves rather than plies and is capped.

#### Scenario: The ply count is the move count, not twice it
- **WHEN** a sink reads a game's ply count
- **THEN** it equals the number of moves in the main line
- **AND** it equals the key count minus one when keys were requested.

#### Scenario: Asking for keys is distinguishable from a game having none
- **WHEN** a sink did not request position keys
- **THEN** the absence of keys is reported as such, rather than as a game with no positions.

### Requirement: Conversion walks are cancellable

The conversion walk SHALL honour a sink-provided cancellation signal and stop
early with partial statistics marked incomplete.

#### Scenario: Sequential walk stops at the signal
- **WHEN** a sink's `cancelled()` returns `true` after N games
- **THEN** `for_each_game` / `for_each_range` deliver no further games
- **AND** the returned `ConvertStats` reports the N delivered games with
  `complete = false`.

#### Scenario: Parallel walk stops at the signal
- **WHEN** a sink's `cancelled()` returns `true` mid-run
- **THEN** `convert_parallel` delivers no further waves
- **AND** the returned `ConvertStats` has `complete = false`
- **AND** games already delivered keep ascending game-number order.

#### Scenario: No signal means today's behaviour
- **WHEN** a sink does not override `cancelled` (default `false`)
- **THEN** all three walks deliver every game and report `complete = true`.

### Requirement: PGN text splits into games by one rule

The library SHALL split PGN text into games at a blank line outside a comment
whose next non-blank line is a tag pair, byte-based over `&[u8]` with lossy
UTF-8 output.

#### Scenario: Comment and RAV shapes do not split
- **WHEN** a blank line or `[` line occurs inside a `{ … }` comment or RAV
- **THEN** no boundary is produced there.

#### Scenario: CRLF and missing trailing newline
- **WHEN** input uses `\r\n` or lacks a final newline
- **THEN** the same games are produced with normalized `\n`.

### Requirement: A read-only database façade

The library SHALL expose a `Database` façade over a validated ChessBase file
set, opened from a base name or any member file, reporting its format
generation, its game count and which optional members it found. It SHALL hold
no writable handle on any source file and SHALL NOT create, modify or delete
anything inside the database's directory.

#### Scenario: Opening a complete set

- **WHEN** `Database::open("Mega Database 2025")` is called on a complete classic set
- **THEN** the database reports generation `Classic` and the game count
- **AND** no file in the directory is opened for writing.

#### Scenario: A member file is given instead of the base name

- **WHEN** `Database::open("Mega Database 2025.cbh")` is called
- **THEN** the siblings are resolved case-insensitively from the same directory
- **AND** the result is identical to opening the base name.

#### Scenario: The mandatory set is incomplete

- **WHEN** a mandatory member is missing
- **THEN** opening fails with a typed error naming the file and its role
- **AND** nothing is left open.

### Requirement: Conversion streams records to a consumer sink

The library SHALL convert a database by streaming each game to a
consumer-implemented sink: the borrowed header fields, the resolved entity
names, the main line as 16-bit `moves2`, and — when the sink asks for them —
the per-position Polyglot keys. The library SHALL NOT write target database
files, SHALL NOT depend on an embedded or external database engine, and SHALL
allocate no per-game row, string or vector across the sink boundary.

#### Scenario: A moves-only conversion

- **WHEN** a sink whose `wants_keys` is `false` consumes a database
- **THEN** every game arrives with its `moves2` main line and resolved tags
- **AND** the walk uses the fast move that maintains neither hash nor checkers.

#### Scenario: An indexed conversion

- **WHEN** a sink whose `wants_keys` is `true` consumes a database
- **THEN** every game also arrives with one Polyglot key per position reached,
  in position order
- **AND** the keys equal those an independent replay of the same `moves2`
  produces.

#### Scenario: Annotations are not wanted

- **WHEN** a sink whose `wants_annotations` is `false` consumes a database
- **THEN** no annotation record is read
- **AND** the conversion is faster than the same conversion with annotations on.

#### Scenario: A game that cannot be decoded

- **WHEN** a record is damaged beyond recovery
- **THEN** the sink's `failed` callback receives the game id and a typed error
- **AND** the conversion continues with the next game.

### Requirement: Parallel conversion is ordered by game id

The library SHALL provide a parallel conversion that delivers games to the
sink in ascending game-number order for any thread count, by cutting the id
space into chunks, decoding each chunk with private buffers, and emitting the
chunks in order. The sink SHALL be called from one thread at a time, and the
sink call sequence SHALL be identical to the sequential conversion's.

#### Scenario: One thread, many threads

- **WHEN** the same database is converted with 1, 2 and 10 threads
- **THEN** the sink observes the same games in the same order
- **AND** the per-game payloads are identical.

#### Scenario: A bounded number of chunks in flight

- **WHEN** a conversion runs on a database larger than memory would allow
- **THEN** at most one wave of chunk buffers per worker is resident
- **AND** buffers are recycled between chunks.

### Requirement: Read-only serving of a database

The library SHALL list a database's games from its header and entity files
alone, SHALL return one game's moves and annotations on demand, and SHALL NOT
read the moves file while listing. Tag queries and position queries are the
consumer's to answer; the library SHALL supply the streams they need.

#### Scenario: Listing games

- **WHEN** a consumer iterates the games of an 11-million-game database
- **THEN** each game yields its header fields and resolved player, tournament,
  annotator and source names
- **AND** the moves file is never opened.

#### Scenario: One game with its annotations

- **WHEN** a consumer asks for game *n* including annotations
- **THEN** it receives the header, the `moves2` main line and the annotation
  items, each carrying the position it belongs to
- **AND** nothing is allocated per item.

#### Scenario: Entity tables for interning

- **WHEN** a consumer needs to intern names into its own tables
- **THEN** it can iterate every player, tournament, annotator and source
  without materialising the tables.

### Requirement: Position keys are a by-product of conversion

A consumer that maintains a position index SHALL be able to receive every
position's Polyglot key from the conversion pass, and the cost of that SHALL
be bounded: the keyed conversion SHALL stay within 15 % of the moves-only
conversion for the same database. An unindexed position search SHALL replay the
source directly with the fast move, in parallel, with progress and
cancellation.

#### Scenario: One pass beats two

- **WHEN** the keyed conversion is measured against a moves-only conversion and
  against a separate pass that would walk the same source again
- **THEN** the keyed conversion's overhead is reported as a fraction of the
  moves-only pass
- **AND** it is materially smaller than the cost of a second walk.

#### Scenario: Cancelling an unindexed search

- **WHEN** a consumer cancels an unindexed position search
- **THEN** the search stops within one chunk
- **AND** no partial result is reported as complete.

### Requirement: A tag search over a raw database is a scan, and this library runs it

The library SHALL answer which games match a predicate over a database's headers
without converting it: it SHALL read the header records in bulk, evaluate the
predicate during the scan, run the scan in parallel, and emit matching games in
ascending game-number order. It SHALL resolve entity names to ids by lookup in
the entity tables, so a name predicate costs one lookup rather than a resolution
per record, and it SHALL return the names of the matching games as slices of the
mapped entity data without copying them. The scan SHALL NOT open the moves file.

A position query is different in kind and SHALL be served two ways: by replaying
the source with progress and cancellation, and by handing the consumer each
position's Polyglot key so the consumer's own index can answer it. The library
SHALL NOT define, write or read a position index format.

#### Scenario: Filtering a whole database

- **WHEN** a consumer filters the games of an 11-million-game database by an
  integer predicate such as an Elo threshold
- **THEN** the matching game numbers arrive in ascending order
- **AND** the same predicate over the same range yields the same set on any
  thread count.

#### Scenario: A name predicate resolves once

- **WHEN** a consumer searches by a player's name
- **THEN** the name resolves to one entity id, and the scan compares that id per
  record
- **AND** the player's and opponent's names for each match are returned as
  borrowed slices, with no per-game string allocated.

#### Scenario: Listing never reads the moves

- **WHEN** a consumer lists or filters a database
- **THEN** the moves file is never opened.

#### Scenario: An unindexed position search can be stopped

- **WHEN** a consumer runs a position search by replay and cancels it
- **THEN** the search stops within one chunk and reports no partial result as
  complete.

### Requirement: Public library contracts and documentation remain consumer-neutral

The library's public documentation, README, crate root headers, module comments, integration examples, and metadata SHALL NOT reference or disclose unreleased proprietary downstream product names, internal schemas, or private target formats. All conversion traits, database façade methods, and command-line interfaces SHALL be documented in terms of generic downstream callers, consumers, and data pipelines.

#### Scenario: Crate documentation uses generic consumer terminology

- **WHEN** a developer inspects public crate documentation, module headers, or `README.md`
- **THEN** all conversion patterns describe generic downstream consumers
- **AND** no references to unreleased proprietary product names or private schema formats appear.

#### Scenario: Conversion examples demonstrate generic consumption

- **WHEN** a developer inspects integration examples such as `examples/convert.rs`
- **THEN** the example demonstrates populating caller-defined application structures
- **AND** the code does not claim or quote internal schemas of unreleased products.

### Requirement: Native versioned 2CBH read-only decoding

The library SHALL preserve all valid 2CBH move and annotation data through a generation-aware public reader and SHALL never silently omit requested, valid annotations or reinterpret a malformed record as a successful unannotated game. When a valid type is not yet supported, it SHALL report a typed unsupported-feature error with record ID, offset and kind; that error SHALL fail the complete parity gate. Corruption SHALL also fail with a distinct typed error and SHALL not panic.

#### Scenario: Unknown typed annotation
- **GIVEN** a v2 annotated game containing an unknown annotation type or invalid length
- **WHEN** an item iterator, PGN writer, verification, public game API or conversion sink processes the game
- **THEN** the error propagates, source ID is counted as failed, and complete export cannot be reported as successful
- **AND** the iterator does NOT silently terminate and the PGN writer does NOT emit the bare game instead.

#### Scenario: Invalid 2CBA trailer is not accepted as an empty annotation tail
- **GIVEN** a 2CBA record with a valid end marker followed by bytes not accounted for in the frame
- **WHEN** a caller iterates annotations or requests lossless checked text extraction
- **THEN** the decoder raises an error rather than silently treating the remaining bytes as absent
- **AND** the diagnostic-only `texts()` helper is not used as a validation oracle; `try_texts()` returns annotation errors.

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

### Requirement: MIT source baseline and clean-room provenance are enforced

Any ported 2CBH implementation SHALL originate only from the verified MIT oschess `v1.0.3` commit `bffebfc498d52b4e4631aed32b2f8e8194d125ff`, retain MIT attribution, and be mapped to existing gigachess and cbvault primitives. Work informed by later AGPL oschess or unlicensed Morphy Java implementation SHALL use the reviewed two-room, facts-only clean-room process with documented provenance. No AGPL/Morphy code, comments, tests or structures SHALL be copied into the distributed MIT implementation.

#### Scenario: A later AGPL bug fix is wanted
- **GIVEN** a behavior present only in a later AGPL oschess release
- **WHEN** cbvault needs to support it
- **THEN** a separate specifier documents independently verifiable format facts and an isolated implementer writes original Rust from the reviewed specification
- **AND** a license/provenance audit and independent oracle comparison are completed before merging.

### Requirement: PGN parity is proved against real local ChessBase export

The strict local gold gate SHALL enforce **byte-for-byte equality** for compared PGN bytes within the explicitly frozen ChessBase-exported sample (not unexported later games) after ONLY documented, individually justified and counted nonsemantic formatting/ChessBase-specific transformations. It SHALL also independently report raw SHA-256, raw byte length and first differing raw byte, without falsely claiming raw equality when transformations were needed. It SHALL compare every PGN tag shared by both exporters, SAN, the entire nested move tree, comments/literal Unicode, NAGs/results and comparable directives. Absent ChessBase GUI-only metadata and proprietary training directives SHALL be named and counted as **coverage exclusions**, not falsely described as byte-identical exported content; no generic similarity threshold is permitted. It SHALL prove exact source-record-to-PGN mapping, counts, duplicates, EOF and absence of illegal moves or unreported annotation failures. Any unapproved difference or unknown valid item exits nonzero.

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

#### Scenario: Full source-slot accounting and exact footer validation
- **GIVEN** a 2CBH source with more allocated header slots than exported games, including non-game and trailing records
- **WHEN** the production strict gold gate has compared the expected game count
- **THEN** it SHALL continue inspecting the entire source header range, fail on any additional eligible game and prove `compared_games + non_game_slots == source_slots`
- **AND** only an explicitly marked smoke test may stop after a sample without claiming full-corpus acceptance.

#### Scenario: Adversarial equivalence cannot mask content changes
- **GIVEN** two different comments with the same unordered characters, a duplicate PGN tag, or different FEN castling semantics
- **WHEN** the strict gold comparator evaluates them
- **THEN** it SHALL reject arbitrary character permutations and duplicate tags, and SHALL prove any FEN representation equivalence by reconstructing identical chess boards
- **AND** presentation exceptions never turn changed moves, annotations, or header values into success.

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

Mega Database 2026 with ChessBase 18 PGN is the only available real 2CBH corpus; no second licensed database is required. A complete frozen 3,410,497-game sample pass SHALL be reported as **validated on the ChessBase 18 exported sample from Mega Database 2026 only**, never universal compatibility or full-db parity for later unexported games. Rare or absent features (Chess960, unusual annotations, embedded quotations, native-only content omitted from PGN) SHALL each be labeled real-verified, synthetic-only, unverified, or unsupported.

#### Scenario: Exported sample lacks Chess960 games
- **GIVEN** the Mega source corpus does not contain a particular rare valid 2CBH feature
- **WHEN** the exported-sample gold comparison passes
- **THEN** the documentation explicitly reports that the feature was not covered by real data and may still have defects
- **AND** original synthetic fixtures exercise the known legal shape without claiming universal field validation.

### Requirement: Public artifacts describe a standalone generation-aware library

Public Rustdoc, all crate READMEs, package descriptions and the repository overview SHALL present cbvault as an independent reusable reader for both classic .cbh and modern .2cbh, rather than as an interface owned by a specific downstream application.

#### Scenario: Publishing a public crate
- **WHEN** a new crate version is prepared and documented
- **THEN** its README and Rustdoc describe both implemented database generations, the supported API and relevant coverage limits
- **AND** CI rejects downstream-product names from tracked documents and paths.

### Requirement: MIT-era upstream licensing is accurately attributed

The project SHALL identify the pinned MIT versions of oschess-cb-bridge used for classic (v1.0.0) and native 2CBH (v1.0.3) code, preserving notices while excluding source code introduced after upstream's switch to AGPL-3.0.

#### Scenario: Public provenance is reviewed
- **WHEN** documentation cites the ancestor implementation
- **THEN** it names the pinned MIT-era foundation and exact source revisions
- **AND** later AGPL-era layout facts, if applicable, are described as independently verified format facts only.

