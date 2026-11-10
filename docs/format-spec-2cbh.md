# Native 2CBH format — implemented reader reference

**Status: implemented read-only (cbvault v0.1.7).** This is the current high-level format reference for the generation-aware `cbvault` and `cbvault-format` 2CBH readers. Earlier reverse-engineering notes, including superseded hypotheses, have been preserved in [historical research](research/2cbh-early-observations-2026-11-05.md). The current Rust implementation and [per-field provenance](provenance.md) are authoritative where the historical observations conflict.

## File families

| Member | Role | Current reader |
|---|---|---|
| `*.2cbh` | Fixed-length game and text headers, database record IDs, ratings and metadata | `cbvault_format::twocbh::Headers` |
| `*.2cbg` | Framed encoded move trees | `cbvault_format::twocbh::Segment`, `cbvault_chess` |
| `*.2cba` | Framed annotation and text records | `cbvault_format::twocbh::Annotations` |
| `*.2lid` | Player, tournament, annotator, source, team and tag names/IDs | `cbvault_format::twocbh::Entities` |
| `*.2lgd`, `*.2lcd`, `.ini` | Auxiliary members | Recognized; not all optional native metadata is parsed |

Use `Database::open("collection.2cbh")`; the explicit member extension selects 2CBH even if a classic `.cbh` with the same basename exists. The reader never writes a member file.

## Verified framing facts

- **Headers:** `.2cbh` header records are **192 bytes**; entity fields generally store IDs pointing into `.2lid`, not embedded player names. Metadata includes dates, result, ratings, ECO and references. A source slot may represent a game, text, analysis or other record; source slots and exported PGN game counts are not interchangeable.
- **Moves/annotations:** `.2cbg` and `.2cba` records have a fixed **0x1A (26-byte) prefix** before the content. The content length `A` is at offset `0x08`, spare capacity `B` at `0x0C`, an 8-byte checksum at `0x10`, and a 2-byte record tag at `0x18`. Payload starts at `0x1A`. The trailer follows the `A + B` bytes; total frame size is `34 + A + B`.
- **Move words:** the content of `.2cbg` uses **16-bit little-endian words**, interpreted with the legal chess board state. Words include standard/Chess960 castling, setup positions, promotions, null moves, variations and end-of-line markers. Examples of control words: `0xFFFA` null move, `0xFFFB` setup start, `0xFFFC` moves start, `0xFFFD` alternative, `0xFFFF` end of line.
- **Annotations:** the 2CBA decoder exposes structured items rather than silently discarding unknown kinds. Referenced but missing/malformed annotation frames and invalid record trailers return typed errors. The diagnostic `texts()` helper is best-effort; `try_texts()` and `iter_items()` preserve decoding failures.

Relevant source: [format implementation](../crates/cbvault-format/src/twocbh/) · [game decoding](../crates/cbvault-chess/src/v2.rs) · [PGN serialization](../crates/cbvault/src/pgn/).

## Public read APIs

The same `Database` façade serves classic `.cbh` and modern `.2cbh`: open sets, read game metadata, decode legal move trees, resolve names, extract PGN, scan tags and replay positions. `GameSink` and `convert_parallel` support consumer-owned storage and index pipelines; ordered output is deterministic across worker counts. The format crate is a **byte reader**, not a second chess engine: legality and `moves2` belong to `gigachess`.

## Verification and limits

A frozen **3,410,497-game** ChessBase 18 PGN sample from Mega Database 2026 was reported to have **zero unexplained canonical-byte mismatches** with cbvault, after individually counted, documented presentation/encoding transformations. **Raw PGN bytes differ** (`raw_equal=false`), especially because proprietary GUI headers and training annotations are not exported identically. A full scan classified **11,743,083 header slots**; the **8,330,763 later eligible game bodies were not compared** with the ChessBase PGN sample.

This demonstrates substantial **sampled** real-data support, not universal parity. Other database versions, rare annotations and quotations, multimedia, proprietary GUI metadata, full training objects and real-world Chess960 remain incompletely verified. [Accepted scope and exact counts](../openspec/changes/archive/2026-11-10-native-2cbh-release-hardening/closeout.md). Native 2CBH editing/writing is out of scope.

## License and provenance

The classic and native 2CBH readers use the **MIT-licensed pre-GPL code** from [oschess-cb-bridge](https://github.com/asavis/oschess-cb-bridge), pinned at classic **v1.0.0** (`ca9e8f8e4389edd6430f02a14b33ff53552bcadc`) and native 2CBH **v1.0.3** (`bffebfc498d52b4e4631aed32b2f8e8194d125ff`). Changes published by upstream **after its GPL license switch were not copied, linked, vendored or ported**. Independently established format facts may inform original code, subject to the [clean-room provenance protocol](provenance.md).
