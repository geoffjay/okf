//! `okf site` end-to-end: the binary generates a site whose dashboard numbers
//! agree with `okf info`/`okf trust`, gated on the `site` feature.
#![cfg(feature = "site")]

use std::process::Command;

fn okf_bin() -> &'static str {
    env!("CARGO_BIN_EXE_okf")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("okf-cli-site-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("policies")).unwrap();
    std::fs::write(
        dir.join("index.md"),
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("policies/travel.md"),
        "---\ntype: Policy\ntitle: Travel\nverified: { by: human:sarah, at: 2026-06-25T09:00:00Z }\n---\n\n# Travel\n\n```mermaid\nflowchart LR\n  A --> B\n```\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("policies/pto.md"),
        "---\ntype: Policy\ntitle: PTO\n---\n\n# PTO\n",
    )
    .unwrap();
    dir
}

#[test]
fn cli_site_generates_a_navigable_site_with_mermaid() {
    let dir = scratch("basic");
    let out = dir.join("site");

    let output = Command::new(okf_bin())
        .arg("site")
        .arg(&dir)
        .arg("--today")
        .arg("2026-09-11")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "okf site failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Landing page, graph, concept pages, and the vendored asset all exist.
    for rel in [
        "index.html",
        "__okf/graph.html",
        "policies/travel.html",
        "policies/pto.html",
        "policies/index.html",
        "assets/mermaid.min.js",
    ] {
        assert!(out.join(rel).is_file(), "missing {rel}");
    }

    // The diagram page loads mermaid; a clean page does not.
    let travel = std::fs::read_to_string(out.join("policies/travel.html")).unwrap();
    assert!(travel.contains(r#"<pre class="mermaid">flowchart LR"#));
    assert!(travel.contains("assets/mermaid.min.js"));
    let pto = std::fs::read_to_string(out.join("policies/pto.html")).unwrap();
    assert!(!pto.contains("assets/mermaid.min.js"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_site_dashboard_agrees_with_info() {
    let dir = scratch("numbers");

    let site = Command::new(okf_bin())
        .args(["site"])
        .arg(&dir)
        .args(["--today", "2026-09-11", "--out"])
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(site.status.success());

    let info = Command::new(okf_bin())
        .args(["info"])
        .arg(&dir)
        .args(["--today", "2026-09-11", "--json"])
        .output()
        .unwrap();
    let info_json: serde_json::Value =
        serde_json::from_slice(&info.stdout).expect("okf info --json");

    let dash = std::fs::read_to_string(dir.join("out/index.html")).unwrap();

    // Concept count agrees.
    let concepts = info_json["concepts_count"].as_u64().unwrap();
    assert_eq!(concepts, 2);
    assert!(dash.contains(&format!("Dashboard — {concepts} concept(s)")));

    // human-reviewed tier: travel is verified by a human, pto is not.
    assert!(dash.contains("human-reviewed"));
    assert!(
        dash.contains(r#"human-reviewed</dt> <dd class="num">1</dd>"#)
            || dash.contains(r#"human-reviewed</dt><dd class="num">1</dd>"#),
        "dashboard human-reviewed count should be 1: {dash}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
