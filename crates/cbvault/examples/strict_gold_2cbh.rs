// Copyright (c) 2026 the cbvault contributors. MIT License.
//! STRICT, LOCAL-ONLY 2CBH vs ChessBase PGN oracle. Never upload private data.
//!
//! Usage:
//! CBVAULT_TEST_DB_2CBH=/private/stem CBVAULT_2CBH_GOLD_PGN=/private/gold.pgn \
//!     cargo run -p cbvault --release --example strict_gold_2cbh
//!
//! This program EXITS NONZERO for any unapproved byte, skipped/failed game,
//! malformed PGN, missing record, or count/EOF discrepancy. It reports BOTH
//! raw stream SHA-256 and canonical token-byte equality. The sole enabled
//! representation exceptions are BOARD-VERIFIED SAN overqualification and Z0
//! versus -- on a legally played null-move node. No comments/tags/annotations
//! are suppressed. ChessBase-specific Unicode/glyph/clock forms need new,
//! separately tested, narrowly reviewed transformations before a gold pass.
//!
//! This is intentionally stricter than the legacy gold_pgn diagnostic. An
//! initial failure is useful evidence, not permission to widen normalization.
//! Mapping uses source record order and complete comparable game content; any
//! order/identity discrepancy fails. Exact duplicate-game source identity
//! cannot be proven from PGN alone and needs a local ChessBase export manifest.

use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};

use cbvault::bridge::{Database, GameBuf, GameHeaderRef};
use cbvault_format::game::RecordKind;
use gigachess::san::san_to_move;
use gigachess::{Board, Move};

fn main() {
    if let Err(e) = run() {
        eprintln!("strict 2CBH gold: FAIL: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let source = std::env::var_os("CBVAULT_TEST_DB_2CBH")
        .map(PathBuf::from)
        .ok_or("CBVAULT_TEST_DB_2CBH is required: private local database stem")?;
    let gold_path = std::env::var_os("CBVAULT_2CBH_GOLD_PGN")
        .map(PathBuf::from)
        .ok_or("CBVAULT_2CBH_GOLD_PGN is required: matching private ChessBase PGN")?;
    let source = cbvault::stem_of(&source);
    let src_ext = ["2cbh", "2cbg", "2lid", "2cba"];
    let mut snapshots = Vec::new();
    for ext in src_ext {
        let p = source.with_extension(ext);
        if !p.exists() {
            if ext == "2cba" { continue; }
            return Err(format!("required 2CBH companion .{ext} is missing"));
        }
        let (len, sha) = file_digest(&p)?;
        snapshots.push((p, len, sha));
    }
    let (gold_len, gold_hash) = file_digest(&gold_path)?;
    let db = Database::open(source.with_extension("2cbh")).map_err(|e| e.to_string())?;
    if db.generation() != cbvault::bridge::Generation::TwoCbh {
        return Err("database generation is not TwoCbh".into());
    }
    let file = std::fs::File::open(&gold_path).map_err(|e| e.to_string())?;
    let mut gold = Games::new(std::io::BufReader::with_capacity(1 << 20, file));
    let mut buffer = GameBuf::new();
    let mut ours_sha = Sha256::new();
    let mut ours_len = 0u64;
    let mut raw_first = None;
    let mut gold_raw_offset = 0u64;
    let mut count = 0u64;
    let mut san_exceptions = 0u64;
    let mut null_exceptions = 0u64;
    let mut differing_games = 0u64;
    let mut first_diff = None;
    let mut non_game_slots = 0u64;
    let mut canonical_bytes = 0u64;

    for id in 1..=db.records() {
        let header: GameHeaderRef<'_> = db.header_ref(id).map_err(|e| format!("header id {id}: {e}"))?;
        if header.is_deleted() || !matches!(header.kind(), RecordKind::Game | RecordKind::Analysis) {
            non_game_slots += 1;
            continue;
        }
        let gold_game = gold.next_game()?.ok_or_else(|| {
            format!("ChessBase PGN ended early before eligible source record ordinal {}", count + 1)
        })?;
        let our_text = db.game_pgn_with(id, &mut buffer, true)
            .map_err(|e| format!("error exporting source record {id}: {e}"))?;
        ours_sha.update(our_text.as_bytes());
        ours_len += our_text.len() as u64;
        if raw_first.is_none() {
            let a = gold_game.raw.as_bytes();
            let b = our_text.as_bytes();
            let mismatch = a.iter().zip(b).position(|(x, y)| x != y);
            if let Some(at) = mismatch.or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len()))) {
                raw_first = Some(gold_raw_offset + at as u64);
            }
        }
        gold_raw_offset += gold_game.raw.len() as u64;
        let ours_game = parse_game(our_text)?;
        let compare = compare_game(&gold_game, &ours_game)?;
        san_exceptions += compare.san;
        null_exceptions += compare.nulls;
        canonical_bytes += compare.bytes;
        count += 1;
        if !compare.equal {
            differing_games += 1;
            if first_diff.is_none() {
                first_diff = Some((id, count, compare.first_byte));
            }
        }
        if count % 100_000 == 0 {
            eprintln!("strict gold processed {count} games, unexplained mismatching games: {differing_games}");
        }
    }
    if gold.next_game()?.is_some() {
        return Err("ChessBase gold contains extra games after the last eligible source record".into());
    }
    for (p, before_len, before_sha) in snapshots {
        let (after_len, after_sha) = file_digest(&p)?;
        if (before_len, before_sha) != (after_len, after_sha) {
            return Err("original source companion changed during verification".into());
        }
    }
    let (end_gold_len, end_gold_hash) = file_digest(&gold_path)?;
    if (gold_len, gold_hash) != (end_gold_len, end_gold_hash) {
        return Err("ChessBase PGN changed during verification".into());
    }
    let our_hash = ours_sha.finish();
    let is_raw_equal = gold_len == ours_len && gold_hash == our_hash;
    eprintln!(
        "strict gold: source_slots={} non_game_slots={} games={} canonical_bytes={} unexplained_games={} verified_san={} verified_null={} raw_equal={} gold_raw_bytes={} ours_raw_bytes={} raw_first_diff_offset={:?} first_unexplained={:?}",
        db.records(), non_game_slots, count, canonical_bytes, differing_games,
        san_exceptions, null_exceptions, is_raw_equal, gold_len, ours_len,
        raw_first, first_diff
    );
    eprintln!("strict gold raw SHA-256: gold={} ours={}", hex(&gold_hash), hex(&our_hash));
    if differing_games != 0 {
        return Err(format!("{differing_games} games have unapproved canonical-byte differences"));
    }
    eprintln!(
        "strict 2CBH gold: PASS only for the current Mega Database corpus; this does not prove unobserved Chess960, training, multimedia or rare annotation layouts"
    );
    Ok(())
}

#[derive(Debug)]
struct Game {
    raw: String,
    tags: Vec<(String, String)>,
    tokens: Vec<Token>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Comment(String),
    Nag(String),
    Open,
    Close,
}

struct Games<R> {
    input: R,
    pending: Option<String>,
    done: bool,
}

impl<R: BufRead> Games<R> {
    fn new(input: R) -> Self {
        Self { input, pending: None, done: false }
    }

    fn next_game(&mut self) -> Result<Option<Game>, String> {
        if self.done && self.pending.is_none() {
            return Ok(None);
        }
        let mut source = String::new();
        let mut in_moves = false;
        let mut in_brace = false;
        if let Some(p) = self.pending.take() {
            source.push_str(&p);
        }
        loop {
            let mut bytes = Vec::new();
            let n = self.input.read_until(b'\n', &mut bytes).map_err(|e| e.to_string())?;
            if n == 0 {
                self.done = true;
                break;
            }
            let line = std::str::from_utf8(&bytes)
                .map_err(|e| format!("ChessBase PGN is not UTF-8 at input offset: {e}; configure explicit encoding"))?;
            let trimmed = line.trim_start();
            let tag_line = !in_brace && trimmed.starts_with('[') && trimmed.trim_end().ends_with(']');
            if tag_line && !source.trim().is_empty() && (in_moves || already_has_event(&source, trimmed)) {
                self.pending = Some(line.to_owned());
                break;
            }
            source.push_str(line);
            if !trimmed.is_empty() && !tag_line && !trimmed.starts_with('%') {
                in_moves = true;
            }
            if !tag_line {
                let mut in_semicolon = false;
                for ch in line.chars() {
                    if ch == ';' && !in_brace {
                        in_semicolon = true;
                    }
                    if in_semicolon {
                        continue;
                    }
                    match ch {
                        '{' if !in_brace => in_brace = true,
                        '}' if in_brace => in_brace = false,
                        _ => {}
                    }
                }
            }
        }
        if source.trim().is_empty() {
            Ok(None)
        } else {
            parse_game(source).map(Some)
        }
    }
}

fn already_has_event(source: &str, new_tag: &str) -> bool {
    if !new_tag.starts_with("[Event ") {
        return false;
    }
    source.lines().any(|s| s.trim_start().starts_with("[Event "))
}

fn parse_game(raw: String) -> Result<Game, String> {
    let mut tags = Vec::new();
    let mut body = String::new();
    let mut in_tags = true;
    for line in raw.lines() {
        let trimmed = line.trim();
        if in_tags && trimmed.starts_with('[') && trimmed.ends_with(']') {
            tags.push(parse_tag(trimmed)?);
        } else {
            if !trimmed.is_empty() { in_tags = false; }
            if !in_tags {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    if tags.is_empty() {
        return Err("PGN game has no header tags".into());
    }
    let tokens = lex(&body)?;
    Ok(Game { raw, tags, tokens })
}

fn parse_tag(line: &str) -> Result<(String, String), String> {
    let inner = line.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
        .ok_or("invalid PGN tag brackets")?;
    let (key, value) = inner.split_once(char::is_whitespace)
        .ok_or("missing PGN tag value")?;
    if key.is_empty() { return Err("empty PGN tag key".into()); }
    let value = value.trim_start();
    let Some(value) = value.strip_prefix('"') else { return Err("PGN tag value is not quoted".into()); };
    let mut text = String::new();
    let mut escaped = false;
    let mut closed = false;
    let mut after = "";
    for (i, ch) in value.char_indices() {
        if escaped {
            text.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            closed = true;
            after = &value[i + ch.len_utf8()..];
            break;
        } else {
            text.push(ch);
        }
    }
    if !closed || escaped || !after.trim().is_empty() {
        return Err("bad PGN tag quotes or escapes".into());
    }
    Ok((key.to_owned(), text))
}

fn lex(body: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '{' {
            chars.next();
            let mut c = String::new();
            let mut closed = false;
            for t in chars.by_ref() {
                if t == '}' { closed = true; break; }
                c.push(t);
            }
            if !closed { return Err("unterminated PGN comment".into()); }
            tokens.push(Token::Comment(c));
        } else if ch == ';' {
            chars.next();
            let mut c = String::new();
            while let Some(&t) = chars.peek() {
                if t == '\n' { break; }
                c.push(t);
                chars.next();
            }
            tokens.push(Token::Comment(c));
        } else if ch == '(' || ch == ')' {
            chars.next();
            tokens.push(if ch == '(' { Token::Open } else { Token::Close });
        } else if ch == '$' {
            chars.next();
            let mut n = String::from("$");
            while chars.peek().is_some_and(char::is_ascii_digit) {
                n.push(chars.next().unwrap());
            }
            if n.len() == 1 { return Err("NAG without a number".into()); }
            tokens.push(Token::Nag(n));
        } else {
            let mut word = String::new();
            while let Some(&t) = chars.peek() {
                if t.is_whitespace() || matches!(t, '{' | '}' | '(' | ')' | '$' | ';') { break; }
                word.push(t);
                chars.next();
            }
            if word.is_empty() {
                return Err("unexpected PGN movetext delimiter".into());
            }
            // Move number and whitespace layout are nonsemantic PGN syntax.
            // Do not erase a SAN, comment, NAG, variation or result token.
            if word.chars().all(|c| c.is_ascii_digit() || c == '.') && word.contains('.') {
                continue;
            }
            tokens.push(Token::Word(word));
        }
    }
    Ok(tokens)
}

struct Compared {
    equal: bool,
    san: u64,
    nulls: u64,
    first_byte: usize,
    bytes: u64,
}

fn compare_game(gold: &Game, ours: &Game) -> Result<Compared, String> {
    let mut gold_tags = gold.tags.clone();
    let mut ours_tags = ours.tags.clone();
    gold_tags.sort();
    ours_tags.sort();
    // Every tag remains significant. No KEEP list or ChessBase-only tag skip.
    if gold_tags != ours_tags {
        return Ok(Compared { equal: false, san: 0, nulls: 0, first_byte: first_byte_diff(
            format!("{gold_tags:?}").as_bytes(), format!("{ours_tags:?}").as_bytes()
        ), bytes: 0 });
    }
    let mut start = Board::startpos();
    if let Some((_, fen)) = ours_tags.iter().find(|(k, _)| k == "FEN") {
        start = gigachess::fen::parse_fen(fen).map_err(|e| format!("invalid FEN in PGN: {e}"))?;
    }
    let mut board = start;
    let mut before_last = start;
    let mut stack = Vec::new();
    let mut san = 0u64;
    let mut nulls = 0u64;
    let mut bytes = 0u64;
    if gold.tokens.len() != ours.tokens.len() {
        return Ok(Compared { equal: false, san, nulls, first_byte: 0, bytes });
    }
    for (idx, (a, b)) in gold.tokens.iter().zip(&ours.tokens).enumerate() {
        bytes += format!("{a:?}").len() as u64;
        match (a, b) {
            (Token::Open, Token::Open) => {
                stack.push((board, before_last));
                board = before_last;
            }
            (Token::Close, Token::Close) => {
                let pair = stack.pop().ok_or("unmatched closing PGN variation")?;
                (board, before_last) = pair;
            }
            (Token::Word(ga), Token::Word(ob)) => {
                if matches!(ob.as_str(), "1-0" | "0-1" | "1/2-1/2" | "*") {
                    if ga != ob { return Ok(Compared { equal: false, san, nulls, first_byte: bytes as usize, bytes }); }
                    continue;
                }
                let null_match = (ga == "Z0" && ob == "--") || (ga == "--" && ob == "Z0");
                let is_null = ob == "--" || ga == "Z0";
                let mv = if is_null {
                    if !null_match && ga != ob { return Ok(Compared { equal: false, san, nulls, first_byte: idx, bytes }); }
                    Move::NULL
                } else {
                    match san_to_move(&board, ob) {
                        Some(mv) => mv,
                        None => return Ok(Compared { equal: false, san, nulls, first_byte: idx, bytes }),
                    }
                };
                if ga != ob {
                    if null_match {
                        nulls += 1;
                    } else if redundant_qualifier(ga, ob)
                        && san_to_move(&board, ga) == Some(mv)
                    {
                        // Both spellings must resolve to the identical LEGAL
                        // move on the actual board, so no alternative legal
                        // candidate exists for the minimal SAN.
                        san += 1;
                    } else {
                        return Ok(Compared { equal: false, san, nulls, first_byte: idx, bytes });
                    }
                }
                before_last = board;
                board.play(mv).map_err(|e| format!("illegal PGN move at token {idx}: {e}"))?;
            }
            _ if a == b => {}
            _ => {
                return Ok(Compared { equal: false, san, nulls, first_byte: idx, bytes });
            }
        }
    }
    if !stack.is_empty() { return Err("unclosed PGN variation".into()); }
    Ok(Compared { equal: true, san, nulls, first_byte: 0, bytes })
}

fn redundant_qualifier(gold: &str, ours: &str) -> bool {
    let g = gold.as_bytes();
    let o = ours.as_bytes();
    if g.len() <= o.len() || g.len() - o.len() > 2 || g[0] != o[0]
        || !matches!(g[0], b'K' | b'Q' | b'R' | b'B' | b'N')
    { return false; }
    let extra = g.len() - o.len();
    g[1..1 + extra].iter().all(|x| b"abcdefgh12345678".contains(x))
        && g[1 + extra..] == o[1..]
}

fn first_byte_diff(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()))
}

// No external hash dependency or platform-specific shell command: the
// standard FIPS-180-4 SHA-256 transform is sufficient for local provenance.
const K: [u32; 64] = [
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4b,
    0xab1c5ed5,0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,
    0x9bdc06a7,0xc19bf174,0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,
    0x4a7484aa,0x5cb0a9dc,0x76f988da,0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,
    0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,0x27b70a85,0x2e1b2138,0x4d2c6dfc,
    0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,
    0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,0x19a4c116,
    0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,
    0xc67178f2,
];

struct Sha256 {
    h: [u32; 8],
    data: [u8; 64],
    len: usize,
    total: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            h: [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19],
            data: [0; 64], len: 0, total: 0,
        }
    }
    fn update(&mut self, mut bytes: &[u8]) {
        self.total = self.total.wrapping_add(bytes.len() as u64);
        while !bytes.is_empty() {
            let n = (64 - self.len).min(bytes.len());
            self.data[self.len..self.len + n].copy_from_slice(&bytes[..n]);
            self.len += n;
            bytes = &bytes[n..];
            if self.len == 64 {
                self.block();
                self.len = 0;
            }
        }
    }
    fn block(&mut self) {
        let mut w = [0u32; 64];
        for (i, chunk) in self.data.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(chunk.try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut h = self.h;
        for i in 0..64 {
            let s1 = h[4].rotate_right(6) ^ h[4].rotate_right(11) ^ h[4].rotate_right(25);
            let ch = (h[4] & h[5]) ^ (!h[4] & h[6]);
            let t1 = h[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = h[0].rotate_right(2) ^ h[0].rotate_right(13) ^ h[0].rotate_right(22);
            let maj = (h[0] & h[1]) ^ (h[0] & h[2]) ^ (h[1] & h[2]);
            let t2 = s0.wrapping_add(maj);
            h = [t1.wrapping_add(t2), h[0], h[1], h[2], h[3].wrapping_add(t1), h[4], h[5], h[6]];
        }
        for (a, b) in self.h.iter_mut().zip(h) {
            *a = a.wrapping_add(b);
        }
    }
    fn finish(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        if self.len > 56 {
            while self.len != 0 { self.update(&[0]); }
        }
        while self.len < 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0; 32];
        for (dst, word) in out.chunks_exact_mut(4).zip(self.h) {
            dst.copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

fn hex(bytes: &[u8]) -> String {
    const H: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 15) as usize] as char);
    }
    out
}

fn file_digest(path: &Path) -> Result<(u64, [u8; 32]), String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut n = 0u64;
    loop {
        let len = file.read(&mut buf).map_err(|e| e.to_string())?;
        if len == 0 { break; }
        hash.update(&buf[..len]);
        n += len as u64;
    }
    Ok((n, hash.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_standard_vectors() {
        let mut h = Sha256::new();
        assert_eq!(hex(&h.finish()), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        h = Sha256::new();
        h.update(b"abc");
        assert_eq!(hex(&h.finish()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn strict_comment_and_annotation_bytes_are_significant() {
        let a = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 {Good} *\n".into()).unwrap();
        let b = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 {Goad} *\n".into()).unwrap();
        assert!(!compare_game(&a, &b).unwrap().equal);
        let c = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 {Good} *\n".into()).unwrap();
        assert!(compare_game(&a, &c).unwrap().equal);
    }

    #[test]
    fn strict_full_tags_and_variations() {
        let a = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *\n".into()).unwrap();
        let mut wrong = parse_game("[Event \"F\"]\n[White \"W\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *\n".into()).unwrap();
        assert!(!compare_game(&a, &wrong).unwrap().equal);
        wrong = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 e5 (1... e6) 2. Nf3 *\n".into()).unwrap();
        assert!(!compare_game(&a, &wrong).unwrap().equal);
    }
}
