# cbvault-cli

The `cbvault` CLI inspects, verifies and exports both **classic `.cbh` and modern `.2cbh`** ChessBase databases, probes/verifies **`.ctg` opening books**, and lists/extracts `.cbv` and password-protected `.cbz` archives.

```bash
cargo install cbvault-cli --version 0.2.0

cbvault info "Collection.2cbh" --games 5
cbvault verify "Collection.2cbh" --threads 8
cbvault pgn "Collection.2cbh" games.pgn --threads 8
cbvault archive list "Collection.cbv"
cbvault archive extract "Collection.cbv" ./out --threads 8
```

**CTG book commands (available from v0.2.0):**

```sh
cbvault book info "MyBook.ctg"
cbvault book probe "MyBook.ctg" --moves e4 c5 Nf3 --json
cbvault book verify "MyBook.ctg" --children --json
```

This feature was originally contributed by [Tulio Vianna](https://github.com/tuliovianna-lab) in [PR #8](https://github.com/itshak/cbvault/pull/8).

Commands support `--json` for machine-readable output. PGN can be written to a file or stdout; diagnostics stay on stderr. Decoding errors are typed, and archive extraction reports failures rather than claiming a partial result is complete.

**Read-only:** no command writes or edits ChessBase databases. Native 2CBH has a verified 3.41-million-game ChessBase PGN export sample, with the coverage limits detailed in the [main README](https://github.com/itshak/cbvault/blob/main/README.md).

The library is MIT-licensed, based on pinned **MIT-era `oschess-cb-bridge`** code (v1.0.0 classic and v1.0.3 2CBH), **not upstream code added after the switch to AGPL-3.0**. See [provenance](https://github.com/itshak/cbvault/blob/main/docs/provenance.md).
