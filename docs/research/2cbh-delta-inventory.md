# 2CBH Delta Inventory — Pinned MIT vs Later AGPL and Morphy Java

> **Clean-room Room 1 deliverable** — Task 3.1 of `openspec/changes/native-2cbh-readonly/`.
> Full source access restricted to Room 1 (researcher/specifier). Room 2 (clean-room implementer)
> is never exposed to this delta comparison or the underlying repositories.

## 1. Version and Source Identification

| Component | Pinned MIT Baseline | Comparison Reference 1 (AGPL-3.0) | Comparison Reference 2 (Unlicensed) |
|---|---|---|---|
| **Repository** | `asavis/oschess-cb-bridge` | `asavis/oschess-cb-bridge` | `Yarin78/morphy` |
| **Commit / Tag** | `v1.0.3` (`bffebfc498d52b4e4631aed32b2f8e8194d125ff`) | `main` (`ba58b3e5258903413bc60c7de59a7f8d6e136c81`) / `v1.2611.201` | `master` branch (`format/v2`) |
| **Licence** | MIT (verified at commit) | AGPL-3.0-only (relicensed at commit `51d4c97`) | None (all rights reserved) |
| **Role in cbvault** | Direct code port foundation (with attribution) | Facts & oracle reference only; zero copied code/text | Format reference only; zero copied Java code/text |

---

## 2. Inventory of Modified Files in Upstream `crates/cbformat`

A total of 70 files changed in `crates/cbformat` between `v1.0.3` and `ba58b3e5`. The table below catalogs the functional modules directly relevant to 2CBH container and format behavior:

| Module | Upstream File | Baseline State (`v1.0.3`) | Later State (`ba58b3e5`) | Behavioral Impact / Observable Delta |
|---|---|---|---|---|
| **Header Records** | `src/v2/record.rs` | Basic classification (game, text, analysis, deleted); date mapped from `0xbc`, moves from `0x8a` for all kinds. | Distinguishes analysis headers (`kind == 2`): reads date from `0x4c`, move count from `0x50`. Games retain `0xbc` and `0x8a`. | Analysis records previously exported with Date `????.??.??` and 0 moves now carry proper date and move counts in headers, PGN, and search indexes. |
| **Entities** | `src/v2/entities.rs` | Fixed 184-byte `.2lid` header assumed; strings parsed as UTF-8 with lossy fallback. Tournament type unread. | Dynamic header size from offset `0x00`; tournament type byte read after start date (offset `0x4a`-equivalent); codepage fallback for non-UTF8 strings. | Correctly parses databases with 216/228/236-byte `.2lid` headers; exposes blitz/rapid/corr flags; prevents corrupted Cyrillic names. |
| **Codepage & Signs** | `src/codepage.rs`, `src/signs.rs` | UTF-8 or Windows-1252 fallback. No PUA translation. | Cyrillic auto-detection (Windows-1251 vs 1252); UTF-8 runs inside single-byte text; Private Use Area (`U+E000..U+E02F`) mapped to Informator symbols & Unicode figurines. | Translates proprietary ChessBase symbols to standard Unicode; restores mis-encoded Ukrainian/Russian letters (`і`, `І`, `ю`, `ч`, `Ч`); decodes Chessable course brackets. |
| **Annotations** | `src/v2/annotations/**` | Basic parsing of text, symbols, squares, arrows. Incomplete timing/evals. | Detailed layout for Game Quotations (`0x13`), Evaluations (`0x26`), Clocks (`0x16`/`0x17`), Engine Eval (`0x21`), Time Spent (`0x07`), Time Control (`0x24`), Medals (`0x22`). | Emits standard PGN comments/tags for `[%evp]`, `[%mdl]`, and quotation text; avoids dropping unknown variable-length annotation payloads. |
| **Move Codec** | `src/movetable/**`, `src/v2/moves.rs` | Standard move table; setup castling assumed in word 2 (Morphy model). | Setup castling corrected to high byte of word 1; promotion capture word order verified as captured piece first, then promotion piece. | Correctly validates and replays setup games that castle; verifies legal captures on promotion plies. |
| **Guiding Texts** | `src/v2/guide.rs`, `src/cbh/guide.rs` | Unparsed opaque records. | Decodes 2CBH guiding text as HTML content from `.2cbg` at offset `0x08`; resolves game links and text links. | Exposes guiding text contents and link metadata without crash or corruption. |

---

## 3. Observable Input/Output Behavioral Deltas

### 3.1 2CBH Analysis Records (`kind == 2`)
- **Input:** 192-byte header with byte `rec[2] == 2`, pointing to move frame in `.2cbg` at offset `0x08`.
- **Observable Output in v1.0.3:**
  - Game row reports Date `????.??.??` and 0 moves.
  - PGN tag `[Date "????.??.??"]`.
- **Observable Output in Later AGPL:**
  - Date read from `rec[0x4c..0x50]` (`u32 LE`: `(year << 9) | (month << 5) | day`).
  - Move count read from `rec[0x50..0x52]` (`u16 LE`).
  - PGN tags include valid `[Date "YYYY.MM.DD"]` and `[Event "<Title>"]` (from `.2lid` GAME_TAG).

### 3.2 Start-Position Castling Rights
- **Input:** Start position marker `0xfffb`, followed by 3-word prefix before piece array.
- **Morphy Java hypothesis:** Word 0 = move number, Word 1 = side to move (low byte) + en passant file (high byte), Word 2 = castling rights.
- **Observed reality (Later AGPL & byte probes):**
  - Word 0 = move number.
  - Word 1 = low byte (side to move: 0=White, 1=Black), **high byte = castling rights** (bits: 1=White O-O-O, 2=White O-O, 4=Black O-O-O, 8=Black O-O).
  - Word 2 = zero in 2,398 of 2,398 sampled setup positions (unconfirmed en passant file).

### 3.3 Private Use Area (PUA) Font Glyphs
- **Input:** Comments and names holding code points in range `U+E000` through `U+E02F`.
- **Observable Output in v1.0.3:**
  - Unmapped private use characters displayed as unrenderable boxes or raw escape codes.
- **Observable Output in Later AGPL:**
  - Figurine range `U+E024..U+E029` mapped to standard Unicode chess pieces: `♔`, `♕`, `♖`, `♗`, `♘`, `♙`.
  - Informator symbols mapped to standard mathematical/editorial Unicode (e.g. `U+E00A` -> `±`, `U+E01A` -> `∓`, `U+E009` -> `∞`, `U+E013` -> `→`).
  - `U+E005` diagram marker dropped in plain-text output.
  - Cyrillic letter repair: Windows-1251 bytes mis-read as Western font signs restored to `і`, `І`, `ю`, `ч`, `Ч`.

### 3.4 Annotation Block Structure
- **Input:** `.2cba` records with tag `0x2000`.
- **Observable Deltas:**
  - Type `0x13` (Game quotation): Parsed into structured metadata and serialized into ChessBase standard PGN quotation text format `Result White - Black (Elo) Event (Round)`.
  - Type `0x26` (Evaluation curve): Parsed into count + array of scores; serialized into PGN comment `[%evp 0,count-1,...]`.
  - Type `0x22` (Medals): Serialized as `[%mdl <bits>]`.
  - Type `0x24` (Time control): 38-byte layout (3 stages of 11 bytes + 4 bytes trailing zeros).

---

## 4. Known Unsupported Cases and Boundaries

1. **Classic .cba Layouts for Modern Annotation Types:**
   - Computer evaluations (`0x21`), time spent (`0x07`), and time controls (`0x24`) have verified 2CBH layouts, but paired classic `.cba` encodings remain undecoded.
2. **Setup Game Quotations with Custom Boards:**
   - Type `0x13` quotations with setup positions have an 11-byte unparsed section between piece array and moves; moves for setup quotations remain raw data.
3. **Annotation Types 0x27 and Rare Variants:**
   - Type `0x27` (2-byte payload, usually value 1) remains unknown in semantic meaning.
   - Squares/arrows with color codes 7, 8, 9 remain unknown.
4. **Guiding Text Formatting (Classic v1/v2):**
   - Classic embedded diagram binary blocks and font tables remain partially specified; 2CBH uses clean HTML stored directly in `.2cbg`.

---

## 5. Room 1 Clean-Room Isolation Certificate

- **Analyst:** Room 1 Specifier Agent.
- **Source Repositories Consulted:** `vendor/upstream-snapshot` (git log and diff across commits `bffebfc4..ba58b3e5`), public Morphy Java repository documentation (`format/v2`).
- **Hygiene Rule:** No lines of code, test source, comments, or unique expressions from later AGPL or Morphy were copied into the workspace. All observations above are recorded strictly as factual byte offsets, data types, value mappings, and algorithmic contracts.
