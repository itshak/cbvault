# Benchmark corpus tooling

This is a standalone, non-published fixture authoring tool; the production library's dependency graph is unchanged. Uses only Gigachess for chess semantics. The private reference PGN and full profiling output stay in git-ignored fixtures-local. No original private text, moves, game IDs or player identities enter the generated corpus. The two recipe JSON files contain only aggregate count distributions; the extreme recipe is deliberately synthetic.

- Compile the lexical aggregate profiler with `g++ -O3 -std=c++20 scripts/bench-corpus/profile.cpp -o /tmp/cbvault-profile`; run it against a local PGN. Counts are lexical and do not prove chess legality.
- Run `scripts/bench-corpus/build-corpus.sh smoke|standard|scale|rich|extreme` to download a checksum-pinned CC0 source, generate deterministic annotations and verify all moves and the source mainline/header digest. Requires Rust, curl, zstd, Python 3 and sha256sum. Set CBVAULT_CORPUS_WORK to a scratch directory. Uses the first N source records; source comments are replaced, and generated comments are synthetic rather than chess advice.
- Build the standalone Cargo project with `cargo build --locked --release --manifest-path scripts/bench-corpus/generator/Cargo.toml`; the `special` binary emits 120 original setup games with promotions (including underpromotion), castling, Chess960, en passant, mate, Unicode and annotation commands. Validate with `validate 120 --exact`.
- `extract-prefix.py` extracts a complete record prefix from generator output, not arbitrary PGN.
- `corpus-manifest.json` pins the actual output PGN and compressed hashes, counts, source and seed. Zstd compressed bytes may depend on compressor version, but uncompressed PGN bytes are the reproducibility gate. Smoke and standard are prefixes of the scale corpus.
- All six generated datasets are in the local work directory. The original reference is never read by the generator; it consumes aggregate recipes only. No large files are tracked.
- Full semantic source/header and mainline equality verified for scale, rich and extreme; smoke/standard are checked prefixes; special is original. Native ChessBase feature round-trip validation still required. Stockfish-quality commentary is not claimed; most alternatives are deterministic legal branches, not engine-selected lines.

## Conversion handoff

Decompress with `zstd -d <file.pgn.zst>`. Import separately into new classic CBH and new 2CBH databases with all comments, symbols, variations and tags retained where supported. Disable filtering/deduplication unless explicitly recorded. Record ChessBase version and settings, game count, rejected records and annotation preservation. Export reference PGNs from each native database. Create CTG from the scale or standard corpus if desired; include CTG/CTO/CTB companions. Manually create native guiding texts, analysis records, training questions, quotations, null moves and other features not faithfully represented by PGN in separate fixtures. Close the application and record file-level hashes before publication. Keep all source artifacts separate from production releases. See docs/benchmark-corpus.md for CI design. Do not mark the baseline or conversion task complete before real native files and CI results exist.

## Promotion overflow v3: expanded special corpus

The original 120 games are preserved. The replacement
`special-expanded-v3.pgn` adds 6,452 cases (6,572 total; 9,304 move-tree plies).
The 5,908-case movement matrix tests every count 3 through 9 queens or 10
rooks/bishops/knights, every file-major ordinal, every geometrically possible
nonzero modulo-eight displacement, and both colors. It also tests quiet and
capture promotions, the newly promoted piece moving, capture of each ordinal
and movement of the highest surviving piece, and eight legal standard-start
games with all eight pawns promoted to the same role. Setup positions have
legal material and moves; historical reachability is not claimed for every
setup. The eight standard-start histories are fully replayed.

Regenerate using Python 3.13.5 and `chess==1.11.2`:

```sh
python promotion-overflow.py special-120.pgn special-expanded-v3.pgn special-expanded-v3.manifest.json --ctg-standard special-ctg-standard-v3.pgn
python promotion-ctg-coverage.py special-ctg-standard-v3.pgn special-ctg-coverage-v3.pgn special-ctg-coverage-v3.manifest.json
```

The latter generates 3,778 distinct reachable matrix cells from the eight
standard-start histories and deterministic cooperative continuations (402,787
plies). Prefer that file for CTG import, with all moves included, no short-game,
rating or result filters, no truncation to opening depth. It has completed
result tags. If ChessBase supports importing setup positions into books, also
create a separate CTG from the expanded corpus and record which setups it
accepted. Do not silently equate a skipped position with a decoded move.

Import `special-expanded-v3.pgn` into **new empty** CBH and 2CBH databases,
not the old 110,120-game accidental aggregate. Preserve variations and tags.
Export native reference PGNs and compare original case IDs, starting positions,
and full ordered move trees independently:

```sh
python promotion-audit.py compare special-expanded-v3.pgn native-export.pgn
python promotion-audit.py edges special-ctg-coverage-v3.pgn > expected.tsv
cargo run --release -p cbvault --example promotion_book_audit -- book.ctg < expected.tsv > book-audit.tsv
```

The audit reports missing parents, missing/wrong or unknown moves, child
presence and raw stored bytes; use `--strict` for a zero-missing-edge gate.
Native CTG underpromotion or high-ordinal representation must be established
from converted data, not inferred from the legal source PGN. The byte table
alone cannot encode thousands of distinct context-independent role/ordinal/delta
combinations in 256 byte values. Context-dependent encodings, aliasing,
limitations and import omissions are possibilities to distinguish empirically.

Source files are CC0, published on the benchmark-corpus-20261125-v1 release.
Native special files are not yet supplied. No commercial or private data is
used by this corpus.
