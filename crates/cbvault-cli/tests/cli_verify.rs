// Copyright (c) 2026 the cbvault contributors (MIT License). See LICENSE.

//! Integration tests for CLI `book verify` exit status behavior.

use std::process::Command;

use cbvault_fixtures::TempDb;
use cbvault_fixtures::ctg::{connected_parent_child_book, malformed_unclosed_book, unknown_move_byte_book};

#[test]
fn cli_verify_exits_with_error_on_damaged_book() {
    let temp = TempDb::create("cli_verify_bin_damaged");
    let ctg_bytes = malformed_unclosed_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let status = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .args(["book", "verify", path.to_str().unwrap()])
        .status()
        .expect("run cbvault book verify");

    assert!(!status.success(), "cbvault book verify must exit with non-zero code on damaged book");
}

#[test]
fn cli_verify_exits_with_error_on_unknown_move() {
    let temp = TempDb::create("cli_verify_bin_unknown");
    let ctg_bytes = unknown_move_byte_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let status = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .args(["book", "verify", path.to_str().unwrap()])
        .status()
        .expect("run cbvault book verify");

    assert!(!status.success(), "cbvault book verify must exit with non-zero code on unknown moves");
}

#[test]
fn cli_verify_missing_children_policy() {
    let temp = TempDb::create("cli_verify_bin_children");
    let ctg_bytes = connected_parent_child_book();
    let path = temp.write(".ctg", &ctg_bytes);

    let status_no_children = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .args(["book", "verify", path.to_str().unwrap()])
        .status()
        .expect("run cbvault book verify without --children");
    assert!(
        status_no_children.success(),
        "cbvault book verify without --children should exit 0 even if leaf positions have missing children"
    );

    let status_children = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .args(["book", "verify", path.to_str().unwrap(), "--children"])
        .status()
        .expect("run cbvault book verify with --children");
    assert!(
        !status_children.success(),
        "cbvault book verify with --children must exit nonzero when child positions are missing"
    );
}
