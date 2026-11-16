// Copyright (c) 2026 the cbvault contributors (MIT License). See LICENSE.

//! Deterministic synthetic CTG (ChessBase opening book) fixture generator for tests.
//!
//! Generates minimal, license-safe binary `.ctg` files in memory without copying
//! any proprietary byte sequences from commercial opening books.

use cbvault_chess::ctg as chess_ctg;
use cbvault_format::ctg::{PAGE_SIZE, STATS_LEN, Stats};
use gigachess::Board;
use std::path::{Path, PathBuf};

use crate::TempDb;

/// Builder for synthetic CTG binary files.
#[derive(Clone, Debug)]
pub struct CtgBuilder {
    bytes: Vec<u8>,
    current_page: usize,
    page_counts: Vec<u16>,
    page_used: Vec<u16>,
}

impl Default for CtgBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CtgBuilder {
    /// Creates a new builder initialized with an empty Page 0 (4096-byte header).
    pub fn new() -> Self {
        let mut bytes = vec![0u8; PAGE_SIZE]; // Page 0
        // Optional dummy signature/metadata on page 0
        bytes[0] = 0x01;
        Self { bytes, current_page: 0, page_counts: vec![0], page_used: vec![0] }
    }

    /// Adds a new data page (4096 bytes) initialized with count=0 and used=4.
    pub fn add_page(&mut self) -> usize {
        let page_idx = self.page_counts.len();
        self.bytes.resize(self.bytes.len() + PAGE_SIZE, 0);
        self.page_counts.push(0);
        self.page_used.push(4);
        self.current_page = page_idx;
        self.sync_header(page_idx);
        page_idx
    }

    /// Number of pages currently in the builder (including Page 0).
    pub fn pages(&self) -> usize {
        self.page_counts.len()
    }

    /// Appends a raw CTG entry to `page_idx`.
    pub fn add_raw_entry(&mut self, page_idx: usize, key: &[u8], moves: &[(u8, u8)], stats: &Stats) {
        assert!(page_idx > 0 && page_idx < self.page_counts.len(), "invalid data page index");
        let page_start = page_idx * PAGE_SIZE;
        let used = self.page_used[page_idx] as usize;
        let entry_start = page_start + used;

        let moves_len = 1 + moves.len() * 2;
        let entry_len = key.len() + moves_len + STATS_LEN;
        assert!(used + entry_len <= PAGE_SIZE, "entry exceeds page capacity");

        let mut entry_bytes = Vec::with_capacity(entry_len);
        // 1. Key bytes
        entry_bytes.extend_from_slice(key);
        // 2. Move section: length byte (1 + 2n) followed by (move_byte, annot_byte) pairs
        entry_bytes.push(moves_len as u8);
        for &(mv, annot) in moves {
            entry_bytes.push(mv);
            entry_bytes.push(annot);
        }
        // 3. Stats section: 33 bytes
        let mut stats_bytes = [0u8; STATS_LEN];
        write_stats(stats, &mut stats_bytes);
        entry_bytes.extend_from_slice(&stats_bytes);

        // Write into page
        let at = entry_start;
        self.bytes[at..at + entry_bytes.len()].copy_from_slice(&entry_bytes);

        self.page_counts[page_idx] += 1;
        self.page_used[page_idx] += entry_len as u16;
        self.sync_header(page_idx);
    }

    /// Encodes a `gigachess::Board` into its CTG key and appends the entry to `page_idx`.
    pub fn add_position(&mut self, page_idx: usize, board: &Board, moves: &[(u8, u8)], stats: &Stats) {
        let stored = chess_ctg::stored_form(board);
        let keys = chess_ctg::keys(&stored);
        assert!(!keys.is_empty(), "board must yield at least one key");
        let (key_buf, key_len) = keys[0];
        self.add_raw_entry(page_idx, &key_buf[..key_len], moves, stats);
    }

    /// Directly overwrites the 4-byte header of `page_idx` with arbitrary `count` and `used`.
    pub fn set_page_header(&mut self, page_idx: usize, count: u16, used: u16) {
        assert!(page_idx > 0 && page_idx < self.page_counts.len());
        self.page_counts[page_idx] = count;
        self.page_used[page_idx] = used;
        self.sync_header(page_idx);
    }

    /// Mutates `page_idx` into an unclosed malformed page (Bug A reproduction):
    /// declares `count = 0` but `used = fake_used > 4`.
    pub fn corrupt_unclosed_page(&mut self, page_idx: usize, fake_used: u16) {
        self.set_page_header(page_idx, 0, fake_used.clamp(4, PAGE_SIZE as u16));
    }

    /// Truncates the total binary to `len` bytes.
    pub fn truncate(&mut self, len: usize) {
        self.bytes.truncate(len);
    }

    /// Builds and returns the final `.ctg` byte vector.
    pub fn build(self) -> Vec<u8> {
        self.bytes
    }

    /// Writes the `.ctg` file to `path`.
    pub fn write_to(&self, path: &Path) -> std::io::Result<PathBuf> {
        std::fs::write(path, &self.bytes)?;
        Ok(path.to_path_buf())
    }

    /// Writes the generated CTG file into `temp` as `<temp.base()>.ctg`.
    pub fn write_temp(&self, temp: &TempDb) -> PathBuf {
        temp.write(".ctg", &self.bytes)
    }

    fn sync_header(&mut self, page_idx: usize) {
        let at = page_idx * PAGE_SIZE;
        let count_bytes = self.page_counts[page_idx].to_be_bytes();
        let used_bytes = self.page_used[page_idx].to_be_bytes();
        self.bytes[at..at + 2].copy_from_slice(&count_bytes);
        self.bytes[at + 2..at + 4].copy_from_slice(&used_bytes);
    }
}

fn write_stats(s: &Stats, b: &mut [u8; STATS_LEN]) {
    // 0..3: games (u24 be)
    b[0] = (s.games >> 16) as u8;
    b[1] = (s.games >> 8) as u8;
    b[2] = s.games as u8;
    // 3..6: wins (u24 be)
    b[3] = (s.wins >> 16) as u8;
    b[4] = (s.wins >> 8) as u8;
    b[5] = s.wins as u8;
    // 6..9: losses (u24 be)
    b[6] = (s.losses >> 16) as u8;
    b[7] = (s.losses >> 8) as u8;
    b[8] = s.losses as u8;
    // 9..12: draws (u24 be)
    b[9] = (s.draws >> 16) as u8;
    b[10] = (s.draws >> 8) as u8;
    b[11] = s.draws as u8;
    // 12..16: raw_score (u32 be)
    b[12..16].copy_from_slice(&s.raw_score.to_be_bytes());
    // 16..19: rated_games (u24 be)
    b[16] = (s.rated_games >> 16) as u8;
    b[17] = (s.rated_games >> 8) as u8;
    b[18] = s.rated_games as u8;
    // 19..23: rating_sum (u32 be)
    b[19..23].copy_from_slice(&s.rating_sum.to_be_bytes());
    // 23..26: opponent_rated_games (u24 be)
    b[23] = (s.opponent_rated_games >> 16) as u8;
    b[24] = (s.opponent_rated_games >> 8) as u8;
    b[25] = s.opponent_rated_games as u8;
    // 26..30: opponent_rating_sum (u32 be)
    b[26..30].copy_from_slice(&s.opponent_rating_sum.to_be_bytes());
    // 30..33: raw_tail
    b[30..33].copy_from_slice(&s.raw_tail);
}

// ---------------------------------------------------------------------------
// Standard Deterministic Fixtures
// ---------------------------------------------------------------------------

/// Generates a minimal, perfectly valid 2-page CTG book (Page 0 + Page 1) containing
/// the standard initial position and move 1.e4.
pub fn minimal_valid_book() -> Vec<u8> {
    let mut builder = CtgBuilder::new();
    let p1 = builder.add_page();

    let start = Board::startpos();
    // 0x5f is 1.e4 (pawn ordinal 4, e-file steps 2)
    let stats = Stats {
        games: 1000,
        wins: 400,
        draws: 300,
        losses: 300,
        raw_score: 5500,
        rated_games: 800,
        rating_sum: 800 * 2400,
        opponent_rated_games: 800,
        opponent_rating_sum: 800 * 2380,
        raw_tail: [0, 0, 0],
    };
    builder.add_position(p1, &start, &[(0x5f, 0x00)], &stats);
    builder.build()
}

/// Generates a synthetic book with connected parent-child positions:
/// - Root: startpos -> 1.e4 (0x9b)
/// - Child: 1.e4 position -> 1...e5 (0xe6)
pub fn connected_parent_child_book() -> Vec<u8> {
    let mut builder = CtgBuilder::new();
    let p1 = builder.add_page();

    let mut board = Board::startpos();
    let root_stats = Stats { games: 100, wins: 40, draws: 30, losses: 30, ..Default::default() };
    // 0x5f is 1.e4 (pawn ordinal 4, e-file steps 2)
    builder.add_position(p1, &board, &[(0x5f, 0x00)], &root_stats);

    // Play 1.e4
    let mv_e4 = gigachess::san::san_to_move(&board, "e4").unwrap();
    board.play(mv_e4).unwrap();

    let child_stats = Stats { games: 60, wins: 25, draws: 20, losses: 15, ..Default::default() };
    // 0xe6 is 1...e5 for Black
    builder.add_position(p1, &board, &[(0xe6, 0x00)], &child_stats);

    builder.build()
}

/// Generates a synthetic book reproducing Bug A (unclosed page entries):
/// Page 1 has count=0 but used=100.
pub fn malformed_unclosed_book() -> Vec<u8> {
    let mut builder = CtgBuilder::new();
    let p1 = builder.add_page();
    builder.corrupt_unclosed_page(p1, 100);
    builder.build()
}

/// Generates a synthetic book with an unmapped unknown move byte (e.g. `0xff`).
pub fn unknown_move_byte_book() -> Vec<u8> {
    let mut builder = CtgBuilder::new();
    let p1 = builder.add_page();
    let start = Board::startpos();
    let stats = Stats { games: 10, wins: 5, draws: 3, losses: 2, ..Default::default() };
    // 0xff is not mapped in MOVE_TABLE
    builder.add_position(p1, &start, &[(0xff, 0x00)], &stats);
    builder.build()
}

/// Generates a synthetic book containing the 5 rare promoted-piece move vectors:
/// `0x89`, `0x2b`, `0x19`, `0xbc`, `0xb4`.
pub fn promoted_test_vectors_book() -> Vec<u8> {
    let mut builder = CtgBuilder::new();
    let p1 = builder.add_page();

    // 1. Vector 0x89: 3rd knight on g8 moving to h6
    let b1 = gigachess::fen::parse_fen("rnk1bbNr/pp4pp/8/2p5/4p3/2N5/PPPB1PPP/R3K1NR w KQ - 0 1").unwrap();
    builder.add_position(p1, &b1, &[(0x89, 0x00)], &Stats::default());

    // 2. Vector 0x2b: 3rd knight on f3 moving to e5
    let b2 = gigachess::fen::parse_fen("rNb1qk1r/ppQ1n2p/5b2/5pp1/2Bp4/5N2/PPP2PPP/RN2RK2 w - - 0 1").unwrap();
    builder.add_position(p1, &b2, &[(0x2b, 0x00)], &Stats::default());

    // 3. Vector 0x19: 3rd knight on f3 moving to d4
    let b3 = gigachess::fen::parse_fen("rNb3nr/pp2k1pp/8/1B3p2/3p4/4bN2/PPP3PP/RN2RK2 w - - 0 1").unwrap();
    builder.add_position(p1, &b3, &[(0x19, 0x00)], &Stats::default());

    // 4. Vector 0xbc: 2nd queen on f3 moving to c3
    let b4 = gigachess::fen::parse_fen("rnQ3nr/p3kppp/8/2b1p3/8/5Q2/PPqP1PPP/RNB1K1NR w KQ - 0 1").unwrap();
    builder.add_position(p1, &b4, &[(0xbc, 0x00)], &Stats::default());

    // 5. Vector 0xb4: 3rd knight on g8 moving to f6
    let b5 = gigachess::fen::parse_fen("rn2qbNr/pp4pp/1k6/2p2Q2/2b1pB2/2N5/PPP2PPP/3RK1NR w K - 0 1").unwrap();
    builder.add_position(p1, &b5, &[(0xb4, 0x00)], &Stats::default());

    builder.build()
}
