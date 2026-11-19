//! `cbvault search <db> [options]`: the "Game data" page of ChessBase's search
//! dialog, as options. A thin shell over [`cbvault::query`]: it reads the
//! options into a [`GameDataQuery`], runs it, and prints the count, the first
//! matches, or their PGN.

use std::io::Write;
use std::time::Instant;

use cbvault::bridge::{Database, GameBuf};
use cbvault::query::{
    AnyText, EloDifference, EloMode, EloQuery, GameDataQuery, PlayerName, SearchOptions, TextMatch, TextsMatch,
    TimeControl, parse_range,
};

use crate::json_str;

pub(crate) const USAGE: &str = "usage:
  cbvault search <db> [criteria] [--from ID] [--to ID] [--threads N] [--limit N] [--pgn OUT] [--json]

criteria (the \"Game data\" page of ChessBase's search dialog; all must hold):
  --white \"Last, First\"   --black \"Last, First\"   --ignore-colours   --wins-only
  --names prefix|contains|exact        how player names match (default prefix)
  --tournament TEXT   --annotator TEXT   --any-text TEXT [--not]
  --texts prefix|contains|exact        how those three match (default contains)
  --year 2020-2026.11   --eco A00-E99/99   --moves 0-22
  --elo 2500-3500 [--elo-mode one|both|avg]   --elo-diff-max N   --elo-diff-min N
  --result 1-0,0-1,1/2,0-0,mate,stalemate,check   (ticked boxes; any of them)
  --text                                search guiding texts instead of games
  --time-control classical|rapid|blitz|correspondence

The count and timings go to stderr; the matches (or --json) to stdout. --limit
caps the listed matches (default 50, 0 = all); --pgn writes every match as PGN.";

/// Runs `cbvault search`.
pub(crate) fn run(mut args: impl Iterator<Item = String>) -> Result<bool, Box<dyn std::error::Error>> {
    let mut db_path: Option<String> = None;
    let mut query = GameDataQuery::default();
    let mut options = SearchOptions {
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        ..SearchOptions::default()
    };
    let mut limit = 50usize;
    let mut pgn: Option<String> = None;
    let mut json = false;
    let mut elo_mode = EloMode::One;

    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} requires a value"));
        match arg.as_str() {
            "--white" => query.white = PlayerName::parse(&value("--white")?),
            "--black" => query.black = PlayerName::parse(&value("--black")?),
            "--ignore-colours" | "--ignore-colors" => query.ignore_colours = true,
            "--wins-only" => query.wins_only = true,
            "--names" => query.names_match = text_match(&value("--names")?)?,
            "--texts" => query.texts_match = TextsMatch(text_match(&value("--texts")?)?),
            "--tournament" => query.tournament = Some(value("--tournament")?),
            "--annotator" => query.annotator = Some(value("--annotator")?),
            "--any-text" => {
                let negate = query.any_text.as_ref().is_some_and(|a| a.negate);
                query.any_text = Some(AnyText { text: value("--any-text")?, negate });
            }
            "--not" => match &mut query.any_text {
                Some(any) => any.negate = true,
                None => query.any_text = Some(AnyText { text: String::new(), negate: true }),
            },
            "--year" => query.date = value("--year")?.parse()?,
            "--eco" => query.eco = Some(value("--eco")?.parse()?),
            "--moves" => query.moves = Some(parse_range(&value("--moves")?)?),
            "--elo" => query.elo = Some(EloQuery { range: parse_range(&value("--elo")?)?, mode: EloMode::One }),
            "--elo-mode" => {
                elo_mode = match value("--elo-mode")?.as_str() {
                    "one" => EloMode::One,
                    "both" => EloMode::Both,
                    "avg" | "average" => EloMode::Average,
                    other => return Err(format!("unknown --elo-mode {other:?}").into()),
                }
            }
            "--elo-diff-max" => query.elo_difference = Some(EloDifference::AtMost(value("--elo-diff-max")?.parse()?)),
            "--elo-diff-min" => query.elo_difference = Some(EloDifference::AtLeast(value("--elo-diff-min")?.parse()?)),
            "--result" => results(&value("--result")?, &mut query)?,
            "--text" => query.text_records = true,
            "--time-control" => {
                query.time_control = Some(match value("--time-control")?.as_str() {
                    "classical" => TimeControl::Classical,
                    "rapid" => TimeControl::Rapid,
                    "blitz" => TimeControl::Blitz,
                    "correspondence" => TimeControl::Correspondence,
                    other => return Err(format!("unknown --time-control {other:?}").into()),
                })
            }
            "--from" => options.first = value("--from")?.parse()?,
            "--to" => options.last = value("--to")?.parse()?,
            "--threads" => options.threads = value("--threads")?.parse()?,
            "--limit" => limit = value("--limit")?.parse()?,
            "--pgn" => pgn = Some(value("--pgn")?),
            "--json" => json = true,
            other if other.starts_with('-') => return Err(format!("unknown option {other:?}\n\n{USAGE}").into()),
            other if db_path.is_none() => db_path = Some(other.to_owned()),
            _ => return Err(USAGE.into()),
        }
    }
    if let Some(elo) = &mut query.elo {
        elo.mode = elo_mode;
    }
    if query.any_text.as_ref().is_some_and(|a| a.text.trim().is_empty()) {
        return Err("--not needs --any-text".into());
    }

    let db = Database::open(db_path.ok_or(USAGE)?)?;
    let t0 = Instant::now();
    let found = cbvault::query::search_with(&db, &query, options)?;
    let seconds = t0.elapsed().as_secs_f64();

    if let Some(path) = &pgn {
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut buf = GameBuf::new();
        for &id in &found.ids {
            match db.game_pgn_with(id, &mut buf, true) {
                Ok(text) => {
                    out.write_all(text.as_bytes())?;
                    out.write_all(b"\n")?;
                }
                Err(e) => eprintln!("game {id}: {e}"),
            }
        }
        out.flush()?;
    }

    let shown = if limit == 0 { found.ids.len() } else { limit.min(found.ids.len()) };
    let mut buf = GameBuf::new();
    let rows: Vec<Row> = found.ids[..shown].iter().map(|&id| row(&db, id, &mut buf)).collect();
    if json {
        let items: Vec<String> = rows.iter().map(Row::json).collect();
        println!(
            "{{\"matches\":{},\"records\":{},\"replayed\":{},\"failed\":{},\"complete\":{},\"seconds\":{seconds:.3},\"games\":[{}]}}",
            found.ids.len(),
            found.records,
            found.replayed,
            found.failed,
            found.complete,
            items.join(",")
        );
    } else {
        for r in &rows {
            println!("{}\t{}\t{}\t{}\t{}\t{}", r.id, r.white, r.black, r.result, r.date, r.event);
        }
        eprintln!(
            "{} matches in {} records ({:.3} s){}{}",
            found.ids.len(),
            found.records,
            seconds,
            if found.replayed > 0 { format!(", {} replayed", found.replayed) } else { String::new() },
            if found.failed > 0 { format!(", {} unreadable", found.failed) } else { String::new() },
        );
        if shown < found.ids.len() {
            eprintln!("(listed the first {shown}; --limit 0 lists all)");
        }
    }
    Ok(found.complete)
}

/// The boxes of `--result`, comma-separated.
fn results(text: &str, query: &mut GameDataQuery) -> Result<(), String> {
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match item {
            "1-0" => query.results.white_wins = true,
            "0-1" => query.results.black_wins = true,
            "1/2" | "½" | "draw" | "1/2-1/2" => query.results.draw = true,
            "0-0" => query.results.both_lost = true,
            "mate" => query.endings.mate = true,
            "stalemate" => query.endings.stalemate = true,
            "check" => query.endings.check = true,
            other => return Err(format!("unknown result {other:?}")),
        }
    }
    Ok(())
}

fn text_match(text: &str) -> Result<TextMatch, String> {
    match text {
        "prefix" => Ok(TextMatch::Prefix),
        "contains" => Ok(TextMatch::Contains),
        "exact" => Ok(TextMatch::Exact),
        other => Err(format!("unknown match mode {other:?}")),
    }
}

/// One listed match.
struct Row {
    id: u32,
    white: String,
    black: String,
    result: &'static str,
    date: String,
    event: String,
}

impl Row {
    fn json(&self) -> String {
        format!(
            "{{\"id\":{},\"white\":{},\"black\":{},\"result\":{},\"date\":{},\"event\":{}}}",
            self.id,
            json_str(&self.white),
            json_str(&self.black),
            json_str(self.result),
            json_str(&self.date),
            json_str(&self.event)
        )
    }
}

/// The listed fields of game `id`; a record that is not a game (a guiding
/// text) or cannot be read is listed by its number alone.
fn row(db: &Database, id: u32, buf: &mut GameBuf) -> Row {
    match db.game_with(id, buf, false) {
        Ok(game) => Row {
            id,
            white: game.white.to_owned(),
            black: game.black.to_owned(),
            result: game.header.result().pgn(),
            date: game.header.played_date().pgn(),
            event: game.event.to_owned(),
        },
        Err(_) => Row {
            id,
            white: String::new(),
            black: String::new(),
            result: "*",
            date: String::new(),
            event: String::new(),
        },
    }
}
