// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! LOCAL ONLY: 2CBH -> ChessBase PGN gold comparison.
//!
//! No real database, PGN, per-game content, name, or diff is shipped, logged to
//! public CI or committed. Both environment variables are mandatory:
//! CBVAULT_TEST_DB_2CBH=/absolute/local/database/stem
//! CBVAULT_2CBH_GOLD_PGN=/absolute/local/chessbase-export.pgn
//!
//! cargo run -p cbvault --release --example gold_2cbh -- --threads 4
//!
//! Unlike the historical classic comparator, this refuses unknown exceptions:
//! only line endings, tag order, common tags and whitespace between PGN tokens
//! are normalized. Z0, redundant SAN qualifiers, comments, NAGs, results and
//! variation shape remain literal, to be investigated against actual games.
//! The report contains counts and byte offsets, NEVER player names or excerpts.
//! This is a prepared *oracle harness*, not proof of parity without local data.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use cbvault::twocbh::Database;

const KEEP: [&str; 13] = [
    "Event", "Site", "Date", "Round", "White", "Black", "Result", "ECO", "WhiteElo", "BlackElo",
    "SetUp", "FEN", "Variant",
];
const CHUNK: usize = 1 << 18;

#[derive(Clone, Copy, Debug, Default)]
struct Fingerprint {
    bytes: u64,
    fnv64: u64,
}

fn fingerprint(path: &Path) -> io::Result<Fingerprint> {
    let mut reader = BufReader::with_capacity(CHUNK, File::open(path)?);
    let mut buf = [0u8; CHUNK];
    let mut result = Fingerprint { bytes: 0, fnv64: 0xcbf29ce484222325 };
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        for &b in &buf[..n] {
            result.fnv64 ^= u64::from(b);
            result.fnv64 = result.fnv64.wrapping_mul(0x100000001b3);
        }
        result.bytes += n as u64;
    }
    Ok(result)
}

/// First offset where two raw PGN streams differ; None if byte-identical.
fn raw_mismatch(a: &Path, b: &Path) -> io::Result<Option<u64>> {
    let mut left = BufReader::new(File::open(a)?);
    let mut right = BufReader::new(File::open(b)?);
    let mut offset = 0;
    let (mut a_buf, mut b_buf) = ([0u8; CHUNK], [0u8; CHUNK]);
    loop {
        let na = left.read(&mut a_buf)?;
        let nb = right.read(&mut b_buf)?;
        let overlap = na.min(nb);
        for (i, (&x, &y)) in a_buf[..overlap].iter().zip(&b_buf[..overlap]).enumerate() {
            if x != y { return Ok(Some(offset + i as u64)); }
        }
        if na != nb { return Ok(Some(offset + overlap as u64)); }
        if na == 0 { return Ok(None); }
        offset += na as u64;
    }
}

/// One game at a time, without retaining the database's PGN in memory.
/// A blank line followed by a new tag outside a brace comment separates
/// games. A pending line is retained when that boundary is found.
struct GameStream<R: BufRead> {
    reader: R,
    pending: Vec<u8>,
    next: Vec<u8>,
    done: bool,
}

impl<R: BufRead> GameStream<R> {
    fn new(reader: R) -> Self {
        Self { reader, pending: Vec::new(), next: Vec::new(), done: false }
    }
    fn read_game(&mut self, out: &mut Vec<u8>) -> io::Result<bool> {
        out.clear();
        if self.done { return Ok(false); }
        let mut in_comment = 0i32;
        let mut seen_movetext = false;
        let mut previous_blank = false;
        loop {
            self.next.clear();
            if !self.pending.is_empty() {
                std::mem::swap(&mut self.pending, &mut self.next);
            } else if self.reader.read_until(b'\n', &mut self.next)? == 0 {
                self.done = true;
                return Ok(!out.is_empty());
            }
            let line = trim(&self.next);
            if line.is_empty() && out.is_empty() { continue; }
            let next_tag = line.first() == Some(&b'[');
            if previous_blank && in_comment == 0 && seen_movetext && next_tag {
                std::mem::swap(&mut self.pending, &mut self.next);
                return Ok(true);
            }
            if !line.is_empty() && !next_tag { seen_movetext = true; }
            previous_blank = line.is_empty();
            // Tag values may contain braces as text, so count comment braces
            // only in non-header PGN lines.
            if !next_tag || in_comment != 0 {
                for &b in line {
                    if b == b'{' { in_comment += 1; }
                    if b == b'}' { in_comment -= 1; }
                }
                in_comment = in_comment.max(0);
            }
            out.extend_from_slice(&self.next);
        }
    }
}

fn trim(raw: &[u8]) -> &[u8] {
    let a = raw.iter().position(|v| !v.is_ascii_whitespace()).unwrap_or(raw.len());
    let b = raw.iter().rposition(|v| !v.is_ascii_whitespace()).map_or(a, |i| i + 1);
    &raw[a..b]
}

/// Normalize allowed PGN formatting only: order of comparable tags, CRLF,
/// whitespace between move tokens and whitespace *inside* brace comments.
/// No SAN, result, NAG or variation tokens are dropped or rewritten.
fn canonical(raw: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    let text = text.trim_start_matches('\u{feff}');
    let mut tags = BTreeMap::<String, String>::new();
    let mut moves = String::new();
    let mut in_tags = true;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if in_tags && line.starts_with('[') && line.ends_with(']') {
            if let Some((key, value)) = tag(line)
                && KEEP.contains(&key.as_str())
            {
                tags.insert(key, value);
            }
        } else if !line.trim().is_empty() {
            in_tags = false;
            moves.push_str(line);
            moves.push('\n');
        }
    }
    let mut out = Vec::with_capacity(raw.len());
    for (k, v) in tags {
        out.extend_from_slice(k.as_bytes());
        out.push(b'=');
        out.extend_from_slice(v.as_bytes());
        out.push(b'\n');
    }
    out.extend_from_slice(b"--MOVES--\n");
    let mut token = String::new();
    let mut chars = moves.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                finish_token(&mut out, &mut token);
                out.push(b'{');
                let mut last_space = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        if out.last() == Some(&b' ') { out.pop(); }
                        out.push(b'}');
                        break;
                    }
                    if inner.is_whitespace() {
                        if !last_space { out.push(b' '); }
                        last_space = true;
                    } else {
                        let mut b = [0; 4];
                        out.extend_from_slice(inner.encode_utf8(&mut b).as_bytes());
                        last_space = false;
                    }
                }
                out.push(b'\n');
            }
            '(' | ')' => {
                finish_token(&mut out, &mut token);
                out.push(c as u8);
                out.push(b'\n');
            }
            _ if c.is_whitespace() => finish_token(&mut out, &mut token),
            _ => token.push(c),
        }
    }
    finish_token(&mut out, &mut token);
    out
}

fn finish_token(out: &mut Vec<u8>, tok: &mut String) {
    if !tok.is_empty() {
        out.extend_from_slice(tok.as_bytes());
        out.push(b'\n');
        tok.clear();
    }
}

fn tag(line: &str) -> Option<(String, String)> {
    let text = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = text.split_once(' ')?;
    let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut unescaped = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' { unescaped.push(chars.next()?); } else { unescaped.push(c); }
    }
    Some((key.to_owned(), unescaped))
}

struct RemoveOnDrop(PathBuf);
impl Drop for RemoveOnDrop {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let stem = PathBuf::from(std::env::var("CBVAULT_TEST_DB_2CBH")
        .map_err(|_| "set CBVAULT_TEST_DB_2CBH to a local full 2CBH database stem")?);
    let gold = PathBuf::from(std::env::var("CBVAULT_2CBH_GOLD_PGN")
        .map_err(|_| "set CBVAULT_2CBH_GOLD_PGN to the matching ChessBase PGN")?);
    let mut threads = 1usize;
    let mut batch = 8192usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--threads" => threads = args.next().ok_or("missing --threads value")?.parse()?,
            "--batch-size" => batch = args.next().ok_or("missing --batch-size value")?.parse()?,
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    let db = Database::open(&stem)?;
    let path = std::env::temp_dir().join(format!("cbvault-2cbh-gold-{}.pgn", std::process::id()));
    let _cleanup = RemoveOnDrop(path.clone());
    let mut file = BufWriter::with_capacity(1 << 20, File::create(&path)?);
    let started = Instant::now();
    let stats = db.export_parallel(&mut file, 1, 0, threads, batch)?;
    file.flush()?;
    drop(file);
    let elapsed = started.elapsed().as_secs_f64();
    let ours_fp = fingerprint(&path)?;
    let gold_fp = fingerprint(&gold)?;
    let raw_diff = raw_mismatch(&path, &gold)?;

    let mut ours = GameStream::new(BufReader::with_capacity(CHUNK, File::open(&path)?));
    let mut expected = GameStream::new(BufReader::with_capacity(CHUNK, File::open(&gold)?));
    let (mut ours_buf, mut expected_buf) = (Vec::new(), Vec::new());
    let (mut games, mut equal) = (0u64, 0u64);
    let mut differing_tags = 0u64;
    let mut differing_moves = 0u64;
    let mut first_differences = Vec::with_capacity(20);
    loop {
        let actual = ours.read_game(&mut ours_buf)?;
        let reference = expected.read_game(&mut expected_buf)?;
        if !actual && !reference { break; }
        games += 1;
        if !(actual && reference) {
            if first_differences.len() < 20 { first_differences.push(games); }
            differing_moves += 1;
            continue;
        }
        let ours_canon = canonical(&ours_buf);
        let gold_canon = canonical(&expected_buf);
        if ours_canon == gold_canon { equal += 1; }
        else {
            let marker = b"--MOVES--\n";
            let our_tags_end = ours_canon.windows(marker.len()).position(|w| w == marker);
            let ref_tags_end = gold_canon.windows(marker.len()).position(|w| w == marker);
            let tags_match = match (our_tags_end, ref_tags_end) {
                (Some(a), Some(b)) => ours_canon[..a] == gold_canon[..b],
                _ => false,
            };
            if !tags_match { differing_tags += 1; }
            if our_tags_end.is_none_or(|a| ref_tags_end.is_none_or(|b| ours_canon[a..] != gold_canon[b..])) {
                differing_moves += 1;
            }
            if first_differences.len() < 20 { first_differences.push(games); }
        }
    }
    println!("2CBH local ChessBase PGN gold gate");
    println!("records={} games={} verified={} deleted={} texts={} analyses={}",
        stats.records, stats.games, stats.verified, stats.deleted, stats.texts, stats.analyses);
    println!("gold_bytes={} our_bytes={} gold_fnv64={:016x} our_fnv64={:016x}",
        gold_fp.bytes, ours_fp.bytes, gold_fp.fnv64, ours_fp.fnv64);
    println!("raw_first_mismatch_byte={raw_diff:?}");
    println!("compared_games={games} canonical_equal={equal} tag_diffs={differing_tags} movetext_diffs={differing_moves}");
    println!("first_diff_game_ordinals={first_differences:?} (no game text printed)");
    println!("export_seconds={elapsed:.2} threads={threads}");
    if differing_tags != 0 || differing_moves != 0 || equal != games || games != stats.games {
        return Err("local 2CBH gold gate FAILED: unexplained canonical differences or missing games".into());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("local 2CBH gold gate: {e}"); ExitCode::FAILURE }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_retains_comments_variations_nags_and_results() {
        let one = b"[White \"Smith\"]\r\n[Event \"A\"]\r\n\r\n1. e4 {a   comment} (1. d4) $1 1-0\r\n";
        let two = b"[Event \"A\"]\n[White \"Smith\"]\n\n1.  e4 {a comment} (1.  d4) $1 1-0\n";
        assert_eq!(canonical(one), canonical(two));
        assert_ne!(canonical(one), canonical(b"[Event \"A\"]\n[White \"Smith\"]\n\n1. e4 1-0\n"));
    }
    #[test]
    fn streaming_split_respects_blank_lines_inside_comments() {
        let source = b"[Event \"A\"]\n\n1. e4 {hi\n\nthere} e5 1-0\n\n[Event \"B\"]\n\n1. d4 0-1\n";
        let mut r = GameStream::new(BufReader::new(&source[..]));
        let mut buf = Vec::new();
        assert!(r.read_game(&mut buf).unwrap());
        assert!(std::str::from_utf8(&buf).unwrap().contains("there"));
        assert!(r.read_game(&mut buf).unwrap());
        assert!(std::str::from_utf8(&buf).unwrap().contains("[Event \"B\"]"));
        assert!(!r.read_game(&mut buf).unwrap());
    }
}
