# advanced-position-search-and-index-feeds

## ADDED Requirements

### Requirement: Partial position and material search is exact

The library SHALL support explicitly defined partial-board and material predicates composed with header filters, and SHALL use gigachess replay as the correctness oracle.

#### Scenario: A fragment leaves squares unconstrained
- **WHEN** a game contains the specified pieces and material
- **THEN** hits equal exhaustive replay under the selected mainline or variation semantics
- **AND** unconstrained squares do not introduce false exclusions

### Requirement: Index feeds expose bounded source revision signals

The library SHALL stream position keys and requested metadata with source identity and cancellation. It SHALL signal unknown or changed source revision rather than imply append-only safety without evidence.

#### Scenario: A source is changed externally
- **WHEN** a consumer resumes an index feed against a different or ambiguous source
- **THEN** it receives an explicit invalidation or uncertainty signal
- **AND** stale records are not represented as current
