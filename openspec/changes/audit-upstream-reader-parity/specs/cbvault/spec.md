# audit-upstream-reader-parity

## ADDED Requirements

### Requirement: Reader parity is evidence-based and exposes structured data

The reader SHALL expose established header, setup, metadata, guiding text and annotation facts in generation-aware structured form. Unestablished fields SHALL remain raw or explicitly unsupported; existing supported behavior SHALL NOT regress.

#### Scenario: A special record is present
- **WHEN** a reference fixture includes analysis, guiding text, nested annotations or a nonstandard setup
- **THEN** established fields match the independent oracle and unknown fields are reported
- **AND** a malformed record produces a typed diagnostic without inventing data

#### Scenario: A reference improvement is adopted
- **WHEN** an AGPL or unlicensed implementation reveals a possible gap
- **THEN** a separate facts specification and independent verification precede implementation
- **AND** parity and benchmark evidence are recorded
