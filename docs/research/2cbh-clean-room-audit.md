# Clean-Room Audit Record: 2CBH Native Reader

> **Status:** APPROVED & FROZEN.
> **Change:** `native-2cbh-readonly` (Task 3.3).
> **Scope:** Independent hygiene, license audit, and source isolation verification for 2CBH format facts.

---

## 1. Clean-Room Protocol & Barrier Verification

In accordance with `AGENTS.md` and `openspec/changes/native-2cbh-readonly/design.md`, a formal two-room clean-room protocol governs all 2CBH format improvements beyond the pinned MIT ancestor (`v1.0.3` @ `bffebfc498d52b4e4631aed32b2f8e8194d125ff`):

| Role | Entity | Permitted Access | Restrictions | Status |
|---|---|---|---|---|
| **Room 1 (Specifier)** | Research Agent | Pinned MIT tree, upstream AGPL commit log & diffs, Morphy format docs. | May write only original factual specifications; zero copied code or prose. | **Complete** — `docs/research/2cbh-delta-inventory.md`, `docs/research/2cbh-after-mit-facts.md` |
| **Independent Reviewer** | Hygiene / License Reviewer | All specifications, diffs, and codebase. | Evaluates text originality, non-infringement, and clean-room independence. | **Approved** — See Section 3 below. |
| **Room 2 (Implementer)** | Implementation Agent | `cbvault` repository, pinned MIT baseline, frozen factual spec, synthetic fixtures. | **Strictly prohibited** from viewing later AGPL source code, Morphy Java code, or Room 1 working notes. | **Enforced** |

---

## 2. Frozen Specification Artifacts & Hashes

The following factual research documents are reviewed, approved, and frozen:

| Artifact | Purpose | SHA-256 Hash |
|---|---|---|
| `docs/research/2cbh-after-mit-facts.md` | Clean factual specification of 2CBH post-MIT layouts, offsets, bitfields, and synthetic recipes. | `92409e10f4bcfbd4546b2961df59f82fe896867d0c4b568c2fde855e05c175cb` |
| `docs/research/2cbh-delta-inventory.md` | Version delta catalog comparing pinned MIT baseline (`bffebfc4`) to later AGPL releases and Morphy Java. | `68e534391f8622596d5eb0f29af7d780a1363bd2d126e947e3eae29dd7f223ec` |

---

## 3. Hygiene & License Audit Findings

### 3.1 Non-Copying Verification ("Could this have been written by someone who never opened the reference?")
- **Code:** Verified zero lines of Rust code from later AGPL commits or Java code from Morphy exist in `docs/research/2cbh-after-mit-facts.md`.
- **Text & Prose:** The explanations, terminology, descriptions, and synthetic recipes are original formulations using cbvault's domain vocabulary and standard chess terminology.
- **Tables & Offsets:** Binary geometry, offsets, and bitfields are documented as objective mathematical facts necessary for interoperability (per 17 U.S.C. § 102(b) and EU Software Directive Art. 6).

### 3.2 Chess Core Independence
- Upstream `oschess` uses `chesscore`.
- The factual spec does not reference `chesscore` internal structures or vocabularies.
- All chess rules, move conversions, and position representations are implemented solely via `gigachess` and 16-bit `moves2`.

### 3.3 No Proprietary Data Leakage
- Zero user names, real game fragments, player ratings, or private database paths are included in the specifications or commit tree.
- Testing is based strictly on synthetic fixture recipes.

---

## 4. Audit Sign-Off

The factual specification `docs/research/2cbh-after-mit-facts.md` satisfies all clean-room hygiene criteria and is approved for implementation use.

- **Review Outcome:** APPROVED.
- **Date:** 2026-11-05.
