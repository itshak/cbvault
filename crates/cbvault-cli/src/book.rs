//! `cbvault book …`: ChessBase opening books (`.ctg`), read-only.
//!
//! - `book info <book>`: pages, positions indexed, index size, damaged pages.
//! - `book probe <book> [--fen FEN] [--moves M…] [--json]`: the book moves of a
//!   position, each with the statistics of the position it leads to.
//! - `book verify <book> [--children] [--json]`: checks every entry. Exits 0
//!   when the book has no damaged pages, all keys round-trip, all move bytes
//!   are known and legal, and (with `--children`) every child position is in the book.

use std::time::Instant;

use cbvault::book::{Book, BookPosition};
use gigachess::Board;

use super::{USAGE, json_str};

type CliResult = Result<bool, Box<dyn std::error::Error>>;

/// Dispatches a `book` subcommand.
pub(crate) fn run(mut args: impl Iterator<Item = String>) -> CliResult {
    match args.next().as_deref() {
        Some("info") => run_info(args),
        Some("probe") => run_probe(args),
        Some("verify") => run_verify(args),
        _ => Err(USAGE.into()),
    }
}

/// Opens a book, timing the index build.
fn open(path: &str) -> Result<(Book, f64), Box<dyn std::error::Error>> {
    let t0 = Instant::now();
    let book = Book::open(path)?;
    Ok((book, t0.elapsed().as_secs_f64()))
}

fn run_info(args: impl Iterator<Item = String>) -> CliResult {
    let mut path = None;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other if !other.starts_with('-') && path.is_none() => path = Some(other.to_string()),
            _ => return Err(USAGE.into()),
        }
    }
    let path = path.ok_or(USAGE)?;
    let (book, secs) = open(&path)?;
    if json {
        println!(
            "{{\"book\":{},\"pages\":{},\"positions\":{},\"index_bytes\":{},\"index_seconds\":{secs:.2},\"damaged_pages\":{}}}",
            json_str(&book.path().display().to_string()),
            book.pages(),
            book.positions(),
            book.index_bytes(),
            book.damaged().len()
        );
    } else {
        println!("book           {}", book.path().display());
        println!("pages          {}", book.pages());
        println!("positions      {}", book.positions());
        println!("index          {:.0} MB, built in {secs:.1} s", book.index_bytes() as f64 / 1e6);
        println!("damaged pages  {}", book.damaged().len());
        for d in book.damaged().iter().take(20) {
            println!("  page {}: {}", d.page, d.detail);
        }
    }
    Ok(book.damaged().is_empty())
}

/// Plays `moves` (SAN or UCI, gigachess spelling) from `board`.
fn play_moves(board: &mut Board, moves: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    for (i, m) in moves.iter().enumerate() {
        let mv = gigachess::san::san_to_move(board, m)
            .or_else(|| board.legal_moves().into_iter().find(|mv| mv.to_string() == *m))
            .ok_or_else(|| format!("move {} ({m}) is not legal here", i + 1))?;
        board.play(mv).map_err(|_| format!("move {} ({m}) is not legal here", i + 1))?;
    }
    Ok(())
}

/// One book move as it will be printed.
struct Row {
    byte: u8,
    annotation: u8,
    san: String,
    uci: String,
    error: Option<String>,
    child: Option<BookPosition>,
}

impl Row {
    /// Games with a result after the move (ChessBase's "N").
    fn results(&self) -> u32 {
        self.child.as_ref().map_or(0, |c| c.stats.results())
    }

    /// Score of the side that plays the move, in per cent. The child's
    /// statistics are from the opponent's side (the side to move there).
    fn score(&self) -> Option<f64> {
        self.child.as_ref()?.stats.score_percent().map(|p| 100.0 - p)
    }

    /// Share of draws, in per cent.
    fn draw_rate(&self) -> Option<f64> {
        self.child.as_ref()?.stats.draw_percent()
    }

    /// The colour ChessBase gives the move, when the combination is established.
    fn colour(&self) -> Option<&'static str> {
        let after = &self.child.as_ref()?.stats;
        cbvault_format::ctg::colour(cbvault_format::ctg::Annotation(self.annotation), after).map(|c| c.as_str())
    }

    /// The mover's average rating: in the position after the move, the side
    /// that just moved is the "other side" (ChessBase's "Av").
    fn mover_rating(&self) -> Option<u32> {
        self.child.as_ref()?.stats.opponent_average_rating()
    }
}

fn run_probe(mut args: impl Iterator<Item = String>) -> CliResult {
    let mut path = None;
    let mut fen = None;
    let mut moves = Vec::new();
    let mut json = false;
    let mut in_moves = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => {
                json = true;
                in_moves = false;
            }
            "--fen" => {
                fen = Some(args.next().ok_or("--fen requires a FEN")?);
                in_moves = false;
            }
            "--moves" => in_moves = true,
            other if in_moves && !other.starts_with("--") => moves.push(other.to_string()),
            other if !other.starts_with('-') && path.is_none() => path = Some(other.to_string()),
            _ => return Err(USAGE.into()),
        }
    }
    let path = path.ok_or(USAGE)?;
    let mut board = match &fen {
        Some(f) => gigachess::fen::parse_fen(f).map_err(|e| format!("bad FEN: {}", e.0))?,
        None => Board::startpos(),
    };
    play_moves(&mut board, &moves)?;
    let (book, secs) = open(&path)?;
    let Some(found) = book.probe(&board)? else {
        if json {
            println!("{{\"fen\":{},\"found\":false}}", json_str(&board.to_fen()));
        } else {
            println!("{}\nnot in the book", board.to_fen());
        }
        return Ok(false);
    };
    let mut rows: Vec<Row> = Vec::with_capacity(found.moves.len());
    for m in &found.moves {
        match m.mv {
            Ok(mv) => rows.push(Row {
                byte: m.byte,
                annotation: m.annotation,
                san: gigachess::san::move_to_san(&board, mv).map_or_else(|| mv.to_string(), |s| s.to_string()),
                uci: mv.to_string(),
                error: None,
                child: book.probe_after(&board, mv)?,
            }),
            Err(e) => rows.push(Row {
                byte: m.byte,
                annotation: m.annotation,
                san: String::new(),
                uci: String::new(),
                error: Some(e.to_string()),
                child: None,
            }),
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.results()));
    if json {
        print_probe_json(&board, &found, &rows);
    } else {
        print_probe_text(&board, &found, &rows, secs);
    }
    Ok(true)
}

fn print_probe_text(board: &Board, found: &BookPosition, rows: &[Row], secs: f64) {
    let s = found.stats;
    println!("{}", board.to_fen());
    println!(
        "N {} ({} games)  +{} ={} -{} for the side to move  av rating: side to move {}, other side {}",
        s.results(),
        s.games,
        s.wins,
        s.draws,
        s.losses,
        s.average_rating().map_or("-".into(), |r| r.to_string()),
        s.opponent_average_rating().map_or("-".into(), |r| r.to_string())
    );
    println!("not established (raw): score field {}, tail bytes {:02x?}", s.raw_score, s.raw_tail);
    println!("index built in {secs:.1} s\n");
    println!(
        "{:<10} {:<6} {:>10} {:>7} {:>6} {:>6}  {:<7} annot",
        "move", "uci", "N", "score", "draws", "av", "colour"
    );
    for r in rows {
        if let Some(e) = &r.error {
            println!("{:<8} {:<6} byte {:#04x}: {e}", "?", "", r.byte);
            continue;
        }
        let score = r.score().map_or("-".into(), |v| format!("{v:.1}%"));
        let draws = r.draw_rate().map_or("-".into(), |v| format!("{v:.1}%"));
        let av = r.mover_rating().map_or("-".into(), |v| v.to_string());
        let games = if r.child.is_some() { r.results().to_string() } else { "absent".into() };
        let a = cbvault_format::ctg::Annotation(r.annotation);
        let eval = r
            .child
            .as_ref()
            .and_then(|c| c.evaluation)
            .map_or(String::new(), |e| e.symbol().map_or_else(|| format!(" [eval {}]", e.0), str::to_string));
        let shown = format!("{}{}{}", r.san, a.symbol().unwrap_or(""), eval);
        println!(
            "{:<10} {:<6} {:>10} {:>7} {:>6} {:>6}  {:<7} {:#04x}",
            shown,
            r.uci,
            games,
            score,
            draws,
            av,
            r.colour().unwrap_or("?"),
            a.0
        );
    }
}

fn print_probe_json(board: &Board, found: &BookPosition, rows: &[Row]) {
    let s = found.stats;
    let items: Vec<String> = rows
        .iter()
        .map(|r| match (&r.error, &r.child) {
            (Some(e), _) => format!("{{\"byte\":{},\"error\":{}}}", r.byte, json_str(e)),
            (None, child) => {
                let child = child.as_ref().map_or("null".into(), |c| {
                    format!(
                        concat!(
                            "{{\"games\":{},\"wins\":{},\"draws\":{},\"losses\":{},\"rated_games\":{},",
                            "\"rating_sum\":{},\"opponent_rated_games\":{},\"opponent_rating_sum\":{}}}"
                        ),
                        c.stats.games,
                        c.stats.wins,
                        c.stats.draws,
                        c.stats.losses,
                        c.stats.rated_games,
                        c.stats.rating_sum,
                        c.stats.opponent_rated_games,
                        c.stats.opponent_rating_sum
                    )
                });
                format!(
                    "{{\"byte\":{},\"annotation\":{},\"symbol\":{},\"colour\":{},\"evaluation\":{},\"san\":{},\"uci\":{},\"after\":{child}}}",
                    r.byte,
                    r.annotation,
                    cbvault_format::ctg::Annotation(r.annotation).symbol().map_or("null".into(), json_str),
                    r.colour().map_or("null".into(), json_str),
                    r.child.as_ref().and_then(|c| c.evaluation).map_or("null".into(), |e| e.0.to_string()),
                    json_str(&r.san),
                    json_str(&r.uci)
                )
            }
        })
        .collect();
    println!(
        concat!(
            "{{\"fen\":{},\"found\":true,\"games\":{},\"wins\":{},\"draws\":{},\"losses\":{},",
            "\"rated_games\":{},\"rating_sum\":{},\"opponent_rated_games\":{},\"opponent_rating_sum\":{},",
            "\"raw_score\":{},\"raw_tail\":[{},{},{}],\"moves\":[{}]}}"
        ),
        json_str(&board.to_fen()),
        s.games,
        s.wins,
        s.draws,
        s.losses,
        s.rated_games,
        s.rating_sum,
        s.opponent_rated_games,
        s.opponent_rating_sum,
        s.raw_score,
        s.raw_tail[0],
        s.raw_tail[1],
        s.raw_tail[2],
        items.join(",")
    );
}

fn run_verify(args: impl Iterator<Item = String>) -> CliResult {
    let mut path = None;
    let mut json = false;
    let mut children = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            "--children" => children = true,
            other if !other.starts_with('-') && path.is_none() => path = Some(other.to_string()),
            _ => return Err(USAGE.into()),
        }
    }
    let path = path.ok_or(USAGE)?;
    let (book, index_secs) = open(&path)?;
    let t0 = Instant::now();
    let r = book.verify(children)?;
    let secs = t0.elapsed().as_secs_f64();
    let damaged_pages = r.damaged_pages.max(book.damaged().len() as u64);
    if json {
        println!(
            concat!(
                "{{\"entries\":{},\"damaged_pages\":{},\"bad_keys\":{},\"invalid_positions\":{},",
                "\"key_round_trips\":{},\"key_mismatches\":{},\"moves_decoded\":{},\"moves_unknown\":{},",
                "\"promotions\":{},\"moves_illegal\":{},\"children_checked\":{},\"children_found\":{},",
                "\"children_missing\":{},\"index_seconds\":{:.2},\"verify_seconds\":{:.2}}}"
            ),
            r.entries,
            damaged_pages,
            r.bad_keys,
            r.invalid_positions,
            r.key_round_trips,
            r.key_mismatches,
            r.moves_decoded,
            r.moves_unknown,
            r.promotions,
            r.moves_illegal,
            children,
            r.children_found,
            r.children_missing,
            index_secs,
            secs
        );
    } else {
        println!("entries            {} (index {index_secs:.1} s, verify {secs:.1} s)", r.entries);
        println!("damaged pages      {}", damaged_pages);
        println!("undecodable keys   {}", r.bad_keys);
        println!("invalid positions  {}", r.invalid_positions);
        println!("keys round-trip    {} (mismatches {})", r.key_round_trips, r.key_mismatches);
        println!("moves decoded      {}", r.moves_decoded);
        println!("moves unknown byte {}", r.moves_unknown);
        println!("  of which promote {} (always to a queen)", r.promotions);
        println!("moves illegal      {}", r.moves_illegal);
        if children {
            println!("children in book   {} (missing {})", r.children_found, r.children_missing);
        } else {
            println!("children           not checked (pass --children)");
        }
    }
    Ok(book.damaged().is_empty()
        && r.damaged_pages == 0
        && r.bad_keys == 0
        && r.invalid_positions == 0
        && r.key_mismatches == 0
        && r.moves_illegal == 0
        && r.moves_unknown == 0
        && (!children || r.children_missing == 0))
}
