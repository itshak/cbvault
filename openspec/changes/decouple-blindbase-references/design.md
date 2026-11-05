## Context

`cbvault`'s architecture is already strictly decoupled from any target database or consumer engine (formalized in ADR-005): it provides a read-only `Database` façade, a generic `GameSink` streaming callback, and zero SQLite or embedded database dependencies. However, because it was developed in tandem with an unreleased downstream application ("BlindBase"), public documents, doc comments, examples, and the GitHub repository description still contain direct references to this project and its internal formats (`.bbdb`/`.bbrb`).

See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**
- Systematically sanitize all current-facing surfaces: `README.md`, crate root docs, module doc comments, examples, and repository metadata.
- Update the GitHub repository description via `gh repo edit` to present `cbvault` as a standalone, production-ready, ultra-fast ChessBase toolkit.
- Ensure all public examples (`examples/convert.rs`) demonstrate clean, idiomatic integration with generic caller-defined structures.
- Maintain 100% API compatibility, zero runtime overhead, and 100% test pass rate across all workspace crates.

**Non-Goals:**
- Rewriting git commit history or modifying immutable historical change archives in `openspec/changes/archive/` (consistent with ADR-006).
- Changing any Rust trait, struct, method signature, or error type (`GameSink`, `Database`, `GameRef`, etc. remain untouched).
- Changing the underlying chess core (`gigachess` remains the sole chess core).

## Decisions

### 1. Standardize on Generic Downstream Terminology
In all current-facing documentation, doc comments, and examples:
- Replace "BlindBase" with "downstream application", "consumer engine", "caller", or "client pipeline".
- Replace `.bbdb` / `.bbrb` with "target database", "custom storage", or "application format".
- *Alternative considered:* Leaving comments as-is. *Rejected* because doc comments are published to `docs.rs` and distributed with crates published to `crates.io`.

### 2. Update GitHub Repository Metadata via `gh`
Update the repository description on GitHub to highlight its standalone value:
> "Fast, memory-efficient, read-only Rust library and CLI for ChessBase databases (.cbh/.2cbh/.cbv/.cbz) on gigachess, with zero-allocation streaming conversion and PGN export. 100% MIT."
- *Alternative considered:* Omitting mention of conversion entirely. *Rejected* because the zero-allocation conversion sink is one of `cbvault`'s crowning architectural achievements.

### 3. Generalize `examples/convert.rs`
Refactor the doc comments and struct annotations in `examples/convert.rs` to depict a general-purpose chess database ingestion pipeline (storing game tags, `moves2` blobs, and Polyglot Zobrist key arrays) rather than citing internal product schema.

### 4. Clean Internal Agent Instructions (`AGENTS.md`)
Update `AGENTS.md` to describe the consumer generically ("consumer applications / downstream engines") while keeping the strict correctness and zero-allocation performance rules intact.

## Risks / Trade-offs

- **[Risk]** Accidental removal of technical context during wording changes.  
  → **Mitigation:** Preserve all technical explanations (why zero-allocation is required, why Polyglot keys are computed in-stride, why pushdown scanning is used), modifying only the branding and product nouns.
- **[Risk]** Breaking doc-tests or rename-guard tests.  
  → **Mitigation:** Run `cargo test --workspace` and `cargo test -p cbvault --test rename_guard` after editing.
