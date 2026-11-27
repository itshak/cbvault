# ChessBase opening books (`.ctg`/`.cto`/`.ctb`): byte-level observations

**Status: implemented read-only** (`cbvault-format::ctg`, `cbvault-chess::ctg`,
`cbvault::book`, `cbvault book …`). Everything below was established by
probing one real book from bytes. Nothing was taken from third-party code. The
probes are small throwaway Python scripts kept locally (not committed). Each fact
below names the probe and the count it rests on, so it can be re-run on another
book.

## Provenance

- **Clean-room.** These facts were derived from bytes only. The partial 2007 spec
  that the Chess Programming Wiki cites is no longer reachable at its original URL
  and was **not** consulted. The wiki summary page was read. It contributed only
  the high-level description: 4 KiB pages, white-to-move storage, colour flip and
  horizontal mirroring.
- **Not used:** `sshivaji/ctgreader`, `sshivaji/ctgexporter`, Daydreamer
  (`book_ctg.c`) and Brutus. `ctgreader` is labelled MIT, but its CTG code says it
  comes from Daydreamer. The Daydreamer repository has **no licence**, and its CTG
  code in turn credits Brutus. So the provenance of that code is unclear, and it
  must not be ported. `jja` (GPL-3) may serve only as an **oracle** in a separate
  process.

## Sample

One book, kept local and never committed: *Fritz Power Book 2025* (`PowerBook2025`).

| File | Size |
|---|---|
| `.ctg` | 2,157,928,448 bytes = 526,838 pages of 4096 bytes |
| `.cto` | 8,387,352 bytes |
| `.ctb` | 131,045 bytes |
| `.ini` | `[DescrCTG]` `Title=PowerBook2025` `Type=30` |

This book holds 27,592,793 position entries.

All multi-byte integers observed are **big-endian**.

## `.ctg` pages

Page 0 is a header page. Only 39 of its bytes are non-zero, and the last one is at
offset 2054. Its fields are **not** decoded yet.

Data pages (page 1 onwards):

| Offset | Size | Meaning | Evidence |
|---|---|---|---|
| 0 | u16 | number of position entries in the page | |
| 2 | u16 | bytes used in the page, header included | Walking the entries ends exactly at this value on **3000/3000** pages probed |
| 4 | … | entries, back to back | |

## Position entry

| Part | Size | Meaning |
|---|---|---|
| encoded position | `byte0 & 0x1F` bytes, `byte0` included | see below |
| move section | `m` bytes, where `m` is its first byte (included) | `(m - 1) / 2` moves of 2 bytes: move byte, annotation byte |
| statistics | 33 bytes | see below |

The framing closes exactly on every probed page (first bullet above). The root
entry has `m = 41`, which is 20 moves: the number of legal moves in the initial
position.

### Encoded position

`byte0` = `flags << 5 | length`:

- bit 6 (`0x40`): castling rights are present;
- bit 5 (`0x20`): an en-passant file is present;
- bit 7: never seen set in this book.

After `byte0`, the bits are read MSB first:

1. **Board:** 64 squares in the order **a1, a2, …, a8, b1, …, h8** (file-major),
   one prefix code per square.
2. **Zero padding** up to a byte boundary, with one exception. When the key has
   **no flags** and its bits already end on a byte boundary, a whole zero byte is
   added (282,640/282,640 aligned flagless keys). An aligned key with castling
   gets no padding (135,333/135,333). The aligned en-passant-only case was never
   observed, so the reader tries both forms for it.
3. **En passant:** if bit 5 is set, 3 bits giving the file (0 = a). They come just
   before the castling bits, or at the very end when there are no castling bits.
4. **Castling:** if bit 6 is set, the last 4 bits, in the order
   **black O-O, black O-O-O, white O-O, white O-O-O**.

Prefix code:

| Square | Code | Square | Code |
|---|---|---|---|
| empty | `0` | | |
| white pawn | `110` | black pawn | `111` |
| white knight | `10010` | black knight | `10011` |
| white bishop | `10100` | black bishop | `10101` |
| white rook | `10110` | black rook | `10111` |
| white king | `100000` | black king | `100001` |
| white queen | `100010` | black queen | `100011` |

The code is complete: the Kraft sum is exactly 1.

Evidence:

- **Root:** the entry with the most games (1,913,818) has 20 moves. A backtracking
  search for a prefix code that reproduces the initial position from its bits finds
  a **single** solution for each candidate square order. After the board exactly 4
  bits are left, and they are `1111` (all castling rights).
- **Orientation:** the initial position is symmetric, so the root alone cannot tell
  "a1 first, king `100000`" from its mirror image "h1 first, king and queen codes
  swapped". The first is sane (one king of each colour, no pawn on rank 1 or 8) on
  **1,048,466/1,048,466** entries. The mirrored one is sane on only 956,498 of them.
- **En passant:** with bit 5 set, the 3 file bits always name a file where a black
  pawn on rank 5 has a white pawn beside it and the squares behind it are empty
  (**2,752/2,752**). The bits right after the board do not (174/2,752).
- **Castling order:** of the 24 possible bit orders, only
  `k q K Q` never sets a right that is physically impossible (king or rook off its
  home square), on **365,029/365,029** entries. The next best order fits 89.6%.
- **Side to move:** every stored position is read as *White to move*. A
  Black-to-move position is looked up with its colours swapped and its ranks
  reversed. This is confirmed by the whole-book check below, where every decoded
  move of a White-to-move entry leads to a Black-to-move position that is found
  that way.
- **Mirroring:** among 683,437 entries with no castling flag, **none** has the white
  king on files a–d. With castling the king is on a–d in 16,106 entries. This
  matches the rule that a position with no castling rights is mirrored horizontally
  when the white king is on files a–d.

### Move bytes

A move byte stands for **(piece type, ordinal, file delta mod 8, rank delta mod 8)**
for the side to move, which is always White:

- **ordinal:** which piece of that type it is, counting in the board's square order
  (a1, a2, …);
- **target square:** `((file + dx) mod 8, (rank + dy) mod 8)`. A delta of +4 and one
  of −4 are the same mod 8. The board removes the ambiguity, because only one of the
  two targets lies on the board.
- **Castling** is the king moving two files: `0x6b` is dx = 2 (O-O) and `0xf6` is
  dx = 6, that is −2 (O-O-O).

Evidence: each byte's candidate set, generated from pseudo-legal moves (sliding
blockers, pawn rules, castling rights and empty squares, but no check test), is
intersected over every occurrence in a third of all pages. Meanings already pinned
to another byte are then excluded. Result: **139 of the 154 bytes seen are resolved
to one meaning, with zero contradictions.** The other 15 bytes occur at most twice
in that sample, probably for pieces that exist only after a promotion, and remain
**unknown**. The full table is below.

**Second pass, by child existence.** Every decoded move leads to a position that is
also in the book. So for each occurrence of a still-unknown byte, the candidates are
the legal moves whose resulting position is in the book and that no other byte of
the entry already names. Over the whole book, all 23 remaining bytes resolved to a
**single** candidate on every occurrence, each one a move of a **second queen**
(ordinal 1). Some of them rest on very few occurrences (1 to 10), so the table marks
them. That makes 162 bytes resolved.

**Promotions.** A move byte carries no promotion piece. For all 874 promotion moves
in the book, the position after promoting to a **queen** is in the book (874/874).
No entry repeats a byte. In 9 cases a position after an under-promotion also exists,
but no move byte leads to it. So a promotion byte decodes to a queen promotion, and
the move list cannot express an under-promotion.

**Open question:** whether the annotation byte carries `!`/`?`-style marks. It is
`0x00` in every move slot of this book.

### Statistics (33 bytes)

| Offset | Size | Meaning | Evidence |
|---|---|---|---|
| 0 | u24 | games | never 0; root = 1,913,818 |
| 3 | u24 | wins (side to move) | |
| 6 | u24 | losses | |
| 9 | u24 | draws | `games == wins + losses + draws` on 99.97% of 1,048,466 entries; `games >= sum` on all of them |
| 12 | u32 | **unknown**: score-like | equals `10*wins + 5*draws` on most small entries, but not at the root (off by 1,456). Expose raw. |
| 16 | u24 | games in which the **side to move** has a rating | `<= games` on all of them |
| 19 | u32 | sum of those ratings | ChessBase's position "Av" (HIARCS root: 2951) |
| 23 | u24 | games in which the **other side** has a rating | `<= games` on all of them |
| 26 | u32 | sum of those ratings | ChessBase's "Av" for a move is this average, truncated, in the position the move leads to: 20/20 HIARCS root moves |
| 30 | u8 | unknown (recommendation?) | `0x00` on every sampled entry of this book |
| 31 | u8 | unknown | `0x00` on every sampled entry |
| 32 | u8 | unknown (commentary?) | `0x00` on every sampled entry |

Wins and losses are counted **from the side to move**. After 1.e4 (a Black-to-move
position), reading the stored losses as White's wins gives White 53.9%; reading them
as Black's wins would give 46%. Fields 30–32 remain hypotheses (see the annotation section below). ChessBase's "Perf"
column is not stored: it matches the opponents' average plus a score-based offset.

## `.cto`

- Header (16 bytes): `00 10 00 01 | 00 1f fe c2 | 00 00 00 00 | 00 04 01 28`.
  The second word is the slot count (2,096,834 = (size − 16) / 4).
- Slots: u32 data-page numbers, where 0 is `.ctg` page 1. `0xFFFFFFFF` means empty.
- **Every data page is the target of exactly one slot** (526,837 used slots,
  526,837 distinct values, and no page without a slot).
- The used slots cluster in `[2^19, 2^20)` (about 521k), with about 5.4k in
  `[2^20, 2^21)` and 81 below `2^19`. This looks like a binary trie stored as a heap
  and indexed by hash bits: a page splits into two children when it fills.
- **The hash function is unknown.** For pages at depth 19, a GF(2)-linear model of
  the 19 index bits over the encoded-position bits is inconsistent in every variant
  tried (bytes padded on the left or the right, high or low index bits). The hash is
  not linear.

Until the hash is established, a reader can find a position by **scanning the
`.ctg`** (27.6M entries in this book), optionally building its own in-memory index.
It must not guess the `.cto` hash.

## `.ctb`

Header (12 bytes): `00 0c 00 01 | 00 04 01 28 | 00 0f fe c8`. Then a bitmap of
1,048,264 bits, with 526,836 bits set and the first zero at bit 262,440 (= `0x40128`,
a value that also appears in the `.cto` header). Probably a page-allocation bitmap.
A read-only reader does not need it.

## Whole-book verification

`cbvault book verify <book> --children`, on this book, in 22 s after a 0.7 s index
build (warm page cache):

| Check | Result |
|---|---|
| entries | 27,592,793 |
| damaged pages, undecodable keys, invalid positions | 0, 0, 0 |
| keys that encode back to the same bytes | 27,592,793 / 27,592,793 |
| move bytes decoded to a legal move | 28,264,190 (unknown 0, illegal 0; 874 queen promotions) |
| resulting positions found in the book | 28,264,190 / 28,264,190 |

## Nine more books

`cbvault book verify <book> --children` on nine other books held by the contributor (local only):

| Book | Positions | Keys round-trip | Moves decoded (unknown / illegal) | Children found |
|---|---|---|---|---|
| HIARCS 15 (`Hiarcs15vBook`) | 32,330,172 | all | 32,808,839 (4 / 0) | all |
| Fritz 20 | 5,668,540 | all | 5,842,778 (0 / 0) | all |
| StrongBook2025 | 2,231,384 | all | 2,273,174 (0 / 0) | all |
| user-built book A | 2,198,992 | all | 2,232,461 (0 / 0) | all |
| user-built book B | 631,863 | all | 640,419 (0 / 0) | all |
| a 2006 theory book | 837,991 | all | 843,643 (1 / 0) | all |
| user-built book C | 143,205 | all | 144,981 (0 / 0) | all |
| two small user-built books | 3,375 and 3,601 | all | 3,395 and 3,610 (0 / 0) | all |

The 5 unknown moves were three more second-queen bytes (`0x1a`, `0xab`, `0xee`),
resolved by child existence like the second pass. On every byte the books share,
HIARCS and the 2006 theory book give the same meaning as the second pass on PowerBook, found
independently. With them, every move of all ten books decodes.

## Annotation byte and the last statistics bytes

PowerBook leaves all of these zero. Other books do not.

**Ground truth (ChessBase, HIARCS 15 root, screenshot by the contributor, 2026-11-14).**
ChessBase shows `0xe1` as `!` in green, `0xc0` as no symbol in green, `0x25` as `!?`
in blue and `0x22` as `?` in red. So the **low 5 bits are the move symbol: 1 = `!`,
2 = `?`, 5 = `!?`**, which is NAG numbering. Other codes are not confirmed and are not
named by the reader. The high 3 bits (the class) are not established. In that book,
classes 6 and 7 were shown green, and class 1 was blue with `!?` and red with `?`.
The same screenshot matched N, %, =% and Av for all 20 moves (see the statistics
table). A later Fritz 20 screenshot showed code 6 as `?!`.

**Colours (11 ChessBase screenshots of HIARCS 15 and Fritz 20, 2026-11-14).** The
colour of a move depends on its annotation byte and on **offset 30 of the position
the move leads to**:

1. **red** when that offset 30 is `0x40` (with no symbol or with `?`);
2. otherwise **blue** when the symbol is `?`, `!?` or `?!` (with `0x00` or `0x80`);
3. otherwise, with no symbol or `!`: **green** when it is `0x80`, **black** when it is
   `0x00`.

Every move in the 11 screenshots fits this rule. Decisive pairs: HIARCS 9.e5? (blue)
and 9.a3? (red) carry the same annotation byte `0x22` and differ only in the flag;
Fritz 20 4...Bc5 has no symbol but `0x40` and is red; Fritz 20 4.Bxc6? has `0x00` and
is blue, not black; HIARCS 2...Nf6 after 1.d4 f5 2.h3 is class 1 with no symbol and
is green, so the class plays no part. Over the whole HIARCS book, `0x40` occurs only
with class 1 and `?` (918,849 moves), so the two sources never conflict there.
**A test book built by the contributor** (17 positions: the Kieninger trap
1.d4 Nf6 2.c4 e5 3.dxe5 Ng4 4.Bf4 Nc6 5.Nf3 Bb4+ 6.Nbd2 Qe7 7.a3 Ngxe5 8.axb4 Nd3#).
The contributor marked 8.axb4 `??` and 8...Nd3# `!!` in ChessBase. The bytes are `0x04` and
`0x03`, so **3 = `!!` and 4 = `??`**, as in NAG numbering. With offset 30 `0x00` in
the resulting position, 8.axb4?? is shown **blue** and 8...Nd3#!! **black**. A first
report of "both black" was mistaken: **ChessBase inverts the colour of the selected
row** (green shows magenta, red cyan, blue yellow, black white), and a screenshot
with another move selected settled it.
The other 14 moves are `0x00` with offset 30 `0x00`, as the rule predicts for a fresh
book.

**Move marks.** ChessBase's move menu offers "Main move" and "Don't play in
tournament". Marking 5.Nf3 and 6.Nbd2 "Main move" set offset 30 of the resulting
positions to `0x80` (green); marking 6...Qe7 "Don't play in tournament" set it to
`0x40` (red). Nothing else changed in the file.

**Position evaluations** (the "+−, =, …" menu) go to **offset 32** of the position
after the evaluated move, numbered as NAGs, and never change the colour. Set and read
back in the test book:

| Move | Black to move after it | Stored | ChessBase shows |
|---|---|---|---|
| 1.d4 | yes | 15 | ⩲ |
| 1...Nf6 | no | 16 | ± |
| 2.c4 | yes | 13 | ∞ |
| 2...e5 | no | 146 | N |
| 3.dxe5 | yes | 132 | ⇆ |
| 3...Ng4 | no | 14 | ⩲ |
| 4...Nc6 | no | 36 | ↑ |
| 5.Nf3 | yes | 44 | compensation |
| 6.Nbd2 | yes | 40 | → |
| 6...Qe7 | no | 15 | ⩱ |
| 7.a3 | yes | 19 | +− |
| 7...Ngxe5 | no | 17 | ∓ |

So the **advantage family (14–19) is stored for the stored, White-to-move board**: when
Black is to move, the code's colour is swapped (15 → ⩲, 19 → +−). The other codes
(∞, ⇆, →, compensation) are stored as they are, whoever is to move. The 16/17 pair
was only seen with White to move; its swap is inferred from the other two pairs.

4.Bf4! (`!` with `0x00`) is shown **black**: without the "Main move" mark no symbol
makes a move green. Combinations nothing showed (`!!` or `??` with `0x80`, other flag
values) are left without a colour.

**ChessBase's N** is `wins + draws + losses`, and its percentages are taken over that
sum. The total `games` is what it shows in parentheses ("N = 216 (222)"). The two
coincide in HIARCS but not in Fritz 20, where every N, % and =% on the screenshots
matches this definition.

- **Statistics offset 12** equals `10*wins + 5*draws` on 32,330,172/32,330,172
  HIARCS entries, 2,198,882/2,198,992 in user-built book A, 27,588,864/27,592,793 in
  PowerBook, 5,119,937/5,668,540 in Fritz 20 and only 199,293/837,991 in
  the 2006 theory book. It stays raw.
- **Annotation byte, HIARCS:** only `0x20 0x22 0x25 0x60 0x80 0xa0 0xc0 0xe1`. They
  read as 3 high bits (a class 1–7) plus 5 low bits (0, 1, 2 or 5). At the root:
  1.e4 and 1.d4 are `0xe1`; 1.c4 and 1.Nf3 `0xc0`; 1.e3, 1.g3, 1.b3 and 1.f4 `0x25`;
  the rest `0x22`. The mover's games-weighted score after the move ranks the low
  bits as 1 (50.9%) > 5 (47.7%) > 2 (44.5%).
- **The 2006 theory book** is set aside as unreliable, at the contributor's judgement: its results are
  skewed and its root annotations are odd. The two bullets below about it are kept
  only as a record.
- **Annotation byte, 2006 theory book:** values 1–8 (plus 7 moves at `0x20` and 5 at
  `0x16`). The scores rank 3 (32.7%) > 1 (24.2%) and 2 (4.0%), 4 (2.2%) lowest.
  This fits PGN NAG numbering (`$1` !, `$2` ?, `$3` !!, `$4` ??, `$5` !?, `$6` ?!),
  but at the root 1.e4 and 1.a4 are both 6, which is odd for a theory book.
- **Statistics offset 30:** `0x80` or `0x40` on HIARCS (31,414,101 and 916,070
  entries) and Fritz 20; zero in the others. Meaning unknown.
- **Statistics offset 32, 2006 theory book:** 10, 11, 13–19, 32, 36, 40, 44, 132, 138 and
  146. These are the NAG position-evaluation codes (=, ∞, ⩲, ⩱, ±, ∓, +−, −+,
  initiative, attack, compensation, counterplay, novelty). The side-to-move score
  is higher for the White-favouring codes than for the Black-favouring ones
  (16: 13.1% vs 17: 1.1%; 18: 18.0% vs 19: 0.3%; 14: 9.5% vs 15: 3.0%). That
  points to the evaluation being relative to the stored side to move, so it would
  flip for a Black-to-move probe. That book's results are skewed overall (18.1%
  on average), so this is only a lead.

The reader names only the confirmed symbols (`Annotation::symbol`). It exposes the
class, the other symbol codes and offsets 30–32 raw (`Annotation::class`,
`Annotation::symbol_code`, `Stats::raw_tail`).

## Move byte table (this book)

First pass: occurrences counted on every third page. The 23 second-queen bytes of the
second pass are listed after the table, with their whole-book occurrence counts.

| byte | piece | ordinal | file delta (mod 8) | rank delta (mod 8) | occurrences (1/3 sample) |
|---|---|---|---|---|---|
| `0xec` | P | 0 | 0 | 1 | 14507 |
| `0xdb` | P | 0 | 0 | 2 | 116663 |
| `0x8b` | P | 0 | 1 | 1 | 343 |
| `0x0c` | P | 0 | 7 | 1 | 42 |
| `0x04` | P | 1 | 0 | 1 | 12144 |
| `0x62` | P | 1 | 0 | 2 | 111830 |
| `0x96` | P | 1 | 1 | 1 | 508 |
| `0xd1` | P | 1 | 7 | 1 | 146 |
| `0x55` | P | 2 | 0 | 1 | 14325 |
| `0x9b` | P | 2 | 0 | 2 | 75591 |
| `0x7c` | P | 2 | 1 | 1 | 3800 |
| `0x27` | P | 2 | 7 | 1 | 1111 |
| `0xcf` | P | 3 | 0 | 1 | 15478 |
| `0xe6` | P | 3 | 0 | 2 | 59081 |
| `0xeb` | P | 3 | 1 | 1 | 1939 |
| `0x06` | P | 3 | 7 | 1 | 2977 |
| `0xbb` | P | 4 | 0 | 1 | 10020 |
| `0x5f` | P | 4 | 0 | 2 | 81184 |
| `0x00` | P | 4 | 1 | 1 | 422 |
| `0xde` | P | 4 | 7 | 1 | 1409 |
| `0xcb` | P | 5 | 0 | 1 | 5875 |
| `0xb1` | P | 5 | 0 | 2 | 60673 |
| `0x2d` | P | 5 | 1 | 1 | 57 |
| `0x82` | P | 5 | 7 | 1 | 236 |
| `0x18` | P | 6 | 0 | 1 | 10042 |
| `0xc8` | P | 6 | 0 | 2 | 42025 |
| `0x5c` | P | 6 | 1 | 1 | 37 |
| `0xc7` | P | 6 | 7 | 1 | 100 |
| `0xa3` | P | 7 | 0 | 1 | 8021 |
| `0x4a` | P | 7 | 0 | 2 | 19540 |
| `0x14` | P | 7 | 1 | 1 | 9 |
| `0x24` | P | 7 | 7 | 1 | 47 |
| `0x34` | N | 0 | 1 | 2 | 15983 |
| `0x8a` | N | 0 | 1 | 6 | 654 |
| `0x36` | N | 0 | 2 | 1 | 15642 |
| `0xe9` | N | 0 | 2 | 7 | 3924 |
| `0xf7` | N | 0 | 6 | 1 | 4591 |
| `0x0f` | N | 0 | 6 | 7 | 1575 |
| `0x85` | N | 0 | 7 | 2 | 7080 |
| `0xda` | N | 0 | 7 | 6 | 650 |
| `0x4c` | N | 1 | 1 | 2 | 3288 |
| `0x2a` | N | 1 | 1 | 6 | 695 |
| `0xd2` | N | 1 | 2 | 1 | 3118 |
| `0xe0` | N | 1 | 2 | 7 | 984 |
| `0xd3` | N | 1 | 6 | 1 | 5263 |
| `0x01` | N | 1 | 6 | 7 | 4195 |
| `0xf3` | N | 1 | 7 | 2 | 12675 |
| `0x30` | N | 1 | 7 | 6 | 1666 |
| `0x2e` | B | 0 | 1 | 1 | 7876 |
| `0x26` | B | 0 | 1 | 7 | 1355 |
| `0xc1` | B | 0 | 2 | 2 | 41959 |
| `0xb2` | B | 0 | 2 | 6 | 664 |
| `0x0d` | B | 0 | 3 | 3 | 5562 |
| `0x3c` | B | 0 | 3 | 5 | 382 |
| `0x6a` | B | 0 | 4 | 4 | 6884 |
| `0x66` | B | 0 | 5 | 3 | 879 |
| `0x15` | B | 0 | 5 | 5 | 282 |
| `0xc3` | B | 0 | 6 | 2 | 3568 |
| `0x3a` | B | 0 | 6 | 6 | 264 |
| `0x1b` | B | 0 | 7 | 1 | 6424 |
| `0x12` | B | 0 | 7 | 7 | 1015 |
| `0xf5` | B | 1 | 1 | 1 | 4093 |
| `0x72` | B | 1 | 1 | 7 | 2420 |
| `0xc2` | B | 1 | 2 | 2 | 1800 |
| `0x79` | B | 1 | 2 | 6 | 820 |
| `0xb9` | B | 1 | 3 | 3 | 613 |
| `0xcc` | B | 1 | 3 | 5 | 467 |
| `0x9f` | B | 1 | 4 | 4 | 2545 |
| `0xca` | B | 1 | 5 | 3 | 6810 |
| `0x32` | B | 1 | 5 | 5 | 486 |
| `0x22` | B | 1 | 6 | 2 | 89277 |
| `0x09` | B | 1 | 6 | 6 | 29874 |
| `0x54` | B | 1 | 7 | 1 | 9725 |
| `0x1d` | B | 1 | 7 | 7 | 2630 |
| `0x81` | R | 0 | 0 | 1 | 566 |
| `0xe8` | R | 0 | 0 | 2 | 11996 |
| `0x7a` | R | 0 | 0 | 3 | 18889 |
| `0x99` | R | 0 | 0 | 4 | 195 |
| `0x87` | R | 0 | 0 | 5 | 106 |
| `0x50` | R | 0 | 0 | 6 | 68 |
| `0xe1` | R | 0 | 0 | 7 | 92 |
| `0x7f` | R | 0 | 1 | 0 | 4294 |
| `0xcd` | R | 0 | 2 | 0 | 13165 |
| `0x98` | R | 0 | 3 | 0 | 2665 |
| `0xef` | R | 0 | 4 | 0 | 508 |
| `0x6e` | R | 0 | 5 | 0 | 51 |
| `0x52` | R | 0 | 6 | 0 | 67 |
| `0x86` | R | 0 | 7 | 0 | 140 |
| `0x7d` | R | 1 | 0 | 1 | 409 |
| `0xf4` | R | 1 | 0 | 2 | 12033 |
| `0xe3` | R | 1 | 0 | 3 | 931 |
| `0xc5` | R | 1 | 0 | 4 | 360 |
| `0xa5` | R | 1 | 0 | 5 | 72 |
| `0x7b` | R | 1 | 0 | 6 | 50 |
| `0x69` | R | 1 | 0 | 7 | 81 |
| `0xc4` | R | 1 | 1 | 0 | 164 |
| `0xa9` | R | 1 | 2 | 0 | 52 |
| `0x0e` | R | 1 | 3 | 0 | 43 |
| `0xad` | R | 1 | 4 | 0 | 273 |
| `0xb5` | R | 1 | 5 | 0 | 1041 |
| `0xd8` | R | 1 | 6 | 0 | 8066 |
| `0x21` | R | 1 | 7 | 0 | 8771 |
| `0x05` | Q | 0 | 0 | 1 | 4386 |
| `0x9d` | Q | 0 | 0 | 2 | 17766 |
| `0x94` | Q | 0 | 0 | 3 | 601 |
| `0x37` | Q | 0 | 0 | 4 | 253 |
| `0xb7` | Q | 0 | 0 | 5 | 204 |
| `0x9a` | Q | 0 | 0 | 6 | 265 |
| `0xfd` | Q | 0 | 0 | 7 | 582 |
| `0x2f` | Q | 0 | 1 | 0 | 1531 |
| `0xf1` | Q | 0 | 1 | 1 | 6964 |
| `0x42` | Q | 0 | 1 | 7 | 755 |
| `0x74` | Q | 0 | 2 | 0 | 685 |
| `0xa2` | Q | 0 | 2 | 2 | 19551 |
| `0x80` | Q | 0 | 2 | 6 | 1085 |
| `0x31` | Q | 0 | 3 | 0 | 254 |
| `0x45` | Q | 0 | 3 | 3 | 581 |
| `0x4b` | Q | 0 | 3 | 5 | 109 |
| `0xe5` | Q | 0 | 4 | 0 | 134 |
| `0xe7` | Q | 0 | 4 | 4 | 598 |
| `0x39` | Q | 0 | 5 | 0 | 213 |
| `0x7e` | Q | 0 | 5 | 3 | 3111 |
| `0x28` | Q | 0 | 5 | 5 | 69 |
| `0x29` | Q | 0 | 6 | 0 | 821 |
| `0xd9` | Q | 0 | 6 | 2 | 7798 |
| `0x61` | Q | 0 | 6 | 6 | 241 |
| `0x8f` | Q | 0 | 7 | 0 | 1522 |
| `0xd7` | Q | 0 | 7 | 1 | 7649 |
| `0xed` | Q | 0 | 7 | 7 | 644 |
| `0x13` | K | 0 | 0 | 1 | 401 |
| `0x0a` | K | 0 | 0 | 7 | 39 |
| `0xbe` | K | 0 | 1 | 0 | 2664 |
| `0x67` | K | 0 | 1 | 1 | 459 |
| `0x44` | K | 0 | 1 | 7 | 48 |
| `0x6b` | K | 0 | 2 | 0 | 283541 |
| `0xf6` | K | 0 | 6 | 0 | 52074 |
| `0x97` | K | 0 | 7 | 0 | 702 |
| `0xdf` | K | 0 | 7 | 1 | 205 |
| `0x8c` | K | 0 | 7 | 7 | 24 |
| `0x08` | see second pass | | | | 1 |
| `0x23` | see second pass | | | | 1 |
| `0x38` | see second pass | | | | 2 |
| `0x3b` | see second pass | | | | 1 |
| `0x41` | see second pass | | | | 2 |
| `0x4d` | see second pass | | | | 1 |
| `0x63` | see second pass | | | | 1 |
| `0x92` | see second pass | | | | 2 |
| `0xa0` | see second pass | | | | 1 |
| `0xb0` | see second pass | | | | 1 |
| `0xc6` | see second pass | | | | 2 |
| `0xc9` | see second pass | | | | 2 |
| `0xf8` | see second pass | | | | 1 |
| `0xfa` | see second pass | | | | 2 |
| `0xfe` | see second pass | | | | 1 |

Second pass (child existence, whole book). In each row the piece is a second queen
(Q, ordinal 1):

| byte | file delta (mod 8) | rank delta (mod 8) | occurrences |
|---|---|---|---|
| `0x03` | 2 | 0 | 1 |
| `0x08` | 4 | 0 | 2 |
| `0x23` | 6 | 6 | 3 |
| `0x38` | 4 | 4 | 6 |
| `0x3b` | 3 | 5 | 2 |
| `0x41` | 5 | 5 | 6 |
| `0x4d` | 0 | 1 | 2 |
| `0x63` | 1 | 7 | 3 |
| `0x6f` | 7 | 7 | 4 |
| `0x8e` | 6 | 2 | 1 |
| `0x92` | 1 | 1 | 3 |
| `0xa0` | 0 | 3 | 2 |
| `0xae` | 3 | 3 | 1 |
| `0xb0` | 0 | 4 | 5 |
| `0xbd` | 0 | 2 | 2 |
| `0xc6` | 0 | 5 | 2 |
| `0xc9` | 0 | 7 | 8 |
| `0xf0` | 7 | 0 | 1 |
| `0xf8` | 1 | 0 | 1 |
| `0xf9` | 6 | 0 | 2 |
| `0xfa` | 3 | 0 | 5 |
| `0xfb` | 2 | 2 | 1 |
| `0xfe` | 5 | 3 | 10 |

## Public 3,410,497-game corpus: additional third-piece codes (2026-11-26)

The first strict CI run verified 85,834,993 positions, 87,463,661 known moves and their children, and exposed 12 unknown occurrences. Two streamed passes through the checksum-pinned public book independently found exactly one legal, previously unmapped code and existing child key per occurrence. The nine new mappings are:

| Byte | Kind | Zero-based ordinal | File delta mod 8 | Rank delta mod 8 | Occurrences |
|---|---|---:|---:|---:|---:|
| 0x0b | Rook | 2 | 7 | 0 | 1 |
| 0x1e | Queen | 2 | 1 | 7 | 1 |
| 0x3d | Knight | 2 | 6 | 7 | 3 |
| 0x46 | Queen | 2 | 5 | 5 | 1 |
| 0x4e | Queen | 2 | 6 | 6 | 1 |
| 0x4f | Bishop | 2 | 7 | 7 | 1 |
| 0x57 | Queen | 2 | 0 | 6 | 1 |
| 0xd4 | Queen | 2 | 2 | 0 | 2 |
| 0xd6 | Rook | 2 | 0 | 7 | 1 |

All 12 positions are now regression tests. The candidate must pass complete verification with 87,463,673 decoded moves and zero unknowns. The v0.2.0 baseline is explicitly pinned to its observed 12 unknown occurrences; the candidate gate is not relaxed.

## Addendum (2026-11-27): Verified Page 0, `.cto`/`.ctb` Split Trie, Alignment, and Move Census

Empirical checks across 9 ChessBase `.ctg`/`.cto`/`.ctb` books (`228,697,037` entries, `232,736,748` moves) established the following format invariants:

- **Page 0 big-endian header words**: offset `0` holds `0x0020_1000`, offset `4` holds `0x0001_0001` (classic/commercial books) or `0x0002_0001` (CB18-generated books), offset `8` holds `0x0000_1000` (`4096`), offset `12` holds `0x7fff_7fff`, offset `16` (`w4`) equals `data_pages` (`pages - 1`), offset `20` holds `0xffff_ffff`, offset `24` (`w6`) equals total stored entry count, and offset `28` (`w7`) equals source game count. Minimal synthetic test fixtures keep Page 0 zeroed.
- **`.cto` / `.ctb` hash-trie geometry**: `.ctb` bytes `[12..]` encode an LSB-first binary split bitmap rooted at slot `0`, where bit `s = 1` means bucket `s` at trie level `b` split into `(s + 2^b, s + 2^(b+1))`. Walking split bits reproduces the exact set of occupied `.cto` leaf slots on every tested book (`occ_lt_low == 0`). Small books can validly have `high > slots` (observed `slots = 7, low = 0, high = 8`), so `high <= slots` must not be enforced as a validity requirement. `.ctb` size equals `12 + high.div_ceil(8)`.
- **Key alignment padding & hygiene**: Across all `228,697,037` stored keys, `bit7_set == 0` and non-zero padding bits `== 0`. When aligned at a byte boundary, `(has_castling = false, has_ep = false)` uses 1 extra `0x00` padding byte (100% across `52.6M+` aligned keys), whereas all aligned keys with castling or en-passant flags set omit the extra byte (`100%` across `29.2M+` aligned keys including aligned `(false, true)` and `(true, true)` cases).
- **256-byte move table**: Exactly 179 bytes are observed across all 9 books, matching the 179 entries mapped in `crates/cbvault-format/src/ctg/moves.rs` (`0` unmapped observed bytes, `0` unused mapped entries), including all 13 `ordinal = 2` third-piece codes. Fourth-piece (`ordinal >= 3`) codes remain unobserved. Underpromotions cannot be expressed in the 1-byte parent move table (`587` underpromotion-only child positions omit the corresponding parent move byte while `18,660` queen promotions have matching child positions).
- **`raw_score` & Hiarcs `raw_tail` behavior**: Games with result `*` add `+5` to `raw_score` when White is to move and `+0` when Black is to move across all tested books (`50%/50%` split on `games > wins + draws + losses`, confirmed `104/108` exact on `special-expanded-v3`). In `Hiarcs14zmBook.ctg`, `15,698,628` entries have `games == 0`, and `494` entries pair `raw_tail[0] & 0x04 != 0` with non-zero `raw_tail[1]`.
