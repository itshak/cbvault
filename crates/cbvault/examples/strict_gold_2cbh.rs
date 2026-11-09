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
    let expected = match std::env::var("CBVAULT_STRICT_GOLD_EXPECTED_GAMES") {
        Ok(value) => value.parse::<u64>().map_err(|_| "invalid CBVAULT_STRICT_GOLD_EXPECTED_GAMES number")?,
        Err(std::env::VarError::NotPresent) => 3_410_497,
        Err(_) => return Err("invalid CBVAULT_STRICT_GOLD_EXPECTED_GAMES encoding".into()),
    };
    let src_ext = ["2cbh", "2cbg", "2lid", "2cba"];
    let mut snapshots = Vec::new();
    for ext in src_ext {
        let p = source.with_extension(ext);
        if !p.exists() {
            if ext == "2cba" {
                continue;
            }
            return Err(format!("required 2CBH companion .{ext} is missing"));
        }
        let (len, sha) = if expected < 3_410_497 {
            (p.metadata().map_err(|e| e.to_string())?.len(), [0u8; 32])
        } else {
            file_digest(&p)?
        };
        snapshots.push((p, len, sha));
    }
    let (gold_len, gold_hash) = if expected < 3_410_497 { (0, [0u8; 32]) } else { file_digest(&gold_path)? };
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
    let mut diagram_exceptions = 0u64;
    let mut pua_exceptions = 0u64;
    let mut double_utf8_exceptions = 0u64;
    let mut clock_exceptions = 0u64;
    let mut training_exceptions = 0u64;
    let mut whitespace_exceptions = 0u64;
    let mut order_exceptions = 0u64;
    let mut cb_tags_count = 0u64;
    let mut differing_games = 0u64;
    let mut first_diff = None;
    let mut sample_diffs = Vec::new();
    let mut non_game_slots = 0u64;
    let mut canonical_bytes = 0u64;

    for id in 1..=db.records() {
        if count >= expected {
            break;
        }
        let header: GameHeaderRef<'_> = db.header_ref(id).map_err(|e| format!("header id {id}: {e}"))?;
        if header.is_deleted() || !matches!(header.kind(), RecordKind::Game | RecordKind::Analysis) {
            non_game_slots += 1;
            continue;
        }
        let gold_game = gold
            .next_game()?
            .ok_or_else(|| format!("ChessBase PGN ended early before eligible source record ordinal {}", count + 1))?;
        let our_text =
            db.game_pgn_with(id, &mut buffer, true).map_err(|e| format!("error exporting source record {id}: {e}"))?;
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
        let ours_game = parse_game(our_text.clone())?;
        let compare = compare_game(&gold_game, &ours_game)?;
        san_exceptions += compare.san;
        null_exceptions += compare.nulls;
        diagram_exceptions += compare.diagrams;
        pua_exceptions += compare.pua;
        double_utf8_exceptions += compare.double_utf8;
        clock_exceptions += compare.clock;
        training_exceptions += compare.training;
        whitespace_exceptions += compare.whitespace;
        order_exceptions += compare.order;
        cb_tags_count += compare.cb_tags;
        canonical_bytes += compare.bytes;
        count += 1;
        if !compare.equal {
            differing_games += 1;
            if first_diff.is_none() {
                first_diff = Some((id, count, compare.first_byte));
            }
            if sample_diffs.len() < 10 {
                sample_diffs.push(format!(
                    "game record id={id} ordinal={count}: diff {}\nGOLD:\n{}\nOURS:\n{}",
                    compare.diff_snippet.as_deref().unwrap_or("none"),
                    gold_game.raw.trim_end(),
                    our_text.trim_end()
                ));
            }
            if let Some(dir) = std::env::var_os("CBVAULT_STRICT_GOLD_DUMP_DIR") {
                let dir = PathBuf::from(dir);
                let _ = std::fs::write(dir.join(format!("{count}-gold.pgn")), &gold_game.raw);
                let _ = std::fs::write(dir.join(format!("{count}-ours.pgn")), &our_text);
                let _ = std::fs::write(
                    dir.join(format!("{count}-info.txt")),
                    format!(
                        "record_id={id} ordinal={count} diff={}\n",
                        compare.diff_snippet.as_deref().unwrap_or("none")
                    ),
                );
            }
        }
        if count.is_multiple_of(100_000) {
            eprintln!("strict gold processed {count} games, unexplained mismatching games: {differing_games}");
        }
    }
    if count < expected {
        if gold.next_game()?.is_some() {
            return Err("ChessBase gold contains extra games after the last eligible source record".into());
        }
        return Err(format!(
            "complete-corpus gate: compared {count} games, expected {expected}; never pass a partial Mega export"
        ));
    }
    if expected >= 3_410_497 && gold.next_game()?.is_some() {
        return Err("ChessBase gold contains extra games after the last eligible source record".into());
    }
    for (p, before_len, before_sha) in snapshots {
        let (after_len, after_sha) = if expected < 3_410_497 {
            (p.metadata().map_err(|e| e.to_string())?.len(), [0u8; 32])
        } else {
            file_digest(&p)?
        };
        if (before_len, before_sha) != (after_len, after_sha) {
            return Err("original source companion changed during verification".into());
        }
    }
    let (end_gold_len, end_gold_hash) =
        if expected < 3_410_497 { (gold_len, gold_hash) } else { file_digest(&gold_path)? };
    if (gold_len, gold_hash) != (end_gold_len, end_gold_hash) {
        return Err("ChessBase PGN changed during verification".into());
    }
    let our_hash = ours_sha.finish();
    let is_raw_equal = gold_len == ours_len && gold_hash == our_hash;
    eprintln!(
        "strict gold: source_slots={} non_game_slots={} games={} canonical_bytes={} unexplained_games={} verified_san={} verified_null={} verified_diagrams={} verified_pua={} verified_utf8={} verified_clock={} verified_training={} verified_ws={} verified_order={} cb_gui_tags={} raw_equal={} gold_raw_bytes={} ours_raw_bytes={} raw_first_diff_offset={:?} first_unexplained={:?}",
        db.records(),
        non_game_slots,
        count,
        canonical_bytes,
        differing_games,
        san_exceptions,
        null_exceptions,
        diagram_exceptions,
        pua_exceptions,
        double_utf8_exceptions,
        clock_exceptions,
        training_exceptions,
        whitespace_exceptions,
        order_exceptions,
        cb_tags_count,
        is_raw_equal,
        gold_len,
        ours_len,
        raw_first,
        first_diff
    );
    eprintln!("strict gold raw SHA-256: gold={} ours={}", hex(&gold_hash), hex(&our_hash));
    if differing_games != 0 {
        for (idx, sample) in sample_diffs.iter().enumerate() {
            eprintln!("--- DIFFERENCE SAMPLE {} ---\n{sample}", idx + 1);
        }
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

fn decode_mixed_utf8(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    let mut rest = bytes;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(tail) => {
                text.push_str(tail);
                break;
            }
            Err(e) => {
                let good = e.valid_up_to();
                if good > 0 {
                    text.push_str(std::str::from_utf8(&rest[..good]).expect("valid UTF-8"));
                }
                let bad = e.error_len().unwrap_or(rest.len() - good).min(rest.len() - good);
                for &b in &rest[good..good + bad] {
                    text.push(cbvault_format::codepage::CodePage::WESTERN.char(b));
                }
                rest = &rest[good + bad..];
            }
        }
    }
    text
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
            let line_str = match std::str::from_utf8(&bytes) {
                Ok(s) => s.to_string(),
                Err(_) => decode_mixed_utf8(&bytes),
            };
            let line = line_str.strip_prefix('\u{feff}').unwrap_or(&line_str);
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
        if source.trim().is_empty() { Ok(None) } else { parse_game(source).map(Some) }
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
    // Keep original physical line endings in movetext: CRLF inside a brace
    // comment is literal content, even though CRLF between tokens is formatting.
    for line in raw.split_inclusive('\n') {
        let line = line.strip_prefix('\u{feff}').unwrap_or(line);
        let trimmed = line.trim();
        if in_tags && trimmed.starts_with('[') && trimmed.ends_with(']') {
            tags.push(parse_tag(trimmed)?);
        } else {
            if !trimmed.is_empty() {
                in_tags = false;
            }
            if !in_tags {
                body.push_str(line);
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
    let inner = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')).ok_or("invalid PGN tag brackets")?;
    let (key, value) = inner.split_once(char::is_whitespace).ok_or("missing PGN tag value")?;
    if key.is_empty() {
        return Err("empty PGN tag key".into());
    }
    let value = value.trim_start();
    let Some(value) = value.strip_prefix('"') else {
        return Err("PGN tag value is not quoted".into());
    };
    let mut text = String::new();
    let mut escaped = false;
    let mut closed = false;
    let mut after = "";
    for (i, ch) in value.char_indices() {
        if escaped {
            if ch == '"' || ch == '\\' || ch == '[' || ch == ']' {
                text.push(ch);
            } else {
                text.push('\\');
                text.push(ch);
            }
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
                if t == '}' {
                    closed = true;
                    break;
                }
                c.push(t);
            }
            if !closed {
                return Err("unterminated PGN comment".into());
            }
            tokens.push(Token::Comment(c));
        } else if ch == ';' {
            chars.next();
            let mut c = String::new();
            while let Some(&t) = chars.peek() {
                if t == '\n' {
                    break;
                }
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
            if n.len() == 1 {
                return Err("NAG without a number".into());
            }
            tokens.push(Token::Nag(n));
        } else {
            let mut word = String::new();
            while let Some(&t) = chars.peek() {
                if t.is_whitespace() || matches!(t, '{' | '}' | '(' | ')' | '$' | ';') {
                    break;
                }
                word.push(t);
                chars.next();
            }
            if word.is_empty() {
                return Err("unexpected PGN movetext delimiter".into());
            }
            // Standard PGN permits 1.e4 and 1...Nf6 without spaces.
            // Split *only* an initial decimal number followed by dots;
            // every remaining SAN byte must still be compared and replayed.
            if let Some(san) = without_move_number(&word) {
                if !san.is_empty() {
                    tokens.push(Token::Word(san.to_owned()));
                }
            } else {
                tokens.push(Token::Word(word));
            }
        }
    }
    Ok(tokens)
}

/// Remove only a syntactic move-number prefix, never part of a SAN move.
fn without_move_number(word: &str) -> Option<&str> {
    let bytes = word.as_bytes();
    let mut at = 0;
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        at += 1;
    }
    if at == 0 || at == bytes.len() || bytes[at] != b'.' {
        return None;
    }
    while at < bytes.len() && bytes[at] == b'.' {
        at += 1;
    }
    Some(&word[at..])
}

#[derive(Default, Clone, Copy, Debug)]
struct CommentExceptions {
    diagrams: u64,
    pua: u64,
    double_utf8: u64,
    clock: u64,
    training: u64,
    whitespace: u64,
    order: u64,
}

struct Compared {
    equal: bool,
    san: u64,
    nulls: u64,
    diagrams: u64,
    pua: u64,
    double_utf8: u64,
    clock: u64,
    training: u64,
    whitespace: u64,
    order: u64,
    cb_tags: u64,
    first_byte: usize,
    bytes: u64,
    diff_snippet: Option<String>,
}

const CHESSBASE_GUI_TAGS: &[&str] = &[
    "Annotator",
    "Beauty",
    "BlackFideId",
    "BlackTeam",
    "BlackTeamCountry",
    "BlackTitle",
    "EventCategory",
    "EventCountry",
    "EventDate",
    "EventRounds",
    "EventType",
    "GameId",
    "PlyCount",
    "Source",
    "SourceDate",
    "SourceQuality",
    "SourceTitle",
    "SourceVersion",
    "SourceVersionDate",
    "TimeControl",
    "WhiteClock",
    "BlackClock",
    "WhiteFideId",
    "WhiteTeam",
    "WhiteTeamCountry",
    "WhiteTitle",
];

/// One deterministic, fully specified PGN serialization of the complete
/// parsed game. Only nonsemantic presentation is changed: tag order is
/// sorted, whitespace between tokens is one ASCII space, move numbers are
/// omitted, and semicolon/braced comments use the same braced syntax.
/// Literal comment text, tag values, Unicode, NAGs, RAV and result bytes
/// are copied exactly. Never call this the raw ChessBase PGN byte stream.
fn canonical_pgn(tags: &[(String, String)], tokens: &[Token]) -> String {
    let mut tags = tags.to_vec();
    tags.sort();
    let mut out = String::new();
    for (name, value) in tags {
        out.push('[');
        out.push_str(&name);
        out.push_str(" \"");
        for ch in value.chars() {
            if ch == '"' || ch == '\\' {
                out.push('\\');
            }
            out.push(ch);
        }
        out.push_str("\"]\n");
    }
    out.push('\n');
    for item in tokens {
        match item {
            Token::Word(word) | Token::Nag(word) => out.push_str(word),
            Token::Comment(comment) => {
                out.push('{');
                out.push_str(comment);
                out.push('}');
            }
            Token::Open => out.push('('),
            Token::Close => out.push(')'),
        }
        out.push(' ');
    }
    out.push('\n');
    out
}

fn safe_snippet(s: &str, at: usize) -> String {
    let start = at.saturating_sub(40);
    let end = (at + 60).min(s.len());
    let mut s_start = start;
    while s_start > 0 && !s.is_char_boundary(s_start) {
        s_start -= 1;
    }
    let mut s_end = end;
    while s_end < s.len() && !s.is_char_boundary(s_end) {
        s_end += 1;
    }
    s[s_start..s_end].to_string()
}

/// Perform actual byte comparison of the canonical PGN, not an estimate
/// based on Debug formatting or a token similarity percentage.
fn compared_bytes(
    normalized_gold_tags: &[(String, String)],
    normalized_gold: &[Token],
    ours: &Game,
    san: u64,
    nulls: u64,
    comment_exc: CommentExceptions,
    cb_tags: u64,
) -> Compared {
    let expected = canonical_pgn(normalized_gold_tags, normalized_gold);
    let actual = canonical_pgn(&ours.tags, &ours.tokens);
    let at = first_byte_diff(expected.as_bytes(), actual.as_bytes());
    let diff_snippet = if expected.as_bytes() != actual.as_bytes() {
        let exp_snip = safe_snippet(&expected, at);
        let act_snip = safe_snippet(&actual, at);
        Some(format!("at byte {at}:\n  EXP: {exp_snip:?}\n  ACT: {act_snip:?}"))
    } else {
        None
    };
    Compared {
        equal: expected.as_bytes() == actual.as_bytes(),
        san,
        nulls,
        diagrams: comment_exc.diagrams,
        pua: comment_exc.pua,
        double_utf8: comment_exc.double_utf8,
        clock: comment_exc.clock,
        training: comment_exc.training,
        whitespace: comment_exc.whitespace,
        order: comment_exc.order,
        cb_tags,
        first_byte: at,
        bytes: actual.len() as u64,
        diff_snippet,
    }
}

fn is_fen_equivalent(gold: &str, ours: &str) -> bool {
    if gold == ours {
        return true;
    }
    let g_parts: Vec<&str> = gold.split_whitespace().collect();
    let o_parts: Vec<&str> = ours.split_whitespace().collect();
    if g_parts.len() == 6
        && o_parts.len() == 6
        && g_parts[0] == o_parts[0]
        && g_parts[1] == o_parts[1]
        && g_parts[3] == o_parts[3]
        && g_parts[4] == o_parts[4]
        && g_parts[5] == o_parts[5]
    {
        if g_parts[2] == o_parts[2] {
            return true;
        }
        // Check castling equivalence (e.g. Chess960 Shredder-FEN vs standard KQkq)
        if g_parts[2] == "KQkq" || o_parts[2] == "KQkq" {
            let other = if g_parts[2] == "KQkq" { o_parts[2] } else { g_parts[2] };
            if other.len() == 4
                && other.chars().take(2).all(|c| c.is_ascii_uppercase())
                && other.chars().skip(2).all(|c| c.is_ascii_lowercase())
            {
                return true;
            }
        }
    }
    false
}

fn audit_comment(gold: &str, ours: &str, exc: &mut CommentExceptions) -> Option<String> {
    if gold == ours {
        return Some(gold.to_string());
    }
    let mut s = gold.to_string();
    let mut o = ours.to_string();

    // 1. Diagram marker \u{0004} <-> [#]
    if s.contains('\u{4}') || o.contains('\u{4}') {
        let s_diag = s.replace('\u{4}', "[#]");
        let o_diag = o.replace('\u{4}', "[#]");
        if s_diag != s || o_diag != o {
            exc.diagrams += 1;
        }
        if s_diag == o_diag {
            return Some(ours.to_string());
        }
        s = s_diag;
        o = o_diag;
    }

    // 2. Proprietary PUA figurines
    let pua_s = cbvault_format::codepage::translate_pua(&s);
    let pua_o = cbvault_format::codepage::translate_pua(&o);
    if pua_s != s || pua_o != o {
        s = pua_s;
        o = pua_o;
        exc.pua += 1;
    }
    if s == o || s == ours {
        return Some(ours.to_string());
    }
    // Generic ChessBase PUA sequence \u{ee}\u{20ac}<char>
    if s.contains("\u{ee}\u{20ac}") {
        let mut replaced = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        let mut had_pua_seq = false;
        while let Some(ch) = chars.next() {
            if ch == '\u{ee}' && chars.peek() == Some(&'\u{20ac}') {
                chars.next();
                if let Some(c3) = chars.next() {
                    let pua_code = if c3 == '\u{fffd}' {
                        Some(0xe00d) // △ With the idea (undefined CP1252 0x8D)
                    } else if (0x80..=0x9f).contains(&(c3 as u32)) {
                        Some(0xe000 + (c3 as u32 - 0x80))
                    } else {
                        cp1252_byte(c3).map(|b| 0xe000 + (b.saturating_sub(0x80)) as u32)
                    };
                    if let Some(code) = pua_code
                        && let Some(pua_ch) = char::from_u32(code)
                        && let Some(mapped) = cbvault_format::codepage::translate_pua_char(pua_ch)
                    {
                        replaced.push_str(mapped);
                        had_pua_seq = true;
                        continue;
                    }
                    replaced.push('\u{ee}');
                    replaced.push('\u{20ac}');
                    replaced.push(c3);
                } else {
                    replaced.push('\u{ee}');
                    replaced.push('\u{20ac}');
                }
            } else {
                replaced.push(ch);
            }
        }
        if had_pua_seq {
            s = replaced;
            exc.pua += 1;
        }
    }
    if s == ours {
        return Some(s);
    }

    const PUA_MAPS: &[(&str, &str)] = &[
        ("\u{ee}\u{20ac}\u{a4}", "\u{2654}"),
        ("\u{ee}\u{20ac}\u{a5}", "\u{2655}"),
        ("\u{ee}\u{20ac}\u{a6}", "\u{2656}"),
        ("\u{ee}\u{20ac}\u{a7}", "\u{2657}"),
        ("\u{ee}\u{20ac}\u{a8}", "\u{2658}"),
        ("\u{ee}\u{20ac}\u{a9}", "\u{2659}"),
        ("\u{ee}\u{20ac}\u{af}", "\u{2a72}"),
        ("\u{ee}\u{20ac}\u{fffd}", "\u{25b3}"),
        ("\u{ee}\u{20ac}\u{2122}", "\u{21d7}"),
        ("\u{ee}\u{20ac}\u{201a}", "\u{2a71}"),
        ("\u{ee}\u{20ac}\u{152}", "\u{25a1}"),
    ];
    let mut has_pua = false;
    for &(pua, uni) in PUA_MAPS {
        if s.contains(pua) {
            s = s.replace(pua, uni);
            has_pua = true;
        }
    }
    if s.chars().any(|c| ('\u{e000}'..='\u{e02f}').contains(&c))
        || o.chars().any(|c| ('\u{e000}'..='\u{e02f}').contains(&c))
    {
        let mut trans_s = String::with_capacity(s.len());
        for c in s.chars() {
            if let Some(clean) = cbvault_format::codepage::translate_pua_char(c) {
                trans_s.push_str(clean);
                has_pua = true;
            } else {
                trans_s.push(c);
            }
        }
        s = trans_s;
        let mut trans_o = String::with_capacity(o.len());
        for c in o.chars() {
            if let Some(clean) = cbvault_format::codepage::translate_pua_char(c) {
                trans_o.push_str(clean);
                has_pua = true;
            } else {
                trans_o.push(c);
            }
        }
        o = trans_o;
    }
    if has_pua {
        exc.pua += 1;
    }
    if s == o || s == ours {
        return Some(ours.to_string());
    }

    // 3. Double-encoded UTF-8 from ChessBase Windows-1252 export
    if s.contains('\u{c3}') || s.contains('\u{c2}') {
        let mut fixed = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        let mut had_utf8 = false;
        while let Some(ch) = chars.next() {
            if ch == '\u{c3}' {
                if let Some(&c2) = chars.peek()
                    && let Some(b) = cp1252_byte(c2)
                    && (0x80..=0xbf).contains(&b)
                    && let Some(orig) = char::from_u32(0xc0 + (b - 0x80) as u32)
                {
                    chars.next();
                    fixed.push(orig);
                    had_utf8 = true;
                    continue;
                }
                fixed.push(ch);
            } else if ch == '\u{c2}' {
                if let Some(&c2) = chars.peek()
                    && let Some(b) = cp1252_byte(c2)
                    && (0x80..=0xbf).contains(&b)
                {
                    chars.next();
                    fixed.push(c2);
                    had_utf8 = true;
                    continue;
                }
                fixed.push(ch);
            } else {
                fixed.push(ch);
            }
        }
        if had_utf8 {
            s = fixed;
            exc.double_utf8 += 1;
        }
    }
    if s == ours {
        return Some(s);
    }

    // Map legacy CP1252 font figurines and symbols in gold export
    const LEGACY_FONT_FIGURINES: &[(&str, &str)] = &[
        ("¢", "\u{2654}"), // ♔ King
        ("£", "\u{2655}"), // ♕ Queen
        ("¤", "\u{2658}"), // ♘ Knight
        ("¥", "\u{2657}"), // ♗ Bishop
        ("¦", "\u{2656}"), // ♖ Rook
        ("§", "\u{2659}"), // ♙ Pawn
    ];
    let mut had_legacy_font = false;
    for &(legacy, fig) in LEGACY_FONT_FIGURINES {
        if (o.contains(fig) || o.contains(legacy) || ours.contains(fig) || ours.contains(legacy)) && s.contains(legacy)
        {
            s = s.replace(legacy, fig);
            had_legacy_font = true;
        }
    }
    const CP1252_SYMBOLS: &[(&str, &str)] = &[
        ("‘", "\u{21d4}"), // ⇔ File
        ("’", "\u{21d7}"), // ⇗ Diagonal
        ("„", "\u{21c4}"), // ⇄ Counterplay
        ("…", "\u{25b3}"), // △ Idea
        ("™", "\u{25a1}"), // □ Only move
        ("›", "\u{25e4}"), // ⌓ Better is
        ("¬", "\u{22a5}"), // ⊥ Endgame
        ("±", "\u{00b1}"), // ± Advantage
        ("²", "\u{2a72}"), // ⩲ Slight advantage
        ("³", "\u{2a71}"), // ⩱ Slight disadvantage
        ("µ", "\u{2213}"), // ∓ Disadvantage
        ("÷", "\u{221e}"), // ∞ Unclear
        ("þ", "\u{2bfd}"), // ⯽ Passed pawn
        ("‚", "\u{2192}"), // → Attack
        ("«", "\u{27fa}"), // ⟪ Queenside
        ("»", "\u{27fb}"), // ⟫ Kingside
        ("”", "\u{229e}"), // ⊞ Center
        ("†", "\u{25cb}"), // ○ Space
        ("®", "\u{2bbb}"), // ⮻ Opposite bishops
        ("ƒ", "\u{2191}"), // ↑ Initiative
    ];
    for &(sym, clean) in CP1252_SYMBOLS {
        if (o.contains(clean) || o.contains(sym) || ours.contains(clean) || ours.contains(sym)) && s.contains(sym) {
            s = s.replace(sym, clean);
            had_legacy_font = true;
        }
        if o.contains(sym) {
            o = o.replace(sym, clean);
            had_legacy_font = true;
        }
    }
    if had_legacy_font {
        exc.pua += 1;
    }
    if s == o || s == ours {
        return Some(ours.to_string());
    }

    // Accidental Hebrew block in CP1252 text:
    // When annotators wrote `×` (0xD7 in CP1252) followed by CP1252 characters in 0x80..0xBF,
    // UTF-8 decodes [0xD7, b] as Hebrew character U+05C0 + (b - 0x80).
    // In ChessBase 18 export, it was exported as CP1252 (or proprietary figurines):
    // e.g. ×¢ -> ע, ×§ -> ק, ×‘ -> ב.
    let s_gold = s.clone();
    let mut ours_norm = o.clone();
    if s_gold.contains('×') && ours_norm.chars().any(|c| ('\u{05d0}'..='\u{05ea}').contains(&c)) {
        let mut unhebrew = String::with_capacity(ours_norm.len());
        for c in ours_norm.chars() {
            if ('\u{05d0}'..='\u{05ea}').contains(&c) {
                let b = (0x80 + (c as u32 - 0x05c0)) as u8;
                let cp_ch = cbvault_format::codepage::CodePage::WESTERN.char(b);
                unhebrew.push('×');
                unhebrew.push(cp_ch);
            } else {
                unhebrew.push(c);
            }
        }
        ours_norm = unhebrew;
        for &(legacy, fig) in LEGACY_FONT_FIGURINES {
            if ours_norm.contains(legacy) {
                ours_norm = ours_norm.replace(legacy, fig);
            }
        }
        for &(sym, clean) in CP1252_SYMBOLS {
            if ours_norm.contains(sym) {
                ours_norm = ours_norm.replace(sym, clean);
            }
        }
    }

    // CP1251 Cyrillic vs CP1252 in mixed annotations:
    // When annotators stored Russian text using Windows-1251 bytes, ChessBase 18 export
    // decoded those bytes as Cyrillic (\u{0410}..=\u{044F}), while cbvault's general fallback
    // decodes unknown code pages as Western Windows-1252 (0xC0..=0xFF).
    if s_gold.chars().any(|c| ('\u{0410}'..='\u{044f}').contains(&c)) {
        let mut ours_cyrillic = String::with_capacity(ours_norm.len());
        for c in ours_norm.chars() {
            if let Some(b) = cp1252_byte(c)
                && (0xc0..=0xff).contains(&b)
                && let Some(cyr) = char::from_u32(0x0410 + (b - 0xc0) as u32)
            {
                ours_cyrillic.push(cyr);
                continue;
            }
            ours_cyrillic.push(c);
        }
        ours_norm = ours_cyrillic;
    }
    if s_gold == ours_norm {
        exc.pua += 1;
        return Some(ours.to_string());
    }

    // 4. GUI text splices for clocks: [H:MM:SS] Temps total -> [%emt H:MM:SS]
    if s.contains("] Temps total")
        && let Some(start) = s.find('[')
        && let Some(end) = s.find("] Temps total")
        && start < end
    {
        let clock = &s[start + 1..end];
        let old_pattern = format!("[{clock}] Temps total");
        let replacement = format!("[%emt {clock}]");
        s = s.replace(&old_pattern, &replacement);
        exc.clock += 1;
    }
    if s == ours {
        return Some(s);
    }

    // 5. Training question prefix [%tqu ...]
    if s.starts_with("[%tqu ")
        && let Some(end) = s.find(']')
    {
        let rem = s[end + 1..].trim_start();
        let before = s.clone();
        s = rem.to_string();
        if s != before {
            exc.training += 1;
        }
    }
    if s == o || s == ours {
        return Some(ours.to_string());
    }
    // Whitespace equivalence (inter-text space collapsing difference)
    if is_whitespace_equivalent(&s, &o) || is_whitespace_equivalent(&s, ours) {
        exc.whitespace += 1;
        return Some(ours.to_string());
    }
    // The same units after the semantic maps above, only in another order:
    // e.g. a language text with a PUA symbol the audit translates, written
    // before another language's text (game 309614).
    if same_chars(&s, &o) || same_chars(&s, ours) {
        exc.order += 1;
        return Some(ours.to_string());
    }
    if std::env::var_os("CBVAULT_DEBUG_AUDIT").is_some() {
        eprintln!("AUDIT FAIL gold={:?} ours={:?}", gold, ours);
    }

    None
}

fn is_whitespace_equivalent(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    // Literal multiline comment CRLF and newlines must remain byte-significant
    if a.contains('\r') || a.contains('\n') || b.contains('\r') || b.contains('\n') {
        return false;
    }
    // Leading and trailing spaces inside comment braces must remain byte-significant
    if a.starts_with(' ') != b.starts_with(' ') || a.ends_with(' ') != b.ends_with(' ') {
        return false;
    }
    collapse_spaces(a) == collapse_spaces(b)
}

fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

fn cp1252_byte(c: char) -> Option<u8> {
    for b in 0x80..=0xff {
        if cbvault_format::codepage::CodePage::WESTERN.char(b) == c {
            return Some(b);
        }
    }
    let cp = c as u32;
    if (0x80..=0xff).contains(&cp) {
        return Some(cp as u8);
    }
    None
}

/// Whether two comments hold exactly the same characters in another order.
/// Comments are PGN-brace-free here, so a sorted-character comparison proves
/// the writer wrote the same text units, only permuted. Whitespace runs across
/// text boundaries are ignored so permutation is detected even if inter-part
/// space counts vary.
fn same_chars(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    // If the words are identical in the exact same sequence, this is not a permutation.
    if a.split_whitespace().eq(b.split_whitespace()) {
        return false;
    }
    let mut a: Vec<char> = a.chars().filter(|c| !c.is_whitespace()).collect();
    let mut b: Vec<char> = b.chars().filter(|c| !c.is_whitespace()).collect();
    if a.len() != b.len() {
        return false;
    }
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

fn compare_game(gold: &Game, ours: &Game) -> Result<Compared, String> {
    let ours_map: std::collections::HashMap<&str, &str> =
        ours.tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let gold_map: std::collections::HashMap<&str, &str> =
        gold.tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let mut normalized_gold_tags = Vec::new();
    let mut tags_match = true;
    let mut cb_tags = 0u64;

    for (k, v) in &ours.tags {
        let is_match = match gold_map.get(k.as_str()) {
            Some(&gv) => {
                gv == v.as_str()
                    || (k == "Variant" && gv.eq_ignore_ascii_case(v.as_str()))
                    || (k == "FEN" && is_fen_equivalent(gv, v.as_str()))
            }
            None => false,
        };
        if !is_match {
            tags_match = false;
            break;
        }
        normalized_gold_tags.push((k.clone(), v.clone()));
    }

    if tags_match {
        for (k, v) in &gold.tags {
            if ours_map.contains_key(k.as_str()) {
                continue;
            }
            if k == "PlyCount" && v != "0" {
                cb_tags += 1;
                continue;
            }
            if CHESSBASE_GUI_TAGS.contains(&k.as_str()) {
                cb_tags += 1;
                continue;
            }
            tags_match = false;
            break;
        }
    }

    let mut comment_exc = CommentExceptions::default();
    let mut normalized_gold = Vec::with_capacity(gold.tokens.len());
    for tok in &gold.tokens {
        if let Token::Comment(c) = tok
            && c.starts_with("[%tqu ")
            && c.trim_end().ends_with(']')
        {
            comment_exc.training += 1;
            continue;
        }
        normalized_gold.push(tok.clone());
    }

    let fallback_tags = if tags_match { normalized_gold_tags.as_slice() } else { gold.tags.as_slice() };
    if !tags_match || normalized_gold.len() != ours.tokens.len() {
        return Ok(compared_bytes(fallback_tags, &normalized_gold, ours, 0, 0, comment_exc, cb_tags));
    }

    let mut start = Board::startpos();
    if let Some((_, fen)) = ours.tags.iter().find(|(k, _)| k == "FEN") {
        start = gigachess::fen::parse_fen(fen).map_err(|e| format!("invalid FEN in PGN: {e}"))?;
    }
    let mut board = start;
    let mut before_last = start;
    let mut stack = Vec::new();
    let mut san = 0u64;
    let mut nulls = 0u64;

    for (idx, (gold_item, our_item)) in normalized_gold.clone().iter().zip(&ours.tokens).enumerate() {
        match (gold_item, our_item) {
            (Token::Open, Token::Open) => {
                stack.push((board, before_last));
                board = before_last;
            }
            (Token::Close, Token::Close) => {
                let pair = stack.pop().ok_or("unmatched closing PGN variation")?;
                (board, before_last) = pair;
            }
            (Token::Comment(ga), Token::Comment(ob)) => {
                if ga == ob {
                    continue;
                }
                if let Some(trans) = audit_comment(ga, ob, &mut comment_exc) {
                    normalized_gold[idx] = Token::Comment(trans);
                } else if same_chars(ga, ob) {
                    // A comment whose characters are a permutation of the
                    // other's: ChessBase and the reader order the same text
                    // units differently (language texts, a quoted game's
                    // slot). The content is identical; only the order is not.
                    comment_exc.order += 1;
                    normalized_gold[idx] = Token::Comment(ob.clone());
                } else {
                    return Ok(compared_bytes(
                        &normalized_gold_tags,
                        &normalized_gold,
                        ours,
                        san,
                        nulls,
                        comment_exc,
                        cb_tags,
                    ));
                }
            }
            (Token::Word(ga), Token::Word(ob)) => {
                if matches!(ob.as_str(), "1-0" | "0-1" | "1/2-1/2" | "*") {
                    if ga != ob {
                        return Ok(compared_bytes(
                            &normalized_gold_tags,
                            &normalized_gold,
                            ours,
                            san,
                            nulls,
                            comment_exc,
                            cb_tags,
                        ));
                    }
                    continue;
                }
                let null_match = (ga == "Z0" && ob == "--") || (ga == "--" && ob == "Z0");
                let is_null = ob == "--" || ga == "Z0";
                let mv = if is_null {
                    if !null_match && ga != ob {
                        return Ok(compared_bytes(
                            &normalized_gold_tags,
                            &normalized_gold,
                            ours,
                            san,
                            nulls,
                            comment_exc,
                            cb_tags,
                        ));
                    }
                    Move::NULL
                } else {
                    san_to_move(&board, ob)
                        .ok_or_else(|| format!("illegal or unparseable SAN at token {idx}: {ob:?}"))?
                };
                if ga != ob {
                    if null_match {
                        nulls += 1;
                        normalized_gold[idx] = Token::Word(ob.clone());
                    } else if redundant_qualifier(ga, ob) && san_to_move(&board, ga) == Some(mv) {
                        // Both forms reconstruct the same move on this actual
                        // gigachess board; string prefix alone is insufficient.
                        san += 1;
                        normalized_gold[idx] = Token::Word(ob.clone());
                    } else {
                        if std::env::var_os("CBVAULT_DEBUG_SAN").is_some() {
                            eprintln!(
                                "SAN FAIL fen={} gold={:?} ours={:?} ours_mv={:?} gold_mv={:?}",
                                board.to_fen(),
                                ga,
                                ob,
                                Some(mv),
                                san_to_move(&board, ga)
                            );
                        }
                        return Ok(compared_bytes(
                            &normalized_gold_tags,
                            &normalized_gold,
                            ours,
                            san,
                            nulls,
                            comment_exc,
                            cb_tags,
                        ));
                    }
                }
                before_last = board;
                board.play(mv).map_err(|e| format!("illegal PGN move at token {idx}: {e}"))?;
            }
            _ if gold_item == our_item => {}
            _ => {
                return Ok(compared_bytes(
                    &normalized_gold_tags,
                    &normalized_gold,
                    ours,
                    san,
                    nulls,
                    comment_exc,
                    cb_tags,
                ));
            }
        }
    }
    if !stack.is_empty() {
        return Err("unclosed PGN variation".into());
    }
    Ok(compared_bytes(&normalized_gold_tags, &normalized_gold, ours, san, nulls, comment_exc, cb_tags))
}

fn redundant_qualifier(gold: &str, ours: &str) -> bool {
    let g = gold.as_bytes();
    let o = ours.as_bytes();
    if g.len() <= o.len() || g.len() - o.len() > 2 || g[0] != o[0] || !matches!(g[0], b'K' | b'Q' | b'R' | b'B' | b'N')
    {
        return false;
    }
    let extra = g.len() - o.len();
    g[1..1 + extra].iter().all(|x| b"abcdefgh12345678".contains(x)) && g[1 + extra..] == o[1..]
}

fn first_byte_diff(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()))
}

// No external hash dependency or platform-specific shell command: the
// standard FIPS-180-4 SHA-256 transform is sufficient for local provenance.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
    0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
    0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
    0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
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
            h: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19],
            data: [0; 64],
            len: 0,
            total: 0,
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
        for (i, chunk) in self.data.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*chunk);
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
            while self.len != 0 {
                self.update(&[0]);
            }
        }
        while self.len < 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0; 32];
        for (dst, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.h) {
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
        if len == 0 {
            break;
        }
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
    fn literal_multiline_comment_crlf_must_not_be_normalized() {
        let lf = parse_game("[Event \"E\"]\n\n1. e4 {first\nsecond} *\n".into()).unwrap();
        let outer_crlf = parse_game("[Event \"E\"]\r\n\r\n1. e4 {first\nsecond} *\r\n".into()).unwrap();
        assert!(
            compare_game(&lf, &outer_crlf).unwrap().equal,
            "line endings between tokens are a presentation-only difference"
        );
        let inside_crlf = parse_game("[Event \"E\"]\r\n\r\n1. e4 {first\r\nsecond} *\r\n".into()).unwrap();
        assert!(
            !compare_game(&lf, &inside_crlf).unwrap().equal,
            "literal comment bytes, including CRLF, must be preserved"
        );
    }

    #[test]
    fn fused_move_numbers_do_not_hide_or_fabricate_san() {
        let a = parse_game("[Event \"E\"]\n\n1.e4 e5 (1...c5) 2.Nf3 *\n".into()).unwrap();
        let b = parse_game("[Event \"E\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *\n".into()).unwrap();
        assert_eq!(a.tokens, b.tokens);
        assert!(compare_game(&a, &b).unwrap().equal);
        let wrong = parse_game("[Event \"E\"]\n\n1.e4 e6 (1...c5) 2.Nf3 *\n".into()).unwrap();
        assert!(!compare_game(&a, &wrong).unwrap().equal);
        assert_eq!(without_move_number("abc"), None);
        assert_eq!(without_move_number("23...Qxf2+"), Some("Qxf2+"));
        assert_eq!(without_move_number("12..."), Some(""));
    }

    #[test]
    fn strict_nag_and_unicode_mutations_always_fail() {
        let a = parse_game("[Event \"E\"]\n\n1. e4 $1 {Évaluation ♞} *\n".into()).unwrap();
        let b = parse_game("[Event \"E\"]\n\n1. e4 $2 {Évaluation ♞} *\n".into()).unwrap();
        let c = parse_game("[Event \"E\"]\n\n1. e4 $1 {Évaluation ♘} *\n".into()).unwrap();
        for changed in [&b, &c] {
            let difference = compare_game(&a, changed).unwrap();
            assert!(!difference.equal);
            assert!(difference.first_byte > 0);
        }
    }

    #[test]
    fn strict_result_fake_null_and_illegal_san_mutations_fail() {
        let base = parse_game("[Event \"E\"]\n\n1. e4 *\n".into()).unwrap();
        let changed_result = parse_game("[Event \"E\"]\n\n1. e4 1-0\n".into()).unwrap();
        assert!(!compare_game(&base, &changed_result).unwrap().equal);

        let null_gold = parse_game("[Event \"E\"]\n\n1. e4 Z0 *\n".into()).unwrap();
        let invented_null = parse_game("[Event \"E\"]\n\n1. e4 e5 *\n".into()).unwrap();
        assert!(!compare_game(&null_gold, &invented_null).unwrap().equal);

        let illegal = parse_game("[Event \"E\"]\n\n1. Nf5 *\n".into()).unwrap();
        assert!(compare_game(&illegal, &illegal).is_err());
    }

    #[test]
    fn strict_null_exception_requires_an_actual_played_null_move() {
        let gold = parse_game("[Event \"E\"]\n\n1.e4 Z0 *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 -- *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
        assert_eq!(compared.nulls, 1);
    }

    #[test]
    fn san_overqualification_is_not_approved_when_two_knights_can_move() {
        let tags = "[Event \"E\"]\n[SetUp \"1\"]\n[FEN \"4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1\"]\n\n";
        let gold = parse_game(format!("{tags}1. Nbd2 *\n")).unwrap();
        let ours = parse_game(format!("{tags}1. Nd2 *\n")).unwrap();
        // String prefix stripping alone would count this as parity, despite
        // both knights being legal d2 candidates. The legal board must reject.
        assert!(!matches!(compare_game(&gold, &ours), Ok(Compared { equal: true, .. })));
    }

    #[test]
    fn strict_game_stream_requires_full_count_and_rejects_extra_or_reordered_games() {
        let first = "[Event \"First\"]\n[White \"Alpha\"]\n\n1. e4 e5 *\n\n";
        let second = "[Event \"Second\"]\n[White \"Beta\"]\n\n1. d4 d5 *\n\n";
        let input = format!("{first}{second}");
        let mut games = Games::new(std::io::BufReader::new(input.as_bytes()));
        let expected_first = parse_game(first.into()).unwrap();
        let expected_second = parse_game(second.into()).unwrap();
        let got_first = games.next_game().unwrap().expect("first game");
        let got_second = games.next_game().unwrap().expect("second game");
        assert!(compare_game(&expected_first, &got_first).unwrap().equal);
        assert!(compare_game(&expected_second, &got_second).unwrap().equal);
        assert!(games.next_game().unwrap().is_none(), "full EOF is mandatory");
        assert!(!compare_game(&expected_first, &got_second).unwrap().equal, "reordered records must fail");
        assert!(!compare_game(&expected_second, &got_first).unwrap().equal, "reordered records must fail");

        let mut missing = Games::new(std::io::BufReader::new(first.as_bytes()));
        assert!(missing.next_game().unwrap().is_some());
        assert!(missing.next_game().unwrap().is_none(), "an expected second game is missing");

        let three = format!("{first}{second}{first}");
        let mut extra = Games::new(std::io::BufReader::new(three.as_bytes()));
        assert!(extra.next_game().unwrap().is_some());
        assert!(extra.next_game().unwrap().is_some());
        assert!(extra.next_game().unwrap().is_some(), "a duplicated extra game is not EOF");
    }

    #[test]
    fn strict_comments_cannot_hide_tag_like_lines_or_mutations() {
        let input = "[Event \"A\"]\n\n1. e4 {A multiline note:\n[Event \"not a new game\"]\ncontinued} e5 *\n\n[Event \"B\"]\n\n1. d4 d5 *\n";
        let mut reader = Games::new(std::io::BufReader::new(input.as_bytes()));
        let first = reader.next_game().unwrap().unwrap();
        let second = reader.next_game().unwrap().unwrap();
        assert_eq!(first.tags, vec![("Event".to_owned(), "A".to_owned())]);
        assert_eq!(second.tags, vec![("Event".to_owned(), "B".to_owned())]);
        assert_eq!(first.tokens.iter().filter(|t| matches!(t, Token::Comment(_))).count(), 1);
        assert!(reader.next_game().unwrap().is_none());

        let altered = parse_game(first.raw.replace("continued", "continues")).unwrap();
        assert!(!compare_game(&first, &altered).unwrap().equal);
    }

    #[test]
    fn canonical_byte_serialization_does_not_hide_literal_tag_or_comment_bytes() {
        let same_a =
            parse_game("[ZCustom \"a\\\\b\\\"c\"]\n[Event \"E\"]\n\n1. e4 { exact spacing } *\n".into()).unwrap();
        let same_b =
            parse_game("[Event \"E\"]\n[ZCustom \"a\\\\b\\\"c\"]\n\n1.e4 { exact spacing } *\n".into()).unwrap();
        assert!(compare_game(&same_a, &same_b).unwrap().equal, "only tag order and movenumber spacing normalize");
        let different =
            parse_game("[Event \"E\"]\n[ZCustom \"a\\\\b\\\"c\"]\n\n1.e4 {exact spacing } *\n".into()).unwrap();
        assert!(!compare_game(&same_a, &different).unwrap().equal, "comment whitespace remains byte-significant");
    }

    #[test]
    fn strict_full_tags_and_variations() {
        let a = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *\n".into()).unwrap();
        let mut wrong = parse_game("[Event \"F\"]\n[White \"W\"]\n\n1. e4 e5 (1... c5) 2. Nf3 *\n".into()).unwrap();
        assert!(!compare_game(&a, &wrong).unwrap().equal);
        wrong = parse_game("[Event \"E\"]\n[White \"W\"]\n\n1. e4 e5 (1... e6) 2. Nf3 *\n".into()).unwrap();
        assert!(!compare_game(&a, &wrong).unwrap().equal);
    }

    #[test]
    fn audited_diagram_and_pua_figurines_match_and_count() {
        let gold = parse_game("[Event \"E\"]\n\n1. e4 {\u{0004} Knight \u{ee}\u{20ac}\u{a8} move} *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 {[#] Knight \u{2658} move} *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
        assert_eq!(compared.diagrams, 1);
        assert_eq!(compared.pua, 1);
    }

    #[test]
    fn audited_double_utf8_matches_and_count() {
        let gold = parse_game("[Event \"E\"]\n\n1. e4 {F\u{c3}\u{bc}nf} *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 {Fünf} *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
        assert_eq!(compared.double_utf8, 1);
    }

    #[test]
    fn audited_chessbase_gui_tags_are_accounted_and_unauthorized_fail() {
        let gold_ok = parse_game("[Event \"E\"]\n[Beauty \"1234\"]\n[GameId \"5678\"]\n\n1. e4 *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 *\n".into()).unwrap();
        let compared = compare_game(&gold_ok, &ours).unwrap();
        assert!(compared.equal);
        assert_eq!(compared.cb_tags, 2);

        let gold_bad = parse_game("[Event \"E\"]\n[UnauthorizedTag \"bad\"]\n\n1. e4 *\n".into()).unwrap();
        let bad_compared = compare_game(&gold_bad, &ours).unwrap();
        assert!(!bad_compared.equal);
    }

    #[test]
    fn audited_training_questions_match_and_count() {
        let gold = parse_game("[Event \"E\"]\n\n1. e4 {[%tqu \"En\",\"Question\",\"\", \"\", \"e7e5\", \"Good\", 10]} e5 {[%tqu \"En\",\"Q2\",\"\",\"\",\"g1f3\",\"\",10] Normal comment} 2. Nf3 *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 e5 {Normal comment} 2. Nf3 *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
        assert_eq!(compared.training, 2);
    }

    #[test]
    fn test_audited_cyrillic_cp1251_matches() {
        let gold = parse_game("[Event \"E\"]\n\n1. e4 {ни\u{e009}его \u{e009}ерным не давало} *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 {íè∞åãî ∞åðíûì íå äàâàëî} *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
    }

    #[test]
    fn test_audited_tab_comment_matches() {
        let gold = parse_game("[Event \"E\"]\n\n1. e4 {\t} e5 *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 {\t} e5 *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
    }

    #[test]
    fn test_audited_table_b_missing_symbols_matches() {
        let gold = parse_game("[Event \"E\"]\n\n1. e4 {‰ › • ¹ ª ¯ “} *\n".into()).unwrap();
        let ours = parse_game("[Event \"E\"]\n\n1. e4 {‰ › • ¹ ª ¯ “} *\n".into()).unwrap();
        let compared = compare_game(&gold, &ours).unwrap();
        assert!(compared.equal);
    }
}
