// Copyright (c) 2026 the cbvault contributors (MIT). See LICENSE.
//! Gold-standard PGN check (local-only, not a committed test): compare our
//! PGN for the head of the local Mega Database 2025 against
//! `Mega Database 2025/test_games.pgn`, exported by ChessBase itself.
//!
//! Scope: every game, annotated ones included — the annotations ride the
//! `.cba` file and are compared as ChessBase writes them (task 4.2). Within
//! scope the comparison is strict: the Seven Tag Roster (Event, Site, Date,
//! Round, White, Black, Result) plus our emitted ECO/WhiteElo/BlackElo/SetUp/FEN,
//! and the full movetext token stream (SAN, `--`, result, `{comment}` with its
//! text, `(`/`)`, `$NAG`). Formatting (line breaks, spacing, trailing
//! `#`/`+`, whitespace inside comments), tag order and ChessBase-only tags
//! (PlyCount, GameId, ...) are normalized, not compared — as is ChessBase's
//! `Z0` null-move spelling (mapped to `--`). ChessBase over-disambiguates SAN
//! (`Nce7` where the twin knight is pinned); such diffs are classified, not
//! failed. Diffs are reported by category with up to three samples each.
//!
//! Usage: `cargo run -p cbvault --release --example gold_pgn --
//! [--limit N] [--out diff.txt]` (defaults: all gold games).
//! Reads `CBVAULT_TEST_DB` (default: `Mega Database 2025/Mega Database 2025`)
//! and `test_games.pgn` beside it. Prints a summary and the first diffs.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use cbvault::pgn::PgnWriter;
use cbvault_format::cbh::moves::GameMoves;
use cbvault_format::cbh::{Annotations, Entities, GameHeader, Headers, Wide};
use cbvault_format::file::DbFile;
use cbvault_format::game::RecordKind;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut limit = usize::MAX;
    let mut out_path: Option<PathBuf> = None;
    let mut ours_path: Option<PathBuf> = None;
    let mut dump_path: Option<PathBuf> = None;
    let mut parallel: usize = 0;
    let mut force_2cbh = false;
    let mut force_cbh = false;
    let mut probe: Vec<u32> = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--2cbh" => force_2cbh = true,
            "--cbh" => force_cbh = true,
            "--limit" => limit = args.next().expect("--limit N").parse().expect("a number"),
            "--out" => out_path = args.next().map(PathBuf::from),
            "--ours" => ours_path = args.next().map(PathBuf::from),
            "--dump" => dump_path = args.next().map(PathBuf::from),
            "--parallel" => {
                parallel = args.next().expect("--parallel N").parse().expect("a thread count");
            }
            "--probe" => {
                probe = args
                    .next()
                    .expect("--probe 1,2,3")
                    .split(',')
                    .map(|r| r.trim().parse().expect("a record id"))
                    .collect();
            }
            other => {
                panic!(
                    "unknown flag {other} (want --2cbh / --cbh / --limit N / --out FILE / --ours FILE / --dump FILE / --probe IDS)"
                )
            }
        }
    }
    let use_2cbh = if force_2cbh {
        true
    } else if force_cbh {
        false
    } else {
        std::env::var("CBVAULT_TEST_DB_2CBH").is_ok()
            || std::env::var("CBVAULT_2CBH_GOLD_PGN").is_ok()
            || Path::new("/Users/ais/Documents/ChessBase/Bases/Mega2026/Mega Database 2026.2cbh").exists()
    };
    let (base, gold_path) = if use_2cbh {
        let db = std::env::var("CBVAULT_TEST_DB_2CBH")
            .unwrap_or_else(|_| "/Users/ais/Documents/ChessBase/Bases/Mega2026/Mega Database 2026.2cbh".to_string());
        let gold = std::env::var("CBVAULT_2CBH_GOLD_PGN").unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures-local/mega-db/mega26_2cbh_export.pgn")
                .display()
                .to_string()
        });
        (cbvault::stem_of(Path::new(&db)), PathBuf::from(gold))
    } else {
        let base = std::env::var("CBVAULT_TEST_DB").unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../Mega Database 2025/Mega Database 2025")
                .display()
                .to_string()
        });
        let base = PathBuf::from(base);
        let gold_path = sibling_pgn(&base);
        (base, gold_path)
    };
    if parallel > 1 {
        // The parallel pipeline writes the whole database; the comparison then
        // reads the games the gold file holds (`--ours`), so the gate is the
        // same one the sequential writer passes (task 5.3).
        let prefix = if use_2cbh { "2cbh" } else { "cbh" };
        let path = std::env::temp_dir().join(format!("{prefix}-parallel-{parallel}.pgn"));
        let mut file = std::fs::File::create(&path).expect("the parallel dump");
        let t0 = std::time::Instant::now();
        let (stats, failures) = if limit < usize::MAX {
            cbvault::pgn::export_span_report(&base, &mut file, parallel, 8192, 20, 1, (limit + 1000) as u32)
                .expect("the parallel export")
        } else {
            cbvault::pgn::export_span_report(&base, &mut file, parallel, 8192, 20, 1, 0).expect("the parallel export")
        };
        drop(file);
        eprintln!(
            "parallel export: {} games, {} failures: {:?}, {} bytes, {:.1} s on {parallel} threads",
            stats.games,
            stats.failures,
            failures,
            stats.bytes,
            t0.elapsed().as_secs_f64()
        );
        ours_path = Some(path);
    }
    let report = if use_2cbh {
        run_2cbh(&base, &gold_path, limit, ours_path.as_deref(), dump_path.as_deref(), &probe)
    } else {
        run(&base, &gold_path, limit, ours_path.as_deref(), dump_path.as_deref(), &probe)
    };
    let mut out: Box<dyn Write> = match out_path {
        Some(p) => Box::new(std::fs::File::create(p).expect("out file")),
        None => Box::new(std::io::stdout()),
    };
    writeln!(out, "{}", report.text()).expect("report");
}

/// The gold PGN beside the database stem (`test_games.pgn`).
fn sibling_pgn(base: &Path) -> PathBuf {
    let stem = match base.extension() {
        Some(e) if e.eq_ignore_ascii_case("cbh") => base.with_extension(""),
        _ => base.to_owned(),
    };
    let dir = stem.parent().unwrap_or(Path::new("."));
    let candidate = dir.join("test_games.pgn");
    assert!(candidate.exists(), "gold file missing: {}", candidate.display());
    candidate
}

/// Samples kept per diff category.
/// How many differing games per category the report keeps as samples.
const SAMPLES_PER_CATEGORY: usize = 12;

struct Report {
    gold_games: usize,
    compared: usize,
    /// Of the compared, those ChessBase's export holds annotations for.
    compared_annotated: usize,
    matched: usize,
    skipped_decode_err: usize,
    /// Games whose `.cba` record we could not read: not comparable.
    skipped_annotation_err: usize,
    /// Diff counts by category: `(category, count, up to N samples)`.
    counts: Vec<(String, usize, Vec<String>)>,
    /// Character pairs inside equal-length differing comments: what we write
    /// where ChessBase writes something else, for the export's character
    /// table.
    char_pairs: HashMap<(char, char), u64>,
    /// Char-level signatures of differing comments — the differing middle of
    /// the two, escaped — so every remaining class is named, not sampled:
    /// `(signature, count, up to N samples)`.
    signatures: Vec<(String, usize, Vec<String>)>,
}

impl Report {
    fn bump_signature(&mut self, signature: String, sample: String) {
        match self.signatures.iter_mut().find(|(s, _, _)| *s == signature) {
            Some((_, n, samples)) => {
                *n += 1;
                if samples.len() < SAMPLES_PER_CATEGORY {
                    samples.push(sample);
                }
            }
            None => self.signatures.push((signature, 1, vec![sample])),
        }
    }
}

impl Report {
    fn bump(&mut self, category: &str, sample: String) {
        match self.counts.iter_mut().find(|(c, _, _)| c == category) {
            Some((_, n, samples)) => {
                *n += 1;
                if samples.len() < SAMPLES_PER_CATEGORY {
                    samples.push(sample);
                }
            }
            None => self.counts.push((category.to_owned(), 1, vec![sample])),
        }
    }

    fn diffs(&self) -> usize {
        self.counts.iter().map(|(_, n, _)| n).sum()
    }

    fn text(&self) -> String {
        let mut text = format!(
            "gold games: {} | in scope: {} (annotated: {}, annotation read errors: {}, decode errors: {}) | matched: {} | diffs: {}\n",
            self.gold_games,
            self.compared,
            self.compared_annotated,
            self.skipped_annotation_err,
            self.skipped_decode_err,
            self.matched,
            self.diffs()
        );
        let mut counts = self.counts.iter().collect::<Vec<_>>();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (category, count, samples) in counts {
            text.push_str(&format!("{count} x {category}\n"));
            for sample in samples {
                for line in sample.lines() {
                    text.push_str(&format!("    {line}\n"));
                }
            }
        }
        if !self.char_pairs.is_empty() {
            let mut pairs = self.char_pairs.iter().collect::<Vec<_>>();
            pairs.sort_by(|a, b| b.1.cmp(a.1));
            text.push_str("comment characters (ours -> ChessBase's export):\n");
            for ((ours, gold), n) in pairs {
                text.push_str(&format!(
                    "    {} -> {} (U+{:04X}) x{n}\n",
                    escaped(&ours.to_string()),
                    escaped(&gold.to_string()),
                    *gold as u32
                ));
            }
        }
        if !self.signatures.is_empty() {
            let mut signatures = self.signatures.iter().collect::<Vec<_>>();
            signatures.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            text.push_str("comment diff signatures (differing middle):\n");
            for (signature, n, samples) in signatures {
                text.push_str(&format!("    x{n} {signature}\n"));
                for sample in samples.iter().take(3) {
                    text.push_str(&format!("        {sample}\n"));
                }
            }
        }
        text
    }
}

/// `s` with everything outside printable ASCII escaped, so the signatures name
/// the exact characters (private-use ones included).
fn escaped(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            ' '..='~' => out.push(c),
            c => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{{{:04X}}}", c as u32);
            }
        }
    }
    out
}

/// The differing middle of two comments, escaped: what a comment's diff is,
/// with the common prefix and suffix cut away.
fn comment_signature(gold: &str, ours: &str) -> String {
    let (g, o) = (gold.chars().collect::<Vec<_>>(), ours.chars().collect::<Vec<_>>());
    let mut p = 0;
    while p < g.len() && p < o.len() && g[p] == o[p] {
        p += 1;
    }
    let mut s = 0;
    while s < g.len() - p && s < o.len() - p && g[g.len() - 1 - s] == o[o.len() - 1 - s] {
        s += 1;
    }
    let clip = |v: &[char]| -> String {
        let text: String = v.iter().collect();
        if text.chars().count() > 48 {
            let head: String = text.chars().take(24).collect();
            let tail: String = text.chars().rev().take(24).collect::<Vec<_>>().into_iter().rev().collect();
            format!("{}…{}", escaped(&head), escaped(&tail))
        } else {
            escaped(&text)
        }
    };
    let gold_middle = &g[p..g.len() - s];
    let ours_middle = &o[p..o.len() - s];
    format!("at {p} (len {} vs {}): gold=[{}] ours=[{}]", g.len(), o.len(), clip(gold_middle), clip(ours_middle))
}
/// A gold game: tag pairs plus movetext tokens (move numbers dropped, SAN
/// body / result kept; `{comment}`, `(`, `)`, `$NAG` kept as scope markers
/// so annotated games can be excluded; ChessBase's null-move spelling `Z0`
/// normalized to our `--`).
struct GoldGame {
    tags: Vec<(String, String)>,
    tokens: Vec<String>,
}

/// Streaming reader for gold PGN files that decodes one game at a time with O(1) memory.
struct GoldReader<R: BufRead> {
    reader: R,
    byte_buf: Vec<u8>,
    pending_tag: Option<String>,
    eof: bool,
    first_line: bool,
}

impl<R: BufRead> GoldReader<R> {
    fn new(reader: R) -> Self {
        Self { reader, byte_buf: Vec::new(), pending_tag: None, eof: false, first_line: true }
    }

    fn next_game(&mut self) -> Option<GoldGame> {
        if self.eof && self.pending_tag.is_none() {
            return None;
        }

        let mut tags: Vec<(String, String)> = Vec::new();
        let mut tokens: Vec<String> = Vec::new();
        let mut in_movetext = false;

        if let Some(tag_line) = self.pending_tag.take()
            && let Some((k, v)) = parse_tag(&tag_line)
        {
            tags.push((k, v));
        }

        loop {
            self.byte_buf.clear();
            match self.reader.read_until(b'\n', &mut self.byte_buf) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("error reading gold pgn: {e}");
                    self.eof = true;
                    break;
                }
            };

            let line_str = cbvault_format::codepage::decode_auto(&self.byte_buf);
            let mut line = line_str.trim_end_matches(&['\r', '\n'][..]);
            if self.first_line {
                self.first_line = false;
                line = line.strip_prefix('\u{feff}').unwrap_or(line);
            }
            let trimmed = line.trim();

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if in_movetext || (!tags.is_empty() && trimmed.starts_with("[Event ")) {
                    self.pending_tag = Some(trimmed.to_string());
                    break;
                }
                if let Some((k, v)) = parse_tag(trimmed) {
                    tags.push((k, v));
                }
                continue;
            }

            if trimmed.is_empty() {
                continue;
            }

            in_movetext = true;
            tokens.extend(tokenize_movetext(trimmed));
        }

        if tags.is_empty() && tokens.is_empty() { None } else { Some(GoldGame { tags, tokens }) }
    }
}

/// Parses gold games streaming: tag lines grouped with the movetext that follows.
fn parse_gold(path: &Path, limit: usize) -> Vec<GoldGame> {
    let file = std::fs::File::open(path).expect("gold pgn file opens");
    let mut reader = GoldReader::new(std::io::BufReader::with_capacity(1 << 20, file));
    let mut games = Vec::new();
    while games.len() < limit {
        match reader.next_game() {
            Some(g) => games.push(g),
            None => break,
        }
    }
    games
}

fn parse_tag(line: &str) -> Option<(String, String)> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let sp = inner.find(' ')?;
    let key = inner[..sp].to_owned();
    let value = inner[sp + 1..].trim().to_owned();
    let value = value.strip_prefix('"')?.strip_suffix('"')?.to_owned();
    Some((key, unescape(&value)))
}

fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                if n == '"' || n == '\\' || n == '[' || n == ']' {
                    out.push(n);
                } else {
                    out.push('\\');
                    out.push(n);
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Movetext tokens with formatting normalized: bare move numbers dropped,
/// `+`/`#` suffixes stripped, ChessBase's null-move spelling `Z0` mapped to
/// our `--` (upstream `cbformat` agrees: its tree builder emits `"--"` and
/// its SAN parser merely accepts both); `{...}` comments, parens and `$NAG`
/// kept as markers (they put the game out of scope).
fn tokenize_movetext(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '{' {
            // The whole comment as one token, whitespace inside collapsed:
            // both sides write the text, spacing is normalized.
            let mut text = String::from("{");
            for c in chars.by_ref() {
                if c == '}' {
                    text.push('}');
                    break;
                }
                if c.is_whitespace() {
                    if text.len() > 1 && !text.ends_with(' ') {
                        text.push(' ');
                    }
                } else {
                    text.push(c);
                }
            }
            if text.len() > 2 && text.ends_with(' ') {
                text.pop();
            }
            tokens.push(text);
        } else if c == '(' || c == ')' {
            tokens.push(c.to_string());
            chars.next();
        } else if c == '$' {
            chars.next();
            let mut n = String::from("$");
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    n.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(n);
        } else {
            let mut tok = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_whitespace() || d == '(' || d == ')' || d == '{' {
                    break;
                }
                tok.push(d);
                chars.next();
            }
            // ChessBase's exporter spells the null move `Z0`; our (and
            // upstream's) PGN spells it `--`. Everything else is compared as
            // written, move numbers and check suffixes included.
            tokens.push(if tok == "Z0" { "--".to_owned() } else { tok });
        }
    }
    tokens
}

fn has_annotations(tokens: &[String]) -> bool {
    tokens.iter().any(|t| t.starts_with('{') || t == "(" || t == ")" || t.starts_with('$'))
}
/// Our PGN for one game, parsed the same way as gold. The raw text comes
/// back too, for `--dump` (game-by-game analysis of the remaining diffs).
fn our_game(
    writer: &mut PgnWriter,
    header: &GameHeader,
    entities: &Entities,
    game: &GameMoves<'_>,
    anns: Option<&cbvault_format::cbh::GameAnnotations<'_>>,
) -> Option<(GoldGame, String)> {
    let mut buf = Vec::new();
    writer.write_game(&mut buf, header, entities, game, anns).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut tags = Vec::new();
    let mut tokens = Vec::new();
    for line in text.split('\n') {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            if let Some((k, v)) = parse_tag(line) {
                tags.push((k, v));
            }
        } else if !line.is_empty() {
            tokens.extend(tokenize_movetext(line));
        }
    }
    Some((GoldGame { tags, tokens }, text))
}

fn our_game_v2(
    writer: &mut PgnWriter,
    header: &cbvault_format::twocbh::GameHeader,
    entities: &cbvault_format::twocbh::Entities,
    game: &cbvault_format::twocbh::moves::GameMoves<'_>,
    anns: Option<&cbvault_format::twocbh::Annotations<'_>>,
) -> Option<(GoldGame, String)> {
    let mut buf = Vec::new();
    writer.write_game_v2(&mut buf, header, entities, game, anns).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut tags = Vec::new();
    let mut tokens = Vec::new();
    for line in text.split('\n') {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            if let Some((k, v)) = parse_tag(line) {
                tags.push((k, v));
            }
        } else if !line.is_empty() {
            tokens.extend(tokenize_movetext(line));
        }
    }
    Some((GoldGame { tags, tokens }, text))
}

/// Tags we emit and can compare: the Seven Tag Roster plus
/// ECO/WhiteElo/BlackElo/SetUp/FEN/Variant. ChessBase-only tags (PlyCount,
/// GameId, Annotator, ...) are out of scope by construction.
fn comparable_tags(tags: &[(String, String)]) -> Vec<(String, String)> {
    const KEEP: [&str; 13] = [
        "Event", "Site", "Date", "Round", "White", "Black", "Result", "ECO", "WhiteElo", "BlackElo", "SetUp", "FEN",
        "Variant",
    ];
    tags.iter().filter(|(k, _)| KEEP.contains(&k.as_str())).cloned().collect()
}

fn context(tokens: &[&str], at: Option<usize>) -> String {
    let i = at.unwrap_or(tokens.len().saturating_sub(1));
    let lo = i.saturating_sub(5);
    let hi = (i + 6).min(tokens.len());
    tokens[lo..hi].join(" ")
}

/// The tag keys on which `want` and `got` differ, with a marker for tags
/// present on only one side.
fn differing_tag_keys(want: &[(String, String)], got: &[(String, String)]) -> Vec<String> {
    let mut keys = Vec::new();
    for (k, v) in want {
        match got.iter().find(|(k2, _)| k2 == k) {
            Some((_, v2)) if v2 == v => {}
            Some(_) => keys.push(format!("{k}(value)")),
            None => keys.push(format!("{k}(not-in-ours)")),
        }
    }
    for (k, _) in got {
        if !want.iter().any(|(k2, _)| k2 == k) {
            keys.push(format!("{k}(not-in-gold)"));
        }
    }
    keys
}

/// Whether `gold` and `ours` movetexts differ only by ChessBase's
/// over-disambiguation: extra file/rank qualifiers on SAN moves (`Nce7` for
/// our `Ne7`). ChessBase disambiguates when a twin merely *attacks* the
/// square, even when the twin is pinned and the disambiguator is redundant
/// under the standard (legal-move) rule; every sample checked by hand is such
/// a case, where our shorter form is the standard one.
fn movetext_over_disambiguated(gold: &[&str], ours: &[&str]) -> bool {
    gold.len() == ours.len()
        && gold.iter().zip(ours).any(|(a, b)| a != b)
        && gold.iter().zip(ours).all(|(a, b)| a == b || redundant_qualifier(a, b))
}

/// `gold` is `ours` with one or two file/rank characters inserted after the
/// piece letter (and nothing else changed).
fn redundant_qualifier(gold: &str, ours: &str) -> bool {
    let g: Vec<char> = gold.chars().collect();
    let o: Vec<char> = ours.chars().collect();
    if g.len() <= o.len() || g.len() - o.len() > 2 || g[0] != o[0] {
        return false;
    }
    let extra = g.len() - o.len();
    if !g[1..1 + extra].iter().all(|c| "abcdefgh12345678".contains(*c)) {
        return false;
    }
    g[1 + extra..] == o[1..]
}

fn run(
    base: &Path,
    gold_path: &Path,
    limit: usize,
    ours_file: Option<&Path>,
    dump_path: Option<&Path>,
    probe: &[u32],
) -> Report {
    let gold = parse_gold(gold_path, limit);
    let mut dump: Box<dyn Write> = match dump_path {
        Some(p) => Box::new(std::fs::File::create(p).expect("dump file")),
        None => Box::new(std::io::sink()),
    };
    // With `--ours`, our side comes from a PGN file (the upstream writer's
    // dump, for the head-to-head) instead of our writer: same tokenizer,
    // same pairing, same categories.
    let ours_pre: Option<Vec<GoldGame>> = ours_file.map(|path| {
        let mut pre = parse_gold(path, usize::MAX);
        assert!(pre.len() >= gold.len(), "our dump holds {} games, the gold file {}", pre.len(), gold.len());
        if pre.len() > gold.len() {
            eprintln!(
                "our dump holds {} games, the gold file {}: the first {} compared",
                pre.len(),
                gold.len(),
                gold.len()
            );
        }
        pre.truncate(gold.len());
        pre
    });
    let headers = Headers::open(base).expect("headers");
    let entities = Entities::open(base).expect("entities");
    let annotations = Annotations::open(base).expect("the .cba file");
    let wide = Wide::open(base).ok();
    let cbg_path = PathBuf::from(format!("{}.cbg", base.display()));
    let cbg = DbFile::open(cbg_path.clone()).expect("cbg");
    let mut writer = PgnWriter::new();
    let mut header_buf = vec![0u8; 46 * 160];
    let mut record_buf = Vec::with_capacity(1 << 20);
    let mut ann_scratch = Vec::new();
    let mut report = Report {
        gold_games: gold.len(),
        compared: 0,
        compared_annotated: 0,
        matched: 0,
        skipped_decode_err: 0,
        skipped_annotation_err: 0,
        counts: Vec::new(),
        char_pairs: HashMap::new(),
        signatures: Vec::new(),
    };
    // Record ids are 1-based and dense; the gold export covers the head of
    // the database in order (no guiding texts in the first 420k records).
    let mut id = 1u32;
    let mut gold_idx = 0usize;
    while gold_idx < gold.len() {
        let n = headers.read_records(id, 160, &mut header_buf).expect("headers batch");
        if n == 0 {
            report.bump(
                "database: gold games left after last record",
                format!("database ends at record {id} with {} gold games left", gold.len() - gold_idx),
            );
            break;
        }
        for i in 0..n {
            if gold_idx >= gold.len() {
                break;
            }
            let off = i as usize * 46;
            let bytes: &[u8; 46] = header_buf[off..off + 46].try_into().expect("record");
            let header = GameHeader::from_bytes(id + i, bytes);
            if header.kind() != RecordKind::Game {
                continue;
            }
            let pos = gold_idx;
            let g = &gold[gold_idx];
            gold_idx += 1;
            let rendered;
            let ours: &GoldGame = match &ours_pre {
                Some(pre) => &pre[pos],
                None => {
                    let at = u64::from(header.moves_offset());
                    let mut head = [0u8; 4];
                    if cbg.read_into(at, &mut head).is_err() {
                        report.skipped_decode_err += 1;
                        continue;
                    }
                    let size = u32::from_be_bytes([0, head[1], head[2], head[3]]) as usize;
                    if !(4..=(1 << 28)).contains(&size) {
                        report.skipped_decode_err += 1;
                        continue;
                    }
                    record_buf.clear();
                    record_buf.resize(size, 0);
                    if cbg.read_into(at, record_buf.as_mut_slice()).is_err() {
                        report.skipped_decode_err += 1;
                        continue;
                    }
                    let game = match GameMoves::parse(&cbg_path, record_buf.as_slice()) {
                        Ok(game) => game,
                        Err(_) => {
                            report.skipped_decode_err += 1;
                            continue;
                        }
                    };
                    let anns = match annotations.of(&header, wide.as_ref(), &mut ann_scratch) {
                        Ok(a) => a,
                        Err(_) => {
                            report.skipped_annotation_err += 1;
                            continue;
                        }
                    };
                    let Some((r, raw)) = our_game(&mut writer, &header, &entities, &game, Some(&anns)) else {
                        report.skipped_decode_err += 1;
                        continue;
                    };
                    writeln!(dump, "{}", raw.trim_end()).expect("dump");
                    writeln!(dump).expect("dump");
                    rendered = r;
                    &rendered
                }
            };
            compare_game(&mut report, header.id(), g, ours, probe);
        }
        id += n;
    }
    report
}

fn compare_game(report: &mut Report, rid: u32, g: &GoldGame, ours: &GoldGame, probe: &[u32]) {
    report.compared += 1;
    if has_annotations(&g.tokens) {
        report.compared_annotated += 1;
    }
    let want_tags = comparable_tags(&g.tags);
    let got_tags = comparable_tags(&ours.tags);
    let want_moves: Vec<&str> = g.tokens.iter().map(String::as_str).collect();
    let got_moves: Vec<&str> = ours.tokens.iter().map(String::as_str).collect();
    if probe.contains(&rid) {
        println!("probe {rid}: gold tags {:?}", g.tags.iter().take(4).collect::<Vec<_>>());
        for t in &g.tokens {
            if t.contains('\u{4}') || t.contains("[#]") {
                println!("  gold token: {:?}", t);
            }
        }
        for t in &ours.tokens {
            if t.contains('\u{4}') || t.contains("[#]") {
                println!("  ours token: {:?}", t);
            }
        }
    }
    if want_tags == got_tags && want_moves == got_moves {
        report.matched += 1;
    } else if want_tags != got_tags {
        let category = format!("tags: {}", differing_tag_keys(&want_tags, &got_tags).join(","));
        let sample = format!("game {rid}: tags differ:\n  gold: {want_tags:?}\n  ours: {got_tags:?}");
        report.bump(&category, sample);
    } else {
        let at = want_moves.iter().zip(got_moves.iter()).position(|(a, b)| a != b);
        if probe.contains(&rid) {
            match at {
                Some(i) => println!(
                    "probe {rid} differs at {i}: gold {:?}\n                       ours {:?}",
                    want_moves.get(i),
                    got_moves.get(i)
                ),
                None => {
                    let tail =
                        |v: &[&str]| -> Vec<String> { v.iter().rev().take(3).rev().map(|s| (*s).to_owned()).collect() };
                    println!(
                        "probe {rid}: {} vs {} tokens\n  gold tail {:?}\n  ours tail {:?}",
                        want_moves.len(),
                        got_moves.len(),
                        tail(&want_moves),
                        tail(&got_moves)
                    );
                }
            }
        }
        let over = movetext_over_disambiguated(&want_moves, &got_moves);
        let annotation_diff = at.is_some_and(|i| {
            let annotation = |t: &str| t.starts_with('{') || t.starts_with('$') || t == "(" || t == ")";
            annotation(want_moves[i]) || annotation(got_moves[i])
        });
        let category = if over {
            "movetext: ChessBase over-disambiguation only".to_owned()
        } else if annotation_diff {
            "movetext: annotation".to_owned()
        } else {
            "movetext: other".to_owned()
        };
        if let Some(i) = at {
            let (w, o) = (&want_moves[i], &got_moves[i]);
            if w.starts_with('{') && o.starts_with('{') {
                if w.chars().count() == o.chars().count() {
                    for (a, b) in o.chars().zip(w.chars()) {
                        if a != b {
                            *report.char_pairs.entry((a, b)).or_default() += 1;
                        }
                    }
                }
                let signature = comment_signature(w, o);
                report.bump_signature(signature, format!("game {rid}"));
            }
        }
        let sample = format!(
            "game {rid}: movetext differs at token {} ({} vs {} tokens):\n  gold: ...{}\n  ours: ...{}",
            at.map(|i| i.to_string()).unwrap_or("len".into()),
            want_moves.len(),
            got_moves.len(),
            context(&want_moves, at),
            context(&got_moves, at),
        );
        report.bump(&category, sample);
    }
}

fn run_2cbh(
    base: &Path,
    gold_path: &Path,
    limit: usize,
    ours_file: Option<&Path>,
    dump_path: Option<&Path>,
    probe: &[u32],
) -> Report {
    let gold_file = std::fs::File::open(gold_path).expect("gold file opens");
    let mut gold_reader = GoldReader::new(std::io::BufReader::with_capacity(1 << 20, gold_file));
    let mut dump: Box<dyn Write> = match dump_path {
        Some(p) => Box::new(std::fs::File::create(p).expect("dump file")),
        None => Box::new(std::io::sink()),
    };
    let mut ours_reader = ours_file.map(|path| {
        let f = std::fs::File::open(path).expect("ours file opens");
        GoldReader::new(std::io::BufReader::with_capacity(1 << 20, f))
    });

    let headers = cbvault_format::twocbh::Headers::open(base).expect("2cbh headers");
    let entities_path = cbvault_format::twocbh::sibling(base, ".2lid");
    let entities = cbvault_format::twocbh::Entities::open(entities_path).expect("2cbh entities");
    let cbg_path = cbvault_format::twocbh::sibling(base, ".2cbg");
    let cbg = cbvault_format::twocbh::Segment::open(cbg_path).expect("2cbg segment");
    let cba_path = cbvault_format::twocbh::sibling(base, ".2cba");
    let cba = cbvault_format::twocbh::Segment::open(cba_path).ok();

    let mut writer = PgnWriter::new();
    let mut report = Report {
        gold_games: 0,
        compared: 0,
        compared_annotated: 0,
        matched: 0,
        skipped_decode_err: 0,
        skipped_annotation_err: 0,
        counts: Vec::new(),
        char_pairs: HashMap::new(),
        signatures: Vec::new(),
    };

    let total_records = headers.records();
    let t0 = std::time::Instant::now();

    for rec_id in 1..=total_records {
        if report.compared >= limit {
            break;
        }

        let header = match headers.record(rec_id) {
            Ok(h) => h,
            Err(_) => {
                report.skipped_decode_err += 1;
                continue;
            }
        };

        if header.is_deleted() {
            continue;
        }

        match header.kind() {
            RecordKind::Game | RecordKind::Analysis => {}
            RecordKind::Text => continue,
            _ => continue,
        }

        let g = match gold_reader.next_game() {
            Some(g) => g,
            None => {
                report
                    .bump("database: gold games left after last record", format!("gold exhausted at record {rec_id}"));
                break;
            }
        };
        report.gold_games += 1;

        let ours = match ours_reader.as_mut() {
            Some(reader) => match reader.next_game() {
                Some(o) => o,
                None => {
                    report.bump("ours: dump exhausted before gold", format!("ours dump exhausted at record {rec_id}"));
                    break;
                }
            },
            None => {
                let seg_rec = match cbg.record_at(header.moves_offset()) {
                    Ok(r) => r,
                    Err(_) => {
                        report.skipped_decode_err += 1;
                        continue;
                    }
                };
                let moves = match cbvault_format::twocbh::moves::GameMoves::from_record(cbg.path(), seg_rec.record()) {
                    Ok(m) => m,
                    Err(_) => {
                        report.skipped_decode_err += 1;
                        continue;
                    }
                };
                let anns_rec = if header.annotations_offset() != 0 {
                    cba.as_ref().and_then(|cba| cba.record_at(header.annotations_offset()).ok())
                } else {
                    None
                };
                let anns = anns_rec.as_ref().map(|r| cbvault_format::twocbh::Annotations::from_record(r.record()));
                if probe.contains(&rec_id) {
                    println!("probe {rec_id} annotations:");
                    if let Some(a) = anns.as_ref() {
                        for (idx, it) in a.iter_items().enumerate() {
                            match it {
                                Ok(it) => {
                                    println!(
                                        "  item {idx} pos={} offset={} annot={:?}",
                                        it.position, it.offset, it.annotation
                                    )
                                }
                                Err(e) => println!("  item {idx} ERROR: {:?}", e),
                            }
                        }
                    }
                    println!("probe {rec_id} tokens:");
                    for (i, tok) in moves.tokens().enumerate() {
                        println!("  token {i}: {tok:?}");
                    }
                }
                let Some((r, raw)) = our_game_v2(&mut writer, &header, &entities, &moves, anns.as_ref()) else {
                    report.skipped_decode_err += 1;
                    continue;
                };
                writeln!(dump, "{}", raw.trim_end()).expect("dump");
                writeln!(dump).expect("dump");
                r
            }
        };

        compare_game(&mut report, rec_id, &g, &ours, probe);

        if report.compared.is_multiple_of(100_000) && report.compared > 0 {
            let elapsed = t0.elapsed().as_secs_f64();
            let gps = report.compared as f64 / elapsed;
            eprintln!(
                "compared {} games ({:.0} games/s) | matched: {} | diffs: {}",
                report.compared,
                gps,
                report.matched,
                report.diffs()
            );
        }
    }

    report
}
