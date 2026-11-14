//! Byte-level ChessBase formats: the classic `.cbh` family, the 2CBH `.2cbh` family,
//! the `.cbv`/`.cbz` containers and the `.ctg` opening books.
//!
//! This crate knows bytes, not chess: it parses headers, records, namebases,
//! codepages and containers, and produces raw move tokens and annotation records.
//! Boards, legality and keys live in `cbvault-chess`.
//!
//! Ported from `cbformat` in `oschess-cb-bridge` (MIT); see `docs/provenance.md`.

pub mod archive;
pub mod cbh;
pub mod codepage;
pub mod ctg;
pub mod des;
pub mod error;
pub mod file;
pub mod game;
pub mod tables;
pub mod twocbh;

pub use error::{Error, Result, Role};
