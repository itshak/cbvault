## ADDED Requirements

### Requirement: Public library contracts and documentation remain consumer-neutral

The library's public documentation, README, crate root headers, module comments, integration examples, and metadata SHALL NOT reference or disclose unreleased proprietary downstream product names, internal schemas, or private target formats. All conversion traits, database façade methods, and command-line interfaces SHALL be documented in terms of generic downstream callers, consumers, and data pipelines.

#### Scenario: Crate documentation uses generic consumer terminology

- **WHEN** a developer inspects public crate documentation, module headers, or `README.md`
- **THEN** all conversion patterns describe generic downstream consumers
- **AND** no references to unreleased proprietary product names or private schema formats appear.

#### Scenario: Conversion examples demonstrate generic consumption

- **WHEN** a developer inspects integration examples such as `examples/convert.rs`
- **THEN** the example demonstrates populating caller-defined application structures
- **AND** the code does not claim or quote internal schemas of unreleased products.

## MODIFIED Requirements

### Requirement: Conversion streams records to a consumer sink

The library SHALL convert a database by streaming each game to a
consumer-implemented sink: the borrowed header fields, the resolved entity
names, the main line as 16-bit `moves2`, and — when the sink asks for them —
the per-position Polyglot keys. The library SHALL NOT write target database
files, SHALL NOT depend on an embedded or external database engine, and SHALL
allocate no per-game row, string or vector across the sink boundary.

#### Scenario: A moves-only conversion

- **WHEN** a sink whose `wants_keys` is `false` consumes a database
- **THEN** every game arrives with its `moves2` main line and resolved tags
- **AND** the walk uses the fast move that maintains neither hash nor checkers.

#### Scenario: An indexed conversion

- **WHEN** a sink whose `wants_keys` is `true` consumes a database
- **THEN** every game also arrives with one Polyglot key per position reached,
  in position order
- **AND** the keys equal those an independent replay of the same `moves2`
  produces.

#### Scenario: Annotations are not wanted

- **WHEN** a sink whose `wants_annotations` is `false` consumes a database
- **THEN** no annotation record is read
- **AND** the conversion is faster than the same conversion with annotations on.

#### Scenario: A game that cannot be decoded

- **WHEN** a record is damaged beyond recovery
- **THEN** the sink's `failed` callback receives the game id and a typed error
- **AND** the conversion continues with the next game.
