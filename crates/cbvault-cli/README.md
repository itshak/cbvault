# cbvault-cli

The `cbvault` CLI inspects, verifies and exports both **classic `.cbh` and modern `.2cbh`** ChessBase databases, and lists/extracts `.cbv` and password-protected `.cbz` archives.

```bash
cargo install cbvault-cli --version 0.1.7

cbvault info "Collection.2cbh" --games 5
cbvault verify "Collection.2cbh" --threads 8
cbvault pgn "Collection.2cbh" games.pgn --threads 8
cbvault archive list "Collection.cbv"
cbvault archive extract "Collection.cbv" ./out --threads 8
```

Commands support `--json` for machine-readable output. PGN can be written to a file or stdout; diagnostics stay on stderr. Decoding errors are typed, and archive extraction reports failures rather than claiming a partial result is complete.

**Read-only:** no command writes or edits ChessBase databases. Native 2CBH has a verified 3.41-million-game ChessBase PGN export sample, with the coverage limits detailed in the [main README](../../README.md).

The library is MIT-licensed, based on pinned **MIT-era `oschess-cb-bridge`** code (v1.0.0 classic and v1.0.3 2CBH), **not upstream code added after the switch to AGPL-3.0**. See [provenance](../../docs/provenance.md).
