# Public benchmark corpus and CI plan

Status: proposal, data not downloaded, converted or published. This is the prerequisite phase of consolidate-readers-and-benchmark. Public fixtures must be separately licensed and documented; code remains MIT. Never use private Mega, commercial books, unauthorized study annotations or proprietary ChessBase sample data.

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
