# ctg-native-index-and-fallback

## MODIFIED Requirements

### Requirement: Opening books open read-only and index themselves

The library SHALL open a book read-only and SHALL use a native CTO lookup only after its format and behavior are independently established. The library SHALL provide a scan-based fallback and MAY reuse a validated persistent index outside the source directory. Position counts SHALL be reported as unknown when not established. Source companions SHALL NOT be modified.

#### Scenario: A verified native index is available
- **WHEN** a supported book opens
- **THEN** position probes work without scanning or sorting the entire CTG
- **AND** exact key comparison protects against collisions

#### Scenario: Native index is absent or unsupported
- **WHEN** native lookup cannot be trusted
- **THEN** a scan or verified cached fallback is available
- **AND** a damaged or incomplete lookup is not reported as a definitive miss

#### Scenario: A fallback cache is stale or interrupted
- **WHEN** its identity or format differs or a build is cancelled
- **THEN** the cache is rejected or discarded
- **AND** no source file is modified
