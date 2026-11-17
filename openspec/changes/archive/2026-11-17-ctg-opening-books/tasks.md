# Tasks

## 1. Establish the facts (from bytes)

- [x] 1.1 Page and entry framing; check that the walk closes on every page
- [x] 1.2 Position key: prefix code, square order, padding rule, en passant, castling, flip and mirror
- [x] 1.3 Move bytes: candidate intersection, then child existence; promotions
- [x] 1.4 Statistics block, ChessBase's N, both sides' ratings
- [x] 1.5 Annotation byte, move marks, position evaluations and colours, checked against ChessBase's display
- [x] 1.6 Record everything with counts and open questions in `docs/research/ctg-observations-2026-11-14.md`

## 2. Implement

- [x] 2.1 `cbvault-format::ctg`: page walk, key codec, move table, statistics, annotation, colour, evaluation
- [x] 2.2 `cbvault-chess::ctg`: stored form, keys, move decoding to legal `Move`s
- [x] 2.3 `cbvault::book`: index, `probe`, `probe_after`, `verify`, damaged-page reporting
- [x] 2.4 `cbvault-cli`: `book info | probe | verify`, with `--json`
- [x] 2.5 Page closure validation in `verify_pages` and CLI exit-code strictness for damaged pages, unknown move bytes, and missing children
- [x] 2.6 Five rare promoted-piece move codes in `MOVE_TABLE` (`0x19`, `0x2b`, `0x89`, `0xb4`, `0xbc`)

## 3. Tests

- [x] 3.1 Synthetic unit tests for every codec and for malformed pages and keys (no book content)
- [x] 3.2 Real-book test gated on `CBVAULT_TEST_CTG`, which says it did not run when unset
- [x] 3.3 Whole-book `verify --children` on ten local books: every key round-trips and every move leads to a position in the book
- [x] 3.4 Synthetic CTG regressions (`ctg_regressions.rs`) for page closure, CLI exit codes, and rare promoted move codes

## 4. Documentation and verification

- [x] 4.1 Ledger rows in `docs/provenance.md`; README and CHANGELOG
- [x] 4.2 `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --workspace`
