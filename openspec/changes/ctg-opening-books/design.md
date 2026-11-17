# Design: a clean-room opening-book reader

## Method and provenance

Every module is **original** code (ledger rows in `docs/provenance.md`). The
facts came from three sources, all recorded in
`docs/research/ctg-observations-2026-11-14.md`:

1. **Byte probes** of real books, run locally by small throwaway scripts. A
   hypothesis was accepted only on a full count: page framing closing exactly on
   every page, a prefix code that is the single solution for the initial position
   and decodes 1,048,466/1,048,466 entries to legal boards, castling bit order
   being the one of 24 orders that never contradicts the board, and so on.
2. **Child existence.** In a real book every decoded move leads to a position that
   is also in the book. That resolved the rare move bytes (second queens) and the
   promotion piece (a queen in 874/874 cases).
3. **ChessBase's own display**, compared with screenshots: move symbols, colours,
   N, both rating averages, the two move marks, and position evaluations. The
   least common combinations were set on purpose in a small test book and read
   back.

The partial 2007 specification cited by the Chess Programming Wiki was
unreachable and was not read. `jja` (GPL-3) was not used, not even as an oracle.

## Layering

| Crate | Module | Responsibility |
|---|---|---|
| `cbvault-format` | `ctg` | bytes only: pages, entries, key codec, move table, statistics, annotation, colour rule, evaluation codes |
| `cbvault-chess` | `ctg` | `gigachess` ↔ stored form; keys; move bytes → legal `Move` |
| `cbvault` | `book` | `Book` façade, in-memory index, `probe`, `verify` |
| `cbvault-cli` | `book` | `info`, `probe`, `verify` |

`gigachess` stays the only chess core. The format crate has no chess
dependency.

## Lookup without the `.cto`

The `.cto` is a hash table of page numbers: every data page is the target of
exactly one slot, and the slots cluster like a binary trie stored as a heap. Its
hash is unknown, and a GF(2)-linear model of it is inconsistent, so it is not
linear. `Book::open` therefore scans the `.ctg` once in parallel (rayon, 2,048
pages per task). For each entry it stores one `u64`: the high bits of a hash of
the key (FNV-1a with a SplitMix64 finaliser, this crate's own choice), packed
with the page number. The array is sorted in parallel. A probe binary-searches
it and compares the key byte for byte on the page, so a collision costs a page
read, never a wrong answer.

## Performance

| Measure (warm page cache, 8-core / 16-thread Windows desktop) | Value |
|---|---|
| index of a 27.6M-position book (2.16 GB `.ctg`) | 0.7 s, 221 MB |
| index of a 32.3M-position book (2.30 GB) | 2.2 s |
| `verify --children` on the 27.6M-position book | 22 s |
| single probe after the index | page reads only |

These are new code paths. No existing benchmark or hot path is affected.

## Damaged input

A page whose header or entries do not fit is listed in `Book::damaged` and the
rest of the book is indexed. Undecodable keys and move bytes come back as typed
errors (`KeyError`, `MoveError`). A move byte whose meaning is not established
is `MoveError::UnknownByte`, never a guess.

## Open questions

- The `.cto` hash, the `.ctb` bitmap and the header page.
- The annotation's high three bits (a "class" 0–7): set by some books, with no
  observed effect on ChessBase's display.
- Statistics offset 12 (equal to `10*wins + 5*draws` in most books, not all) and
  offset 31.
