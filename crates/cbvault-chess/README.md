# cbvault-chess

The sole chess-rules bridge for **classic `.cbh` and modern `.2cbh`** database move records. This MIT-licensed crate converts their move tokens to 16-bit `moves2`, validates legal replay and variations, and emits SAN/FEN/UCI and incremental Polyglot position keys using [`gigachess`](https://crates.io/crates/gigachess).

`cbvault-format` handles bytes; `cbvault-chess` handles board state. There is no second chess engine. Synthetic tests cover castling and Chess960; large-scale ChessBase PGN parity was reported on a 3.41-million-game Mega Database 2026 sample, not on every possible native feature.

The underlying classic/2CBH parsing work derives from pinned **MIT versions of `oschess-cb-bridge`** (v1.0.0/v1.0.3); **no code from later AGPL-licensed releases was reused**. See the [provenance ledger](../../docs/provenance.md).

[Main README](../../README.md) · [API docs](https://docs.rs/cbvault-chess)
