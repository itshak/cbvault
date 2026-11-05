# Tasks — decouple-blindbase-references

## 1. Public Documentation & Repository Metadata

- [x] 1.1 Update `README.md` subtitle, feature descriptions, and non-goals to remove BlindBase and `.bbdb`/`.bbrb` mentions, framing `cbvault` as a general-purpose, zero-allocation conversion and reading library for any downstream chess engine or tool. Verify: `git grep -i "blindbase" README.md` returns 0 hits.
- [x] 1.2 Update the GitHub repository description using `gh repo edit` to a professional, consumer-neutral summary of `cbvault`'s capabilities. Verify: `gh repo view --json description` reflects the new description without BlindBase mentions.

## 2. Crate & Module Doc Comments

- [x] 2.1 Update `crates/cbvault/src/bridge/mod.rs` to replace BlindBase-facing phrasing with generic consumer-facing documentation. Verify: `cargo doc -p cbvault --no-deps` compiles cleanly without warnings.
- [x] 2.2 Update `crates/cbvault/src/bridge/search.rs` to describe UI form and query needs generically. Verify: `cargo test -p cbvault --lib bridge::search` passes.
- [x] 2.3 Update `crates/cbvault/src/pgn/split.rs` to replace BlindBase port commentary with a standalone algorithmic description of PGN game boundary detection. Verify: `cargo test -p cbvault --lib pgn::split` passes.
- [x] 2.4 Update `crates/cbvault-chess/src/lib.rs`, `crates/cbvault-chess/README.md`, and test headers to describe Polyglot Zobrist key conventions generically. Verify: `cargo test -p cbvault-chess` passes.

## 3. Examples & Internal Docs

- [x] 3.1 Refactor `crates/cbvault/examples/convert.rs` comments and struct definitions to depict a generic chess database ingestion pipeline. Verify: `cargo check --example convert` passes.
- [x] 3.2 Update `docs/bridge.md` and `docs/port-inventory.md` to speak of downstream consumers and target stores generically. Verify: `git grep -i "blindbase" docs/` returns no current-facing references.
- [x] 3.3 Update `AGENTS.md` and `openspec/config.yaml` to describe downstream consumer integration under generic terminology without private branding. Verify: `openspec validate --all --strict` passes.

## 4. Verification & Cleanliness

- [x] 4.1 Run a repository-wide check to verify that no current-facing source or active documentation file references the retired term. Verify: check finds no occurrences outside immutable archives.
- [x] 4.2 Run the full workspace gate set: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `openspec validate --all --strict`. Verify: all checks pass with zero warnings and zero errors.
