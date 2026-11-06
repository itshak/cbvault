# Native 2CBH read-only support: pinned MIT foundation, factual parity, local ChessBase gold

## Why

`cbvault` already has an MIT, high-throughput, read-only classic ChessBase reader and a partly inspected `.2cbh` family, but refuses to open 2CBH games because `.2cbg` is not decoded. This excludes newer ChessBase personal libraries from downstream GUI and analysis tools. The earlier assumption that no reusable 2CBH implementation exists is **superseded** by the finding of a working **MIT Rust reader in `asavis/oschess-cb-bridge` tag `v1.0.3`**, plus comprehensive format documentation and a Java reader/writer in Morphy. The current oschess branch is AGPL-3.0 and must not be ported.

This change is **planned, not implemented**. Complete this in cbvault as a separately gated library capability. The most important release criterion is not a successful demo, but **local-only byte-level parity against real ChessBase's own PGN export**, with the pre-existing, fully evidenced differences kept intact.

## What Changes

**Phase 1 — pinned MIT foundation**
- Pin the exact oschess tag `v1.0.3` (**annotated tag points to commit `bffebfc498d52b4e4631aed32b2f8e8194d125ff`**, verified 2026-11-05). Verify the MIT LICENSE and `Cargo.toml` *at that commit* and the provenance of each file before porting.
- Integrate only its MIT-licensed **2CBH format functionality**: record framing/checksum, 16-bit move-word table/variation tree, setup/Chess960/null moves, entity database, annotations and PGN-related facts; map to existing cbvault modules.
- **Correct, do not preserve, current false assumptions** in the partial `twocbh` parser: record tag at +0x18/+0x19, content at +0x1a, +0x10 checksum, B spare capacity, `.2lid` players rather than “inline player names” in .2cbh; use independently validated or pinned-MIT source facts. Keep the measurements of unchanged frame geometry.
- Reuse **gigachess as the one chess core**, `moves2` representation, current PGN writer, callback/sink API, zero-allocation hot-path and ordered-parallel export. Do not bring in upstream `chesscore`, alternate SAN or second board implementation.
- Preserve MIT notice in each ported/modified file and update `docs/provenance.md` and `THIRD_PARTY_NOTICES.md`; classic CBH and CBV/CBZ behavior stays byte-identical to current baseline.

**Phase 2 — independent later improvements**
- Diff the pinned MIT code against later oschess **AGPL-3.0** releases and study Morphy's unlicensed Java implementation/format docs **for factual gaps only**: start positions, entity variants, rare annotations, damaged files, sorting/cross-links, performance. Do **not** copy later AGPL or Morphy Java code, text, tests, tables, or source structure into cbvault.
- Use the existing formal **two-room clean-room protocol** (specifier -> reviewed factual specification -> isolated implementer -> independent oracle comparison), preserve actual source-exposure audit and reproducible facts. Merely assigning two AI agents is not a legal clean room.
- Implement only specifically verified missing behaviors in original MIT code; maintain an explicit unsupported/unknown ledger and typed failures. License/provenance review is a go/no-go gate; get legal advice where needed.

**Phase 3 — critical local-only ChessBase gold and performance**
- Run against a **real locally held `.2cbh` database**, export the **same IDs / contiguous range (preferably all games)** from ChessBase itself as PGN on the user's machine, and compare cbvault's output **game by game, bytes by bytes after explicitly documented format-only normalization** using the current `crates/cbvault/examples/gold_pgn.rs` precedent.
- Every non-exact discrepancy must be categorized with the *same limited, evidenced exceptions as classic .cbh*: ChessBase null spell `Z0` vs our `--`; ChessBase unnecessary SAN file/rank qualifier when the competing piece is **pinned and illegal** (our minimal legal SAN `Nd2`, not `Nbd2` when only one knight can legally play). Standard byte-layout normalization is allowed only for line endings/whitespace, tag order and ChessBase-only tags already excluded by classic comparator; comments, NAGs, variations and substantive metadata **must not be silently dropped**. Annotations, tags and "other" unknown differences are not automatically waivers.
- Record exact totals for tested/correct/uncorrect/partially supported records, per-category counts, illegal move count, annotation-read errors, game ID mapping, first mismatching canonical byte and file hash where applicable. **Zero unexplained differences, zero silent game omissions, zero illegal moves** is the release gate; valid/known exceptions must be reported, never blindly skipped.
- All real databases, ChessBase exports, diffs with player names, licenses, proprietary binaries and oracle outputs are **local-only**, under user-chosen paths; nothing real may be committed or uploaded to the repository/CI, GitHub Actions artifacts, tickets, agent transcripts or chat attachments. Public CI uses only generated synthetic fixtures.

## Capabilities

### Modified Capabilities
- `cbvault`: adds native, read-only 2CBH family listing/game access, PGN and deterministic validation to its existing database facade and CLI, preserving CBH/CBV correctness and ABI where practical.

## Non-goals

- Writing/modifying ChessBase `.2cbh`, `.2cbg`, `.2cba`, `.2lid`, `.2lgd`, `.2lcd` or rebuilding its native indexes.
- Linking or vendoring later AGPL `oschess` code or unlicensed Morphy source, reverse-engineering ChessBase executables as a prerequisite, committing licensed real game content.
- Requiring downstream consumer UI/MCP implementation, inventing a second chess rules engine, bundling Mega Database.
- Claiming bit-identical raw PGN files when ChessBase's line wrapping/tag order differs; the gold comparator must state exactly **what bytes** it canonicalizes and why.

## Impact and references

Modules: `crates/cbvault-format/src/twocbh/**`, `crates/cbvault-chess/**` (only a decoder adapter, not rules), `crates/cbvault/src/bridge/**`, `crates/cbvault/src/pgn/**`, CLI, fixture builder, `docs/format-spec-2cbh.md`, `docs/provenance.md`, `THIRD_PARTY_NOTICES.md`. Follow AGENTS.md, ADR-001/002/003/004/005/007, the existing `gold_pgn` comparator and `docs/gigachess-san-disambiguation.patch`.

- MIT baseline: https://github.com/asavis/oschess-cb-bridge/tree/v1.0.3/crates/cbformat
- Pin: https://github.com/asavis/oschess-cb-bridge/tree/bffebfc498d52b4e4631aed32b2f8e8194d125ff
- AGPL later source (reference for facts, NOT portable): https://github.com/asavis/oschess-cb-bridge
- Morphy v2 format reference (unlicensed Java; facts only): https://github.com/Yarin78/morphy/tree/master/format/v2
