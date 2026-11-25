//! Deterministic standalone PGN benchmark corpus authoring, using gigachess.
use gigachess::{
    Board, Color, Move,
    san::{move_to_san, san_to_move},
};
use serde_json::Value;
use std::{
    env, fs,
    io::{self, BufRead, Write},
};
struct R(u64);
impl R {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn n(&mut self, n: usize) -> usize {
        (self.next() % (n as u64)) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / 9007199254740992.0 < p
    }
}
struct H(Vec<(usize, u64)>, u64);
impl H {
    fn new(p: &Value, k: &str, nonzero: bool) -> Self {
        let mut v: Vec<_> = p["histograms"][k]
            .as_object()
            .unwrap()
            .iter()
            .map(|(a, b)| (a.parse::<usize>().unwrap(), b.as_u64().unwrap()))
            .filter(|(a, _)| !nonzero || *a > 0)
            .collect();
        v.sort_unstable();
        let mut sum = 0;
        for (_, n) in &mut v {
            sum += *n;
            *n = sum;
        }
        Self(v, sum)
    }
    fn get(&self, r: &mut R) -> usize {
        if self.1 == 0 {
            return 0;
        }
        let k = r.next() % self.1;
        self.0[self.0.partition_point(|(_, n)| *n <= k)].0
    }
}
struct Profile {
    a: f64,
    c: f64,
    v: f64,
    n: f64,
    comments: H,
    vars: H,
    depth: H,
    nags: H,
    len: H,
    branch: f64,
}
impl Profile {
    fn new(p: &Value) -> Self {
        let t = &p["totals"];
        let f = |k: &str| t[k].as_u64().unwrap() as f64;
        Self {
            a: f("annotated_games") / f("games"),
            c: f("commented_games") / f("annotated_games"),
            v: f("variation_games") / f("annotated_games"),
            n: f("nag_games") / f("annotated_games"),
            comments: H::new(p, "comments", true),
            vars: H::new(p, "variations", true),
            depth: H::new(p, "depth", true),
            nags: H::new(p, "nags", true),
            len: H::new(p, "comment_bytes", false),
            branch: f("variation_plies") / f("variations"),
        }
    }
}
struct Node {
    b: Board,
    m: Option<Move>,
    san: String,
    children: Vec<usize>,
    comment: String,
    nags: Vec<u8>,
    depth: usize,
}
impl Node {
    fn root(b: Board) -> Self {
        Self { b, m: None, san: String::new(), children: vec![], comment: String::new(), nags: vec![], depth: 0 }
    }
}
fn add(nodes: &mut Vec<Node>, p: usize, m: Move, san: String, depth: usize) -> Result<usize, String> {
    let mut b = nodes[p].b;
    b.play(m).map_err(|_| "illegal move".to_owned())?;
    let id = nodes.len();
    nodes.push(Node { b, m: Some(m), san, children: vec![], comment: String::new(), nags: vec![], depth });
    nodes[p].children.push(id);
    Ok(id)
}
fn clean_tokens(text: &str) -> Vec<String> {
    let mut res = Vec::new();
    let mut tok = String::new();
    let (mut comment, mut semi, mut depth) = (false, false, 0u32);
    let emit = |tok: &mut String, res: &mut Vec<String>| {
        if !tok.is_empty() {
            let s = tok.rsplit('.').next().unwrap().trim_end_matches(['!', '?']);
            if !s.is_empty() && !s.starts_with('$') && !matches!(s, "1-0" | "0-1" | "1/2-1/2" | "*") {
                res.push(s.to_owned());
            }
            tok.clear();
        }
    };
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
                emit(&mut tok, &mut res);
                comment = true
            }
            ';' => {
                emit(&mut tok, &mut res);
                semi = true
            }
            '(' => {
                emit(&mut tok, &mut res);
                depth += 1
            }
            ')' => depth = depth.saturating_sub(1),
            _c if depth > 0 => {}
            c if c.is_whitespace() => emit(&mut tok, &mut res),
            c => tok.push(c),
        }
    }
    emit(&mut tok, &mut res);
    res
}
const TEXT: &[&str] = &[
    "Compare this position with the main continuation.",
    "The alternative changes the arrangement of the pieces.",
    "Development and central control deserve consideration.",
    "Положение требует внимательного расчёта вариантов.",
    "Die Figurenstellung verändert sich nach diesem Zug.",
    "Una continuación alternativa cambia la estructura.",
    "Analyse de la position et des possibilités de développement.",
    "יש לבחון את האפשרויות בעמדה.",
];
const NAGS: &[u8] = &[1, 2, 3, 4, 5, 6, 10, 13, 14, 15, 16, 17, 18, 19, 22, 23, 32, 36];
fn render(nodes: &[Node], p: usize, chosen: usize, siblings: bool, out: &mut String) {
    use std::fmt::Write as _;
    let b = &nodes[p].b;
    let n = &nodes[chosen];
    let _ = write!(out, "{}{} {} ", b.fullmove_number(), if b.turn() == Color::White { "." } else { "..." }, n.san);
    for x in &n.nags {
        let _ = write!(out, "${} ", x);
    }
    if !n.comment.is_empty() {
        let _ = write!(out, "{{{}}} ", n.comment);
    }
    if siblings {
        for &alt in &nodes[p].children[1..] {
            out.push('(');
            render(nodes, p, alt, false, out);
            out.push_str(") ");
        }
    }
    if let Some(&child) = n.children.first() {
        render(nodes, chosen, child, true, out);
    }
}
fn game(raw: &str, p: &Profile, r: &mut R, rich: bool) -> Result<(String, usize, usize), String> {
    let mut tags = String::new();
    let mut text = String::new();
    let mut result = "*".to_string();
    let mut board = Board::startpos();
    for l in raw.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            if let Some(x) = l.strip_prefix("[Result \"").and_then(|s| s.strip_suffix("\"]")) {
                result = x.to_owned();
            }
            if let Some(x) = l.strip_prefix("[FEN \"").and_then(|s| s.strip_suffix("\"]")) {
                board = gigachess::fen::parse_fen(x).map_err(|_| "bad FEN".to_owned())?;
            }
            tags.push_str(l);
            tags.push('\n');
        } else {
            text.push_str(l);
            text.push('\n');
        }
    }
    let tokens = clean_tokens(&text);
    let mut nodes = vec![Node::root(board)];
    let mut parent = 0;
    for token in tokens {
        let m = san_to_move(&nodes[parent].b, &token).ok_or_else(|| format!("invalid public-source SAN {token}"))?;
        parent = add(&mut nodes, parent, m, token, 0)?;
    }
    let main = nodes.len() - 1;
    if main > 0 && (rich || r.chance(p.a)) {
        let u = r.next() as f64 / u64::MAX as f64;
        let c = rich || u < p.c;
        let v = rich || u > 1.0 - p.v;
        let nag = rich || r.chance(p.n);
        if v {
            let target = p.vars.get(r).min(580);
            let maxdepth = p.depth.get(r).clamp(1, 11);
            for _ in 0..target {
                for _ in 0..16 {
                    let id = if nodes.len() > main + 1 && maxdepth > 1 && r.chance(0.40) {
                        main + 1 + r.n(nodes.len() - main - 1)
                    } else {
                        r.n(main)
                    };
                    if nodes[id].children.is_empty() || nodes[id].depth >= maxdepth {
                        continue;
                    }
                    let legal = nodes[id].b.legal_moves();
                    let candidates: Vec<_> = legal
                        .iter()
                        .copied()
                        .filter(|m| !nodes[id].children.iter().any(|&i| nodes[i].m == Some(*m)))
                        .collect();
                    if candidates.is_empty() {
                        continue;
                    }
                    let dep = nodes[id].depth + 1;
                    let mut m = candidates[r.n(candidates.len())];
                    let mut par = id;
                    let len = (1 + r.n((p.branch * 2.0).round() as usize)).min(48);
                    for _ in 0..len {
                        let san = move_to_san(&nodes[par].b, m).ok_or("SAN generation failed")?.to_string();
                        par = add(&mut nodes, par, m, san, dep)?;
                        let legal = nodes[par].b.legal_moves();
                        if legal.is_empty() {
                            break;
                        }
                        m = legal[r.n(legal.len())];
                    }
                    break;
                }
            }
        }
        if c {
            let n = p.comments.get(r).min(nodes.len() - 1);
            let mut ids: Vec<_> = (1..nodes.len()).collect();
            for j in 0..n {
                let k = j + r.n(ids.len() - j);
                ids.swap(j, k);
                let nd = &mut nodes[ids[j]];
                let len = p.len.get(r);
                let multilingual = r.chance(0.37);
                let base = if len < 16 {
                    if multilingual {
                        ["Вариант.", "Variante.", "חלופה."][r.n(3)]
                    } else {
                        ["Compare.", "Idea.", "Try this."][r.n(3)]
                    }
                } else if len < 48 {
                    if multilingual {
                        ["Новая идея в позиции.", "Eine andere Möglichkeit.", "Otra continuación."][r.n(3)]
                    } else {
                        ["Compare the two lines.", "An alternative plan.", "Consider this continuation."][r.n(3)]
                    }
                } else {
                    TEXT[if multilingual { r.n(TEXT.len()) } else { r.n(3) }]
                };
                nd.comment = if len > base.len() {
                    std::iter::repeat_n(base, (len / (base.len() + 1)).max(1)).collect::<Vec<_>>().join(" ")
                } else {
                    base.to_owned()
                };
                let m = nd.m.unwrap();
                if r.chance(0.105) {
                    use std::fmt::Write as _;
                    let _ = write!(nd.comment, " [%cal G{}{}]", m.from(), m.to());
                }
                if r.chance(0.086) {
                    use std::fmt::Write as _;
                    let _ = write!(nd.comment, " [%csl Y{}]", m.to());
                }
                if rich && r.chance(0.1) {
                    nd.comment.push_str(" [%eval 0.25] [%clk 0:05:00]");
                }
            }
        }
        if nag {
            let n = p.nags.get(r).min((nodes.len() - 1) * 4);
            for _ in 0..n {
                let idx = 1 + r.n(nodes.len() - 1);
                let nd = &mut nodes[idx];
                let nag = NAGS[r.n(NAGS.len())];
                if !nd.nags.contains(&nag) {
                    nd.nags.push(nag);
                    nd.nags.sort_unstable();
                }
            }
        }
    }
    let side = nodes.len() - 1 - main;
    tags.push('\n');
    if let Some(&child) = nodes[0].children.first() {
        render(&nodes, 0, child, true, &mut tags);
    }
    tags.push_str(&result);
    tags.push_str("\n\n");
    Ok((tags, main, side))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: generator PROFILE COUNT SEED realistic|rich < source.pgn > enriched.pgn".into());
    }
    let p = Profile::new(&serde_json::from_str(&fs::read_to_string(&args[1])?)?);
    let limit: usize = args[2].parse()?;
    let mut r = R(args[3].parse()?);
    let rich = match args[4].as_str() {
        "rich" => true,
        "realistic" => false,
        _ => return Err("bad mode".into()),
    };
    let mut raw = String::new();
    let mut n = 0;
    let (mut main, mut side) = (0usize, 0usize);
    let mut w = io::BufWriter::new(io::stdout().lock());
    let process = |raw: &str,
                   r: &mut R,
                   w: &mut io::BufWriter<io::StdoutLock<'_>>,
                   n: usize,
                   main: &mut usize,
                   side: &mut usize|
     -> Result<(), Box<dyn std::error::Error>> {
        let (g, m, s) = game(raw, &p, r, rich).map_err(|e| format!("game {}: {}", n + 1, e))?;
        w.write_all(g.as_bytes())?;
        *main += m;
        *side += s;
        Ok(())
    };
    for l in io::stdin().lock().lines() {
        let l = l?;
        if l.starts_with("[Event ") && !raw.is_empty() {
            process(&raw, &mut r, &mut w, n, &mut main, &mut side)?;
            n += 1;
            raw.clear();
            if n == limit {
                break;
            }
            if n % 100000 == 0 {
                eprintln!("processed {n}");
            }
        }
        raw.push_str(&l);
        raw.push('\n');
    }
    if n < limit && !raw.trim().is_empty() {
        process(&raw, &mut r, &mut w, n, &mut main, &mut side)?;
        n += 1;
    }
    w.flush()?;
    if n != limit {
        return Err(format!("source exhausted at {n}").into());
    }
    eprintln!("games={n} mainline_plies={main} variation_plies={side}");
    Ok(())
}
