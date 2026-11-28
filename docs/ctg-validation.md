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

## 2026-11-27 Comprehensive CTO/CTB/CTG Validation & Integration

Exhaustive structural, chess-semantic, CTO/CTB routing, differential, fuzz, concurrency, and statistical validation completed across 9 local `.ctg`/`.cto`/`.ctb` books totaling **228,697,037** stored positions and **232,736,748** moves:

- **Native `.cto`/`.ctb` directory validation**: `0` missed routes, `0` ambiguous routes, `0` duplicate page slots, and `0` unreferenced data pages across all 9 books (`228,697,037 / 228,697,037` entries reachable). Verified `.ctb` as an exact LSB-first binary trie split bitmap whose leaf set equals occupied `.cto` slots across every tested book, and fixed `high > slots` rejection for small books (`slots = 7, low = 0, high = 8` on the 3 synthetic `special-*-v3` books).
- **Structural & chess round-trip verification**: `0` damaged pages, `0` unclosed pages, `0` bad keys, `0` invalid positions, `0` key mismatches (`bit7_set == 0`, `pad_nonzero == 0`), `0` duplicate position keys, `0` unknown moves, `0` illegal moves, and `0` missing child positions (`18,660` queen promotions decoded with child present; `587` underpromotion-only children omit the parent move byte due to 1-byte CTG move-table encoding).
- **256-byte move census & third-piece verification**: Across all `232,736,748` move instances in the 9 books, exact bijection with the 179 mapped bytes in `MOVE_TABLE` (`0` unmapped observed bytes, `0` unused mapped entries), including all 13 `ordinal = 2` codes (`0x0b`, `0x19`, `0x1e`, `0x2b`, `0x3d`, `0x46`, `0x4e`, `0x4f`, `0x57`, `0x89`, `0xb4`, `0xd4`, `0xd6`) and `0` omitted `ordinal >= 2` children across all multi-piece books.
- **Controlled full-tree PGN parity & `raw_score` / `raw_tail` semantics**:
  - `special-ctg-standard-v3.ctg` (`8` games) and `special-ctg-coverage-v3.ctg` (`3,778` games) matched **212 / 212 (100.00%)** normalized nodes across games, wins, draws, losses, and `raw_score` up to ChessBase's 33-ply export cutoff. All three local `special-*-v3.ctg` books were generated with a 33-ply cutoff and excluded `[SetUp "1"]` games, so post-ply-33 promotion/overflow lines reside in their source PGNs and `.cbh`/`.2cbh` sets rather than the `.ctg` files.
  - Across `CorrBook2024`, `StrongBook2025`, `PowerBook2025`, `3_4M_Golden`, and `special-expanded-v3`, PGN games with unresolved result `*` add `+5` to `raw_score` when White is to move and `+0` when Black is to move; `Hiarcs14zmBook` also contains `15,698,628` `games == 0` entries and `494` entries pairing `raw_tail[0] & 0x04 != 0` with non-zero `raw_tail[1]` (`raw_tail` remains preserved raw).
- **Differential, corruption, concurrency, fuzz, and paired benchmarks**:
  - `Book`, `NativeBook`, and `AdaptiveBook` produced identical `offset`, `stats`, `evaluation`, `moves`, and `children` (`0` mismatches) on 100% of positions in `special-*-v3` (212 each), `CorrBook2024` (1,399,432), and `StrongBook2025` (2,231,384), plus >2M stratified positions each in `PowerBook2025`, `Hiarcs14zmBook`, `3_4M_Golden`, and `realistic-3410497` and >480k negative/collision probes per book across mapped and `CBVAULT_NO_MMAP=1` modes.
  - 19 structural/companion corruption cases, duplicate-key detection and lowest-page tie-breaking, 16-thread concurrent `AdaptiveBook` validation/fallback tests, and >4.2M libFuzzer runs (`ctg_key`, `ctg_page`, `ctg_cto`) passed with 0 panics.
  - Paired benchmarks vs `adaptive` baseline: `NO_MMAP=1` validation improved from `55.74s` to `0.64s` on `PowerBook2025` (`87x`); mapped `86.77M`-entry validation improved from `7.09s` to `0.75s` (`9.45x`), CLI `book probe` on 8 GB books executes in `0.11–0.12s` at `<3 MB` peak footprint, and bounded scan reduces `PowerBook2025` peak footprint from `383 MB` to `213 MB`.

### Follow-up integration

The local real-book validation branch is the base of the combined reader
integration. Explicit per-reader I/O policies apply to CTG/CTO/CTB and the
scan fallback. `AdaptiveBook::open_verified` validates complete routing (or
scans on failure) before returning any lookup result; it rejects structurally
damaged fallback sources. Ordinary adaptive opening retains its fast exact
hit behavior: before validation, a stale directory may conceal an earlier
canonical-key candidate or lower-page duplicate. Routing completeness is
not proof of numerical statistics or move legality.

`BookStream` opens only CTG, exposing physical-order raw and optionally
decoded records with a page/entry budget, cooperative cancellation,
progress, and strict/recoverable diagnostics. Decoded positions use their
canonical stored orientation, not a guessed original game orientation.
No native directory or scan index is required; raw scratch space is one
4 KiB page. In recoverable decoded traversal, malformed keys or illegal
moves produce diagnostics and are not delivered as valid decoded records.
A budget stop or recovery with errors is not a complete traversal.

### Final acceptance validation (`validation/ctg-final-acceptance`)

#### 1. Integration baseline and `BookStream` validation

The integration baseline (`85ed978`) passed its public CI, native benchmark,
and combined routing/stream acceptance workflows. On the public `85.8M`-entry
(`87.5M`-move) scale gate, `BookStream` completed cleanly with bounded resident
memory in positioned mode (`positioned raw` in `8.05s`, `2,352 KiB` RSS;
`positioned decoded` in `257.57s`, `3,160 KiB` RSS; `mapped raw` in `6.93s`;
`mapped decoded` in `256.72s`).

Local 6-mode (`oracle`, `raw`, `decoded` x `mapped`, `positioned`) acceptance
runs across five reference and commercial books (`1.40M` to `86.77M` entries)
matched baseline entry and move-occurrence totals identically while keeping
`positioned raw` footprint near `1.26 MiB` and `positioned decoded` under
`2.95 MiB`.

#### 2. Bitboard piece-ordinal experiment (evaluated, not adopted)

`experiment/ctg-bitboard-ordinals` (`c27d177`) was audited across all nine
local books (`228,697,037` entries, `232,736,748` moves) and produced `0`
ordinal or move-sum diffs against the baseline reader. However, its public paired
benchmark workflow failed (`KeyError: 'open_allocations'` in `native-book-bench.py`),
and alternating local release runs on `24.6M`–`27.8M`-entry books showed a
`~10–12%` slowdown on multi-threaded `verify --children` despite minor
(`~4–12%`) single-thread probe speedups. It remains unadopted on this branch.

#### 3. Underpromotion omission and opt-in child probing

Scanning six local books established `18,660` stored queen-promotion parent
continuations, `26` `queen + underpromotion` parent entries (where only the
queen move appears in the parent table), and `587` underpromotion-only child
positions (`614` instances total: `256` knight, `295` rook, `63` bishop; `489`
captures, `125` quiet) whose promotion moves are omitted from parent CTG move
tables. Across the corpus, `4` parent entries satisfied
`encoded_other + promo_children > parent.games`, proving transposition ambiguity:
child-position existence cannot be safely attributed as a stored parent
continuation.

`DiscoveredPromotion` and opt-in `probe_promotion_children` methods on `Book`,
`NativeBook`, and `AdaptiveBook` distinguish natively stored parent
continuations (`encoded_in_parent`) from legal promotion targets discovered only
by probing the child position key.

#### 4. Special promotion corpus and 1-byte move-table limits

All three local special v3 books stopped at `212` entries and ply `33`
(`17.<White>` leaves with `0` moves), before ply-`37+` promotions; additionally,
games starting from `[SetUp "1"]` FEN roots were excluded on import. In
`MOVE_TABLE`, `179` byte slots are already mapped (covering up to 3 queens,
rooks, bishops, and knights per color: ordinals `0..=2`), leaving `77` free slots
versus `592` distinct White-canonical `(role, ordinal, df, dr)` cells
(`count >= 3`, ordinals `0..=9`) present in the deep coverage PGN. Arbitrary
4th+ promoted pieces therefore cannot all fit in CTG's 1-byte move table.

**Manual ChessBase regeneration parameters (non-destructive v4 copy):**
1. Open the full `.2cbh`/`.cbh` source database without filtering out mainline
   move depth (note `[SetUp "1"]` games are ignored by standard CTG import).
2. Set opening book creation length to unlimited or at least `260` plies
   (`>= 130` full moves, exceeding the max `257`-ply coverage PGN lines; max
   standard PGN is `222` plies / `111` moves).
3. Enable unconditional variation inclusion (`Include variations`) and set
   minimum games per line to `1`.
4. Create a new destination book (`*-v4.ctg`, without overwriting `v3` fixtures)
   and verify ply coverage beyond ply `37` rather than relying on Page 0 header
   bytes (ChessBase does not serialize the dialog length limit there).

#### 5. Controlled statistical oracle findings

Independent replay of all `212 / 212` entries in the three special v3 books
matched `games`, `wins`, `draws`, `losses`, and `raw_score` identically,
confirming unfinished (`*`) games add `+5` to `raw_score` for White-to-move
entries and `+0` for Black-to-move entries. Whole-database replay of a
`3,410,497`-game `.2cbh`/`.ctg` pair confirmed per-visit root counting
(`3,398,310 + 12` mainline start-position revisits `= 3,398,322`), selective RAV
inclusion (`rav_ply0 = 14,102`), and 32-bit Elo accumulator halving
(`rated_games /= 2, rating_sum /= 2` on `u32::MAX` overflow).
