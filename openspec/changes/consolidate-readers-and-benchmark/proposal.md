# Consolidate CBH/2CBH readers and establish reproducible performance gates

## Why

The core product is fast, accurate reading. Classic and v2 paths have diverged; CI currently compiles benchmarks but does not execute a representative real-data benchmark. Establish a public corpus and baseline BEFORE changing the hot path.

## What Changes

Unify positioned I/O, record windows and bounded ordered conversion; remove redundant allocations and parsing; improve search and resource management; preserve format-specific validation. Introduce a public CI benchmark corpus with explicitly permissive data licensing.

## Dependencies and priority

First active change; baseline phase is a hard prerequisite to reader optimization.

## Non-goals

No source writing, CTG changes, persistent application-specific index format, or unmeasured speed claims.

## Licensing and provenance

MIT code, preserve historical MIT attribution. AGPL, GPL and unlicensed references are facts/oracles only under the two-room process in docs/provenance.md. Never upload restricted data.

## Impact

Capability: cbvault. See design.md and tasks.md for API, verification and benchmark gates.
