## ADDED Requirements

### Requirement: A ChessBase-style "Game data" search

The library SHALL answer a query holding the fields of the Game data page of
ChessBase's search dialog over a classic or a 2CBH database: two player lines
with "ignore colours" and "wins only", tournament, annotator, a date range, an
ECO range, a move-count range, an Elo range with its mode and a rating
difference, the result boxes, mate, stalemate and check, guiding texts, "any
text" with "not", and the time control. Every field SHALL be optional, every
field given SHALL hold, and an empty query SHALL match every game.

The search SHALL return the matching game numbers in ascending order, and the
same numbers at any thread count.

#### Scenario: An empty query is every game
- **WHEN** a query sets no field
- **THEN** every game of the database matches, and no guiding text or deleted record does.

#### Scenario: Ignore colours and wins only
- **WHEN** a query names a player in the White line, ignores colours and asks for wins only
- **THEN** a game matches when that player won it with either colour
- **AND** "wins only" without a player in the White line is refused as an invalid query.

#### Scenario: Names match without case and accents
- **WHEN** a player line holds `gonzalez`
- **THEN** a player stored as `González` matches
- **AND** by default the text must match the start of the name.

#### Scenario: Ranges with open or partial ends
- **WHEN** a date range's upper end is a year and a month, or an ECO range's upper end has no sub-code
- **THEN** the whole month, or every sub-code, is inside the range
- **AND** a game without a date, or without an ECO code, is outside every range.

#### Scenario: Elo modes
- **WHEN** an Elo range is given in the mode one, both or average
- **THEN** a game matches when one rating, both ratings, or their average is inside it
- **AND** the average and the rating difference need both ratings known.

#### Scenario: Header-only queries never read the moves file
- **WHEN** a query sets no ending (mate, stalemate or check)
- **THEN** only the header file and the namebases are read.

#### Scenario: Endings replay only the kept games
- **WHEN** a query asks for mate, stalemate or check
- **THEN** only the games every other criterion kept are replayed, and the final position of the main line decides
- **AND** a Chess960 game is replayed from the start its header's number builds
- **AND** a game that cannot be replayed is counted and never matched.

#### Scenario: What the format does not tell is refused
- **WHEN** a query asks for the time control of a classic database
- **THEN** the search fails with an unsupported-query error instead of guessing from an undecoded byte.

### Requirement: Names resolve in both generations

The query SHALL resolve player, tournament, annotator and source texts to
entity-id sets in classic and 2CBH databases alike, in one pass per namebase,
before the header scan. In a 2CBH database an annotator SHALL be looked up in
the player table, which the header's annotator id indexes.

#### Scenario: A 2CBH database is searched by name
- **WHEN** a query names a player, a tournament or an annotator on a 2CBH database
- **THEN** the games of every matching entity are found.
