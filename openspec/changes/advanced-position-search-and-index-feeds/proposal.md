# Advanced position search and incremental index feeds

## Why

Consumers need efficient partial positions, material searches and incremental indexing without tying the library to a persistent application database schema.

## What Changes

Add predicates, efficient streaming/index feeds, revision signals and resource-aware search primitives using gigachess.

## Dependencies and priority

After ctg-opening-tree-streaming-api; see docs/roadmap.md. Preparation may overlap, but do not displace core reader quality.

## Non-goals

No application-owned index storage schema or UI, no source writes, no guarantees for unsupported external edits.

## Licensing and provenance

MIT code, preserve historical MIT attribution. AGPL, GPL and unlicensed references are facts/oracles only under the two-room process in docs/provenance.md. Never upload restricted data.

## Impact

Capability: cbvault. See design.md and tasks.md for API, verification and benchmark gates.
