# ChessBase 2CBH Post-MIT Factual Specification

> **Status:** Clean-room factual reference (Room 1 deliverable, Task 3.2).
> **Provenance:** Clean-room factual extraction by Room 1 Specifier.
> Contains zero copied code, zero AGPL text, zero Java snippets, and zero proprietary database content.
> Formulated purely as mathematical offsets, data types, value mappings, and reproducible synthetic recipes.

---

## 1. Container and Header Record Architecture (`.2cbh`)

The `.2cbh` file is a fixed-record sequential container composed of 192-byte (`0xc0`) records.
Record 0 is the file header; records 1 through $N$ correspond to database entries (games, guiding texts, analyses, deleted slots).

### 1.1 Record Classification (`rec[2]`)
Offset `0x02` of each 192-byte record determines its semantic category:

| Kind Byte (`rec[2]`) | Category | Meaning & Structural Roles |
|---|---|---|
| `0` | **Deleted / Unused** | Free slot or deleted game. Offset `0x08` and `0x10` may hold recycled offsets. Skipped by game iterators. |
| `1` | **Standard Game** | Standard chess game or Chess960 game with moves in `.2cbg` and optional annotations in `.2cba`. |
| `2` | **Analysis** | Standalone analysis line or engine analysis board, with move tree in `.2cbg`. |
| `3` | **Guiding Text** | Non-game instructional text, with HTML body stored in `.2cbg`. |

### 1.2 Layout Comparison: Games vs Analyses vs Guiding Texts

The byte layout within the 192-byte record varies according to the record category:

| Offset Range | Type | Standard Game (`kind == 1`) | Analysis (`kind == 2`) | Guiding Text (`kind == 3`) |
|---|---|---|---|---|
| `0x00..0x01` | `u16 LE` | Flags / Version markers | Flags / Version markers | Flags / Version markers |
| `0x02` | `u8` | `1` | `2` | `3` |
| `0x03` | `u8` | State (1 = active) | State (1 = active) | State (1 = active) |
| `0x08..0x10` | `u64 LE` | Offset in `.2cbg` (moves) | Offset in `.2cbg` (moves) | Offset in `.2cbg` (HTML body) |
| `0x10..0x18` | `u64 LE` | Offset in `.2cba` (annotations) | Offset in `.2cba` (annotations) | Unused / Zero |
| `0x18..0x20` | `u64 LE` | White Player Entity ID (`.2lid`) | Title Game Tag ID (`.2lid`) | Title Game Tag ID (`.2lid`) |
| `0x20..0x28` | `u64 LE` | Black Player Entity ID (`.2lid`) | Unused / `-1` | Author Player Entity ID (`.2lid`) |
| `0x28..0x30` | `u64 LE` | Tournament Entity ID (`.2lid`) | Author Player Entity ID (`.2lid`)| Unused / `-1` |
| `0x30..0x38` | `u64 LE` | Annotator Entity ID (`.2lid`) | Annotator Entity ID (`.2lid`) | Unused / `-1` |
| `0x38..0x40` | `u64 LE` | Source Entity ID (`.2lid`) | Source Entity ID (`.2lid`) | Unused / `-1` |
| `0x40..0x48` | `u64 LE` | White Team Entity ID (`.2lid`) | Unused / `-1` | Unused / `-1` |
| `0x48..0x50` | `u64 LE` | Black Team Entity ID (`.2lid`) | Unused / `-1` | Unused / `-1` |
| `0x4c..0x50` | `u32 LE` | Unused / Reserved | **Played Date** (packed) | Unused |
| `0x50..0x52` | `u16 LE` | Game Tag Entity ID (`.2lid`) | **Mainline Move Count** | Unused |
| `0x58` | `u8` | Result (0=*, 1=1/2, 2=1-0, 3=0-1) | Result | `0` |
| `0x60..0x62` | `i16 LE` | White Elo Rating | `0` | `0` |
| `0x68..0x70` | 8 bytes | White Rating Authority String | Unused | Unused |
| `0x70..0x72` | `i16 LE` | Black Elo Rating | `0` | `0` |
| `0x78..0x80` | 8 bytes | Black Rating Authority String | Unused | Unused |
| `0x80..0x82` | `u16 LE` | ECO Code | ECO Code (unlisted in UI) | Unused |
| `0x8a..0x8c` | `u16 LE` | **Mainline Move Count** | Unused / Internal | Unused |
| `0xbc..0xc0` | `u32 LE` | **Played Date** (packed) | Secondary Date / Internal | Unused |

### 1.3 Packed Date Encoding
Dates are packed into 32-bit unsigned little-endian integers:
$$\text{packed} = (\text{year} \ll 9) \mid (\text{month} \ll 5) \mid \text{day}$$
- $\text{year} = \text{packed} \gg 9$
- $\text{month} = (\text{packed} \gg 5) \mathbin{\&} 0\text{x}0\text{f}$
- $\text{day} = \text{packed} \mathbin{\&} 0\text{x}1\text{f}$
- Month 0 or Day 0 represents unknown date components (`????.??.??`).

---

## 2. Entity Management Architecture (`.2lid`)

The `.2lid` file is the global name and entity repository for a 2CBH database.

### 2.1 Variable-Length Header
The header size is indicated dynamically by the 32-bit unsigned little-endian integer at offset `0x00`:
- Standard observed sizes: 184 bytes (`0xb8`), 216 bytes (`0xd8`), 228 bytes (`0xe4`), 236 bytes (`0xec`).
- Offset `0x80` begins an array of entity block pointers formatted as `(i32, i32)` index-size pairs.
- Entity data payload begins strictly at offset $\text{header\_size}$.

### 2.2 Entity Kinds
The standard entity types mapped at index offsets:
- `0`: **Player** (contains `last_name`, `first_name` strings).
- `1`: **Tournament** (contains `title`, `place` strings, `u32 LE` date, and 1 byte `event_type`).
- `2`: **Game Tag** (contains tag / title string).
- `3`: **Annotator / Source** (contains text label string).

### 2.3 Tournament Event Type Byte
Following the tournament start date at the end of the tournament record sits a 1-byte field encoding time control and tournament style:
- Bit `0x20`: **Blitz** time control.
- Bit `0x40`: **Rapid** time control.
- Bit `0x80`: **Correspondence** event.
- Bits `0x00..0x1f`: Tournament format / category code.

---

## 3. Move Stream and Game Framing (`.2cbg`)

### 3.1 Record Container Geometry
All variable-length records in `.2cbg` (and `.2cba`) share the self-delimiting frame structure:

```
[0x00..0x08]  Magic: 88 77 66 55 44 33 22 11 (LE u64: 0x1122334455667788)
[0x08..0x0c]  Length A: u32 LE (content payload length)
[0x0c..0x10]  Spare B: u32 LE (spare capacity bytes reserved for in-place edit)
[0x10..0x18]  Checksum: u64 BE (big-endian 8-slice modulo sum of content)
[0x18..0x1a]  Tag: u16 LE (variant: 1=Normal, 2=Chess960, 0x2000=Annotation)
[0x1a..0x1a+A] Content payload: A bytes
[0x1a+A..0x1a+A+B] Spare padding: B zero bytes
[0x1a+A+B..0x1a+A+B+8] Trailer: u64 LE = A + B + 34
```

### 3.2 8-Slice Big-Endian Checksum Formula
For content of length $A$:
- If $A < 8$: the content is zero-extended to 8 bytes and read as a big-endian 64-bit integer.
- If $A \ge 8$: divide content into 8 contiguous slices of length $m = \lfloor A / 8 \rfloor$. Byte $i \in [0, 7]$ of the checksum is:
$$\text{checksum}[i] = \left( \sum_{b \in \text{slice}[i]} b \right) \bmod 256$$
The resulting 8 bytes are written directly in big-endian order at offset `0x10`.

### 3.3 Set-Up Position Word Encodings
A move stream beginning with `0xfffb` specifies a non-standard start position.
It is followed by a 3-word header ($3 \times 16$-bit words) preceding the piece placement list:
1. **Word 0:** Move number (1-based integer).
2. **Word 1:**
   - **Low byte (`word & 0xff`):** Side to move ($0 = \text{White}$, $1 = \text{Black}$).
   - **High byte (`word >> 8`):** Castling rights bitfield:
     - Bit 0 (`0x01`): White O-O-O (Queenside).
     - Bit 1 (`0x02`): White O-O (Kingside).
     - Bit 2 (`0x04`): Black O-O-O (Queenside).
     - Bit 3 (`0x08`): Black O-O (Kingside).
3. **Word 2:** Reserved / en passant file indicator (zero in standard practice).

### 3.4 Move-Word Enumeration Structure
16-bit little-endian words:
- `0x0000..0xb128`: Empty-board pseudo-legal piece moves (`movetable`).
- `0xb129..0xb12c`: Standard castling moves.
- `0xb12d..0xc02c`: Chess960 castling moves.
- `0xc02d..0xc30c`: Set-up position piece placement words.
- `0xfffa`: **Null move** (`--`).
- `0xfffb`: **Start position marker**.
- `0xfffc`: **Moves section start marker**.
- `0xfffd`: **Alternative variation branch marker**.
- `0xffff`: **End of line / variation pop marker**.

### 3.5 Promotion-Capture Word Ordering
When a pawn on rank 7 (White) or rank 2 (Black) makes a capturing move with promotion:
The 16 possible move words for a given origin square are indexed primarily by the **captured piece role** (Queen, Knight, Bishop, Rook), and secondarily by the **promoted piece role** (Queen, Knight, Bishop, Rook).

---

## 4. Annotations Architecture (`.2cba`)

All `.2cba` records carry outer framing tag `0x2000`. Content holds contiguous annotation blocks ordered by move index (or position `-1`).

### 4.1 Annotation Types and Wire Formats

| Type ID | Name | Payload Geometry | PGN Representation |
|---|---|---|---|
| `0x03` | Move / Position Symbols | 3 bytes: `[move_nag, pos_nag, prefix_nag]` | Standard NAGs (`$1`, `$14`, etc.) |
| `0x04` | Colored Squares | `u32 LE` count $N$, then $N/2$ pairs of `(u8 color, u8 square)` | Display metadata / GUI shapes |
| `0x05` | Colored Arrows | `u32 LE` count $N$, then $N/3$ triples of `(u8 color, u8 from, u8 to)` | Display metadata / GUI shapes |
| `0x07` | Time Spent | 4 bytes reserved, then `[u8 sec, u8 min, u8 hr]` | Move clock timing |
| `0x13` | Game Quotation | 6 length-prefixed strings, 35-byte binary block, 2 rating strings, 29-byte start block, 5-byte moves stream | Formatted comment: `Result White - Black (Elo) Event` |
| `0x16` | White Clock | `u32 LE` centiseconds | Position clock tag |
| `0x17` | Black Clock | `u32 LE` centiseconds | Position clock tag |
| `0x21` | Engine Evaluation | 3 signed `i16 LE` words: `[score, kind, depth]` | Engine evaluation comment |
| `0x22` | Medals | `u32 LE` bitfield | `[%mdl <bits>]` in PGN comment |
| `0x24` | Time Control | 38 bytes: `0x01` byte, 3 stages $\times 11$ bytes, `u32 LE` 0 | TimeControl PGN tag |
| `0x26` | Evaluation Curve | Position -1, `0x01` byte, `u32 LE` length, `u16 LE` count, array of 4-byte evaluations | `[%evp 0,count-1,v0,v1,...]` |

### 4.2 Square Numbering in Graphics (`0x04`, `0x05`)
Squares are numbered from 1 to 64 **file by file**:
$$\text{square\_id} = \text{file} \times 8 + \text{rank} + 1 \quad (\text{file, rank } \in [0, 7])$$
- `a1` = 1, `a2` = 2, ..., `a8` = 8.
- `b1` = 9, `b2` = 10, ..., `b8` = 16.
- `h1` = 57, ..., `h8` = 64.
Colors: 2 = Green, 3 = Yellow, 4 = Red.

---

## 5. Character Encodings and Private Use Area (PUA) Glyphs

### 5.1 Encodings
While 2CBH defaults to UTF-8 for all text fields, single-byte text occurs in databases originating from localized Windows environments:
- Western single-byte: Windows-1252.
- Cyrillic single-byte: Windows-1251 (identified by characteristic byte distributions).

### 5.2 Private Use Area (PUA) Glyph Translation Table
ChessBase proprietary font glyphs are stored in Unicode Private Use Area block `U+E000..U+E02F`. Clean PGN conversion maps these to standard Unicode equivalents:

| PUA Code Point | Standard Unicode | Symbol Name / Meaning |
|---|---|---|
| `U+E000` | `⯹` (`U+2BF9`) | With compensation |
| `U+E001` | `⮺` (`U+2BBA`) | Pair of bishops |
| `U+E002` | `⮻` (`U+2BBB`) | Bishops of opposite colors |
| `U+E004`, `U+E02E` | `⩱` (`U+2A71`) | White has slight advantage |
| `U+E005` | `""` (empty) | Diagram marker (omitted in text) |
| `U+E007`, `U+E02F` | `⩲` (`U+2A72`) | Black has slight advantage |
| `U+E008` | `⯽` (`U+2BFD`) | Passed pawn |
| `U+E009` | `∞` (`U+221E`) | Unclear position |
| `U+E00A` | `±` (`U+00B1`) | White has clear advantage |
| `U+E00C` | `⌓` (`U+25E4`) | Better is |
| `U+E00D` | `△` (`U+25B3`) | With the idea |
| `U+E00E` | `⟪` (`U+27FA`) | Queenside |
| `U+E00F` | `⟫` (`U+27FB`) | Kingside |
| `U+E010` | `×` (`U+00D7`) | Weak point |
| `U+E012` | `↑` (`U+2191`) | Initiative |
| `U+E013` | `→` (`U+2192`) | Attack |
| `U+E017` | `⇆` (`U+21C4`) | Counterplay |
| `U+E018` | `⇔` (`U+21D4`) | File |
| `U+E019` | `⇗` (`U+21D7`) | Diagonal |
| `U+E01A` | `∓` (`U+2213`) | Black has clear advantage |
| `U+E01D` | `⨀` (`U+2A00`) | Zugzwang |
| `U+E01E` | `⊞` (`U+229E`) | Center |
| `U+E01F` | `⊥` (`U+22A5`) | Endgame |
| `U+E021` | `□` (`U+25A1`) | Only move |
| `U+E023` | `○` (`U+25CB`) | Space |
| `U+E024` | `♔` (`U+2654`) | White King figurine |
| `U+E025` | `♕` (`U+2655`) | White Queen figurine |
| `U+E026` | `♖` (`U+2656`) | White Rook figurine |
| `U+E027` | `♗` (`U+2657`) | White Bishop figurine |
| `U+E028` | `♘` (`U+2658`) | White Knight figurine |
| `U+E029` | `♙` (`U+2659`) | White Pawn figurine |

---

## 6. Synthetic Reproduction Recipes

To test the implementation without any copyrighted or user database files, construct synthetic files as follows:

### 6.1 Analysis Record Recipe
1. Write 192 bytes into `.2cbh`:
   - Set `rec[2] = 2` (analysis).
   - Set `rec[0x08..0x10] = moves_offset`.
   - Set `rec[0x18..0x20] = title_tag_id`.
   - Set `rec[0x28..0x30] = author_player_id`.
   - Set `rec[0x4c..0x50] = (2024 << 9) | (11 << 5) | 5` (Date: 2024.11.05).
   - Set `rec[0x50..0x52] = 4` (move count = 4 plies).
2. Write `.2cbg` move frame:
   - Tag = `1`.
   - Moves payload = `[0xfffc, e2e4_word, e7e5_word, g1f3_word, b8c6_word, 0xffff]`.
3. Verification assert:
   - Facilitated read reports Event = Title, Date = `2024.11.05`, plies = 4.

### 6.2 Setup Castling Rights Recipe
1. Construct `.2cbg` move frame:
   - Tag = `1`.
   - Prefix words:
     - Word 0: `1` (Move 1).
     - Word 1: `(0x03 << 8) | 0x00` (White to move, rights: White O-O and White O-O-O).
     - Word 2: `0x0000`.
   - Setup pieces: standard 32 piece placement words.
   - Moves: `[0xfffc, e1g1_castle_word, 0xffff]`.
2. Verification assert:
   - Move `e1g1` replays legally as `O-O`.

---

## 7. Unknowns and Explicit Error Boundaries

1. **Unidentified Annotation Types:**
   - Type `0x27` (2-byte payload, usually 1): treated as opaque raw data without failing record parsing.
   - Types `0x08`, `0x16`, `0x17`, `0x22`, `0x23`, `0x25` with 4-byte payloads: parsed safely using length checks.
2. **Corrupted Checksums / Truncated Frames:**
   - Mismatched 8-slice BE checksum: typed error `Error::corrupt`.
   - Mismatched trailer length: typed error `Error::corrupt`.
   - Non-terminating variation stack: typed error `Error::corrupt`.
