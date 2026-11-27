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

#[test]
fn cli_verify_exits_nonzero_on_duplicate_keys_and_corrupt_native_cto() {
    use cbvault_chess::ctg::{key_candidates, stored_form};
    use cbvault_format::ctg::PAGE_SIZE;
    use gigachess::Board;

    let temp = TempDb::create("cli_verify_dup_and_native");
    let st = stored_form(&Board::startpos());
    let (k, l) = key_candidates(&st).next().unwrap();
    let mut entry = Vec::new();
    entry.extend_from_slice(&k[..l]);
    entry.push(1);
    entry.extend_from_slice(&[0u8; 33]);
    let mut page = vec![0u8; PAGE_SIZE];
    let pos = 4 + entry.len();
    page[0..2].copy_from_slice(&2u16.to_be_bytes());
    page[2..4].copy_from_slice(&((4 + 2 * entry.len()) as u16).to_be_bytes());
    page[4..pos].copy_from_slice(&entry);
    page[pos..pos + entry.len()].copy_from_slice(&entry);

    let mut ctg = vec![0u8; PAGE_SIZE];
    ctg.extend_from_slice(&page);
    let path = temp.write(".ctg", &ctg);

    // Duplicate keys fail verify even without --native
    let status_dup = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .args(["book", "verify", path.to_str().unwrap()])
        .status()
        .expect("run cbvault book verify on duplicate keys");
    assert!(!status_dup.success(), "duplicate keys must fail strict verify");

    // Verify --native succeeds with valid single-slot CTO/CTB even with CBVAULT_NO_MMAP=1, and fails when CTO is missing/corrupt
    let clean_path = TempDb::create("cli_verify_native_clean");
    let ctg_clean = connected_parent_child_book();
    let clean = clean_path.write(".ctg", &ctg_clean);
    let status_missing_native = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .args(["book", "verify", clean.to_str().unwrap(), "--native"])
        .status()
        .expect("run --native missing cto");
    assert!(!status_missing_native.success(), "--native must fail when .cto is missing");

    let mut cto = vec![0u8; 20];
    cto[0..4].copy_from_slice(&0x0010_0001u32.to_be_bytes());
    cto[4..8].copy_from_slice(&1u32.to_be_bytes());
    clean_path.write(".cto", &cto);
    let mut ctb = vec![0u8; 12];
    ctb[0..4].copy_from_slice(&0x000c_0001u32.to_be_bytes());
    clean_path.write(".ctb", &ctb);

    let status_ok_unmapped = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .env("CBVAULT_NO_MMAP", "1")
        .args(["book", "verify", clean.to_str().unwrap(), "--native"])
        .status()
        .expect("run --native valid no-mmap");
    assert!(status_ok_unmapped.success(), "--native must pass with valid single-slot cto/ctb under NO_MMAP");

    // Probe works cleanly on unmapped AdaptiveBook via CLI too
    let probe_out = Command::new(env!("CARGO_BIN_EXE_cbvault"))
        .env("CBVAULT_NO_MMAP", "1")
        .args(["book", "probe", clean.to_str().unwrap(), "--json"])
        .output()
        .expect("run book probe no-mmap");
    assert!(probe_out.status.success());
    let stdout = String::from_utf8_lossy(&probe_out.stdout);
    assert!(stdout.contains("\"found\":true"), "unexpected probe output: {stdout}");
}
