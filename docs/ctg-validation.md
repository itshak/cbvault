# CTG reader: independent compatibility and statistical validation

This report records the **2026-11-17** independent local validation of the read-only ChessBase `.ctg` reader originally contributed by [Tulio Vianna](https://github.com/tuliovianna-lab) in [PR #8](https://github.com/itshak/cbvault/pull/8), with subsequent maintainer fixes, at revision `450cc665808cde56c95238a3010c51e570ba43cf`. It separates checks that the reader can perform directly from comparisons with a separately generated PGN reference.

The test books, PGN exports, raw comparisons and local audit scripts are **not distributed**. They are proprietary or derived from proprietary material; none is required to build or use cbvault.

## Whole-book structural and chess verification

All three books were checked using the release CLI's `book verify <path.ctg> --children --json` command, not just a sample. Each scan checked entry keys, board reconstruction, move decoding and legality, and the existence of the resulting child position.

| Test book | Position entries | Decoded move occurrences | Unknown or illegal moves | Missing children | Damaged pages / key mismatches |
|---|---:|---:|---:|---:|---:|
| Fritz PowerBook 2025 | 24,643,475 | 25,228,093 | 0 | 0 | 0 |
| HIARCS 14zm | 27,817,532 | 28,232,671 | 0 | 0 | 0 |
| ChessBase CTG generated from a Mega Database 2026 PGN export | 86,769,585 | 88,117,033 | 0 | 0 | 0 |
| **Total (three distinct books)** | **139,230,592** | **141,577,797** | **0** | **0** | **0** |

These numbers describe **position entries** and **move occurrences**. They are not game counts. All scans completed successfully. Fritz was also verified with `CBVAULT_NO_MMAP=1` (positional reads), with the same counters.

This is strong evidence of CTG structure, move table and position-key compatibility on these books. However, checking that a child exists in the book is not an independent proof of its numerical statistics.

## Independently replayed PGN versus the golden CTG

The golden CTG was created in ChessBase from a frozen PGN exported from Mega Database 2026. The independently parsed source PGN contained:

- **3,410,497 game records**, including a BOM-prefixed first record.
- **2,122** nonstandard starting positions, excluded from the standard opening-root comparison.
- **77,337** standard-start records with no moves.
- **3,331,038** standard-start games with mainline moves.
- **252,655,861** independently replayed mainline plies, without SAN parse errors or disagreement with recorded `PlyCount` values. Null-move-derived artificial positions were excluded from the position sample.
- A separate check of **6,000** PGN games distributed across five file regions using `python-chess`, with zero parse errors and zero `PlyCount` disagreements.

The validation sampled all positions reached at plies 0–2, plus positions selected by a deterministic Polyglot hash predicate (lowest eight bits zero), **after replaying all source games** to calculate statistics.

| Statistical comparison | Result |
|---|---:|
| Distinct sampled PGN positions | 736,703 |
| Found in the golden CTG | 339,155 |
| Not in the golden CTG | 397,548 |
| Exact on all eight fields without symmetry normalization | 337,575 / 339,155 (99.53%) |
| Additional exact matches after CTG position-symmetry normalization | 1,098 |
| **Exact on all eight fields after normalization** | **338,673 / 339,155 (99.86%)** |
| Present positions with at least one difference | 482 |

The eight fields were **games, wins, draws, losses, rated-game count and Elo sum for the side to move, and rated-game count and Elo sum for its opponent**. The comparison accounts for CTG's color/rank normalization and file reflection when applicable. In one test position, an apparent 1-versus-1,309 discrepancy disappeared when the PGN's 1,308 equivalent color-flipped occurrences were included; the win/draw/loss and Elo fields also matched.

**Do not interpret 99.86% as global CTG completeness or as exact parity for all CTG positions.** It is the proportion of the *339,155 present positions in this deterministic PGN sample* whose eight counters agreed.

### Why do some sampled PGN positions not occur in the CTG?

Sampled position coverage depends strongly on the earliest ply at which the position appeared in the PGN:

| Earliest ply | Found | Missing |
|---|---:|---:|
| 0–32 | 166,081 | 0 |
| 33–48 | 151,157 | 30,331 |
| 49–64 | 21,763 | 128,766 |
| 65–100 | 154 | 181,633 |
| 101+ | 0 | 56,818 |

The first missing sampled position occurred at ply 34. The pattern is **consistent with a depth-dependent ChessBase book-generation selection/cutoff**, not a demonstrated reader bug. The actual import and tree-generation settings were not preserved, so the exact policy is unknown.

### Remaining statistical differences

- **482** matched positions differ under independent mainline counting after symmetry normalization. Most are small differences: **427** are +1 game in the CTG and **33** are +2.
- The PGN contains **1,225,801 variation branches** in **86,009 games**. Including *every* variation makes **431 of those 482** discrepancies disappear, but causes **127 previously exact positions** to disagree. Neither "mainlines only" nor "all variations" reproduces ChessBase's inclusion policy exactly.
- The starting-position root reports **3,331,137** games in the CTG versus **3,331,038** independent standard-start PGN mainlines (+99). The root Elo counters also differ from a direct full-PGN sum, whereas many non-root move-statistics fields agree exactly.

We therefore do **not** claim universal or byte-for-byte statistical parity against this independently exported PGN. Without the original ChessBase book-building options, remaining differences cannot confidently be attributed to reader decoding. A small, deliberately constructed PGN and CTG generated with **recorded ChessBase settings** would be the right oracle for an exhaustive exact-counter test, especially for the root Elo fields.

## Repeatable checks for maintainers

Public generated/synthetic regression tests run in ordinary CI without commercial fixtures:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
openspec validate ctg-opening-books --strict
```

Users who own a compatible CTG may perform local-only whole-book validation:

```sh
cbvault book info "/path/to/MyBook.ctg"
cbvault book probe "/path/to/MyBook.ctg" --moves e4 c5 Nf3 --json
cbvault book verify "/path/to/MyBook.ctg" --children --json
CBVAULT_NO_MMAP=1 cbvault book verify "/path/to/MyBook.ctg" --children --json
```

The source has optional real-book tests gated by `CBVAULT_TEST_CTG`. Do not upload commercial books, PGNs, extracts, binary pages or large derived datasets to the repository or public CI.

For format details, review [the clean-room CTG observations](research/ctg-observations-2026-11-14.md), [provenance](provenance.md) and the source's `cbvault::book` API. This remains a **read-only CTG reader**: it does not interpret the `.cto` hash or `.ctb` bitmap, and book move encoding does not identify under-promotion pieces.
