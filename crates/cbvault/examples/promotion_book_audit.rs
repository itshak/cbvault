//! Compare a CTG book against independently enumerated source PGN move edges.
//! A missing edge is reported, never treated as a successfully decoded move.
#![allow(missing_docs)]
use cbvault::book::Book;
use gigachess::fen::parse_fen;
use std::io::{self, BufRead};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: promotion_book_audit <book.ctg> < edges.tsv")?;
    let strict = std::env::args().skip(2).any(|a| a == "--strict");
    let book = Book::open(path)?;
    let mut failures = 0u64;
    let mut counts = std::collections::BTreeMap::<&str, u64>::new();
    println!("case\tfen\texpected\tstatus\tchild_present\tstored_moves");
    for line in io::stdin().lock().lines() {
        let line = line?;
        let mut fields = line.split('\t');
        let label = fields.next().ok_or("missing case")?;
        let fen = fields.next().ok_or("missing FEN")?;
        let expected = fields.next().ok_or("missing move")?;
        if fields.next().is_some() {
            return Err("unexpected field".into());
        }
        let board = parse_fen(fen).map_err(|_| format!("bad FEN: {fen}"))?;
        let legal = board.legal_moves();
        let mv = legal
            .iter()
            .find(|m| m.to_string() == expected)
            .copied()
            .ok_or_else(|| format!("source move not legal in Gigachess: {fen}: {expected}"))?;
        let child = book.probe_after(&board, mv)?.is_some();
        let (status, stored) = if let Some(p) = book.probe(&board)? {
            let known = p.moves.iter().any(|m| m.mv == Ok(mv));
            let unknown = p.moves.iter().any(|m| m.mv.is_err());
            let values = p
                .moves
                .iter()
                .map(|m| {
                    let decoded = m.mv.map_or_else(|e| format!("ERR:{e}"), |x| x.to_string());
                    format!("{:02x}:{}", m.byte, decoded)
                })
                .collect::<Vec<_>>()
                .join(",");
            let status = if known {
                "decoded"
            } else if unknown {
                "unknown_or_missing"
            } else {
                "missing_or_wrong_move"
            };
            (status, values)
        } else {
            ("missing_parent", String::new())
        };
        *counts.entry(status).or_default() += 1;
        if status != "decoded" || !child {
            failures += 1;
        }
        println!("{label}\t{fen}\t{expected}\t{status}\t{child}\t{stored}");
    }
    eprintln!("coverage={counts:?}; damaged_pages={}", book.damaged().len());
    if strict && (failures != 0 || !book.damaged().is_empty()) {
        return Err(format!("{failures} missing, wrong or undecoded edges").into());
    }
    Ok(())
}
