//! # cbvault — ChessBase databases and opening books
//!
//! Read classic `.cbh` and modern `.2cbh` databases, `.ctg` opening books,
//! and `.cbv`/`.cbz` archives directly from Rust, without proprietary libraries
//! or converting the source files. All access is **read-only**.
//!
//! ## Databases
//!
//! The generation-aware [`bridge`] API opens game databases, reads headers,
//! replays moves through `gigachess`, exports annotated PGN, searches, and
//! streams games into consumer-defined sinks. See [`bridge::Database`],
//! [`bridge::GameSink`], [`bridge::for_each_game`] and
//! [`bridge::convert_parallel`] for game-oriented workflows.
//!
//! ## ChessBase CTG opening books
//!
//! The [`book`] module exposes [`book::Book`] for indexed position queries,
//! move annotations, position/result/rating statistics and whole-book validation.
//! Pass the `.ctg` file or an unambiguous book base path; the `.cto` and `.ctb`
//! companion files are not required. The library scans the `.ctg` once and
//! builds its own in-memory index (about eight bytes per indexed position).
//!
//! ```rust,no_run
//! use cbvault::book::Book;
//! use gigachess::Board;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let book = Book::open("MyBook.ctg")?;
//!     let start = Board::startpos();
//!
//!     if let Some(position) = book.probe(&start)? {
//!         println!("{} games; {} book moves", position.stats.games, position.moves.len());
//!         for book_move in &position.moves {
//!             if let Ok(chess_move) = book_move.mv {
//!                 println!("{chess_move}");
//!             }
//!         }
//!     }
//!
//!     // Strict verification is opt-in: check all stored positions and
//!     // whether each decoded move leads to another position in the book.
//!     let report = book.verify(true)?;
//!     assert!(book.damaged().is_empty());
//!     assert_eq!(report.damaged_pages + report.bad_keys + report.invalid_positions
//!         + report.key_mismatches + report.moves_unknown + report.moves_illegal
//!         + report.children_missing, 0);
//!     Ok(())
//! }
//! ```
//!
//! The example compiles without a commercial book, but needs a locally owned
//! `.ctg` at runtime. See [`book::NativeBook`], [`book::AdaptiveBook`],
//! [`book::BookPosition`], [`book::BookPositionStats`], [`book::BookTree`],
//! [`book::DiscoveredPromotion`], [`book::BookMove`], [`book::BookStats`],
//! and [`book::VerifyReport`] for typed results.
//!
//! **Scope:** The reader is read-only. [`book::NativeBook`] and
//! [`book::AdaptiveBook`] use `.cto`/`.ctb` native directory lookup with
//! exhaustive validation and automatic fallback to [`book::Book`]; under-promotion
//! pieces are omitted from `.ctg` parent move tables and never guessed.
//! For independently measured verification coverage and limitations, see
//! [the public CTG validation report](https://github.com/itshak/cbvault/blob/main/docs/ctg-validation.md).
//!
//! ## Attribution
//!
//! CTG support was originally contributed by
//! [Tulio Vianna](https://github.com/tuliovianna-lab) in
//! [pull request #8](https://github.com/itshak/cbvault/pull/8).
//! The classic CBH and 2CBH parsing foundations derive from pinned MIT-era
//! `oschess-cb-bridge` releases; see the repository's provenance ledger.

pub mod book;
pub mod bridge;
pub mod pgn;
pub mod replay;

pub use bridge::stem_of;
