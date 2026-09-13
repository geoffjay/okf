//! Integration tests: `generate` over tempdir bundles, asserting the
//! milestone acceptance criteria — mermaid interception, link rewriting,
//! panels, dashboard numbers, and the no-raw-HTML security posture.

use okf_core::Date;
use okf_web::{SiteOptions, generate};
use std::path::{Path, PathBuf};

/// A scratch bundle on disk, cleaned up on drop.
struct TestBundle {
    root: PathBuf,
}

impl TestBundle {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("okf-web-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, rel: &str, contents: impl AsRef<str>) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents.as_ref()).unwrap();
    }

    fn site(&self) -> PathBuf {
        self.root.join("site")
    }

    fn page(&self, rel: &str) -> String {
        std::fs::read_to_string(self.site().join(rel)).unwrap()
    }
}

impl Drop for TestBundle {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(name: &str, with_mermaid: bool) -> TestBundle {
    let b = TestBundle::new(name);
    b.write(
        "index.md",
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Travel](policies/travel.md)\n",
    );
    b.write(
        "log.md",
        "# Update Log\n\n## 2026-08-20\n* **Update**: Adjusted `policies/travel`.\n",
    );
    let mermaid = if with_mermaid {
        "\n```mermaid\nflowchart LR\n  Travel --> Calculator\n```\n"
    } else {
        ""
    };
    b.write(
        "policies/travel.md",
        format!(
            "---\ntype: Policy\ntitle: Travel policy\ndescription: Reimbursement rules.\n\
             status: stable\ngenerated: {{ by: agent/gemini, at: 2026-06-20T22:53:05Z }}\n\
             verified: {{ by: human:sarah, at: 2026-06-25T09:00:00Z }}\n\
             stale_after: 2026-12-31T00:00:00Z\ntags: [hr, travel]\nsources:\n  \
             - id: mileage-guide\n    resource: https://example.com/mileage\n    \
             title: Standard mileage guidelines\n---\n\n\
             # Travel policy\n\n\
             Employees are reimbursed at approved rates.[^mileage-guide]\n\n\
             See the [calculator](../computations/calc.md), [PTO](pto.md), and \
             the [missing draft](future.md).\n\n\
             ## Rates\n\nStandard rates apply.\n\n\
            [^mileage-guide]: Standard mileage reimbursement guidelines{mermaid}\n"
        ),
    );
    b.write(
        "policies/pto.md",
        "---\ntype: Policy\ntitle: PTO\n---\n\n# Paid time off\n\nSee [Travel](travel.md).\n",
    );
    b.write(
        "computations/calc.md",
        "---\ntype: Attested Computation\nruntime: sql\n---\n\n# Calc\n\n",
    );
    b
}

fn run(b: &TestBundle, today: Option<Date>) -> okf_web::SiteSummary {
    generate(SiteOptions {
        root: b.root.clone(),
        out_dir: b.site(),
        today,
    })
    .unwrap()
}

#[test]
fn generates_pages_mirroring_the_bundle_layout() {
    let b = fixture("layout", false);
    let summary = run(&b, None);
    // 3 concepts + dashboard + graph + 2 subdirectory indexes.
    assert_eq!(summary.pages, 7);
    for rel in [
        "policies/travel.html",
        "policies/pto.html",
        "computations/calc.html",
        "index.html",
        "__okf/graph.html",
        "policies/index.html",
        "computations/index.html",
    ] {
        assert!(
            b.site().join(rel).is_file(),
            "missing {rel} under {}",
            b.site().display()
        );
    }
}

#[test]
fn mermaid_pages_ship_the_asset_clean_pages_do_not() {
    let dirty = fixture("mermaid", true);
    let summary = run(&dirty, None);
    assert_eq!(summary.mermaid_pages, 1);
    assert!(dirty.site().join("assets/mermaid.min.js").is_file());

    let clean = fixture("clean", false);
    let summary = run(&clean, None);
    assert_eq!(summary.mermaid_pages, 0);
    // The graph page always has a diagram, so the asset ships anyway; but a
    // concept page without diagrams carries no mermaid script tag.
    let page = clean.page("policies/pto.html");
    assert!(
        !page.contains("assets/mermaid.min.js"),
        "clean concept page must not load mermaid"
    );
    assert!(summary.pages > 0);
}

#[test]
fn concept_page_has_mermaid_script_and_escaped_source() {
    let b = fixture("script", true);
    run(&b, None);
    let page = b.page("policies/travel.html");
    assert!(
        page.contains(r#"<pre class="mermaid">flowchart LR"#),
        "diagram source in the pre fallback"
    );
    assert!(
        page.contains(r#"<script src="../assets/mermaid.min.js">"#),
        "diagram page loads the vendored asset with the depth-correct path"
    );
    assert!(
        page.contains("mermaid.run("),
        "diagram page bootstraps mermaid"
    );
}

#[test]
fn links_navigate_between_generated_pages() {
    let b = fixture("links", false);
    run(&b, None);
    let travel = b.page("policies/travel.html");
    // Nested page links up to the computations dir.
    assert!(
        travel.contains(r#"href="../computations/calc.html""#),
        "relative rewrite keeps pages navigable: {travel}"
    );
    assert!(travel.contains(r#"href="../policies/pto.html""#));
    // Broken links are retained (permitted), pointing at the would-be page.
    assert!(
        travel.contains(r#"href="../policies/future.html""#),
        "broken link keeps a would-be target: {travel}"
    );
    // PTO's body links back to travel.
    let pto = b.page("policies/pto.html");
    assert!(
        pto.contains(r#"href="../policies/travel.html""#),
        "body link rewritten: {pto}"
    );
}

#[test]
fn frontmatter_panels_render_with_badges_and_sources() {
    let b = fixture("panels", false);
    let today = Date::parse("2026-09-11");
    run(&b, today);
    let page = b.page("policies/travel.html");

    assert!(page.contains("tier-human"), "trust tier badge");
    assert!(page.contains("status-stable"), "status badge");
    assert!(page.contains("fresh until"), "staleness badge");
    assert!(page.contains("generated"), "generated row");
    assert!(page.contains("human:sarah"), "verification row");
    // Sources panel with the URL linked safely.
    assert!(
        page.contains(r#"rel="noopener noreferrer""#),
        "external source URLs get rel attributes"
    );
    assert!(page.contains("Standard mileage guidelines"));
    assert!(page.contains("cited"), "footnote→source join shows cited");
    // TOC anchors use GitHub-style slugs.
    assert!(page.contains(r##"href="#rates""##));
    // Backlinks panel: PTO links to travel? No — travel links OUT to pto.
    // Backlinks on travel = concepts linking TO travel = index.md only, and
    // index files are not concepts, so the panel is absent; check links-out
    // panel instead.
    assert!(page.contains("Links"), "outgoing links panel present");
}

#[test]
fn script_tag_in_frontmatter_is_escaped() {
    let b = TestBundle::new("xss");
    b.write(
        "index.md",
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Evil](evil.md)\n",
    );
    b.write(
        "evil.md",
        "---\ntype: Doc\ntitle: <script>alert(1)</script>\ndescription: \"<img src=x onerror=alert(2)>\"\n---\n\n# Harmless\n",
    );
    run(&b, None);
    let page = b.page("evil.html");
    assert!(
        !page.contains("<script>alert(1)"),
        "title script must be escaped: {page}"
    );
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!page.contains("<img src=x"));
    // Producer scripts never survive: every <script> on the page is one of
    // the generator's own trusted inline bootstrap snippets (theme
    // boot/toggle), never bundle content.
    for script in page.match_indices("<script").map(|(i, _)| &page[i..]) {
        assert!(
            script.contains("localStorage.getItem('okf-theme')")
                || script.contains("document.querySelector('.theme-btn')")
                || script.contains("mermaid"),
            "unexpected script content: {script}"
        );
    }
    assert!(!page.contains("<script>alert"));
}

#[test]
fn raw_html_in_bodies_is_dropped() {
    let b = TestBundle::new("raw-html");
    b.write(
        "index.md",
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Doc](doc.md)\n",
    );
    b.write(
        "doc.md",
        "---\ntype: Doc\n---\n\n# Doc\n\nInline <b>bold</b> and:\n\n<div data-x=\"1\">block html</div>\n",
    );
    run(&b, None);
    let page = b.page("doc.html");
    let article = page
        .split("<article>")
        .nth(1)
        .unwrap_or_default()
        .split("</article>")
        .next()
        .unwrap_or_default();
    assert!(
        !article.contains("<div"),
        "raw block HTML dropped: {article}"
    );
    assert!(!article.contains("<b>bold</b>"), "raw inline HTML dropped");
}

#[test]
fn dashboard_numbers_match_trust_output() {
    let b = fixture("dashboard", false);
    let today = Date::parse("2026-09-11");
    run(&b, today);
    let dash = b.page("index.html");

    // From the fixture: tiers are travel=human, pto=unverified, calc=unverified.
    assert!(dash.contains("human-reviewed"));
    // The numbers are rendered inside <dd class="num">; assert exact counts
    // by pulling them out of the trust panel.
    let trust = dash
        .split("Trust")
        .nth(1)
        .unwrap_or_default()
        .split("Status")
        .next()
        .unwrap_or_default();
    for (needle, count) in [
        ("unverified", 2),
        ("machine-confirmed", 0),
        ("human-reviewed", 1),
    ] {
        let expected = format!(r#">{count}</dd>"#);
        let block = trust
            .split(needle)
            .nth(1)
            .unwrap_or_default()
            .split("</dl>")
            .next()
            .unwrap_or_default();
        assert!(
            block.contains(&expected),
            "{needle} count should render {count}: {block}"
        );
    }
    // As-of date pinned by --today.
    assert!(dash.contains("as of 2026-09-11"));
}

#[test]
fn graph_page_carries_flowchart_source() {
    let b = fixture("graph", false);
    run(&b, None);
    let graph = b.page("__okf/graph.html");
    assert!(graph.contains("flowchart LR"), "{graph}");
    // The source is HTML-escaped inside <pre> (the DOM decodes it back for
    // mermaid): a broken edge appears as `-.-&gt;|broken|`.
    assert!(
        graph.contains("-.-&gt;|broken|"),
        "broken edge renders as a dashed mermaid edge: {graph}"
    );
    // Phantom node for the broken target, plus the real concept.
    assert!(graph.contains("policies/travel"));
    assert!(
        graph.contains("policies/future"),
        "phantom node for broken link"
    );
}

#[test]
fn determinism_pin_applies_to_staleness() {
    let b = TestBundle::new("stale");
    b.write(
        "index.md",
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Old](old.md)\n",
    );
    b.write(
        "old.md",
        "---\ntype: Doc\nstale_after: 2026-01-01T00:00:00Z\n---\n\n# Old\n",
    );
    let today = Date::parse("2026-09-11").unwrap();
    run(&b, Some(today));
    let page = b.page("old.html");
    assert!(
        page.contains("stale since 2026-01-01"),
        "pinned --today marks stale: {page}"
    );
    // A pinned date before stale_after reads fresh.
    let fresh_day = Date::parse("2025-06-01").unwrap();
    run(&b, Some(fresh_day));
    let page = b.page("old.html");
    assert!(page.contains("fresh until"), "pre-stale pin reads fresh");
}

#[test]
fn empty_bundle_still_generates_special_pages() {
    let b = TestBundle::new("empty");
    b.write("index.md", "---\nokf_version: \"0.2\"\n---\n\n# Index\n");
    let summary = run(&b, None);
    assert!(b.site().join("index.html").is_file());
    assert!(b.site().join("__okf/graph.html").is_file());
    assert_eq!(summary.pages, 2, "dashboard + graph (no subdirectories)");
}

#[test]
fn parse_errors_do_not_abort_generation() {
    let b = TestBundle::new("broken");
    b.write(
        "index.md",
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Good](good.md)\n",
    );
    // Frontmatter fails to parse: this file becomes a parse error, not a
    // concept. Generation must still succeed for the rest.
    b.write("bad.md", "---\ntype: [unclosed\n---\n\n# Bad\n");
    b.write("good.md", "---\ntype: Doc\n---\n\n# Good\n");
    let summary = run(&b, None);
    assert!(b.site().join("good.html").is_file());
    assert_eq!(
        summary.pages, 3,
        "good + dashboard + graph (root has no subdir)"
    );
}

#[test]
fn out_dir_is_created_when_missing() {
    let b = fixture("outdir", false);
    let out = b.root.join("nested/deeper/site");
    assert!(!out.exists());
    generate(SiteOptions {
        root: b.root.clone(),
        out_dir: out.clone(),
        today: None,
    })
    .unwrap();
    assert!(out.join("index.html").is_file());
    let _ = Path::new(&b.root);
}
