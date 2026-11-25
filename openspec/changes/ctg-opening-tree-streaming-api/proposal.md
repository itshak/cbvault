# CTG opening-tree API, streaming and hot-path optimization

## Why

Consumers need one position's complete move statistics and responsive navigation, not eager decoding of every child continuation. Whole-book processing separately needs streaming.

## What Changes

Add one-call tree and stats-only APIs, raw/decoded streaming, progress/cancellation, efficient move decode and typed warnings.

## Dependencies and priority

After ctg-native-index-and-fallback; see docs/roadmap.md. Preparation may overlap, but do not displace core reader quality.

## Non-goals

No book writing, UI widgets or consumer-specific naming; no requirement that point lookup scan entire book.

## Licensing and provenance

MIT code, preserve historical MIT attribution. AGPL, GPL and unlicensed references are facts/oracles only under the two-room process in docs/provenance.md. Never upload restricted data.

## Impact

Capability: cbvault. See design.md and tasks.md for API, verification and benchmark gates.
