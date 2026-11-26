# Public benchmark corpus and CI plan

Status: public source PGNs generated and validated; native conversion audit in progress. The 25,000-game native smoke tier has passed local replay and cross-format byte parity. This is the prerequisite phase of consolidate-readers-and-benchmark. Public fixtures must be separately licensed and documented; code remains MIT. Never use private Mega, commercial books, unauthorized study annotations or proprietary ChessBase sample data.

## Sources

- Primary: https://database.lichess.org/ — rated standard-game dumps CC0. Pin https://database.lichess.org/standard/lichess_db_standard_rated_2018-01.pgn.zst (17,945,784 games, 5.47 GB reported at review); stream complete PGN records, do not split on blank lines alone. Includes clocks and a subset with evaluations and generated comments. Confirm exact counts and SHA-256 on acquisition. Alternative: choose a smaller pinned month or use partial download for a fixed prefix, record precise bytes and game selection. For full corpus keep source download off CI.
- Historical OTB candidate: https://figshare.com/articles/dataset/Chess_Database/4276523 (~3.5M, declared CC BY 4.0; attribution and change notice required; verify upstream compilation/annotation provenance and actual contents before publication). https://juan-ignacio-perotti.github.io/chess-dataset/ describes formats. Not approved by this document alone.
- Optional Lumbra's Gigabase https://lumbrasgigabase.com/en/download-area-en/ — main corpus CC BY-NC 4.0, separate Lichess broadcast corpus CC BY-SA 4.0. Keep NC data local and noncommercial, not public canonical corpus; obtain explicit unrestricted redistribution permission if desired. Annotation density unverified; census sample before relying on it. Raw game facts do not establish rights to compilation or commentary.
- Do not scrape copyrighted annotations or assume public PGN/study means permissively licensed. Use CC0 Lichess data and original deterministic annotations for the default corpus; record license of all generated additions.

## Corpus tiers (proposed targets, adjust measured disk and runtime)

| Tier | Games | Purpose |
|---|---:|---|
| smoke | 10,000–25,000 | CI per PR, both formats, generated malformed fixtures separate |
| standard | 500,000–1,000,000 | regular benchmark, real headers and mixed annotations |
| scale | 3,000,000–5,000,000 | scheduled/manual, if runner disk/time allows |
| rich | 10,000–50,000 | actual evaluated games plus deterministic nested RAV, NAGs, comments, UTF-8, clocks/evals, arrows/squares |
| special | 50–200 | manually authored ChessBase-specific texts, analysis, training, quotations, setup, Chess960, promotions, castling, missing tags |

Choose deterministic stream-order or hash-based sampling with separate strata for evaluated vs unevaluated games. Keep source IDs and generator seed/version, no duplicates within a tier except explicitly labeled stress fixtures. Generate legal variations through gigachess, original texts, known expected annotation values. PGN does not faithfully express every native feature: maintainer authors special cases in ChessBase itself. Census original PGN and both imported databases; record rejected records and unsupported fields, never silently count PGN import loss as reader success.

## Maintainer conversion handoff

1. Generate/pin source PGNs, include license and manifest, SHA-256, source count, selection seed, annotation census. Keep same inputs for both formats.
2. In ChessBase create NEW classic and 2CBH databases, import the exact same PGN separately, disable deduplication/filtering unless recorded; record app version, settings, record counts and any rejected games. Verify move/annotation retention. Add authored special features to both where supported. Export each back to PGN with all supported tags/variations/comments; preserve a field-level oracle for non-PGN native data and record intentional differences. Optional build CTG from same source with recorded depth/settings, include CTO/CTB and other needed companions.
3. Close ChessBase, copy complete file families to clean directories, exclude unrelated metadata or personal data; verify distribution rights for generated artifacts and data, including any app-specific embedded assets. Archive with zstd/zip, record file-level and archive SHA-256 and exact sizes. Include LICENSE-DATA, attribution, manifest and conversion notes. Store no large binary directly in git; publish immutable versioned release assets (split below host limits) or approved public object storage only after audit. Cache is an optimization, not the source of truth.

## CI execution

Use standard public GitHub-hosted runners (standard runner compute is free for public repositories; storage limits/charges still apply: https://docs.github.com/en/billing/concepts/product-billing/github-actions). Pin runner image, Rust/toolchain, baseline commit, corpus version and manifest. PR smoke: small corpus, synthetic/malformed tests, exact hashes and allocation gates on Linux; existing macOS/Windows tests retained, run public smoke on all platforms when feasible. Nightly or manual: paired base/head on same runner, standard tier both generations, mmap/unmapped, 1/2/4 workers up to available CPUs; scale on manual only if disk and memory preflight allows. Set timeout/concurrency, read-only permissions; avoid privileged pull_request_target for untrusted PR code. No secrets or commercial corpus. Download once, verify SHA-256, extract safely, measure actual disk headroom (runner storage is limited), keep large inputs out of job artifacts, upload compact JSON/CSV summaries only. Report unavailable corpus as failed configured benchmark, private gold NOT RUN. Avoid mandatory timing thresholds until noise quantified; correctness failure always blocks.

## Workloads and output

Benchmark actual release-mode library/CLI code: header scan, selective and broad filters, random game read (fixed seed), mainline/tree decode, key feed, annotations on/off, PGN export to counting/hash sink and real file separately, sequential/parallel, cold-ish/warm access, memory modes, CTG open/probe/tree/stream when available. Report games, plies, bytes, wall time, throughput, P50/P95/P99 for random probes, peak RSS, allocation counts, workers, CPU/OS/toolchain, commit and corpus hash. Exclude download, compile, decompress from timed region. Alternate base/candidate, warmup, at least five paired repetitions where practical; report median, dispersion, relative changes and noisy runs. Distinguish filesystem cache conditions; do not claim cold OS cache without control. Keep existing gold semantic/byte normalization and strict sequential/parallel identity gates; establish independent expected counts and hashes per format before optimizing. Store baseline v0.2.0 and subsequent benchmark records with exact environment; no made-up benchmark values.


## Native CI ingestion (2026-11-26)

The maintainer supplied native archive checksums and CBH/2CBH companions. The
smoke fixture is pinned in `scripts/bench-corpus/native-manifest.json`; extraction
rejects path traversal, non-regular members and unexpected extensions and drops
AppleDouble and application INI metadata. The release asset is a normalized
repack of the audited native files, not a private database. The source corpus
contains 25,000 CC0 Lichess games plus original synthetic annotations. Native
source slots differ (classic 25,000; 2CBH 25,001 including its non-game slot).
Both readers replay 1,648,722 main-line plies and 1,710,550 total plies and
export identical PGN bytes. These counts are independently pinned to the source
PGN. The PGN byte hash is a cbvault regression baseline, not an independently
exported ChessBase oracle; native annotation semantic parity is not yet proven.

The new native workflow runs a bounded Linux PR smoke and manual smoke, pairing
v0.2.0 (commit 59f66f921e82a813f96dd6f289084d7ca9f99416) with the candidate,
checking counts and exact output hashes in sequential, parallel, and unmapped
modes before measuring five paired repetitions. The reports contain timings,
CLI counts, environment and dispersion; timing is not a release gate yet.
The existing three-platform correctness workflow remains unchanged.

Audit blockers: classic 1M fails at game 226476, while 2CBH passes. The special
archive contains 110120 games instead of 120; quarantine it until provenance
and intended contents are verified. The CTG corpus, independent ChessBase
reference PGNs, full scale audit, allocation and RSS gates, and broader
workloads remain pending. No private gold data is uploaded.

The rich (100,000 games) and extreme (10,000 games, over 10M variation
plies) native corpora also passed both full replay and matching cross-format PGN
export. These are separate release assets; manual workflow tiers are available
for each, with a weekly rich run after merging to main. Scale native CBH and
2CBH have correct header counts (3,410,497 games; 2CBH has an extra reserved
slot); full replay found two classic failures (games 226476 and 2712614), zero
2CBH failures. The 3.43 GiB upload contains an approximately 7.94 GB CTG
file. Its verified archive is kept local until book testing and split assets
are possible; only the DB files were extracted to preserve disk space.

### Hardened native gate

Fixed two classic mode-0 decoder failures: at most three pieces of each
non-pawn role have compact codes; overflow promoted pieces remain square-only
even after captures, while a later promotion can claim the next compact number.
Two embedded native move-record regressions cover the originally failing games.
Full 1M and 3,410,497-game CBH and 2CBH replay and export now match exactly
(0 failures; scale 227,472,245 mainline and 236,686,657 total plies).
The standard and scale DB-only assets are published separately from CTG.
Their comparison baseline is the first corrected revision 72c629e; the three
smaller corpora retain v0.2.0 as baseline.

CI gates exact source record counts, replay counts, full PGN SHA-256, companion
hashes, and pinned workload fingerprints. Full replay/export is tested at
1/2/4 workers and without mmap. Additional paired API workloads exercise
header count, broad Elo filter, selective two-player Elo filter, seeded 2,048
random game reads, and annotated/unannotated random PGN export. Search tests
check ordered match IDs at 1/2/4 workers; random reads check exact move/output
fingerprints, P50/P95/P99 latency and operation counts. Separate allocator
passes measure alloc/realloc calls (all worker threads); GNU time measures
process peak RSS on Linux. Allocation and timing observations are not hard
performance thresholds. The API fingerprint is FNV-1a, a deterministic
regression checksum; full export uses SHA-256. The existing PR gate covers
smoke and extreme; weekly runs cover standard, rich and extreme; scale is
available manually, with all five triggered on the current roadmap branch.
Special-120 remains excluded pending maintainer regeneration. No private gold
is uploaded, and independent ChessBase-export annotation parity is still pending.

### CTG scale workload

The maintainer's scale opening book is packaged separately from the CBH/2CBH
datasets. It consists of the 7.94 GB CTG plus CTO and CTB companions, with
file-level and multipart SHA-256 checks in native-book-manifest.json. The
fetcher validates individual parts and the concatenated stream, and only
extracts the three exact expected regular files. This avoids permanently
extracting the book on aiserver. The dedicated workflow runs complete key
round-trip, legal move and child-existence verification on both baseline and
candidate; it then pairs three measured open and deterministic probe passes
(2,048 bounded opening walks), comparing stable statistics and move
fingerprints, with peak RSS and latency percentiles. The workflow is manual or
monthly after integration, and runs on the CTG benchmark branch during initial
validation. No assumption of successful CTG verification is made before that
real-data run completes.

### Experiment: borrowed native entity names

The 2CBH scanner previously allocated raw payloads and decoded owned names for
every matching row. The experiment borrows validated mapped UTF-8 strings,
retains lossy replacement and positioned-read fallbacks, and uses those fields
when populating reusable random-game buffers. Local alternating three-pair
runs (4 workers, matching IDs and move/PGN fingerprints) reduced standard
broad search from 0.268 s to 0.162 s and 3,059,135 to 1,569 allocation calls;
scale broad search from 1.083 s to 0.644 s and 10,591,101 to 4,858 calls.
Scale selective search improved 1.23x; standard random-move reads 1.21x.
Random PGN timings were mixed. These are preliminary local measurements,
not GitHub-runner performance claims. Unicode, invalid UTF-8, malformed
entity records and unmapped name resolution have explicit regression coverage.

### Promotion overflow and random-read follow-up

The zero-copy 2CBH name experiment passed every GitHub corpus. On the 3.4M
four-worker broad search it achieved 1.47x speedup and reduced allocation
calls from 10,591,062 to 4,816; standard broad search was 1.52x faster.
CTG full verification passed all 85,834,993 entries and 87,463,673 moves,
with zero unknown or illegal moves, damaged pages or missing children.

The original 120-case special PGN is superseded for conversion by the
6,572-game promotion overflow v3 source, with a separate 3,778-game
standard-start CTG coverage source. Native special conversion is pending;
independent move-tree and CTG edge audit tools are included. See the
benchmark-corpus tooling README for exact conversion instructions.
The next performance candidate eliminates per-record path clones,
validates offsets without arithmetic overflow, borrows names in PGN tag
writing, and reuses a lazy PGN writer inside GameBuf for random export.
Local targeted mapped/unmapped tests and Clippy passed; paired CI is the
performance authority.

### Verified reader experiments (2026-11-26)

All timings below are paired measurements on the checksum-pinned public native
corpus. They are not comparisons to other libraries. The ID-only API deliberately
returns less data than a named scan; it is appropriate when the consumer needs
only IDs, not a replacement for callers that require metadata.

| Experiment | Evidence | Result |
|---|---|---|
| Reused PGN buffers | [45847322315](https://github.com/itshak/cbvault/actions/runs/45847322315) | All five tiers pass. |
| CTG key/entry allocations and legal generation | [45849147120](https://github.com/itshak/cbvault/actions/runs/45849147120) | 85,834,993 entries and 87,463,673 moves; zero strict errors. Open 3.85 -> 2.56 s; open allocations 257,505,985 -> 1,006; lookup p95 3.89 -> 1.64 us. |
| ID-only scan | [45850795524](https://github.com/itshak/cbvault/actions/runs/45850795524) | All five tiers pass. Scale mapped four-worker all-ID scan 35.68x CBH, 23.51x 2CBH; broad 18.24x and 12.82x. Unmapped 2CBH broad 51.10x. Old classic named scan cannot run unmapped; no ratio claimed. |
| Reusable context | [45852216322](https://github.com/itshak/cbvault/actions/runs/45852216322) | All four tiers pass. Scale 16,384-record repeated requests: 1.57x CBH, 1.27x 2CBH, mapped four workers. |
| Child statistics | [45851071642](https://github.com/itshak/cbvault/actions/runs/45851071642) | Full verification and all-child fingerprint pass; 0.648 -> 0.572 s, 188,976 -> 32,700 lookup allocations. Parent p95 increased on this runner; do not claim universally faster lookup. |
| Bounded fallback index | [45851279337](https://github.com/itshak/cbvault/actions/runs/45851279337) | Correct but no demonstrated reduction in process peak RSS or material startup gain. Kept experimental. |
| Reusable record scratch | [45852516392](https://github.com/itshak/cbvault/actions/runs/45852516392) | All four tiers pass, but random export timings mixed, so not consolidated wholesale. |
| Native CTO directory | [45851944861](https://github.com/itshak/cbvault/actions/runs/45851944861) | Every key routed to its real page, zero missing or ambiguous routes. Native directory open 0.10 ms (not full reader startup). Native-reader differential benchmark separately pending. |

The integration branch combines the verified CTG hot path, child statistics,
ID-only scan, reusable execution context, continuous conversion and bounded position replay. Both latter experiments passed their full scale gate. Separate private golden PGNs, additional CTG variants and
maintainer-created promotion-overflow companions are not validated by these
runs. No compatibility or release claim until the combined CI gates pass.

### Roadmap experiment: bounded streaming and keyed replay

The original consolidation design (tasks 1.6–1.8) identifies repeated header
reads, wave barriers and cancellation as explicit optimization targets.
The v2 experiment reuses the already-read header and at most two arenas per
worker, streams results continuously in source order, and checks cancellation
before each delivery and during worker decoding. It adds exact-prefix tests
across worker counts, batch sizes and stop positions. A separate paired
stream benchmark measures every emitted game, move and key, exact-position
search hits for the root and after e4, and cancellation tail/overshoot.
Source game and ply counts remain hard gates, and the ordered fingerprints
must agree across both readers, revisions, worker counts and tested mapping
modes. Timing and allocation results are reported separately; no performance
gain is claimed until CI completes. Cancellation measurements intentionally
report different delivered counts on old and new implementations rather than
requiring the old wave overshoot to be preserved.

### Integrated streaming and position replay (2026-11-26)

- Continuous conversion: [45850830515](https://github.com/itshak/cbvault/actions/runs/45850830515), all five tiers pass. Mapped 2CBH scale four-worker keyed conversion 8.162 -> 7.549 s, cancellation delivers 1,024 instead of 8,192 games, cancellation tail 6.11 -> 2.13 ms.
- Bounded position replay: [45857489568](https://github.com/itshak/cbvault/actions/runs/45857489568), all three tiers pass in both mapping modes. Scale first-hit latency improves 403.4x CBH and 389.65x 2CBH, mapped four workers; callback cancellation at 1,024 hits reduces replay from 3,410,497 to 8,192 games. Full replay throughput stays within 2%; exact complete-search fingerprints agree.
- Combined CI [45856202092](https://github.com/itshak/cbvault/actions/runs/45856202092) failed on the outdated fixed move-table cardinality assertion (170 vs 179); uniqueness itself passed. Updated the assertion to include the nine previously verified mappings, and retain the uniqueness gate.
- Native/adaptive reader differential runs passed [45856298025](https://github.com/itshak/cbvault/actions/runs/45856298025) and [45857248389](https://github.com/itshak/cbvault/actions/runs/45857248389). Further real-book CTG validation is deferred to the maintainer's local PowerBook test; not included in this integration.

### Second consolidated reader batch (2026-11-26)

All four paired runs passed every configured tier; full-scale corpus has 3,410,497 games. Four-worker mapped direct position matching improves CBH root/e4/absent 1.047x/1.040x/1.049x, 2CBH 1.009x/1.055x/1.035x ([45872032842](https://github.com/itshak/cbvault/actions/runs/45872032842)). The rich v2 tier is slightly slower; report mixed results rather than universal gains.

2CBH cache policy ([45872292876](https://github.com/itshak/cbvault/actions/runs/45872292876)): scale keyed stream 7.848 -> 7.001 s; root 7.245 -> 6.363 s; e4 7.237 -> 6.322 s. All four cache modes checked against full position keys, including nulls, variations and Chess960. Small smoke timings mixed.

Shared v2 header windows ([45875468249](https://github.com/itshak/cbvault/actions/runs/45875468249)): scale mapped named selective 104.18 -> 75.41 ms; positioned named selective 647.52 -> 177.91 ms, broad 1.702 -> 1.362 s; positioned position replay 1.06x, streaming 1.057x. Exact full metadata fingerprints.

ID page ([45875714097](https://github.com/itshak/cbvault/actions/runs/45875714097)): first 100 IDs, exact total, four workers, scale mapped all filter 2.28x CBH and 1.37x 2CBH versus materializing all IDs; positioned peak process RSS 30,776 -> 3,968 KiB CBH and 36,200 -> 10,236 KiB 2CBH. Selective pages may have negligible gain. Sources must remain immutable through both passes.

Previous combined gates passed [45872314054](https://github.com/itshak/cbvault/actions/runs/45872314054) and [45872314117](https://github.com/itshak/cbvault/actions/runs/45872314117). This batch is consolidated pending new combined gates. CTG native/adaptive real-book work remains deferred.
