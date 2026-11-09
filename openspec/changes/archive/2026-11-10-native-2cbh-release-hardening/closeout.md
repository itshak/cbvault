# 2CBH strict read-only acceptance — archived November 10, 2026

## Accepted sample

The owner approved a frozen **3,410,497-game ChessBase 18 PGN export** from Mega Database 2026 as sufficient for the 2CBH release-hardening gold gate. The private test agent reported **zero unexplained canonical-byte differences** across **1,772,375,564 canonical bytes** using commit [70b61c8](https://github.com/itshak/cbvault/commit/70b61c8b6225ece9513e0c4533a251cad01c1ff3). This is not raw PGN equality (`raw_equal=false`); formats, GUI-only headers and proprietary presentation differ.

The frozen export covers the first 3,410,497 eligible games and analyses, interleaved with 1,439 non-game records. The complete .2cbh header set contains 11,743,083 slots. A separate full header scan classified 8,330,763 eligible later games and 384 later non-game records, but **did not independently decode and compare their move/annotation bodies**. The acceptance claim covers the exported 3.41M sample, not all games in Mega 2026.

## Reported classified differences

- Board-verified redundant SAN qualifiers: **99,130**; null-move Z0/--: **1,984**
- Diagram markers: **782**; proprietary glyph/PUA symbols: **237**; text-order adjustments: **93**; whitespace adjustments: **2**
- Excluded from comparable PGN: **2,084** proprietary training directives and **51,415,248** ChessBase GUI-only tag occurrences across **26** recognized tag names. These are *not* byte-equal metadata; some may have independent semantic value.
- Unexplained sample differences: **zero**; raw equality: **false**. The private agent reported no unexpected decode or annotation-read failures.

The previous private agent reported a source-to-PGN identity audit using header offsets 0x98 (`GameId`) and 0xb0 (`Beauty`) checked against ChessBase PGN tags. The licensed source, oracle PGN, hashes and per-game diffs stayed local. Public CI cannot reproduce this evidence independently.

## Public verification

The final implementation/report commit [70b61c8](https://github.com/itshak/cbvault/commit/70b61c8b6225ece9513e0c4533a251cad01c1ff3) follows merged [PR #5](https://github.com/itshak/cbvault/pull/5). The complete [five-job CI run 42710306541](https://github.com/itshak/cbvault/actions/runs/42710306541) passed on Ubuntu, Windows, macOS, nightly fuzzing and single-chess-core validation. Public CI uses generated synthetic data and tests the strict oracle's negative and positive rules; it is **not a rerun of the proprietary Mega reference corpus**.

## Scope and remaining coverage gaps

The accepted feature set includes read-only native .2cbh/.2cbg/.2cba/.2lid, annotated game replay and variations, metadata/namebase lookups, generation-aware APIs and position search, PGN export, and ordered parallel conversion. Unknown/malformed native annotation frames fail closed rather than silently truncating games.

Only **one real v2 corpus** has a local ChessBase oracle. Other database generations/layouts, rare native annotations, quotations, embedded multimedia, real-world Chess960, and proprietary training/GUI metadata excluded from comparable PGN are not universally verified. Native format writing/editing is intentionally out of scope. No new crate publication, release tag or downstream integration was performed.

**Archive decision:** owner accepted the significant sample as sufficient on November 10, 2026; the permanent capability spec and public documentation now state the sample boundary precisely. Re-run the local gold sample if future parser/oracle changes affect these contracts.
