# consolidate-readers-and-benchmark

## MODIFIED Requirements

### Requirement: Source privacy and local-only large database acceptance

Private or commercially licensed ChessBase sources, exports, metadata and per-game diffs SHALL remain local. Public CI SHALL use generated synthetic fixtures or explicitly approved, independently redistributable benchmark artifacts with pinned hashes, provenance and license manifests. Local private gold gates remain separate and SHALL report NOT RUN when absent.

#### Scenario: CI runs without a Mega Database
- **GIVEN** no private source is available
- **WHEN** public CI runs
- **THEN** synthetic tests and configured public corpus tests run
- **AND** private gold reports NOT RUN and no private source is uploaded

#### Scenario: A public fixture is unapproved or has a mismatched hash
- **WHEN** a corpus manifest is missing its provenance, license approval or expected hash, or a downloaded file disagrees
- **THEN** the job fails closed before decoding it

## ADDED Requirements

### Requirement: Public paired reader benchmarks precede optimization

The project SHALL establish reproducible release-mode CBH and 2CBH baselines on a public real-game corpus and an annotation-rich corpus before modifying reader hot paths. Measurements SHALL report workload counts, versions, corpus hashes, throughput, latency, allocations, RSS and correctness hashes where applicable.

#### Scenario: A performance change is evaluated
- **WHEN** base and candidate run on the same runner and verified corpus
- **THEN** repeated paired results and dispersion are reported
- **AND** byte-level correctness and allocation gates are enforced independently of timing

### Requirement: Shared generation-aware reading and bounded conversion

Both reader generations SHALL share applicable I/O and scheduling primitives while preserving their distinct format validation and public semantics. Streaming SHALL preserve source order, bounded resources and cooperative cancellation.

#### Scenario: Parallel conversion and cancellation agree
- **WHEN** both generations are converted with multiple worker counts and I/O modes
- **THEN** complete outputs equal sequential output byte for byte
- **AND** cancelled delivery is a strict prefix with no silent corruption

#### Scenario: Wrong-generation access or damaged framing
- **WHEN** a consumer uses a fallible accessor or encounters malformed framing
- **THEN** a typed error is returned rather than a panic or misleading empty result
