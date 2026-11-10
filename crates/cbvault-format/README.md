# cbvault-format

**Read-only binary-format parser for classic `.cbh` and modern `.2cbh` ChessBase databases, plus `.cbv` and password-protected `.cbz` archives.** This crate handles byte-level headers, framed move records, annotations, entities and archive codecs. Chess move legality and SAN live in `cbvault-chess`/`gigachess`.

| Input | What is decoded |
|---|---|
| Classic `.cbh`, `.cbg`, `.cba`, namebases and extended offsets | Header metadata, move records, annotations and supporting binary structures |
| Modern `.2cbh`, `.2cbg`, `.2cba`, `.2lid` | 192-byte headers, framed 16-bit move words, structured annotations and entity names |
| `.cbv` / `.cbz` | All four archive compression modes, including stored, LZ, Huffman and Huffman+LZ |

The tested reference archive extracted **all 3,871 members / 3.61 GB byte-identically** against reference output. Native 2CBH higher-level game/PGN decoding is available through the public [`cbvault`](https://crates.io/crates/cbvault) crate.

**MIT provenance:** the classic and 2CBH readers were based on [MIT-era `oschess-cb-bridge`](https://github.com/asavis/oschess-cb-bridge) (classic [v1.0.0](https://github.com/asavis/oschess-cb-bridge/tree/ca9e8f8e4389edd6430f02a14b33ff53552bcadc), 2CBH [v1.0.3](https://github.com/asavis/oschess-cb-bridge/tree/bffebfc498d52b4e4631aed32b2f8e8194d125ff)); **code added after its GPL switch was not copied or linked.** Archive codecs were implemented independently from format facts.

[Main README](../../README.md) · [Format research](../../docs/format-spec-2cbh.md) · [Provenance](../../docs/provenance.md)
