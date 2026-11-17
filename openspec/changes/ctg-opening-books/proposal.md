# Read-only ChessBase opening books (`.ctg`)

## Why

Much curated opening knowledge lives in ChessBase opening books: a `.ctg` file
with `.cto`, `.ctb` and `.ini` companions. ChessBase, Fritz and the engines run
inside their interface use these books, and HIARCS and the Fritz Power Books ship
in this format. There is no maintained, permissively licensed reader (issue #3).
`jja` reads CTG but is GPL-3. `sshivaji/ctgreader` is labelled MIT, but its CTG
code comes from the unlicensed Daydreamer engine, which in turn credits Brutus,
so it cannot be ported.

## What Changes

- **A clean-room `.ctg` reader** in `cbvault-format::ctg`: the page walk with
  typed errors, the position key codec (prefix-coded board, padding, en passant,
  castling), the move-byte table, the 33-byte statistics block, and the
  annotation byte. Every fact was established from bytes and from ChessBase's
  own display of real books. No CTG implementation or specification was read.
- **The chess bridge** in `cbvault-chess::ctg`: the stored form of a `gigachess`
  position (books store White to move; Black to move is colour-flipped, and a
  castle-free position with the white king on files a–d is mirrored), the lookup
  keys, and move bytes to legal moves (castling king → rook, promotion to a
  queen).
- **`cbvault::book::Book`**: open a book, build an in-memory index by scanning
  the `.ctg` once (8 bytes per position), `probe` a position, `probe_after` a
  move, and `verify` a whole book. The book's `.cto` hash is not established and
  is not used.
- **Decoded book metadata**: games, wins/draws/losses, both sides' average
  ratings, ChessBase's "N" (games with a result), move symbols (`!` `?` `!!` `??`
  `!?` `?!`), the "Main move" and "Don't play in tournament" marks, position
  evaluations (oriented for the side to move), and the colour ChessBase gives
  each move.
- **CLI**: `cbvault book info | probe | verify`.

## Non-goals

- Writing or modifying books. Read-only, like the rest of the project.
- The `.cto` hash. A lookup through it would need the hash function, which is
  not established; guessing it is not implementing it.
- Export to PolyGlot or PGN. It is a natural follow-up, not part of this change.
- Naming the fields that are still unknown: the annotation's high three bits
  (a "class"), statistics offset 31, the header page, and the `.ctb` bitmap.

## Licensing and provenance

Original MIT code, written from facts that this change established itself. The
evidence (probe method, counts, ChessBase screenshots described in words) is in
`docs/research/ctg-observations-2026-11-14.md`; ledger rows are in
`docs/provenance.md`. No book content is committed. Real books are reached only
through `CBVAULT_TEST_CTG`.

## Impact

New modules only: `cbvault-format::ctg`, `cbvault-chess::ctg`, `cbvault::book`
and `cbvault-cli`'s `book` subcommand. No existing hot path or output changes.
