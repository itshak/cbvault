# CTG native CTO lookup and persistent fallback

## Why

Book::open scans and sorts every position, costing 8 bytes per position plus temporary peak memory. Native lookup may eliminate this burden; fallback must remain reliable.

## What Changes

Research CTO and companion semantics, introduce independently verified native probing and optional persistent fallback, bounded build resources and explicit damage policy.

## Dependencies and priority

After audit-upstream-reader-parity; see docs/roadmap.md. Preparation may overlap, but do not displace core reader quality.

## Non-goals

No CTG writing, speculative hash, application-managed index policy or fabricated counts.

## Licensing and provenance

MIT code, preserve historical MIT attribution. AGPL, GPL and unlicensed references are facts/oracles only under the two-room process in docs/provenance.md. Never upload restricted data.

## Impact

Capability: cbvault. See design.md and tasks.md for API, verification and benchmark gates.
