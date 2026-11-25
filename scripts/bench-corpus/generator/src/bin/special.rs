//! Original, intentionally synthetic special-position PGN fixtures (CC0 data).
use gigachess::{
    Color,
    san::{move_to_san, san_to_move},
};
use std::io::{self, Write};
const CASES: &[(&str, &str, &str, bool)] = &[
    ("white queen promotion", "4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a8=Q+", false),
    ("white rook promotion", "4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a8=R+", false),
    ("white bishop underpromotion", "4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a8=B", false),
    ("white knight underpromotion", "4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a8=N", false),
    ("black queen promotion", "4k3/8/8/8/8/8/p7/4K3 b - - 0 1", "a1=Q+", false),
    ("white en passant", "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "exd6", false),
    ("black en passant", "4k3/8/8/8/3pP3/8/8/4K3 b - e3 0 1", "dxe3", false),
    ("white kingside castling", "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "O-O", false),
    ("white queenside castling", "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "O-O-O", false),
    ("Chess960 kingside castling", "4k3/8/8/8/8/8/8/RK5R w AH - 0 1", "O-O", true),
    ("Chess960 queenside castling", "4k3/8/8/8/8/8/8/RK5R w AH - 0 1", "O-O-O", true),
    ("mate in one", "7k/8/5KQ1/8/8/8/8/8 w - - 0 1", "Qg7#", false),
];
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    let mut count = 0;
    for rep in 0..10 {
        for &(label, fen, requested, c960) in CASES {
            let mut b = gigachess::fen::parse_fen(fen).map_err(|_| format!("bad FEN {label}"))?;
            let m = san_to_move(&b, requested).ok_or_else(|| format!("bad SAN {label}: {requested}"))?;
            let san = move_to_san(&b, m).ok_or("cannot render SAN")?.to_string();
            let num = b.fullmove_number();
            let dots = if b.turn() == Color::White { "." } else { "..." };
            let mut alternatives = Vec::new();
            for &alt in b.legal_moves().iter().filter(|&&a| a != m).take(3) {
                let s = move_to_san(&b, alt).ok_or("cannot render alternative")?.to_string();
                let mut next = b;
                next.play(alt).map_err(|_| "illegal alternative")?;
                alternatives.push(s);
            }
            b.play(m).map_err(|_| "illegal main move")?;
            let result = if label == "mate in one" { "1-0" } else { "*" };
            count += 1;
            writeln!(
                out,
                "[Event \"Synthetic cbvault fixture {count}: {label}\"]\n[Site \"Fixture laboratory\"]\n[Date \"2026.??.??\"]\n[Round \"{rep}\"]\n[White \"Synthetic White {rep}\"]\n[Black \"Synthetic Black {rep}\"]\n[Result \"{result}\"]\n[SetUp \"1\"]\n[FEN \"{fen}\"]"
            )?;
            if c960 {
                writeln!(out, "[Variant \"Chess960\"]")?;
            }
            writeln!(out, "[WhiteElo \"2400\"]\n[BlackElo \"2300\"]\n[Annotator \"Original fixture generator\"]\n")?;
            write!(
                out,
                "{num}{dots} {san} $1 {{Original test: {label}. Русский текст. Deutsch. עברית. 日本語. ♘ ± ½. [%eval 0.25] [%clk 0:02:03] [%cal G{}{}] [%csl Y{}]",
                m.from(),
                m.to(),
                m.to()
            )?;
            if rep == 9 {
                for _ in 0..125 {
                    write!(out, " Long independently authored test text.")?;
                }
            }
            write!(out, "}} ")?;
            for a in alternatives {
                write!(out, "({num}{dots} {a} $5 {{Independent alternative.}}) ")?;
            }
            writeln!(out, "{result}\n")?;
        }
    }
    eprintln!("generated {count} synthetic cases");
    Ok(())
}
