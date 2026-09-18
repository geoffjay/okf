//! Integration tests for `okf search`: the shared query syntax, metadata
//! and body hit classes, JSON shape, `--today` staleness, `--limit`, and
//! the zero-hit exit code.

mod common;

use common::TempDir;
use std::process::Command;

fn okf() -> Command {
    Command::new(env!("CARGO_BIN_EXE_okf"))
}

/// A bundle with two concepts: one stale with a tag, one fresh and broken.
fn bundle() -> TempDir {
    let tmp = TempDir::new();
    let root = tmp.path().join("b");
    std::fs::create_dir_all(root.join("policies")).unwrap();
    std::fs::write(
        root.join("index.md"),
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Travel](policies/travel.md)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("policies/travel.md"),
        "---\n\
         type: Policy\n\
         title: Travel and expense policy\n\
         tags: [hr, finance]\n\
         status: stable\n\
         stale_after: 2026-01-01T00:00:00Z\n\
         verified: { by: human:sarah, at: 2026-06-25T09:00:00Z }\n\
         ---\n\n\
         # Reimbursement rates\n\n\
         Employees are reimbursed for personal vehicle mileage.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("policies/remote.md"),
        "---\n\
         type: Policy\n\
         title: Remote work policy\n\
         tags: [hr]\n\
         status: draft\n\
         ---\n\n\
         Remote work links to a [missing concept](missing.md).\n\
         The word mileage also appears here.\n",
    )
    .unwrap();
    tmp
}

fn run(tmp: &TempDir, args: &[&str]) -> (String, String, i32) {
    let out = okf()
        .args(["search", "--bundle"])
        .arg(tmp.path().join("b"))
        .args(args)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(0),
    )
}

#[test]
fn search_text_hits_metadata_then_bodies() {
    let tmp = bundle();
    let (stdout, _stderr, code) = run(&tmp, &["mileage"]);
    assert_eq!(code, 0);
    // "mileage" is not a subsequence of any title, so it has no metadata
    // hits — it lands purely as body hits, one per matching line.
    assert!(stdout.contains("0 metadata hit(s), 2 body hit(s)"));
    // Body rows with line numbers (body line 1 = the heading).
    assert!(stdout.contains("policies/travel:3"));
    assert!(stdout.contains("policies/remote:2"));
}

#[test]
fn search_filters_compose() {
    let tmp = bundle();
    // #hr keeps both policies; status:draft narrows to remote.
    let (stdout, _, code) = run(&tmp, &["#hr", "status:draft"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("policies/remote [draft] Remote work policy"));
    assert!(!stdout.contains("[stable] Travel"));

    // is:stale narrows to travel (stale_after 2026-01-01 < today).
    let (stdout, _, _) = run(&tmp, &["is:stale"]);
    assert!(stdout.contains("policies/travel"));
    assert!(!stdout.contains("policies/remote"));

    // is:broken narrows to remote (its missing.md link).
    let (stdout, _, _) = run(&tmp, &["is:broken"]);
    assert!(stdout.contains("policies/remote"));
    assert!(!stdout.contains("policies/travel"));

    // tier:human-reviewed narrows to travel.
    let (stdout, _, _) = run(&tmp, &["tier:human-reviewed"]);
    assert!(stdout.contains("policies/travel"));
    assert!(!stdout.contains("policies/remote"));
}

#[test]
fn search_today_pins_staleness() {
    let tmp = bundle();
    // Before the stale_after date: nothing is stale.
    let (stdout, _, _) = run(&tmp, &["is:stale", "--today", "2025-06-01"]);
    assert!(stdout.contains("0 metadata hit(s)"), "{stdout}");
    // After: travel is stale.
    let (stdout, _, _) = run(&tmp, &["is:stale", "--today", "2027-01-01"]);
    assert!(stdout.contains("policies/travel"));
}

#[test]
fn search_limit_truncates() {
    let tmp = bundle();
    // Both concepts mention mileage in their bodies; limit 1 keeps one row.
    let (stdout, _, _) = run(&tmp, &["mileage", "--limit", "1"]);
    let body_rows = stdout
        .lines()
        .filter(|l| l.contains(":2") || l.contains(":3"))
        .count();
    assert_eq!(body_rows, 1, "one body row kept: {stdout}");
}

#[test]
fn search_json_shape() {
    let tmp = bundle();
    let (stdout, _stderr, code) = run(&tmp, &["mileage", "--json"]);
    assert_eq!(code, 0);
    let val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(val["query"], "mileage");
    let body_hits = val["body_hits"].as_array().unwrap();
    assert!(
        body_hits
            .iter()
            .any(|h| h["id"].as_str() == Some("policies/travel") && h["line"].as_u64() == Some(3))
    );
    // Snippet carries the match offset for highlighting.
    assert!(body_hits.iter().all(|h| h["start"].is_u64()));
}

#[test]
fn search_zero_hits_exits_zero() {
    let tmp = bundle();
    let (stdout, _stderr, code) = run(&tmp, &["zzz-no-such-term"]);
    assert_eq!(code, 0, "read-only introspection never fails on empty");
    assert!(stdout.contains("0 metadata hit(s), 0 body hit(s)"));
}
