//! Streaming semantic check of source and generated PGN.
use gigachess::{Board, san::san_to_move};
use sha2::{Digest, Sha256};
use std::{
    env,
    io::{self, BufRead},
};
#[derive(Default)]
struct Counts {
    games: u64,
    main: u64,
    side: u64,
}
fn validate(raw: &str, counts: &mut Counts, digest: &mut Sha256) -> Result<(), String> {
    let mut board = Board::startpos();
    let mut result = None;
    let mut text = String::new();
    let mut header = true;
    for line in raw.lines() {
        let l = line.trim();
        if header && l.starts_with('[') {
            digest.update(l.as_bytes());
            digest.update(b"\n");
            if let Some(s) = l.strip_prefix("[FEN \"").and_then(|s| s.strip_suffix("\"]")) {
                board = gigachess::fen::parse_fen(s).map_err(|_| "invalid FEN")?;
            }
            if let Some(s) = l.strip_prefix("[Result \"").and_then(|s| s.strip_suffix("\"]")) {
                result = Some(s.to_owned());
            }
        } else if !l.is_empty() {
            header = false;
            text.push_str(l);
            text.push('\n');
        }
    }
    digest.update(b"\0");
    let (mut previous, mut stack, mut tok, mut comment, mut semi, mut seen_result) =
        (None, Vec::<(Board, Option<Board>)>::new(), String::new(), false, false, false);
    let emit = |tok: &mut String,
                board: &mut Board,
                previous: &mut Option<Board>,
                stack: &[(Board, Option<Board>)],
                counts: &mut Counts,
                digest: &mut Sha256,
                seen_result: &mut bool|
     -> Result<(), String> {
        if tok.is_empty() {
            return Ok(());
        }
        let s = tok.rsplit('.').next().unwrap().trim_end_matches(['!', '?']);
        if matches!(s, "1-0" | "0-1" | "1/2-1/2" | "*") {
            if stack.is_empty() {
                if result.as_deref() != Some(s) {
                    return Err("result differs from header".into());
                }
                *seen_result = true;
            }
        } else if !s.is_empty() && !s.starts_with('$') {
            if *seen_result && stack.is_empty() {
                return Err("move after result".into());
            }
            let m = san_to_move(board, s).ok_or_else(|| format!("invalid SAN {s}"))?;
            *previous = Some(*board);
            board.play(m).map_err(|_| format!("illegal move {s}"))?;
            if stack.is_empty() {
                counts.main += 1;
                digest.update(m.word().to_le_bytes());
            } else {
                counts.side += 1;
            }
        }
        tok.clear();
        Ok(())
    };
    macro_rules! emit {
        () => {
            emit(&mut tok, &mut board, &mut previous, &stack, counts, digest, &mut seen_result)?
        };
    }
    for c in text.chars() {
        if semi {
            if c == '\n' {
                semi = false;
            }
            continue;
        }
        if comment {
            if c == '}' {
                comment = false;
            }
            continue;
        }
        match c {
            '{' => {
                emit!();
                comment = true
            }
            ';' => {
                emit!();
                semi = true
            }
            '(' => {
                emit!();
                let prev = previous.ok_or("RAV without prior move")?;
                stack.push((board, previous));
                board = prev;
                previous = None;
            }
            ')' => {
                emit!();
                let (b, p) = stack.pop().ok_or("unmatched close RAV")?;
                board = b;
                previous = p;
            }
            '}' => return Err("unmatched close comment".into()),
            c if c.is_whitespace() => {
                emit!();
            }
            c => tok.push(c),
        }
    }
    emit!();
    if comment || !stack.is_empty() {
        return Err("unclosed comment or RAV".into());
    }
    if !seen_result {
        return Err("missing game result".into());
    }
    digest.update(b"\xff\xff");
    counts.games += 1;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limit: u64 = env::args().nth(1).ok_or("usage: validate COUNT [--exact] < PGN")?.parse()?;
    let exact = env::args().nth(2).is_some_and(|x| x == "--exact");
    if limit == 0 {
        return Err("COUNT must be positive".into());
    }
    let mut counts = Counts::default();
    let mut digest = Sha256::new();
    let mut raw = String::new();
    for l in io::stdin().lock().lines() {
        let l = l?;
        if l.starts_with("[Event ") && !raw.is_empty() {
            validate(&raw, &mut counts, &mut digest).map_err(|e| format!("game {}: {e}", counts.games + 1))?;
            raw.clear();
            if counts.games == limit {
                if exact {
                    return Err("additional game after expected count".into());
                }
                break;
            }
        }
        raw.push_str(&l);
        raw.push('\n');
    }
    if counts.games < limit && !raw.trim().is_empty() {
        validate(&raw, &mut counts, &mut digest).map_err(|e| format!("game {}: {e}", counts.games + 1))?;
    }
    if counts.games != limit {
        return Err(format!("expected {limit}, got {}", counts.games).into());
    }
    println!(
        "{{\"games\":{},\"mainline_plies\":{},\"variation_plies\":{},\"mainline_and_headers_sha256\":\"{:x}\"}}",
        counts.games,
        counts.main,
        counts.side,
        digest.finalize()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_variations_do_not_change_mainline_digest() {
        let plain = "[Event \"Test\"]\n[Result \"*\"]\n\n1. e4 e5 2. Nf3 *";
        let rich = "[Event \"Test\"]\n[Result \"*\"]\n\n1. e4 (1. d4 d5 (1... Nf6) 2. c4) e5 2. Nf3 $1 {test} *";
        let mut a = Counts::default();
        let mut b = Counts::default();
        let mut x = Sha256::new();
        let mut y = Sha256::new();
        validate(plain, &mut a, &mut x).unwrap();
        validate(rich, &mut b, &mut y).unwrap();
        assert_eq!(a.main, b.main);
        assert_eq!(b.side, 4);
        assert_eq!(x.finalize(), y.finalize());
    }
    #[test]
    fn rejects_bad_san_and_unclosed_variations() {
        for text in ["1. e5 *", "1. e4 (1. d4 *", "1. e4 {unterminated"] {
            let raw = format!("[Event \"Test\"]\n[Result \"*\"]\n\n{text}");
            assert!(validate(&raw, &mut Counts::default(), &mut Sha256::new()).is_err());
        }
    }
}
