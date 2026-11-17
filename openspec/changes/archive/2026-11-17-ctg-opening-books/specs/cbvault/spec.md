# ctg-opening-books

## ADDED Requirements

### Requirement: Opening books open read-only and index themselves

The library SHALL open a ChessBase opening book from its `.ctg` file or base name,
read-only, and SHALL build its own in-memory index by scanning the `.ctg`. It SHALL
NOT use the `.cto` table, whose hash function is not established, and SHALL NOT
write to any file of the book.

#### Scenario: A book opens and reports its size

- **WHEN** a book is opened
- **THEN** the number of pages and of positions indexed is reported
- **AND** no file in the book's directory is modified

#### Scenario: A damaged page does not fail the open

- **WHEN** a data page's header or entries do not fit the page
- **THEN** the page is reported as damaged with its number and a reason
- **AND** the readable entries of the rest of the book are indexed
- **AND** the reader does not panic

### Requirement: A position is looked up and its book moves decoded

The library SHALL look a `gigachess` position up in the book, applying the colour
flip when Black is to move and the mirror for a castle-free position with the white
king on files a–d. It SHALL return the position's statistics and its book moves as
legal moves, with castling as king → rook. A move byte whose meaning is not
established SHALL be returned as a typed error and SHALL NOT be guessed.

#### Scenario: The initial position is found

- **WHEN** the initial position is probed in a real book
- **THEN** its book moves decode to legal moves
- **AND** the position after each of them is found in the book

#### Scenario: An unknown move byte is reported, not guessed

- **WHEN** a move byte has no established meaning
- **THEN** that move is reported as an unknown byte
- **AND** the other moves of the position are still returned

### Requirement: Book statistics are decoded

The library SHALL expose each position's games, wins, draws and losses, the rating
counts and sums of both sides, and ChessBase's N as wins plus draws plus losses.
Statistics fields whose meaning is not established SHALL be exposed raw.

#### Scenario: N counts only games with a result

- **WHEN** a position records more games than wins, draws and losses together
- **THEN** N is the sum of wins, draws and losses
- **AND** the total number of games is still available

### Requirement: Book annotations are decoded only where established

The library SHALL decode the move symbols `!` `?` `!!` `??` `!?` `?!`, the "Main
move" and "Don't play in tournament" marks, the position evaluations with the
advantage family oriented for the real side to move, and the colour ChessBase gives
each move. Codes and combinations not established SHALL give no value, never a
guess.

#### Scenario: An evaluation is oriented for the side to move

- **WHEN** a position with Black to move carries a stored advantage evaluation
- **THEN** the evaluation returned is the one for the real colours
- **AND** neutral evaluation signs are returned as stored

#### Scenario: An unobserved colour combination has no colour

- **WHEN** a move's symbol and mark form a combination never observed
- **THEN** no colour is returned for it

### Requirement: A whole book can be verified

The library SHALL check every entry of a book: that its key decodes and encodes
back to the same bytes, that its move bytes decode to legal moves, and optionally
that the position after each decoded move is in the book, reporting the counts.

#### Scenario: A sound book verifies cleanly

- **WHEN** a sound book is verified with the child check
- **THEN** every key round-trips and no decoded move is illegal
- **AND** the number of resulting positions missing from the book is reported

#### Scenario: An unclosed page is counted as damaged

- **WHEN** a data page's entries do not terminate at the declared bytes-used boundary
- **THEN** the page is counted as a damaged page
- **AND** verification continues for the remaining pages

### Requirement: The CLI reads opening books

The `cbvault` CLI SHALL offer `book info`, `book probe` (from the initial position,
a FEN, or a sequence of SAN or UCI moves) and `book verify`, each with a stable
`--json` output.

#### Scenario: A position is probed by moves

- **WHEN** `cbvault book probe <book> --moves e4 c5` is run
- **THEN** the book moves of that position are printed with N, score, draws,
  average rating, colour and annotation

#### Scenario: CLI verification fails on damaged books, unknown moves, or missing children

- **WHEN** `cbvault book verify` is run on a book with damaged pages, undecodable keys, invalid positions, key mismatches, illegal moves, unknown move bytes, or missing children when `--children` is requested
- **THEN** the command exits with a non-zero exit status
- **AND** the counts of damaged pages, unknown moves, or missing children are reported
