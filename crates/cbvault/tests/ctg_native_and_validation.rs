//! Comprehensive CTG/CTO/CTB validation, corruption, concurrency,
//! differential parity, duplicate-key, and small-geometry regression tests.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

use cbvault::book::{AdaptiveBook, Book, LookupMode, NativeBook, NativeProbe};
use cbvault_chess::ctg::{key_candidates, stored_form};
use cbvault_format::ctg::cto::validate_directory_bytes;
use cbvault_format::ctg::{Entries, PAGE_SIZE};
use gigachess::Board;
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
