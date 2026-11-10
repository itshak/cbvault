## Why

`cbvault` is a standalone, 100% MIT-licensed, high-performance Rust reader for ChessBase formats with growing community visibility. Today, public-facing surfaces — including `README.md`, GitHub repository descriptions, crate-level doc comments, and example programs — frequently reference "downstream software" (an unreleased downstream product under development) and its internal database concepts (`.db`/`.db`).

This coupling prematurely discloses unreleased product branding and architecture, while misleading open-source users into assuming `cbvault` is a private, single-purpose helper crate rather than an independent, production-ready library. Decoupling these references now ensures `cbvault` presents as a professional, standalone open-source project while protecting the privacy of the unreleased product.

## What Changes

- **Public README & Positioning:** Rewrite `README.md` header, features, and non-goals to frame `cbvault` as a general-purpose, zero-allocation conversion and reading engine for any downstream chess application, database pipeline, or analysis tool, eliminating references to unreleased consumer names.
- **GitHub Repository Metadata:** Update the GitHub repository description (via GitHub CLI `gh repo edit`) from *"Rust reader for ChessBase .cbh/.cbv/.cbz on the gigachess engine, with a downstream software conversion API..."* to a professional, general-purpose description highlighting high-throughput parsing, zero-allocation sinks, and complete `.cbv`/`.cbz` extraction.
- **Crate & Module Documentation:** Neutralize module doc comments across `crates/cbvault/src/bridge/mod.rs`, `crates/cbvault/src/bridge/search.rs`, `crates/cbvault/src/pgn/split.rs`, `crates/cbvault-chess/src/lib.rs`, and `crates/cbvault-chess/README.md`. Frame all integration patterns around generic callers, consumers, and downstream sinks.
- **Examples & Test Commentary:** Refactor `crates/cbvault/examples/convert.rs` and benchmark descriptions to demonstrate generic target row ingestion without referencing unreleased schema names.
- **Documentation & Research Files:** Review and generalize `docs/bridge.md` and `docs/port-inventory.md` to refer to downstream consumer requirements generically.
- **Maintain Internal Context Cleanly:** Preserve essential downstream performance requirements in `AGENTS.md` and architecture decisions under generic terminology ("the consumer application"), avoiding public exposure.

## Capabilities

### Modified Capabilities

- `cbvault`: Updates public capability specifications to require generic, consumer-neutral terminology for streaming conversion sinks, PGN utilities, and database façades, prohibiting proprietary downstream product branding on public library surfaces.

## Impact

- **Public APIs:** Zero breaking changes. `Database`, `GameSink`, `convert_parallel`, `split_games`, and all error types retain exact signatures, ABI, and zero-allocation semantics.
- **Dependencies:** Unchanged. Zero SQLite or database engine dependencies.
- **Repository Hygiene:** Clean, professional open-source presentation on GitHub and crates.io; confidentiality of unreleased downstream project preserved.
