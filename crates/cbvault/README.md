# cbvault

**Read classic `.cbh`, modern `.2cbh` databases and `.ctg` opening books directly from Rust.** This MIT-licensed, read-only library exposes a generation-aware `Database` façade for game metadata, move trees, annotations, PGN export, search and ordered parallel conversion.

```rust
use cbvault::bridge::{Database, GameBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("Collection.2cbh")?; // Or Collection.cbh
    let mut buf = GameBuf::new();
    println!("{} records ({})", db.records(), db.generation());
    println!("{}", db.game_pgn_with(1, &mut buf, true)?);
    Ok(())
}
```

**CTG opening books (available from v0.2.0).** Use `cbvault::book::Book::open("MyBook.ctg")`, then `book.probe(&gigachess::Board::startpos())?` for book moves and position statistics, or `book.verify(true)?` to validate all keys, legal moves and child positions. The reader builds its own index without requiring the `.cto` or `.ctb` companion files. See [the public CTG validation results](https://github.com/itshak/cbvault/blob/main/docs/ctg-validation.md) and [the Rustdoc API](https://docs.rs/cbvault/latest/cbvault/book/index.html). The original CTG feature was contributed by [Tulio Vianna](https://github.com/tuliovianna-lab) in [PR #8](https://github.com/itshak/cbvault/pull/8).

With `GameSink`, select only the mainline moves, position keys and annotations your consumer needs. `convert_parallel` preserves source-game order while processing on multiple workers. Search by headers or replay chess positions without importing the entire database to another format.

**Evidence:** a frozen ChessBase 18 PGN export of **3,410,497** Mega Database 2026 games passed the strictly normalized comparison with zero unexplained differences. Raw PGN differs due to ChessBase-specific presentation fields; **8,330,763** later eligible games were not gold-compared. Rare annotations, multimedia and real Chess960 need additional real-corpus coverage.

**MIT provenance:** ported from the pinned MIT-era versions of [`oschess-cb-bridge`](https://github.com/asavis/oschess-cb-bridge), classic at [v1.0.0](https://github.com/asavis/oschess-cb-bridge/tree/ca9e8f8e4389edd6430f02a14b33ff53552bcadc) and 2CBH at [v1.0.3](https://github.com/asavis/oschess-cb-bridge/tree/bffebfc498d52b4e4631aed32b2f8e8194d125ff). **No later AGPL-licensed upstream source code was reused.** See the [provenance ledger](https://github.com/itshak/cbvault/blob/main/docs/provenance.md).

[Main README](https://github.com/itshak/cbvault/blob/main/README.md) · [API docs](https://docs.rs/cbvault) · [Release notes](https://github.com/itshak/cbvault/blob/main/CHANGELOG.md)
