//! Comprehensive CTG/CTO/CTB validation, corruption, concurrency,
//! differential parity, duplicate-key, and small-geometry regression tests.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

use cbvault::book::{AdaptiveBook, Book, DiscoveredPromotion, LookupMode, NativeBook, NativeProbe};
use cbvault_chess::ctg::{key_candidates, stored_form};
use cbvault_format::ctg::cto::validate_directory_bytes;
use cbvault_format::ctg::{Entries, PAGE_SIZE};
use gigachess::Board;
use gigachess::Role;
use gigachess::fen::parse_fen;
use gigachess::san::san_to_move;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("cbvault_ctg_val_{name}_{pid}_{stamp}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn write(&self, file: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path.join(file);
        fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn ctg_entry(key: &[u8], moves: &[(u8, u8)], games: u32, wins: u32, losses: u32, draws: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(key);
    out.push((1 + moves.len() * 2) as u8);
    for &(b, a) in moves {
        out.push(b);
        out.push(a);
    }
    let mut st = [0u8; 33];
    st[0..3].copy_from_slice(&games.to_be_bytes()[1..]);
    st[3..6].copy_from_slice(&wins.to_be_bytes()[1..]);
    st[6..9].copy_from_slice(&losses.to_be_bytes()[1..]);
    st[9..12].copy_from_slice(&draws.to_be_bytes()[1..]);
    let score = wins * 10 + draws * 5;
    st[12..16].copy_from_slice(&score.to_be_bytes());
    out.extend_from_slice(&st);
    out
}

fn ctg_page(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_SIZE];
    let count = entries.len() as u16;
    let mut pos = 4usize;
    for e in entries {
        page[pos..pos + e.len()].copy_from_slice(e);
        pos += e.len();
    }
    page[0..2].copy_from_slice(&count.to_be_bytes());
    page[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    page
}

fn key_for(fen: &str) -> Vec<u8> {
    let b = parse_fen(fen).unwrap();
    let st = stored_form(&b);
    let (k, l) = key_candidates(&st).next().unwrap();
    k[..l].to_vec()
}

fn build_ctg(pages: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; PAGE_SIZE];
    for p in pages {
        assert_eq!(p.len(), PAGE_SIZE);
        out.extend_from_slice(p);
    }
    out
}

/// Builds a valid level-order trie CTO/CTB covering every key across pages `1..=N`.
/// Keeps `slots = 7, low = 0, high = 8` (`high > slots`) to mirror small real ChessBase books.
fn build_small_trie_cto_ctb(data_pages: u32) -> (Vec<u8>, Vec<u8>) {
    assert!((1..=4).contains(&data_pages));
    let mut cto = vec![0u8; 16 + 7 * 4];
    cto[0..4].copy_from_slice(&0x0010_0001u32.to_be_bytes());
    cto[4..8].copy_from_slice(&7u32.to_be_bytes());
    // Fill slots 0..7 as empty sentinel
    for s in 0..7usize {
        cto[16 + s * 4..20 + s * 4].copy_from_slice(&u32::MAX.to_be_bytes());
    }
    // Single bucket 0 for 1 page, split root 0 -> leaves 3..6 for up to 4 pages
    let bitmap: u8 = match data_pages {
        1 => {
            cto[16..20].copy_from_slice(&0u32.to_be_bytes());
            return (cto, {
                let mut ctb = vec![0u8; 13];
                ctb[0..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
                ctb[4..8].copy_from_slice(&0u32.to_be_bytes());
                ctb[8..12].copy_from_slice(&8u32.to_be_bytes());
                ctb[12] = 0x00;
                ctb
            });
        }
        2 => {
            cto[16 + 4..20 + 4].copy_from_slice(&0u32.to_be_bytes());
            cto[16 + 8..20 + 8].copy_from_slice(&1u32.to_be_bytes());
            0x01
        }
        3 => {
            cto[16 + 12..20 + 12].copy_from_slice(&0u32.to_be_bytes());
            cto[16 + 16..20 + 16].copy_from_slice(&1u32.to_be_bytes());
            cto[16 + 8..20 + 8].copy_from_slice(&2u32.to_be_bytes());
            0x03
        }
        4 => {
            for i in 0..4usize {
                let s = 3 + i;
                cto[16 + s * 4..20 + s * 4].copy_from_slice(&(i as u32).to_be_bytes());
            }
            0x07
        }
        _ => unreachable!(),
    };
    let mut ctb = vec![0u8; 13];
    ctb[0..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
    ctb[4..8].copy_from_slice(&0u32.to_be_bytes());
    ctb[8..12].copy_from_slice(&8u32.to_be_bytes());
    ctb[12] = bitmap;
    (cto, ctb)
}

fn book_with_root_and_e4() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let root = Board::startpos();
    let e4 = san_to_move(&root, "e4").unwrap();
    let mut after_e4 = root;
    after_e4.play(e4).unwrap();
    let e5 = san_to_move(&after_e4, "e5").unwrap();
    let mut after_e5 = after_e4;
    after_e5.play(e5).unwrap();

    // e2e4 move byte is 95 (0x5f), e7e5 color-flipped is also 95 (0x5f)
    let k0 = key_for(&root.to_fen());
    let k1 = key_for(&after_e4.to_fen());
    let k2 = key_for(&after_e5.to_fen());
    let e0 = ctg_entry(&k0, &[(0x5f, 0)], 10, 5, 2, 3);
    let e1 = ctg_entry(&k1, &[(0x5f, 0)], 10, 2, 5, 3);
    let e2 = ctg_entry(&k2, &[], 10, 5, 2, 3);

    let p1 = ctg_page(&[e0, e1, e2]);
    let ctg = build_ctg(&[p1]);
    let (cto, ctb) = build_small_trie_cto_ctb(1);
    (ctg, cto, ctb)
}

#[test]
fn small_book_high_gt_slots_opens_validates_and_matches_scan() {
    // Mirrors ChessBase small books (slots = 7, low = 0, high = 8).
    let dir = TempDir::new("small_high_gt_slots");
    let root = Board::startpos();
    let e4 = san_to_move(&root, "e4").unwrap();
    let mut after_e4 = root;
    after_e4.play(e4).unwrap();
    let k0 = key_for(&root.to_fen());
    let k1 = key_for(&after_e4.to_fen());

    let p1 = ctg_page(&[ctg_entry(&k0, &[(0x5f, 0)], 4, 2, 1, 1)]);
    let p2 = ctg_page(&[ctg_entry(&k1, &[], 4, 1, 2, 1)]);
    let mut p3 = ctg_page(&[]);
    p3[0..4].copy_from_slice(&[0, 0, 0, 4]);
    let p4 = ctg_page(&[]);
    // Put each key onto the exact page selected by slots_for (& 3 + 3 in slots 3..=6)
    let mut page_buckets: [Vec<Vec<u8>>; 4] = Default::default();
    for (k, ent) in [(&k0, ctg_entry(&k0, &[(0x5f, 0)], 4, 2, 1, 1)), (&k1, ctg_entry(&k1, &[], 4, 1, 2, 1))] {
        let h = cbvault_format::ctg::cto::native_hash(k);
        let slot = ((h & 3) + 3) as usize;
        page_buckets[slot - 3].push(ent);
    }
    let pages: Vec<Vec<u8>> = page_buckets.iter().map(|es| ctg_page(es)).collect();
    let _ = (p1, p2, p3, p4);
    let ctg = build_ctg(&pages);
    let (cto, ctb) = build_small_trie_cto_ctb(4);
    let path = dir.write("small.ctg", &ctg);
    dir.write("small.cto", &cto);
    dir.write("small.ctb", &ctb);

    let scan = Book::open(&path).unwrap();
    assert!(scan.verify(true).unwrap().ok(true));

    let native = NativeBook::open(&path).unwrap();
    assert_eq!(native.bounds(), (7, 0, 8));
    assert_eq!(native.validate().unwrap(), 2);

    let adaptive = AdaptiveBook::open(&path).unwrap();
    assert_eq!(adaptive.mode(), LookupMode::NativeUnverified);
    assert_eq!(adaptive.probe_tree(&root).unwrap(), NativeProbe::Found(scan.probe_tree(&root).unwrap().unwrap()));
    assert_eq!(adaptive.mode(), LookupMode::NativeUnverified);

    let missing = parse_fen("7k/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
    assert_eq!(adaptive.probe(&missing).unwrap(), NativeProbe::Absent);
    assert_eq!(adaptive.mode(), LookupMode::NativeValidated);
    assert_eq!(
        adaptive.probe_tree(&after_e4).unwrap(),
        NativeProbe::Found(scan.probe_tree(&after_e4).unwrap().unwrap())
    );
    assert_eq!(adaptive.probe_tree(&missing).unwrap(), NativeProbe::Absent);
}

#[test]
fn native_and_adaptive_uppercase_extensions_open_cleanly() {
    let dir = TempDir::new("upper_ext");
    let (ctg, cto, ctb) = book_with_root_and_e4();
    let path = dir.write("UPPER.CTG", &ctg);
    dir.write("UPPER.CTO", &cto);
    dir.write("UPPER.CTB", &ctb);

    let native = NativeBook::open(&path).unwrap();
    assert_eq!(native.validate().unwrap(), 3);
    let adaptive = AdaptiveBook::open(&path).unwrap();
    assert!(matches!(adaptive.probe(&Board::startpos()).unwrap(), NativeProbe::Found(_)));
}

#[test]
fn all_nineteen_corruption_scenarios_fail_closed_or_fall_back() {
    let root = Board::startpos();
    let missing = parse_fen("7k/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
    let (ctg, cto, ctb) = book_with_root_and_e4();

    // 1. Missing .ctg
    {
        let dir = TempDir::new("c1");
        let missing_ctg = dir.path.join("none.ctg");
        assert!(Book::open(&missing_ctg).is_err());
        assert!(NativeBook::open(&missing_ctg).is_err());
        assert!(AdaptiveBook::open(&missing_ctg).is_err());
    }
    // 2. Missing .cto -> Native fails, Adaptive falls back to Scan
    {
        let dir = TempDir::new("c2");
        let p = dir.write("b.ctg", &ctg);
        dir.write("b.ctb", &ctb);
        assert!(NativeBook::open(&p).is_err());
        let ab = AdaptiveBook::open(&p).unwrap();
        assert_eq!(ab.mode(), LookupMode::Scan);
        assert!(matches!(ab.probe(&root).unwrap(), NativeProbe::Found(_)));
    }
    // 3. Missing .ctb -> Native fails, Adaptive falls back to Scan
    {
        let dir = TempDir::new("c3");
        let p = dir.write("b.ctg", &ctg);
        dir.write("b.cto", &cto);
        assert!(NativeBook::open(&p).is_err());
        let ab = AdaptiveBook::open(&p).unwrap();
        assert_eq!(ab.mode(), LookupMode::Scan);
    }
    // 4. Empty .ctg (0 bytes)
    {
        let dir = TempDir::new("c4");
        let p = dir.write("b.ctg", &[]);
        assert!(Book::open(&p).is_err());
        assert!(NativeBook::open(&p).is_err());
    }
    // 5. Truncated header page (< 4096 bytes)
    {
        let dir = TempDir::new("c5");
        let p = dir.write("b.ctg", &[0u8; 4095]);
        assert!(Book::open(&p).is_err());
    }
    // 6. Non-multiple of 4096 bytes (.ctg trailing truncated page)
    {
        let dir = TempDir::new("c6");
        let mut bad = ctg.clone();
        bad.extend_from_slice(&[0u8; 512]);
        let p = dir.write("b.ctg", &bad);
        assert!(Book::open(&p).is_err());
    }
    // 7. Empty / truncated .cto header (< 16 bytes)
    {
        let dir = TempDir::new("c7");
        let p = dir.write("b.ctg", &ctg);
        dir.write("b.cto", &cto[..12]);
        dir.write("b.ctb", &ctb);
        assert!(NativeBook::open(&p).is_err());
        assert_eq!(AdaptiveBook::open(&p).unwrap().mode(), LookupMode::Scan);
    }
    // 8. Truncated .cto slot table (size mismatch)
    {
        let dir = TempDir::new("c8");
        let p = dir.write("b.ctg", &ctg);
        dir.write("b.cto", &cto[..cto.len() - 4]);
        dir.write("b.ctb", &ctb);
        assert!(NativeBook::open(&p).is_err());
        assert_eq!(AdaptiveBook::open(&p).unwrap().mode(), LookupMode::Scan);
    }
    // 9. Truncated .ctb bitmap
    {
        let dir = TempDir::new("c9");
        let p = dir.write("b.ctg", &ctg);
        dir.write("b.cto", &cto);
        dir.write("b.ctb", &ctb[..12]); // high=8 requires 1 bitmap byte
        assert!(NativeBook::open(&p).is_err());
        assert_eq!(AdaptiveBook::open(&p).unwrap().mode(), LookupMode::Scan);
    }
    // 10. Corrupt .cto magic
    {
        let dir = TempDir::new("c10");
        let p = dir.write("b.ctg", &ctg);
        let mut bad_cto = cto.clone();
        bad_cto[0] ^= 0x01;
        dir.write("b.cto", &bad_cto);
        dir.write("b.ctb", &ctb);
        assert!(NativeBook::open(&p).is_err());
        assert_eq!(AdaptiveBook::open(&p).unwrap().mode(), LookupMode::Scan);
    }
    // 11. Corrupt .ctb magic
    {
        let dir = TempDir::new("c11");
        let p = dir.write("b.ctg", &ctg);
        let mut bad_ctb = ctb.clone();
        bad_ctb[3] ^= 0x01;
        dir.write("b.cto", &cto);
        dir.write("b.ctb", &bad_ctb);
        assert!(NativeBook::open(&p).is_err());
        assert_eq!(AdaptiveBook::open(&p).unwrap().mode(), LookupMode::Scan);
    }
    // 12. Invalid .ctb bounds (low > high, low >= slots)
    {
        let dir = TempDir::new("c12");
        let p = dir.write("b.ctg", &ctg);
        let mut bad_ctb = ctb.clone();
        bad_ctb[4..8].copy_from_slice(&9u32.to_be_bytes()); // low=9 > high=8
        dir.write("b.cto", &cto);
        dir.write("b.ctb", &bad_ctb);
        assert!(NativeBook::open(&p).is_err());
        assert_eq!(AdaptiveBook::open(&p).unwrap().mode(), LookupMode::Scan);
    }
    // 13. Out-of-range page reference in .cto
    {
        let dir = TempDir::new("c13");
        let p = dir.write("b.ctg", &ctg);
        let mut bad_cto = cto.clone();
        bad_cto[16..20].copy_from_slice(&99u32.to_be_bytes());
        dir.write("b.cto", &bad_cto);
        dir.write("b.ctb", &ctb);
        let nb = NativeBook::open(&p).unwrap();
        assert!(nb.validate().is_err());
        assert!(nb.probe(&root).is_err());
        let ab = AdaptiveBook::open(&p).unwrap();
        assert!(matches!(ab.probe(&root).unwrap(), NativeProbe::Found(_)));
        assert_eq!(ab.mode(), LookupMode::Scan);
    }
    // 14. Duplicate page reference in .cto
    {
        let dir = TempDir::new("c14");
        let p = dir.write("b.ctg", &ctg);
        let mut bad_cto = cto.clone();
        bad_cto[20..24].copy_from_slice(&0u32.to_be_bytes()); // two slots -> page 1
        dir.write("b.cto", &bad_cto);
        dir.write("b.ctb", &ctb);
        let nb = NativeBook::open(&p).unwrap();
        assert!(nb.validate().is_err());
        let ab = AdaptiveBook::open(&p).unwrap();
        assert_eq!(ab.probe(&missing).unwrap(), NativeProbe::Absent);
        assert_eq!(ab.mode(), LookupMode::Scan);
    }
    // 15. Unreferenced data page in .ctg + occupied slot < low
    {
        let dir = TempDir::new("c15");
        let mut two_pages = ctg.clone();
        two_pages.extend_from_slice(&ctg_page(&[]));
        let p = dir.write("b.ctg", &two_pages);
        dir.write("b.cto", &cto); // only points to data page 0, ignores page 1
        dir.write("b.ctb", &ctb);
        let nb = NativeBook::open(&p).unwrap();
        assert!(nb.validate().is_err());
        // Check validate_directory_bytes directly rejects slot < low
        let mut cto_below = cto.clone();
        cto_below[16..20].copy_from_slice(&0u32.to_be_bytes());
        let mut ctb_high = vec![0u8; 13];
        ctb_high[0..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
        ctb_high[4..8].copy_from_slice(&1u32.to_be_bytes()); // low = 1
        ctb_high[8..12].copy_from_slice(&8u32.to_be_bytes());
        assert!(validate_directory_bytes(2, &cto_below, &ctb_high).is_err());
    }
    // 16. Wrong collision route (slot 0 empty, slot 1 points to valid page 1, so misses startpos cleanly until validate falls back!)
    {
        let dir = TempDir::new("c16");
        let k0 = key_for(&root.to_fen());
        let mut two = build_ctg(&[ctg_page(&[ctg_entry(&k0, &[], 5, 2, 1, 2)]), ctg_page(&[])]);
        let _ = &mut two;
        let p = dir.write("b.ctg", &two);
        // Set low=1, high=8, slot 1 -> data page 0, slot 2 -> data page 1.
        // Put key in data page 0 even if native_hash(&k0) routes to slot 2! Or just put k0 on page 2 while routing slot 1/2 wrong:
        let h = cbvault_format::ctg::cto::native_hash(&k0);
        let wrong_slot = if (h & 1) == 0 { 2 } else { 1 };
        let right_slot = if wrong_slot == 2 { 1 } else { 2 };
        let mut bad_cto = vec![0u8; 16 + 7 * 4];
        bad_cto[0..4].copy_from_slice(&0x0010_0001u32.to_be_bytes());
        bad_cto[4..8].copy_from_slice(&7u32.to_be_bytes());
        for s in 0..7usize {
            bad_cto[16 + s * 4..20 + s * 4].copy_from_slice(&u32::MAX.to_be_bytes());
        }
        // Swap right_slot -> data page 1 (empty), wrong_slot -> data page 0 (holds k0)
        bad_cto[16 + right_slot * 4..20 + right_slot * 4].copy_from_slice(&1u32.to_be_bytes());
        bad_cto[16 + wrong_slot * 4..20 + wrong_slot * 4].copy_from_slice(&0u32.to_be_bytes());
        let mut ctb1 = vec![0u8; 13];
        ctb1[0..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
        ctb1[4..8].copy_from_slice(&1u32.to_be_bytes()); // low=1
        ctb1[8..12].copy_from_slice(&8u32.to_be_bytes()); // high=8
        dir.write("b.cto", &bad_cto);
        dir.write("b.ctb", &ctb1);

        let nb = NativeBook::open(&p).unwrap();
        assert_eq!(nb.probe(&root).unwrap(), NativeProbe::UnverifiedMiss);
        assert!(nb.validate().is_err()); // validate catches unroutable key!
        let ab = AdaptiveBook::open(&p).unwrap();
        // AdaptiveBook turns the false negative into validation -> fallback Scan -> Found!
        assert!(matches!(ab.probe(&root).unwrap(), NativeProbe::Found(_)));
        assert_eq!(ab.mode(), LookupMode::Scan);
    }
    // 17. Corrupt page bytes_used / unclosed page / bad key / bit flip inside entry
    {
        let dir = TempDir::new("c17");
        let mut bad = ctg.clone();
        bad[PAGE_SIZE + 3] = 2; // bytes_used = 2 < 4
        let p = dir.write("bad_used.ctg", &bad);
        assert_eq!(Book::open(&p).unwrap().damaged().len(), 1);
        assert!(!Book::open(&p).unwrap().verify(false).unwrap().ok(false));

        // Flip bits inside trailing entry so bytes_used doesn't close page cleanly
        let mut unclosed = ctg.clone();
        let used = u16::from_be_bytes([unclosed[PAGE_SIZE + 2], unclosed[PAGE_SIZE + 3]]);
        unclosed[PAGE_SIZE + 2..PAGE_SIZE + 4].copy_from_slice(&(used + 1).to_be_bytes());
        let p2 = dir.write("unclosed.ctg", &unclosed);
        let b2 = Book::open(&p2).unwrap();
        assert!(!b2.verify(false).unwrap().ok(false));
        assert!(b2.probe(&root).is_err()); // find_on_page validates entries.closed()!
    }
    // 18. Non-zero padding / bit7 set in key + unknown move byte + illegal move byte
    {
        let dir = TempDir::new("c18");
        let mut k = key_for(&root.to_fen());
        k[0] |= 0x80; // reserved bit 7
        let p = dir.write("bad_bit7.ctg", &build_ctg(&[ctg_page(&[ctg_entry(&k, &[], 1, 1, 0, 0)])]));
        let rep = Book::open(&p).unwrap().verify(false).unwrap();
        assert!(!rep.ok(false));
        assert_eq!(rep.key_mismatches, 1);

        let illegal = ctg_entry(&key_for(&root.to_fen()), &[(0x40, 0), (0x00, 0)], 1, 1, 0, 0);
        let p_mv = dir.write("bad_mv.ctg", &build_ctg(&[ctg_page(&[illegal])]));
        let rep2 = Book::open(&p_mv).unwrap().verify(false).unwrap();
        assert_eq!(rep2.moves_unknown, 1);
        assert_eq!(rep2.moves_illegal, 1);
        assert!(!rep2.ok(false));
    }
    // 19. Entries::parse never panics on short or malformed buffers
    for len in [0usize, 1, 3, 4, 16, 4096] {
        let data = vec![0xffu8; len];
        let _ = Entries::parse(&data, 1);
    }
}

#[test]
fn duplicate_keys_detected_by_verifier_and_resolved_by_lowest_page() {
    let dir = TempDir::new("dup_keys");
    let root = Board::startpos();
    let k0 = key_for(&root.to_fen());
    let h = cbvault_format::ctg::cto::native_hash(&k0);

    let p1 = ctg_page(&[ctg_entry(&k0, &[(0x5f, 0)], 11, 5, 3, 3)]);
    let p2 = ctg_page(&[ctg_entry(&k0, &[(0x5f, 0)], 99, 90, 5, 4)]);
    let ctg = build_ctg(&[p1, p2]);

    // Case A: Both pages are candidate slots for k0 (slot 0 -> page 1, level-1 child slot -> page 2).
    let mut cto = vec![0u8; 16 + 7 * 4];
    cto[0..4].copy_from_slice(&0x0010_0001u32.to_be_bytes());
    cto[4..8].copy_from_slice(&7u32.to_be_bytes());
    for s in 0..7usize {
        cto[16 + s * 4..20 + s * 4].copy_from_slice(&u32::MAX.to_be_bytes());
    }
    let level1_slot = (h & 1) as usize + 1;
    cto[16..20].copy_from_slice(&0u32.to_be_bytes()); // slot 0 -> page 1 (games=11)
    cto[16 + level1_slot * 4..20 + level1_slot * 4].copy_from_slice(&1u32.to_be_bytes()); // slot -> page 2 (games=99)
    let mut ctb = vec![0u8; 13];
    ctb[0..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
    ctb[4..8].copy_from_slice(&0u32.to_be_bytes()); // low = 0
    ctb[8..12].copy_from_slice(&8u32.to_be_bytes()); // high = 8
    ctb[12] = 0x01; // node 0 split indicator
    let path = dir.write("dup.ctg", &ctg);
    dir.write("dup.cto", &cto);
    dir.write("dup.ctb", &ctb);

    let scan = Book::open(&path).unwrap();
    let rep = scan.verify(false).unwrap();
    assert_eq!(rep.duplicate_keys, 1);
    assert!(!rep.ok(false));
    let p_scan = scan.probe(&root).unwrap().unwrap();
    assert_eq!(p_scan.stats.games, 11);

    // Native & Adaptive resolve candidate duplicate slots to the lowest physical page (page 1 -> games=11).
    let ab = AdaptiveBook::open(&path).unwrap();
    assert_eq!(ab.probe(&root).unwrap(), NativeProbe::Found(p_scan.clone()));

    // Case B: Cross-page duplicate where one copy sits on an unroutable slot triggers validation fallback.
    let dir_fb = TempDir::new("dup_keys_unroutable");
    let path_fb = dir_fb.write("dup_fb.ctg", &ctg);
    let (cto_fb, ctb_fb) = build_small_trie_cto_ctb(2);
    dir_fb.write("dup_fb.cto", &cto_fb);
    dir_fb.write("dup_fb.ctb", &ctb_fb);
    let ab_fb = AdaptiveBook::open(&path_fb).unwrap();
    ab_fb.validate().unwrap();
    assert_eq!(ab_fb.mode(), LookupMode::Scan);
    assert_eq!(ab_fb.probe(&root).unwrap(), NativeProbe::Found(p_scan));

    // Case C: Same-page duplicate keys are also caught and resolve deterministically to the first entry.
    let dir_same = TempDir::new("dup_same_page");
    let same_p1 = ctg_page(&[ctg_entry(&k0, &[(0x5f, 0)], 17, 7, 5, 5), ctg_entry(&k0, &[(0x5f, 0)], 88, 80, 4, 4)]);
    let same_ctg = build_ctg(&[same_p1]);
    let (same_cto, same_ctb) = build_small_trie_cto_ctb(1);
    let spath = dir_same.write("dup_same.ctg", &same_ctg);
    dir_same.write("dup_same.cto", &same_cto);
    dir_same.write("dup_same.ctb", &same_ctb);
    let same_scan = Book::open(&spath).unwrap();
    let same_rep = same_scan.verify(false).unwrap();
    assert_eq!(same_rep.duplicate_keys, 1);
    assert!(!same_rep.ok(false));
    assert_eq!(same_scan.probe(&root).unwrap().unwrap().stats.games, 17);
    let same_ab = AdaptiveBook::open(&spath).unwrap();
    assert_eq!(same_ab.probe(&root).unwrap(), NativeProbe::Found(same_scan.probe(&root).unwrap().unwrap()));
}

#[test]
fn concurrent_adaptive_book_validation_and_fallback_are_race_free() {
    // Part A: valid book -> 16 threads query hits and misses concurrently
    let dir = TempDir::new("concurrent_valid");
    let (ctg, cto, ctb) = book_with_root_and_e4();
    let path = dir.write("book.ctg", &ctg);
    dir.write("book.cto", &cto);
    dir.write("book.ctb", &ctb);
    let ab = Arc::new(AdaptiveBook::open(&path).unwrap());
    let scan = Arc::new(Book::open(&path).unwrap());
    let root = Board::startpos();
    let missing = parse_fen("7k/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
    let barrier = Arc::new(Barrier::new(16));
    let mut handles = Vec::new();
    for t in 0..16usize {
        let ab = Arc::clone(&ab);
        let scan = Arc::clone(&scan);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            for i in 0..50usize {
                if (t + i) % 2 == 0 {
                    assert_eq!(ab.probe(&root).unwrap(), NativeProbe::Found(scan.probe(&root).unwrap().unwrap()));
                } else {
                    assert_eq!(ab.probe(&missing).unwrap(), NativeProbe::Absent);
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(ab.mode(), LookupMode::NativeValidated);
    assert_eq!(ab.positions(), Some(3));

    // Part B: corrupt route -> 16 threads fallback concurrently without torn state or false negatives
    let dir2 = TempDir::new("concurrent_fallback");
    let path2 = dir2.write("bad.ctg", &ctg);
    let mut bad_cto = cto;
    bad_cto[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    dir2.write("bad.cto", &bad_cto);
    dir2.write("bad.ctb", &ctb);
    let ab2 = Arc::new(AdaptiveBook::open(&path2).unwrap());
    let barrier2 = Arc::new(Barrier::new(16));
    let mut handles2 = Vec::new();
    for _ in 0..16usize {
        let ab2 = Arc::clone(&ab2);
        let barrier2 = Arc::clone(&barrier2);
        handles2.push(std::thread::spawn(move || {
            barrier2.wait();
            assert!(matches!(ab2.probe(&root).unwrap(), NativeProbe::Found(_)));
            assert_eq!(ab2.mode(), LookupMode::Scan);
            assert!(ab2.fallback_reason().is_some());
        }));
    }
    for h in handles2 {
        h.join().unwrap();
    }
}

#[test]
fn explicit_book_modes_and_verified_fallback_match_scan() {
    use cbvault::book::ReadMode;
    let dir = TempDir::new("explicit-book-mode");
    let (ctg, cto, ctb) = book_with_root_and_e4();
    let path = dir.write("mode.ctg", &ctg);
    dir.write("mode.cto", &cto);
    dir.write("mode.ctb", &ctb);
    let low = AdaptiveBook::open_verified(&path, ReadMode::Positioned).unwrap();
    let fast = AdaptiveBook::open_verified(&path, ReadMode::MappedPreferred).unwrap();
    assert_eq!(low.read_mode(), ReadMode::Positioned);
    assert_eq!(fast.read_mode(), ReadMode::MappedPreferred);
    assert_eq!(low.mode(), LookupMode::NativeValidated);
    assert_eq!(low.probe_tree(&Board::startpos()).unwrap(), fast.probe_tree(&Board::startpos()).unwrap());
    drop((low, fast));
    fs::remove_file(path.with_extension("cto")).unwrap();
    let fallback = AdaptiveBook::open_verified(&path, ReadMode::Positioned).unwrap();
    assert_eq!(fallback.mode(), LookupMode::Scan);
    assert_eq!(fallback.read_mode(), ReadMode::Positioned);
    assert!(matches!(fallback.probe(&Board::startpos()).unwrap(), NativeProbe::Found(_)));
}

#[test]
fn verified_open_does_not_accept_unreachable_preferred_duplicate() {
    use cbvault::book::ReadMode;
    use cbvault_format::ctg::cto::native_hash;
    let dir = TempDir::new("stale-positive");
    let root = Board::startpos();
    let key = key_for(&root.to_fen());
    let ctg =
        build_ctg(&[ctg_page(&[ctg_entry(&key, &[], 11, 5, 3, 3)]), ctg_page(&[ctg_entry(&key, &[], 99, 90, 5, 4)])]);
    let (mut cto, ctb) = build_small_trie_cto_ctb(2);
    // Both pages have directory slots, but the preferred copy is on
    // the wrong branch. The native exact hit is not authoritative.
    let slot = (native_hash(&key) & 1) as usize + 1;
    let other = 3 - slot;
    cto[16 + slot * 4..20 + slot * 4].copy_from_slice(&1u32.to_be_bytes());
    cto[16 + other * 4..20 + other * 4].copy_from_slice(&0u32.to_be_bytes());
    let path = dir.write("stale.ctg", &ctg);
    dir.write("stale.cto", &cto);
    dir.write("stale.ctb", &ctb);
    let fast = AdaptiveBook::open_with_mode(&path, ReadMode::Positioned).unwrap();
    let NativeProbe::Found(hit) = fast.probe_stats(&root).unwrap() else { panic!("expected native hit") };
    assert_eq!(hit.stats.games, 99);
    let verified = AdaptiveBook::open_verified(&path, ReadMode::Positioned).unwrap();
    assert_eq!(verified.mode(), LookupMode::Scan);
    let NativeProbe::Found(hit) = verified.probe_stats(&root).unwrap() else { panic!("expected recovered hit") };
    assert_eq!(hit.stats.games, 11);
}

#[test]
fn index_free_stream_is_ordered_bounded_and_recovers_damage() {
    use cbvault::book::{BookStream, ReadMode, StreamEnd, StreamEvent, StreamOptions, StreamPolicy};
    use std::cell::Cell;
    let dir = TempDir::new("stream");
    let root = Board::startpos();
    let key = key_for(&root.to_fen());
    let entry = ctg_entry(&key, &[(0x5f, 0)], 10, 5, 2, 3);
    let valid = ctg_page(&[entry.clone(), entry.clone()]);
    let path = dir.write("stream.ctg", &build_ctg(&[valid.clone(), valid.clone()]));
    for mode in [ReadMode::MappedPreferred, ReadMode::Positioned] {
        let stream = BookStream::open_with_mode(&path, mode).unwrap();
        let mut offsets = Vec::new();
        let mut progress = 0;
        let stats = stream
            .visit(&StreamOptions { progress_every_pages: 1, ..Default::default() }, |event| {
                match event {
                    StreamEvent::Raw(e) => {
                        assert_eq!(e.key, key);
                        offsets.push(e.offset);
                    }
                    StreamEvent::Progress(s) => {
                        assert!(!s.complete());
                        progress += 1;
                    }
                    _ => panic!("unexpected event"),
                }
                true
            })
            .unwrap();
        assert!(stats.complete());
        assert_eq!((stats.pages, stats.entries, stats.delivered, progress), (2, 4, 4, 2));
        assert!(offsets.windows(2).all(|w| w[0] < w[1]));
        let decoded = stream
            .visit(&StreamOptions { decode: true, ..Default::default() }, |event| {
                let StreamEvent::Decoded { raw, board, position } = event else { panic!("expected decoded entry") };
                assert_eq!(raw.offset, position.offset);
                assert_eq!(board.zobrist(), root.zobrist());
                assert_eq!(position.moves[0].mv, Ok(san_to_move(&root, "e4").unwrap()));
                true
            })
            .unwrap();
        assert!(decoded.complete());
        let limited = stream.visit(&StreamOptions { max_entries: Some(1), ..Default::default() }, |_| true).unwrap();
        assert_eq!((limited.end, limited.delivered), (StreamEnd::EntryBudget, 1));
        let page = stream.visit(&StreamOptions { max_pages: Some(1), ..Default::default() }, |_| true).unwrap();
        assert_eq!((page.end, page.delivered), (StreamEnd::PageBudget, 2));
        let stop = Cell::new(false);
        let cancel = || stop.get();
        let cancelled = stream
            .visit(&StreamOptions { cancelled: Some(&cancel), ..Default::default() }, |_| {
                stop.set(true);
                true
            })
            .unwrap();
        assert_eq!((cancelled.end, cancelled.delivered), (StreamEnd::Cancelled, 1));
        let stopped = stream.visit(&StreamOptions::default(), |_| false).unwrap();
        assert_eq!((stopped.end, stopped.delivered), (StreamEnd::SinkStopped, 1));
    }
    let mut broken = valid.clone();
    broken[0..2].copy_from_slice(&3u16.to_be_bytes()); // claims a third entry
    let path = dir.write("broken.ctg", &build_ctg(&[broken, valid]));
    let stream = BookStream::open_with_mode(&path, ReadMode::Positioned).unwrap();
    assert!(stream.visit(&StreamOptions::default(), |_| true).is_err());
    let mut damage = 0;
    let stats = stream
        .visit(&StreamOptions { policy: StreamPolicy::Recover, ..Default::default() }, |event| {
            if let StreamEvent::Damage { page, .. } = event {
                assert_eq!(page, 1);
                damage += 1;
            }
            true
        })
        .unwrap();
    assert_eq!((stats.end, stats.errors, stats.delivered, damage), (StreamEnd::Exhausted, 1, 4, 1));
    assert!(!stats.complete());
    assert!(AdaptiveBook::open_verified(&path, ReadMode::Positioned).is_err());
}

#[test]
fn promotion_and_underpromotion_discovery_distinguishes_parent_continuations() {
    let dir = TempDir::new("underpromo-discovery");
    // 1. Queen promotion only (c7c8q via 0xec)
    let p_q = "4k3/2P5/8/8/8/8/8/6K1 w - - 0 1";
    let c_q = "2Q1k3/8/8/8/8/8/8/6K1 b - - 0 1";
    // 2. Knight underpromotion only (e7e8n omitted)
    let p_n = "7k/4P3/8/8/8/8/8/6K1 w - - 0 1";
    let c_n = "4N2k/8/8/8/8/8/8/6K1 b - - 0 1";
    // 3. Rook underpromotion only (d7d8r omitted)
    let p_r = "7k/3P4/8/8/8/8/8/6K1 w - - 0 1";
    let c_r = "3R3k/8/8/8/8/8/8/6K1 b - - 0 1";
    // 4. Bishop underpromotion only (b7b8b omitted)
    let p_b = "7k/1P6/8/8/8/8/8/6K1 w - - 0 1";
    let c_b = "1B5k/8/8/8/8/8/8/6K1 b - - 0 1";
    // 5a. Multiple choices from same parent: Q encoded (0xec) + N & R children present
    let p_multi_q = "2k5/5P2/8/8/8/8/8/6K1 w - - 0 1";
    let c_mq_q = "2k2Q2/8/8/8/8/8/8/6K1 b - - 0 1";
    let c_mq_n = "2k2N2/8/8/8/8/8/8/6K1 b - - 0 1";
    let c_mq_r = "2k2R2/8/8/8/8/8/8/6K1 b - - 0 1";
    // 5b. Multiple underpromotions without Q (N & B omitted on g7g8)
    let p_multi_u = "1k6/6P1/8/8/8/8/8/6K1 w - - 0 1";
    let c_mu_n = "1k4N1/8/8/8/8/8/8/6K1 b - - 0 1";
    let c_mu_b = "1k4B1/8/8/8/8/8/8/6K1 b - - 0 1";
    // 6a. Capture promotions (e7xf8q encoded via 0x8b; e7xf8r & e7xd8n omitted; child sum > parent games)
    let p_cap = "3n1r1k/4P3/8/8/8/8/8/6K1 w - - 0 1";
    let c_cap_q = "3n1Q1k/8/8/8/8/8/8/6K1 b - - 0 1";
    let c_cap_r = "3n1R1k/8/8/8/8/8/8/6K1 b - - 0 1";
    let c_cap_n = "3N1r1k/8/8/8/8/8/8/6K1 b - - 0 1";
    // 6b. Black-to-move capture underpromotion (e2xd1n omitted; distinct king squares from 3-piece WTM cases)
    let p_blk = "8/k7/8/8/8/8/4p3/3R3K b - - 0 1";
    let c_blk_n = "8/k7/8/8/8/8/8/3n3K w - - 0 1";

    let entries = vec![
        ctg_entry(&key_for(p_q), &[(0xec, 0)], 10, 5, 2, 3),
        ctg_entry(&key_for(c_q), &[], 10, 2, 5, 3),
        ctg_entry(&key_for(p_n), &[], 3, 1, 1, 1),
        ctg_entry(&key_for(c_n), &[], 3, 1, 1, 1),
        ctg_entry(&key_for(p_r), &[], 2, 0, 1, 1),
        ctg_entry(&key_for(c_r), &[], 2, 1, 0, 1),
        ctg_entry(&key_for(p_b), &[], 1, 0, 0, 1),
        ctg_entry(&key_for(c_b), &[], 1, 0, 0, 1),
        ctg_entry(&key_for(p_multi_q), &[(0xec, 0)], 8, 3, 3, 2),
        ctg_entry(&key_for(c_mq_q), &[], 5, 2, 2, 1),
        ctg_entry(&key_for(c_mq_n), &[], 2, 1, 1, 0),
        ctg_entry(&key_for(c_mq_r), &[], 1, 0, 0, 1),
        ctg_entry(&key_for(p_multi_u), &[], 4, 2, 1, 1),
        ctg_entry(&key_for(c_mu_n), &[], 3, 1, 1, 1),
        ctg_entry(&key_for(c_mu_b), &[], 1, 0, 1, 0),
        ctg_entry(&key_for(p_cap), &[(0x8b, 0)], 5, 2, 1, 2),
        ctg_entry(&key_for(c_cap_q), &[], 4, 1, 2, 1),
        ctg_entry(&key_for(c_cap_r), &[], 2, 0, 1, 1),
        ctg_entry(&key_for(c_cap_n), &[], 1, 0, 0, 1),
        ctg_entry(&key_for(p_blk), &[], 2, 1, 0, 1),
        ctg_entry(&key_for(c_blk_n), &[], 2, 0, 1, 1),
    ];
    let ctg = build_ctg(&[ctg_page(&entries)]);
    let (cto, ctb) = build_small_trie_cto_ctb(1);
    let path = dir.write("underpromo.ctg", &ctg);
    dir.write("underpromo.cto", &cto);
    dir.write("underpromo.ctb", &ctb);

    let scan = Book::open(&path).unwrap();
    let r = scan.verify(true).unwrap();
    assert!(r.ok(true), "{r:?}");
    assert_eq!(r.promotions, 3); // strictly c7c8q, f7f8q, e7xf8q

    let native = NativeBook::open(&path).unwrap();
    assert_eq!(native.validate().unwrap(), entries.len() as u64);
    let adaptive = AdaptiveBook::open(&path).unwrap();
    assert_eq!(adaptive.mode(), LookupMode::NativeUnverified);

    let check = |fen: &str, expected: &[(&str, bool, u32)]| {
        let b = parse_fen(fen).unwrap();
        let s_res = scan.probe_promotion_children(&b).unwrap().unwrap();
        let NativeProbe::Found(n_res) = native.probe_promotion_children(&b).unwrap() else { panic!() };
        let NativeProbe::Found(a_res) = adaptive.probe_promotion_children(&b).unwrap() else { panic!() };
        assert_eq!(s_res, n_res);
        assert_eq!(n_res, a_res);

        // Stored parent continuations must strictly reflect parent bytes (never omitted underpromotions).
        let tree = scan.probe_tree(&b).unwrap().unwrap();
        let encoded_count = expected.iter().filter(|&&(_, enc, _)| enc).count();
        assert_eq!(tree.parent.moves.len(), encoded_count);
        for m in &tree.parent.moves {
            assert_eq!(m.mv.unwrap().promotion(), Some(Role::Queen));
        }

        let got: Vec<(String, bool, u32)> = s_res
            .iter()
            .map(|d: &DiscoveredPromotion| (d.mv.to_string(), d.encoded_in_parent, d.child.stats.games))
            .collect();
        for &(uci, enc, g) in expected {
            assert!(got.contains(&(uci.to_string(), enc, g)), "{fen}: missing ({uci}, {enc}, {g}) in {got:?}");
        }
        assert_eq!(got.len(), expected.len(), "{fen}: unexpected count in {got:?}");
    };

    check(p_q, &[("c7c8q", true, 10)]);
    // Escalates unverified AdaptiveBook on first missing candidate cleanly:
    check(p_n, &[("e7e8n", false, 3)]);
    assert_eq!(adaptive.mode(), LookupMode::NativeValidated);
    check(p_r, &[("d7d8r", false, 2)]);
    check(p_b, &[("b7b8b", false, 1)]);
    check(p_multi_q, &[("f7f8q", true, 5), ("f7f8r", false, 1), ("f7f8n", false, 2)]);
    check(p_multi_u, &[("g7g8b", false, 1), ("g7g8n", false, 3)]);
    check(p_cap, &[("e7f8q", true, 4), ("e7f8r", false, 2), ("e7d8n", false, 1)]);
    check(p_blk, &[("e2d1n", false, 2)]);

    let missing_parent = parse_fen("7k/8/8/8/8/8/P7/6K1 w - - 0 1").unwrap();
    assert!(scan.probe_promotion_children(&missing_parent).unwrap().is_none());
    assert!(matches!(adaptive.probe_promotion_children(&missing_parent).unwrap(), NativeProbe::Absent));
}

#[test]
fn strict_ctb_bitmap_audit_rejects_changed_split_bits() {
    use cbvault::book::ReadMode;
    use cbvault_format::ctg::{Ctg, cto::NativeDirectory};
    let dir = TempDir::new("ctb-bitmap");
    let (ctg, cto, ctb) = book_with_root_and_e4();
    let path = dir.write("bitmap.ctg", &ctg);
    dir.write("bitmap.cto", &cto);
    dir.write("bitmap.ctb", &ctb);
    for mode in [ReadMode::MappedPreferred, ReadMode::Positioned] {
        let reader = Ctg::open_with_mode(&path, mode).unwrap();
        let directory = NativeDirectory::open(&reader).unwrap();
        assert_eq!(directory.validate_bitmap().unwrap(), 1);
    }
    let mut corrupt = ctb.clone();
    corrupt[12] ^= 1;
    dir.write("bitmap.ctb", &corrupt);
    let reader = Ctg::open_with_mode(&path, ReadMode::Positioned).unwrap();
    let directory = NativeDirectory::open(&reader).unwrap();
    assert!(directory.validate_bitmap().is_err());
    // Header/size checks alone cannot detect the changed bit.
    assert!(directory.validate_pages().is_ok());
}
