# Audit upstream CBH and 2CBH read parity

## Why

Recent upstream work fixes subtle format semantics and improves structured records. Consolidated readers and a benchmark baseline should be available first.

## What Changes

Create a coverage matrix, establish missing format facts independently, and close validated gaps in both generations.

## Dependencies and priority

After consolidate-readers-and-benchmark; see docs/roadmap.md. Preparation may overlap, but do not displace core reader quality.

## Non-goals

No writing, UI-specific code, speculative decoding, or new proprietary source redistribution.

## Licensing and provenance

MIT code, preserve historical MIT attribution. AGPL, GPL and unlicensed references are facts/oracles only under the two-room process in docs/provenance.md. Never upload restricted data.

## Impact

Capability: cbvault. See design.md and tasks.md for API, verification and benchmark gates.
