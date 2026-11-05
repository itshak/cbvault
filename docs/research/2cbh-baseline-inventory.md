# Baseline inventory and pre-change measurements: 2CBH native reader

## 1. Inventory of existing components

### `cbvault_format::twocbh`
- **Current role:** Partial read-only implementation of container framing for `.2cbh`, `.2cbg`, `.2cba`.
- **Verified parts:** 192-byte `.2cbh` header records, 12-byte `.2cbg`/`.2cba` file headers, record framing with magic `0x1122334455667788`, `total == 34 + A + B`, self-delimiting trailer.
- **Deficiencies / Misinterpretations:**
  - Move stream marked undecoded (`GameMoves::is_decoded() == false`, `.stream() == None`).
  - Frame content assumed to start at `0x18` rather than `0x1a` (tag is 2 bytes at `0x18`, content at `0x1a`).
  - Checksum at `0x10` (big-endian 64-bit 8-part sum) treated as unknown 8-byte value.
  - Parameter `B` at `0x0c` treated as unknown codec parameter rather than spare capacity.
  - Player/tournament names assumed inline in `.2cbh` record (misidentifying rating type strings at `0x68`/`0x78` as "names") rather than entity IDs at `0x18`/`0x20` pointing into `.2lid`.
  - `.2lid`, `.2lgd`, `.2lcd` completely unread.
  - `.2cba` annotations only parsed for bulk clear-text strings rather than move/position blocks.

### `cbvault::bridge::Database` facade
- **Current role:** Read-only access facade for ChessBase databases.
- **Current behavior:** `Database::open` checks `generation_of(&stem)`; if not `Generation::Classic`, returns `Error::MissingFile { path: sibling(&stem, ".cbh"), role: Role::Headers }`.
- **Target:** Unify opening so both Classic (`.cbh`) and 2CBH (`.2cbh`) open cleanly into `Database`, reporting games, entities, moves, annotations, and streaming into `GameSink`.

### `gigachess` and `moves2`
- **Role:** Single chess core for rules, move generation, legality, FEN/SAN/UCI, Chess960, Polyglot Zobrist keys.
- **Move currency:** 16-bit `moves2` (`from | to<<6 | promo<<12`); castling encoded as king->rook square (`e1h1`, `e1a1`).
- **Null moves:** Handled as `Board::make_null_move` / `Move::NULL`.
- **Constraint:** Zero external chess engines or secondary move structures. Pinned MIT code's `chesscore` must be replaced completely by `gigachess`.

### `PgnWriter`
- **Current role:** Streaming zero-allocation PGN serialization with buffered formatting.
- **Capabilities:** Standard Seven Tag Roster + ECO/Elo/Round/Date/FEN/SetUp, mainline and nested variation parentheses, `--` null move notation, NAGs, comments, and parallel chunk export.

### Gold comparator (`crates/cbvault/examples/gold_pgn.rs`)
- **Current role:** Differential validation of cbvault PGN against native ChessBase exports.
- **Historical baseline (ADR-002):**
  - Games: 419,385 reference games.
  - Exact match: 407,350 games.
  - Deliberate justified differences: 11,977 games (pinned knight minimal SAN `Nd2` vs `Nbd2`, `Z0` vs `--`).
  - Annotation differences: 50 games.
  - Other/tags: 8 games.
  - Errors: 0 annotation-read errors, 0 move-decode errors.

## 2. Test and performance baseline

- Workspace tests: all passing (`cargo test --workspace`).
- Throughput baseline: classic CBH achieves >50,000 games/sec sequential decode and >250,000 games/sec parallel decode.
- Memory/allocation baseline: zero per-game heap allocations in the conversion hot path (`GameSink`).
