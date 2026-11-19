// Copyright (c) 2026 the cbvault contributors (MIT License). See LICENSE.

//! Integration tests for `cbvault search`: the options reach the query, the
//! JSON is the matches, and a bad option is an error rather than a search.

use std::process::Command;

use cbvault_fixtures::TempDb;
use cbvault_fixtures::classic::{self, Builder, Tok};
use gigachess::Board;

/// Three games: Morphy-Anderssen 1-0 (2600/2500), Anderssen-Morphy 0-1
/// (2500/2600), Carlsen-Morphy ½ (2850/2600), and a guiding text.
fn db(name: &str) -> TempDb {
    let record = classic::move_record(
        0,
        None,
        None,
        &classic::encode(&Board::startpos(), &[Tok::Mv("e2e4"), Tok::Mv("e7e5"), Tok::End], 0, false),
    );
    let mut b = Builder::new();
    let carlsen = b.player("Carlsen", "Magnus");
    for (white, black, result, elo) in
        [(0u32, 1u32, 2u8, (2600u16, 2500u16)), (1, 0, 0, (2500, 2600)), (carlsen, 0, 1, (2850, 2600))]
    {
        let h = b.game(&record);
        h[0x09..0x0c].copy_from_slice(&white.to_be_bytes()[1..]);
        h[0x0c..0x0f].copy_from_slice(&black.to_be_bytes()[1..]);
        h[0x1b] = result;
        h[0x1f..0x21].copy_from_slice(&elo.0.to_be_bytes());
        h[0x21..0x23].copy_from_slice(&elo.1.to_be_bytes());
    }
    b.text(&[(0, b"A guiding text")]);
    b.write(name)
}

fn search(temp: &TempDb, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .arg("search")
        .arg(temp.base())
        .args(args)
        .output()
        .expect("run cbvault search");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn search_options_reach_the_query() {
    let temp = db("cli_search_options");
    let (ok, json) = search(&temp, &["--white", "Morphy", "--ignore-colours", "--wins-only", "--json"]);
    assert!(ok);
    assert!(json.starts_with("{\"matches\":2,"), "{json}");
    assert!(json.contains("\"id\":1,") && json.contains("\"id\":2,"), "{json}");

    let (_, json) = search(&temp, &["--elo", "2550-", "--elo-mode", "both", "--result", "1/2", "--json"]);
    assert!(json.starts_with("{\"matches\":1,"), "{json}");
    assert!(json.contains("\"white\":\"Carlsen, Magnus\""), "{json}");

    let (_, json) = search(&temp, &["--text", "--json"]);
    assert!(json.starts_with("{\"matches\":1,"), "{json}");
}

#[test]
fn search_writes_the_matches_as_pgn() {
    let temp = db("cli_search_pgn");
    let out = temp.dir().join("found.pgn");
    let (ok, _) = search(&temp, &["--black", "Anderssen", "--pgn", out.to_str().unwrap()]);
    assert!(ok);
    let pgn = std::fs::read_to_string(&out).expect("the PGN file");
    assert_eq!(pgn.matches("[Event ").count(), 1, "{pgn}");
    assert!(pgn.contains("[Black \"Anderssen\"]"), "{pgn}");
}

#[test]
fn a_bad_option_is_an_error_not_a_search() {
    let temp = db("cli_search_bad");
    assert!(!search(&temp, &["--elo-mode", "median"]).0);
    assert!(!search(&temp, &["--year", "2026.13"]).0);
    assert!(!search(&temp, &["--wins-only"]).0, "wins only needs a White player");
    assert!(!search(&temp, &["--time-control", "blitz"]).0, "a classic database cannot tell the time control");
}
