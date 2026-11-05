# 2026-11-05 in-progress 2CBH audit (MIT baseline)

Status: **preliminary corrected parser, NOT shippable PGN support**. This
review covers the committed source on the dedicated review branch; no
ChessBase-owned real database or PGN comparison was performed here.

## Confirmed issues addressed in this branch

| Severity | Finding | Correction / verification |
|---|---|---|
| P0 | 8-byte opaque tag at 0x10 incorrectly included the real 2-byte tag in the content | Check checksum at 0x10, parse u16 tag at 0x18, content at 0x1a |
| P0 | Synthetic fixtures reproduced the wrong framing layout, creating false confidence | Generate checksum-correct frames with A content and B spare bytes; assert byte offsets and detect corruption |
| P0 | 0x68/0x78 metadata treated as inline player names; Head returned absent IDs/results/ELO | Use signed .2lid IDs at 0x18/0x20 and pinned MIT record offsets; expose rating regions only as raw bytes |
| P1 | Record type read from byte 0, cannot distinguish game/text/analysis correctly | Byte 2 selects record kind, byte 0 supplies text/deleted flags |
| P1 | Raw move stream described as opaque compression | Parse 16-bit little-endian markers and expose borrowed structural tokens; still explicitly reject any claim that raw words are moves2 |
| P1 | Segment::record_at clones a PathBuf per game even with memory mapping | Pass borrowed path, read mapped head without copying, add record_into using capacity-reused caller-owned bytes |
| P1 | A lone .2cbg sidecar is reported as a database | Require .2cbh for the twocbh module's probe; accept upper-case member extensions |
| P2 | Diagnostic annotation text scanner always builds a Vec<String> | Add borrowed printable-run visitor; keep existing allocating wrapper for tooling only |

## Still open: release blockers and high-priority audit

1. **No .2lid entity decoding**: cannot produce correct White/Black/Event/etc.
2. **No legal replay or Chess960/variation/setup decode**: the raw v2
   move words are *not* gigachess moves2. Do not convert via casts or
   output guessed empty games. Integrate gigachess move enumeration with
   exact capture/legality validation.
3. **No annotation-to-ply mapping**: printable text runs are diagnostic,
   cannot be attached to PGN yet; comments, multilingual labels, NAGs,
   clocks/evaluations require independent typed parsing and full coverage.
4. **Main bridge/CLI still reject .2cbh**; only the format crate has the
   preliminary reader. When integrated, deleted/guiding text/analysis
   record counts must be handled explicitly rather than treated as games.
5. **Source-mutation safety**: memory maps of actively modified/truncated
   files can be unsafe at the OS level. Test snapshot/size checking and
   read-only positional fallbacks, especially on Windows.
6. **Real database + ChessBase PGN oracle missing**: local-only canonical
   byte comparison and raw hash/mismatch accounting remain mandatory.
   No private samples or PGNs in repo, CI or public issue logs.
7. **Performance is unmeasured**: source review shows removal of a
   PathBuf clone and a reusable pread buffer, not a throughput figure.
   Measure allocation counts (not merely retained Vec capacity), mmap vs
   non-mmap, per-game decode, complete export and ordered-parallel bytes;
   compare against the existing .cbh baselines on the SAME machine.
8. **No current guarantee against a classic-format regression**:
   cargo fmt/clippy/tests plus .cbh/.cbv/.cbz gold and throughput checks
   must pass before integration; apply ADR-001, ADR-002 and ADR-003.
9. **Provenance audit remains ongoing**: this branch uses only the
   v1.0.3 MIT snapshot. Do not merge any later AGPL or Morphy code
   without the specified independent facts-only review workflow.

## Upstream facts and license

Only the pinned
[oschess-cb-bridge v1.0.3 commit](https://github.com/asavis/oschess-cb-bridge/tree/bffebfc498d52b4e4631aed32b2f8e8194d125ff)
is used for implementation facts and MIT adaptation, especially
`crates/cbformat/src/v2/{frame,record,moves,bytes}.rs`.
The license file at that revision is MIT with copyright (c) 2026
the oschess-cb-bridge contributors. No second chess core is introduced.
The existing build graph has not been independently checked here.

## Exit conditions

Must pass `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features`, synthetic and fuzz tests,
`openspec validate native-2cbh-readonly --strict`, and the
local-only gold/parity/alloc/throughput gates. Do not mark the entire
OpenSpec change as complete because these framing fixes land.
