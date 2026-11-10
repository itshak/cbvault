<h1 align="center">cbvault</h1>

<p align="center">
  <strong>Read ChessBase databases directly from Rust.</strong><br>
  Native, read-only support for classic <code>.cbh</code> and modern <code>.2cbh</code>, with fast PGN export, search, parallel conversion and complete <code>.cbv</code>/<code>.cbz</code> archive extraction.
</p>

<p align="center">
  <a href="https://crates.io/crates/cbvault"><img alt="cbvault version on crates.io" src="https://img.shields.io/crates/v/cbvault?style=flat-square&logo=rust"></a>
  <a href="https://docs.rs/cbvault"><img alt="docs.rs documentation" src="https://img.shields.io/docsrs/cbvault?style=flat-square"></a>
  <a href="https://github.com/itshak/cbvault/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/itshak/cbvault/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/itshak/cbvault/blob/main/LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square"></a>
</p>

**One Rust API, two ChessBase database generations, no forced conversion.** cbvault is designed for chess software, opening explorers, research tools, batch-processing pipelines and anyone who needs to work with large ChessBase collections **without ChessBase itself**. It is an independent MIT-licensed project, not affiliated with ChessBase.

## Why cbvault?

| What you need | What cbvault provides |
|---|---|
| **Read existing databases** | Open classic **`.cbh`** and modern **`.2cbh`** file sets *in place*. Keep stable source game IDs; no imported copy or proprietary runtime required. |
| **Build chess applications** | One generation-aware `Database` API for headers, players and events, moves, variations, annotated games and PGN. |
| **Search millions of games** | Indexed-style metadata filters and name lookups, plus on-demand position replay and Polyglot position keys, with cancellation and early stopping. |
| **Process without loading everything** | Borrowed game records, optional keys and annotations, memory-mapped reads where supported, and deterministic **parallel** conversion/export. |
| **Recover ChessBase archives** | Extract **`.cbv`** and password-protected **`.cbz`** using all four archive compression modes. |
| **Own your integration** | Pure Rust, **MIT**, read-only source access, typed errors, and `gigachess` as the sole chess-rules engine. |

Unlike a workflow that requires importing an entire collection into a new database format, **cbvault can read the original files directly**. It is a library and CLI rather than a full chess GUI. The downstream application chooses how to store, index or display the results.

## Get started

Requires **Rust 1.88+**.

```bash
cargo add cbvault@0.1.7
cargo install cbvault-cli --version 0.1.7
```

Open a database with Rust. Pass either a classic `.cbh` file, a modern `.2cbh` file, or an unambiguous database base path:

```rust
use cbvault::bridge::{Database, GameBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("MyDatabase.2cbh")?;
    println!("{}: {} records", db.generation(), db.records());

    let mut buffer = GameBuf::new();
    // Include annotations in the PGN for game record 1.
    let pgn = db.game_pgn_with(1, &mut buffer, true)?;
    println!("{pgn}");
    Ok(())
}
```

Or export games without writing Rust:

```bash
cbvault info "MyDatabase.2cbh" --games 5
cbvault verify "MyDatabase.2cbh" --threads 8
cbvault pgn "MyDatabase.2cbh" games.pgn --threads 8
cbvault archive list "Collection.cbv"
cbvault archive extract "Collection.cbv" ./extracted --threads 8
```

The same commands also work with classic `.cbh` databases; archive commands accept `.cbz` with `--password`. The CLI can emit machine-readable JSON and redirect PGN to stdout.

## What you can build

**Direct database browsing.** List games, inspect player/event metadata, open a game with its annotations and variations, or export a selected record to PGN. Headers can be read independently of move decoding.

**Search and preparation.** Filter by players, ECO, result, year, rating or round. Resolve multiple matching player IDs instead of silently picking one. Scan positions with legal replay, stop when enough results are found, or generate Polyglot keys for a separate opening index.

**High-throughput data pipelines.** Use `GameSink` with `for_each_game`, `for_each_range` or `convert_parallel`. Each callback receives borrowed game data; the sink can request moves, keys and/or native annotations as needed. Parallel processing **preserves source record order**, so consumers do not need to reorder games afterward.

**Portable PGN export.** Export entire sets or game ranges from **both `.cbh` and `.2cbh`**, including supported annotations and variations. Sequential and parallel paths are regression-tested for deterministic output.

**Archive migration and recovery.** List or extract `.cbv`/`.cbz` members without implementing a ChessBase archive reader yourself. On the tested reference archive, cbvault reproduced **all 3,871 members (3.61 GB extracted) byte-for-byte** against the reference extraction.

See [Rust API documentation](https://docs.rs/cbvault), [CLI crate](https://crates.io/crates/cbvault-cli), and the [consumer API guide](https://github.com/itshak/cbvault/blob/main/docs/bridge.md).

## Verification: measured, not assumed

| Area | Evidence | Boundary |
|---|---|---|
| **Native `.2cbh` read and PGN** | **3,410,497 games** in a frozen Mega Database 2026 / ChessBase 18 exported PGN sample; **zero reported unexplained canonical-byte differences** after counted, audited presentation exceptions | This is **not raw PGN byte identity**, universal compatibility, or a comparison of the later **8,330,763 eligible games** (their headers were classified only). |
| **Archive extraction** | **3,871 / 3,871** members, **3.61 GB** extracted, byte-identical to reference on the tested archive | Verified on the stated reference corpus, not every possible archive |
| **Classic `.cbh` throughput** | **1.17–1.20× faster** than the MIT `cbformat` reference during like-for-like *verify* benchmarks on a 10-core Apple Silicon machine | A measured classic-CBH workload, **not** a universal speed comparison or a benchmark for 2CBH |
| **Platform and correctness** | GitHub CI on **Windows, macOS and Linux**, generated malformed-input tests, fuzz targets, Clippy and example tests | Real proprietary ChessBase oracle data stays local; CI cannot rerun it |

**Known limits:** cbvault is read-only and does **not** edit/write ChessBase databases, parse PGN as an input format, or provide a GUI. Other real 2CBH corpora, rare native annotations and multimedia, ChessBase-specific GUI/training metadata and real-world Chess960 need further verification. The project remains pre-1.0. For exact comparisons and exclusions, see the [archived 2CBH acceptance report](https://github.com/itshak/cbvault/blob/main/openspec/changes/archive/2026-11-10-native-2cbh-release-hardening/closeout.md).

## Architecture

| Crate | Responsibility |
|---|---|
| [`cbvault`](https://crates.io/crates/cbvault) | Public read-only database API: classic/2CBH opening, search, game sinks, PGN and conversion |
| [`cbvault-format`](https://crates.io/crates/cbvault-format) | Binary records for `.cbh`/`.2cbh` families and `.cbv`/`.cbz` archives |
| [`cbvault-chess`](https://crates.io/crates/cbvault-chess) | Legal move replay and 16-bit moves2 conversion using [`gigachess`](https://crates.io/crates/gigachess) |
| [`cbvault-cli`](https://crates.io/crates/cbvault-cli) | `cbvault` command-line interface for inspection, verification, export and archive extraction |

No second chess engine is bundled. The library reads source databases without modifying them, leaving index/storage design to consumers.

## Provenance and license — important

**cbvault is MIT-licensed and based on the MIT-era `cbformat` implementation from [oschess-cb-bridge](https://github.com/asavis/oschess-cb-bridge).** The pinned upstream foundations are:

- **Classic CBH:** MIT [v1.0.0, commit `ca9e8f8e`](https://github.com/asavis/oschess-cb-bridge/tree/ca9e8f8e4389edd6430f02a14b33ff53552bcadc).
- **Native 2CBH:** MIT [v1.0.3, commit `bffebfc4`](https://github.com/asavis/oschess-cb-bridge/tree/bffebfc498d52b4e4631aed32b2f8e8194d125ff).

**We did not copy, vendor, link or port the upstream code introduced after its switch to AGPL-3.0.** Later format observations were reconciled as independently verified facts, not source-code reuse. The MIT notice and modifications are tracked in the [provenance ledger](https://github.com/itshak/cbvault/blob/main/docs/provenance.md) and [third-party notices](https://github.com/itshak/cbvault/blob/main/THIRD_PARTY_NOTICES.md). The archive codecs were independently implemented from format facts. No ChessBase proprietary code or databases are distributed.

**License:** [MIT](https://github.com/itshak/cbvault/blob/main/LICENSE). ChessBase is a trademark of its owner; cbvault is independent and not affiliated with or endorsed by ChessBase.

## More information

[API guide](https://github.com/itshak/cbvault/blob/main/docs/bridge.md) · [Format specifications](https://github.com/itshak/cbvault/blob/main/SPEC.md) · [2CBH research and limitations](https://github.com/itshak/cbvault/blob/main/docs/format-spec-2cbh.md) · [Performance/codec reference](https://github.com/itshak/cbvault/blob/main/docs/cbv-reference.md) · [Changelog](https://github.com/itshak/cbvault/blob/main/CHANGELOG.md) · [Contributing](https://github.com/itshak/cbvault/blob/main/CONTRIBUTING.md)
