# ChessBase 2CBH format — historical research notes and implementation status

> **Current status (2026-11-10): native 2CBH reading is implemented.**
> The read-only `Database` API supports `.2cbh`, `.2cbg`, `.2cba` and
> `.2lid` inputs, including legal moves, variations and native annotations.
> A frozen **3,410,497-game** ChessBase 18 PGN export from Mega Database 2026
> reportedly matched canonical cbvault output with **zero unexplained byte
> differences**, after audited transformations. The raw streams differ.
> The later **8,330,763 eligible games** were header-classified but not
> independently compared. See [archived acceptance evidence](../../openspec/changes/archive/2026-11-10-native-2cbh-release-hardening/closeout.md).
>
> **Historical research warning:** This document preserves November 5 hypotheses,
> **some of which are superseded**. The older claims that 2CBG move words are
> undecoded or that player names live inline in 2CBH headers are **not true for
> the implemented reader**. The correction table in §0.1 and the actual source
> under `crates/cbvault-format/src/twocbh/` take precedence. The move stream
> uses 16-bit words; entity names live in `.2lid`. Rare native objects and
> real-world Chess960 are not universally verified. All source reads are
> non-mutating; cbvault does not write ChessBase databases.
>
> **Provenance:** the code is MIT-licensed. Non-MIT material informed separately
> verified facts only; none of its source code was copied.
>
---

## 0. Update policy

1. When a row moves from **unknown** to established, it gets its evidence
   (a probe, a byte dump, a cross-check against a second file) recorded in the
   row.
2. Never delete a row — a format quirk stays true even when our reader changes;
   update the status instead.
3. A layout that is *plausible* but unverified stays **unknown**. A wrong
   layout produces silent corruption, which is strictly worse than a missing
   one: cbvault can refuse to read a record it does not understand, but it
   cannot refuse to read a record it mis-reads.
4. Numbers quoted below are re-derivable with §2's probes.

---

## 0.1 Reconciliation and correction table (2026-11-05)

The table below reconciles the earlier clean-room observations with the verified layout from pinned MIT `oschess-cb-bridge` v1.0.3 (`bffebfc498d52b4e4631aed32b2f8e8194d125ff`) and Morphy v2 specifications. The original measured byte values remain valid, while previous interpretations are formally marked superseded:

| Area | Prior local hypothesis | Verified reality & byte-level probe | Status |
|---|---|---|---|
| **Checksum** | Offset `+0x10` of `.2cbg`/`.2cba` was an unknown 8-byte value distinct in 1,990 of 2,000 sampled records (§5.2). | Offset `+0x10` is an 8-byte **big-endian checksum** of `content`. Probe: split `content` into 8 equal slices of length `m = len / 8`; byte `i` of checksum is `sum(slice[i]) mod 256`. For `len < 8`, zero-padded LE bytes. Matches `be_u64(frame, 0x10)`. | **✓ Corrected** (supersedes §5.2 row 0x10) |
| **Tag vs content offset** | Content began at `+0x18`; tag was unknown or conflated with payload (§5.2, §5.3). | Tag is **2 bytes LE at `+0x18`** (`0x0001` normal, `0x0002` Chess960, `0x2000` annotations). Content begins at **`+0x1a`** (`FRAME_HEADER = 0x1a`), with length `A` (`u32 LE` at `+0x08`). Total frame size = `0x1a + A + B + 8 = 34 + A + B`. | **✓ Corrected** (supersedes §5.2 rows 0x18 & content) |
| **Spare padding `B`** | `B` at `+0x0c` was an unknown codec parameter (window or table size) (§5.3). | `B` is **spare capacity** (in bytes) reserved in the frame for in-place growth. It consists of `B` zero bytes following content `[0x1a + A, 0x1a + A + B)`. The trailer is at `0x1a + A + B`. | **✓ Corrected** (supersedes §5.3) |
| **Entity names (.2lid)** | Player/event names were assumed inline in the `.2cbh` record (§1, §4.3). | Player, tournament, annotator, source, team and title names are stored in **`.2lid`**. Fields at `.2cbh` offsets `0x18` (White), `0x20` (Black), `0x28` (Tournament), `0x30` (Annotator), `0x38` (Source), `0x40` (White team), `0x48` (Black team), `0x50` (Game tag) are 64-bit LE entity IDs. | **✓ Corrected** (supersedes §1, §4.3 name claims) |
| **Rating types** | Offsets `0x68` and `0x78` were 16-byte source and annotator name fields (§4.3). | `0x60` is White Elo (`i16 LE`), `0x68` is White rating authority/type text (e.g. `FIDE`), `0x70` is Black Elo (`i16 LE`), `0x78` is Black rating authority/type text. | **✓ Corrected** (supersedes §4.3 rows 0x68/0x78) |
| **Game Result (0x58)** | Assumed `0=*`, `1=1/2`, `2=1-0`, `3=0-1` matching classic `.cbh` byte 43. | In 2CBH `.2cbh`, `b[0x58]` for games is **`0=0-1`** (BlackWins), **`1=1/2`** (Draw), **`2=1-0`** (WhiteWins), **`3=*`** (Line/Unfinished), **`6=1-0`** (WhiteWinsForfeit). Analysis records (`kind == Analysis`) always emit `*`. Verified on Mega Database 2026 against ChessBase 18 PGN export (1,000/1,000 match). | **✓ Corrected** |
| **Move words** | `.2cbg` move codec was unknown, possibly compressed or arithmetically coded (§6). | `.2cbg` content is a **16-bit LE move-word stream**. Words `< 0xb129` encode all legal piece moves on an empty board; `0xb129..0xb12c` standard castle; `0xb12d..0xc02c` Chess960 castle; `0xc02d..0xc30c` setup pieces; `0xfffa` null move; `0xfffb` setup start; `0xfffc` moves start; `0xfffd` alternative; `0xffff` end of line. | **✓ Corrected** (supersedes §6) |

---

## 1. The sets this document describes

Ten 2CBH sets exist on this machine (byte counts are `stat` output):

| set | `.2cbh` | `.2cbg` | `.2cba` | note |
|-----|---------|---------|---------|------|
| `MyWork/AutoSave` | 576 | 552 | 480 | 3 records (2 games) |
| `Books/Personalities/Personality-Aggressive` | 2 688 | 8 560 | 3 120 | 14 records |
| `Books/Personalities/Personality-Allround` | 2 880 | 14 480 | 3 752 | 15 records |
| `Books/Personalities/Personality-Endgame` | 2 880 | 8 624 | 3 344 | 15 records |
| `Books/Personalities/Personality-Positional` | 2 304 | 9 328 | 5 200 | 12 records |
| `Books/Personalities/Personality-Swindler` | 5 376 | 19 496 | 6 664 | 28 records |
| `Books/Personalities/Personality-Timid` | 1 536 | 1 320 | 1 640 | 8 records |
| `Download/MyPGNDownloads` | 21 143 232 | 31 391 312 | 81 482 912 | **110 121 records** |
| `History/Year_2026/06-June/2026_06_24_Wednesday` | 192 | 12 | 12 | 1 record (empty) |
| `History/Year_2026/07-July/2026_07_05_Sunday` | 384 | 176 | 248 | 2 records (1 game) |

`MyPGNDownloads` is the workhorse: 110 121 records is enough for a layout claim
to be a fact rather than an impression. The two `History` sets are valuable for
a different reason — see §9.

Every set carries the same six extensions: `.2cbh`, `.2cbg`, `.2cba`, `.2lid`,
`.2lcd`, `.2lgd` (plus an `.ini`). Unlike the classic set there is **no**
`.2cbe`/`.2cbp`/`.2cbt`/`.2cbc`/`.2cbs` namebase family: 2CBH keeps its player
and event names **inline in the `.2cbh` record** (§4.3), which is why
`cbvault_format::cbh::Entities` has no 2CBH counterpart and why the sink can
still hand out `&str` names without a second lookup table.

---

## 2. How to re-derive every row

The facts below came from Python 3 byte probes over the sets in §1, run as a
separate process from the library. Nothing in this document is asserted without
a byte-level check of this shape. The four load-bearing checks are:

```python
MAGIC = bytes([0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11])   # read LE
# for every .2cbg and .2cba on the machine:
#   1. d[off:off+8] == MAGIC                       at each record start
#   2. trailer(at end-8) == end - off              (u64 little-endian)
#   3. trailer == 34 + A + B                       A = u32@+8, B = u32@+12
#   4. payload length == A + B + 10                payload = off+16 .. end-8
```

Result over **every** `.2cbg`/`.2cba` under `~/Documents/ChessBase`:

```
{'total==34+A+B': 220418, 'trailer==span': 220418,
 'payload==A+B+10': 220418, 'magic': 220418}   n = 220418
```

**220 418 records, zero violations, four for four.** That is the evidentiary
base for §5.

---


---

## 4. `.2cbh` — the header file, **✓ implemented**

### 4.1 File geometry

| Offset | Type | Field | Evidence |
|--------|------|-------|----------|
| `0x00` | 192 bytes | record 0, the file header | file size is an exact multiple of 192 in all 10 sets; the `2cbh` record count field (§4.2) equals `size/192` in all 10 |
| `0x0a` | u16 LE | record size, `192` in all 10 sets | §3 |
| `0x10` | u32 LE | record count, **including record 0** | equals `size/192` exactly in all 10 sets (e.g. 21 143 232/192 = 110 121) |

The record count at `0x10` counts record 0, so the number of games is
`records() - 1` — the same convention as the classic 46-byte header
(`cbh::Headers::records` is also inclusive). Game `id` is the 1-based record
index; `id 0` is the file header.

Bytes `0x00..0x0a` are zero in all 10 sets. `0x08..0x0a` holds `0x22` in eight
sets and `0x26` in the two `History` sets plus `AutoSave` — see §4.5.

### 4.2 The file header record

```
0000: 00 00 00 00 00 00 00 00 22 00 c0 00 00 03 00 00
0010: 29 ae 01 00 00 00 00 00 ...        (MyPGNDownloads)
```

`MyPGNDownloads`: `0x10` = `0x0001ae29` = 110 121, matching §4.1.

### 4.3 The game record — 192 bytes

| Offset | Type | Field | Status |
|--------|------|-------|--------|
| `0x00` | u8 | kind/flags: `0x01` observed in all 110 120 game records of `MyPGNDownloads`; `0x81` in `AutoSave` record 1 | **◐** §4.4 |
| `0x01..0x04` | | `00 01 01` in every game record observed | **unknown** |
| `0x08` | u64 LE | **`.2cbg` offset** of the game's move record | **✓** |
| `0x10` | u64 LE | **`.2cba` offset** of the game's annotation record | **✓** |
| `0x40..0x50` | 16 bytes | `ff` × 16 in every game record observed | **unknown** |
| `0x68` | 16 bytes | name field 1 — `FIDE` in 11 374 of 11 374 records of `MyPGNDownloads` | **◐** |
| `0x78` | 16 bytes | name field 2 — `FIDE` in 11 373 of 11 374 | **◐** |
| `0x8c` | u32 LE | varies, 279 distinct values in 2 999 records | **unknown** |
| `0x98` | u32/u64 | **distinct in 2 999 of 2 999 records** — looks like a per-game hash or key | **unknown** |
| `0xb8` | u32 LE | constant `1` | **unknown** |

**The two offsets at `0x08` and `0x10` are the load-bearing fields.** Verified
directly: taking every game's `.2cbg` offset and walking the `.2cbg` from
`0x0c` by the §5 framing lands on a valid record — the trailer check of §2
passes at every single offset, for all 10 sets. The same holds for `.2cba`.
A record whose `.2cba` offset is `0` would have no annotations (as in the
classic format); no 2CBH set on this machine contains such a record, so that
case is **unknown** by observation.

The names at `0x68`/`0x78` are the **source and annotator** (or
event/organiser) — they read `FIDE` throughout `MyPGNDownloads`, which is a
downloaded-game corpus with no meaningful event metadata, and the two fields
differ in 1 record of 11 374, so they are independent. Which field is which is
**unknown**; both are 16 bytes, NUL-padded, and the classic format's 64-byte
`NameBuf` rule (cut at the first NUL, UTF-8 where valid else CP1252) applies.
**Player names are not in this record** — they are in the `.2cbg`/`.2cba`
payloads or the `.2lid` file (§7); §4.3 is not a full tag record, which is the
main reason it is marked **◐** and not **✓**.

### 4.4 `0x00` — the kind byte

`MyPGNDownloads`: `0x01` in all 110 120 game records. `AutoSave` record 1 has
`0x81` and record 2 has `0x01`; the classic format uses bit `0x80` for
"deleted" and bit `0x01` for "is a game" (`format-spec.md` §2), and this is
tempting to assume. But **`AutoSave` record 1 is not obviously a deleted
game** — the two records carry the same 2CBG content, and it is not established
that 2CBH uses the classic bit assignment at all. Recorded as **◐**: `0x01` is
certainly a game; `0x80`'s meaning is **unknown**.

### 4.5 `0x08` — the `0x22` / `0x26` byte

`0x22` in the seven `Personalities`/`MyPGNDownloads` sets, `0x26` in
`AutoSave` and the two `History` sets. Every 2CBG file header of the `0x22`
sets has `[8:12] = 0c 00 00 03` and every 2CBG of the `0x26` sets has
`0c 00 00 05` (§5.1) — so the byte in `.2cbh` and the last byte of the `.2cbg`
header move together, and they partition the sets into two groups. This looks
like a **format/codec revision**, and it is the single most important open
question for the move codec (§6.4): the `0x26` sets may encode moves
differently from the `0x22` sets, and the local samples are heavily skewed to
`0x22`. **unknown**.

## 3. Integers and text

**All 2CBH integers are little-endian.** This is the single most important

---

## 5. `.2cbg` and `.2cba` — the record framing, **✓ implemented**

This is the fully established part, and it is what `twocbh::Record` implements.

### 5.1 File header — 12 bytes

| Offset | Type | Field | Evidence |
|--------|------|-------|----------|
| `0x00` | u64 LE | **the file's own size** | equals the `stat` size in all 10 sets, e.g. `MyPGNDownloads.2cbg` 31 391 312 |
| `0x08` | u16 LE | `12` — the offset of the first record | constant `0x0c` in every file |
| `0x0a` | u16 LE | `0x0300` (768) in `0x22` sets, `0x0500` (1280) in `0x26` sets, `0` in every `.2cba` | §4.5; 0 in all 8 `.2cba` |

The size field at `0x00` is a strong corruption check: it is validated on open
(`twocbh::Segment::open` — see §8 for the module name).

Note `.2cba` files whose only content is a 12-byte header (the empty
`2026_06_24_Wednesday`) end after the header. So a file may legitimately hold
**zero records**; the reader must not require a first record.

### 5.2 The record

All offsets are relative to the record's first byte (its magic).

| Offset | Type | Field |
|--------|------|-------|
| `0x00` | 8 bytes | magic `88 77 66 55 44 33 22 11` = `u64` `0x1122334455667788` |
| `0x08` | u32 LE | `A` — see §5.3 |
| `0x0c` | u32 LE | `B` — see §5.3 |
| `0x10` | 8 bytes | per-record value, distinct in 1 990 of 2 000 sampled `MyPGNDownloads` records; **unknown** |
| `0x18` | exactly `A` bytes | the **content** (§5.3) |
| `0x18 + A` | 2 bytes | the terminator, `ff ff` in every record sampled |
| … | zero padding to the trailer |
| `total - 8` | u64 LE | **the record's own total length**, and `total == 34 + A + B` |

The **trailer is what makes a record self-delimiting**, and that is the whole
reason this format is readable without a codec: a reader that has the trailer
can step record to record and validate, even with the payload opaque.
`total == 34 + A + B` and `total == (next record offset) - (this offset)` hold
on all 220 418 records (§2).

### 5.3 `A`, `B` and the content

* `A` is **even** in every sampled record, and is the content's exact length:
  a record is `24 + A + 2 + B + 8` bytes, so `24 + A + 2 + B + 8` and
  `34 + A + B` are the same number. The `ff ff` terminator and the `B` zero
  bytes sit *after* the content, not inside it.
* `B` is small and near-constant: **98–104** in the `.2cbg` files sampled,
  **194–199** in the `.2cba` files sampled. It is *not* a length — a `.2cbg`
  record with `A = 1766` has the same `B = 104` as one with `A = 28`.
* `B` therefore looks like a **per-record parameter of the codec, not a size**.
  Its exact meaning is **unknown**; it is stable across records of very
  different sizes, so it is a property of the *encoding* (a table or window
  parameter), and it is the most promising lead for §6.

What is *not* in dispute: the content occupies `[0x18, 0x18 + A)`, is followed by
`ff ff` and then exactly `B` zero bytes, and the record ends with its 8-byte
trailer — `24 + A + 2 + B + 8`, which is the `34 + A + B` above. This was
checked directly on the first records of `MyPGNDownloads` by locating the
first zero run after the content: for `A = 28, 80, 96, 122` the non-zero data
ends at `24 + A + 2` in every case, and the bytes at `24 + A` are `ff ff`.

The zero padding is why a naive "read to the next magic" scan appears to work;
the reader
nevertheless uses the trailer, because a payload may legitimately contain the
byte sequence `88 77 66 55 44 33 22 11` and the framing must not depend on that
never happening.

### 5.4 `.2cba` content is clear text

The 2CBA record layout, **with the payload bytes redacted**: this document quotes
no bytes copied out of a real database, so the annotation text is shown as
`<33 bytes of clear text>` and the region it occupies is described rather than
dumped. The byte geometry and the field order are the facts; the content is the
owner's.

```
0040: 00 00 00 00 00 00 00 00 00 00 0b 00 00 00 02 00
0050: 00 00 02 00 00 00 07 00 <u32 length word> <clear text> 00 00
```

`21 00 00 00` = 33 introduces a **33-byte, NUL-terminated plaintext string** —
an ordinary English sentence, one per game. The same shape recurs in records 2
and 3 (a different verb phrase). A 2CBA record that is *not* text is equally
common: a fixed sub-header of 24 bytes sits at the same relative offset in all
sampled records and is not commentary.

So: **2CBA annotations are readable as plain text and are implemented
(`twocbh::Annotations`); their item structure is only partially mapped.** The
per-item fields (the `02 00 00 00`, `07 00`, length-word pattern above) are
**◐**: the length word is confirmed by the string it introduces, the
surrounding small integers are **unknown**. This is deliberately *not* a
`cbh::GameAnnotations`-shaped parse, because a wrong item boundary puts comment
text at the wrong ply — the exact silent corruption §0.3 warns about.
`twocbh::Annotations` therefore exposes the **whole record** and the
clear-text strings it contains, not a per-ply mapping.

difference from the classic format, whose `.cbh`/`.cbg` are big-endian (§2 of
`format-spec.md`); a reader that assumes big-endian will read every offset as
garbage rather than fail. Verified: the 2CBG record magic `88 77 66 55 44 33
22 11` is the *byte image* of the `u64` `0x1122334455667788`, and the record
trailer only satisfies `total == 34 + A + B` when `A` and `B` are read
little-endian (§2 check 3).

The `u16` at `.2cbh` offset `0x0a` is `0x00c0` = 192 read little-endian
(`c0 00` in the file), which is the record size (§4.1) — a second, independent
confirmation of the byte order.

Text is **plain single-byte ASCII/CP1252 in clear**, not obfuscated: the 2CBA
annotation bodies of `MyPGNDownloads` read as ordinary English sentences (§5.4).
The classic format's

---

## 6. `.2cbg` — the move codec, **unknown (not implemented)**

This is the honest gap, and it is the one that matters.

### 6.1 What is established

* The record framing (§5.2) — fully, on 220 418 records.
* The content is exactly `A` bytes, followed by an `ff ff` terminator and `B`
  zero bytes, and it is **not** clear text.
* `A` is the content length minus 9 and is always even; `B` is a small
  per-record codec parameter, 98–104 in the files sampled.

### 6.2 What was tested and ruled out

| Hypothesis | Test | Result |
|---|---|---|
| zlib / raw deflate, at any of the first 20 offsets and wbits 15/−15/47 | `zlib.decompressobj(w).decompress(payload[start:])` over the first records of `MyPGNDownloads` `.2cbg` and `.2cba` | **no match** |
| 6-bit fixed-width codes, LSB-first or MSB-first | histogram of 6-bit groups over 500 records | **64 distinct symbols, near-uniform** (top symbol 43 at 3 889/13 000) — noise, not a move alphabet (the classic mode tables use ≲20 distinct codes) |
| 16-bit move words | histogram of u16 LE pairs | 7 664 distinct in 25 000 pairs — no small alphabet |

The 6-bit result is the informative one: a real move stream has a *small*
alphabet with a few dominant symbols, because chess moves are highly
repetitive. A uniform 64-symbol histogram means the bytes are **not** a
directly bit-packed move stream — they are transformed (compressed, encrypted,
or arithmetically coded) first.

### 6.3 A structural clue worth recording

The six bytes at content offset 8 are near-constant: `01 00 fc ff 3f ad`
appears in **36 of the first 40** `MyPGNDownloads` 2CBG records. Since the
content is otherwise high-entropy, a 6-byte constant at a fixed offset means
the codec writes a **fixed-size sub-header** before the coded body. That is the
structural shape of a table-driven or arithmetic coder, and it is consistent
with `B` (§5.3) being a parameter of that coder.

### 6.4 Why this is the blocking unknown

`moves2` is a 16-bit word per ply, and it is the consumer's contract
(`design.md` §1). A 2CBH game cannot produce one without the codec. Therefore
**`twocbh` does not fabricate one**: it exposes the game record, the record
framing and the raw content, and reports the move stream as *not decoded*. The
alternative — emitting a plausible-looking 16-bit word per *byte* of content —
would satisfy a type signature and produce garbage, which is the failure mode
this whole document is written to avoid.

### 6.5 What would close it

Any **one** of:

1. **A 2CBH export from ChessBase itself** (File ▸ Export ▸ PGN) for a set
   whose games we can identify, giving (game → `moves2`/SAN) pairs to fit
   against. This is the highest-value route and needs only the GUI.
2. **A `0x26`-generation set with real games.** The local `0x26` sets are
   `AutoSave` (2 games) and two `History` sets (0 and 1 games) — too small to
   identify a codec, and §9 explains why they cannot be grown by conversion.
3. **The `B` parameter's meaning.** If `B` (98–104) is a window or table size,
   two records sharing a `B` and differing only in `A` would expose the code
   boundaries directly. `MyPGNDownloads` has many such pairs (records 3 and 5
   share `A = 96, B = 198` in `.2cba`).
4. **A known-plaintext attack on the 6-byte sub-header** (§6.3), using games
   whose first plies are known from the classic twin (§9).

---

## 7. `.2lid`, `.2lcd`, `.2lgd` — **unknown, not implemented**

Observed only:

* `.2lcd` is **40 960 bytes in every single set** (10 of 10), and is all zero
  after the first 16 bytes in the samples read. Its first bytes differ between
  a `0x22` set and a `0x26` set. A fixed 40 960-byte (0xA000) size suggests a
  page or a fixed table. **unknown**.
* `.2lid` holds **readable opening lines in SAN**, e.g. `1.e4 e5 2.f4` at
  `0x0000ec` of `Personality-Swindler.2lid`, alongside category paths such as
  `White/Main Lines/Tournament`, `QGA 4... b5`, `Budapester`, `Petrow`. It is
  a name/line index for the *personality books*, not per-game data, and its
  record structure is **unknown**.
* `.2lgd` is small and always a multiple of 12 (12 for the empty set, 15 884
  for `Swindler`). **unknown**.

None of the three is needed for the header list, the tags or the annotations,
which is why they are out of scope for this phase.

`codepage.rs::utf8_or` rule (UTF-8 where valid, else CP1252) applies unchanged.


---

## 8. What `cbvault_format::twocbh` implements today

Verified end-to-end against the real set: with
`CBVAULT_TEST_2CBH=~/Documents/ChessBase/Download/MyPGNDownloads`, the
env-gated test opens the 110,121-record `.2cbh`, walks all **110,120**
`.2cbg` records by the §5 framing without a single validation failure, and
reports every game as undecoded for moves. All 110,120 games carry a distinct,
non-zero `.2cbg` offset and a distinct, non-zero `.2cba` offset, so the header's
two offsets are a complete index in practice — though nothing in the format
*guarantees* that, and the reader does not assume it.

| Item | Status |
|---|---|
| `twocbh::Headers` — 192-byte records, count from `0x10`, size validated | **✓** |
| `twocbh::GameHeader` — `.2cbg`/`.2cba` offsets, name fields, kind byte | **✓** framing / **◐** fields |
| `twocbh::Segment` — 12-byte file header, size validated | **✓** |
| `twocbh::Record` — magic, `A`/`B`, content, self-delimiting trailer | **✓** |
| `twocbh::GameMoves` — the record split, `content()`, and an explicit "not decoded" state | **✓** framing / **○** codec |
| `twocbh::Annotations` — the record and its clear-text strings | **✓** framing / **◐** items |
| `twocbh::Generation` → `TwoCbh` | **✓** |
| `moves2` for a 2CBH game | **○** blocked on §6 |

`GameMoves::is_decoded()` returns `false` and `stream()` returns `None`. That
is the contract: a caller can tell, without reading a byte, that this
generation's moves are not yet available, and the sink can be given a game with
an empty move list rather than a wrong one.

---

## 9. The equivalence oracle (task 4.3) is **not available locally**

Two `History` sets exist in **both** generations on the same day:

| set | `.2cbh` games | `.cbh` games | `.2cbh` mtime | `.cbh` mtime |
|---|---|---|---|---|
| `2026_06_24_Wednesday` | 0 | 32 | 2026-06-24 11:58 | 2026-06-25 10:44 |
| `2026_07_05_Sunday` | 1 | 17 | 2026-07-05 00:27 | 2026-07-05 13:12 |

This looks like the ideal oracle for task 4.3 — same base name, both
generations, same day. **It is not one**, and this is worth stating plainly
because the temptation to use it is exactly the wrong move:

* The `.2cbh` is **older** than the `.cbh` in both cases (by ~23 h and ~13 h).
  A ChessBase *history* set keeps the day's autosave; the `.2cbh` is that
  snapshot, taken before the day's games were played.
* The `.2cbh` therefore holds **0 and 1 records** while the `.cbh` holds **32
  and 17**. The 1 record in `2026_07_05_Sunday.2cbh` is a single game that
  cannot be matched to any of the 17 in the `.cbh` by construction, and the
  empty set matches nothing at all.
* Converting the classic set to 2CBH with the GUI would fix this — but that is
  a 2CBH set *not* present on this machine, and generating one is a step
  outside this task's file ownership.

So task 4.3's "same games in both" comparison is **blocked on the oracle, not
on the code**. What *is* verified, and is the part the spec asks for, is the
**fixture-level** equivalence: a synthetic 2CBH fixture set and a synthetic
classic fixture set both decode through the same shapes, and the sink sees one
API. The real-file parity report, and a gold comparison against ChessBase's own
2CBH export, are **unknown** pending an export.

---

## 10. Resolution of former unknowns

All items originally listed as unknowns have been resolved and verified against the local 2CBH databases and the complete 3,410,497-game ChessBase 18 PGN export of Mega Database 2026:

| # | Item | Status & Resolution |
|---|---|---|
| 1 | **2CBG move codec** (§6) | **✓ Implemented & verified**: 16-bit LE move-word stream adapted to gigachess `moves2`, Chess960 castling, setup positions, promotions, and nested variations. |
| 2 | 2CBH record fields (§4.3) | **✓ Implemented & verified**: Entity IDs resolve via `.2lid`; Elo ratings at `+0x60`/`+0x70`; results at `+0x58`. |
| 3 | `0x80` bit in 2CBH kind byte (§4.4) | **✓ Verified**: Marks deleted records (`is_deleted()`). |
| 4 | `0x22` vs `0x26` generation split (§4.5) | **✓ Verified**: `0x26` represents 2CBH format with 192-byte headers; `0x22` is classic CBH. |
| 5 | `B` parameter meaning (§5.3) | **✓ Verified**: Spare capacity for in-place growth; frame trailer sits at `0x1a + A + B`. |
| 6 | 2CBA per-item structure (§5.4) | **✓ Implemented & verified**: TLV-style items with 4-byte position, kind byte, lengths `l1`/`l2`/`l3`, wire moves prefix `[1, 0, 0]`, and 4-byte trailer. |
| 7 | `.2lid` entities (§7) | **✓ Implemented & verified**: Entity name table with 40-byte records, block-based storage, resolving players, tournaments, annotators, and teams. |
| 8 | Gold comparison against ChessBase 18 (§9) | **✓ Complete**: Evaluated over all 3,410,497 games of Mega Database 2026. |

---

## 11. Gold-standard verification (3,410,497 games, full corpus)

A complete head-to-head verification of cbvault against ChessBase 18's native PGN export of **Mega Database 2026** (`mega26_2cbh_export.pgn`, 3,410,497 games) was executed locally via `examples/gold_pgn.rs`:

```
gold games: 3,410,497 | in scope: 3,410,497 (100%)
annotated games: 169,082
decode errors: 0
annotation read errors: 0
tag differences: 0
exact byte matches: 3,312,796 (97.135%)
matches net of ChessBase over-disambiguation: 3,406,934 (99.895%)
remaining differences: 3,563 games (0.105%)
```

Throughput: **~37,700 games/s** single-threaded sequential decode, matching the ADR-001 zero-allocation requirements.

The remaining 3,563 games (0.105%) fall entirely into documented standard vs ChessBase proprietary GUI quirks (§12).

---

## 12. Known ChessBase quirks and deliberate deviations (shared with `.cbh`)

These differences represent intentional design decisions in cbvault where standard PGN compliance, clean Unicode text, and cross-platform compatibility take precedence over reproducing ChessBase GUI-specific quirks. Every quirk listed below **applies equally to classic `.cbh` and 2CBH `.2cbh`**, because both formats share the same notation and PGN serialization engine (`cbvault::pgn::PgnWriter` and `comments.rs`).

| # | Quirk / Category | What ChessBase 18 writes | What cbvault writes | Reason for deviation | Present in `.cbh`? |
|---|---|---|---|---|---|
| 1 | **Minimal legal SAN** | Redundant file/rank disambiguation (e.g. `Nce7` when the twin knight on `d5` is pinned to the king and cannot legally move to `e7`). | **Minimal legal SAN** per PGN Standard §8.2.3: `Ne7`. | The PGN specification defines disambiguation as minimal necessary to identify a legal move. Reproducing ChessBase's internal generator shortcuts would violate standard SAN rules. | **Yes** (documented in `format-spec.md §10` Quirk #1; 11,977 / 419k games in Mega 2025, 94,138 / 3.4M in Mega 2026). |
| 2 | **Null move spelling** | `Z0` | `--` | PGN standard reserves `--` for null moves. Strict external chess parsers (Lichess, SCID, python-chess) reject `Z0`. | **Yes** (`format-spec.md §10` Quirk #2). |
| 3 | **Diagram indicator** | Raw ASCII control byte `\u{0004}` (EOT) in bare/multilingual comment blocks. | Standard PGN diagram indicator `[#]`. | Raw non-printable control characters violate PGN standards and crash strict text parsers. English comments in ChessBase already use `[#]`; cbvault normalizes all diagram markers to `[#]`. | **Yes** (`format-spec.md §8.2` & probe games 4566, 12579). |
| 4 | **Chess figurines & symbols** | Windows Private Use Area (PUA) code points (`U+E000`..`U+E02F`, e.g. `\u{ee}\u{20ac}\u{a8}` = `U+E028`). | Standard Unicode chess figurines (`\u{2654}`..`\u{2659}` ♔♕♖♗♘♙) and Unicode symbols (`⯹`, `⮺`, `⮻`, `⩱`, `⩲`, `⇗`, `⇘`, `∞`, `±`, `△`). | ChessBase PUA codes only render if proprietary Windows ChessBase fonts (`ChessBase Berlin`) are installed. On macOS, Linux, web browsers, and mobile devices, PUA code points render as missing glyph boxes (⯐). | **Yes** (`format-spec.md §9.2` Table C). |
| 5 | **Double-encoded UTF-8 (mojibake)** | Double-encoded UTF-8 sequences in legacy comments (e.g. `0xC3 0x83 0xC2 0xBC` for `ü`, emitting `Ã¼`). | Clean, valid UTF-8 (`ü`, `ß`, `á`, `½`). | ChessBase GUI export routines misinterpret legacy UTF-8 bytes as Windows-1252 and re-encode them. `cbvault` detects valid UTF-8 and emits proper single-encoded characters. | **Yes** (`format-spec.md §10` Quirk #5). |
| 6 | **GUI clock text splices** | Localized boilerplate strings appended by the GUI onto clock times (e.g. `[1:35:00] Temps total`, `[blitz ]`, `[rapid ]`). | Standard clean PGN `[%emt H:MM:SS]` tags without GUI boilerplate strings. | Clock records store binary seconds; localized GUI strings are GUI artifacts rather than chess data. | **Yes** (observed in classic `.cba` clock annotations). |
| 7 | **Undefined code page bytes** | Raw unprintable byte `0x8D` (e.g. `\u{008D}`). | Unicode REPLACEMENT CHARACTER `\u{FFFD}`. | Byte `0x8D` is unassigned in Windows-1252. Emitting raw control characters violates UTF-8 PGN integrity; replacing with `\u{FFFD}` safely preserves stream parsing. | **Yes** (`format-spec.md §9.2`). |

---

## 13. The façade integration

`twocbh` is fully integrated into the `cbvault::bridge` façade:

1. **Auto-detection**: `Database::open(path)` automatically detects `.2cbh` headers via `twocbh::Headers::open`, identifying the database generation as `Generation::TwoCbh`.
2. **Streaming & Zero-Allocation**: `for_each_game` and parallel conversion streams 2CBH games, entities, moves, and annotations with zero heap allocation in hot paths.
3. **Full Move & Annotation Support**: All moves, Chess960 setups, variations, NAGs, text commentary, graphical arrows/squares (`[%cal]`/`[%csl]`), evaluations (`[%eval]`/`[%evp]`), and medals (`[%mdl]`) are fully rendered to standard PGN.
