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
        title: None,
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
fn code_pages_ship_shiki_and_load_it_only_on_code_pages() {
    let b = fixture("shiki", false);
    // The fixture's travel page has no code fences; add one.
    b.write(
        "policies/travel.md",
        "---\ntype: Policy\ntitle: Travel policy\n---\n\n\
             # Travel policy\n\n\
             ```rust\nfn main() {}\n```\n",
    );
    let summary = run(&b, None);
    assert_eq!(summary.code_pages, 1, "one concept page with a code fence");
    assert!(b.site().join("assets/shiki.min.js").is_file());

    let code_page = b.page("policies/travel.html");
    assert!(
        code_page.contains(r#"<script src="../assets/shiki.min.js">"#),
        "code page loads the vendored shiki with the depth-correct path"
    );
    assert!(
        code_page.contains("window.okfShiki"),
        "code page carries the shiki boot"
    );
    assert!(
        code_page.contains(r#"<code class="md-code language-rust">"#),
        "the escaped source stays as the no-JS fallback: {code_page}"
    );

    // A page without code fences loads neither shiki nor its boot.
    let plain_page = b.page("policies/pto.html");
    assert!(
        !plain_page.contains("shiki.min.js"),
        "plain page must not load shiki"
    );
    assert!(
        !plain_page.contains("okfShiki"),
        "plain page must not carry the boot"
    );

    // A bundle with no code fences at all ships no shiki asset.
    let clean = fixture("shiki-clean", false);
    let clean_summary = run(&clean, None);
    assert_eq!(clean_summary.code_pages, 0);
    assert!(
        !clean.site().join("assets/shiki.min.js").is_file(),
        "clean bundle ships no shiki"
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
                || script.contains("localStorage.getItem('okf-nav')")
                || script.contains("document.querySelector('.theme-btn')")
                || script.contains("document.getElementById('nav-toggle')")
                || script.contains("nav.querySelectorAll('.dir-toggle')")
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
fn inline_scripts_are_wellformed() {
    // The generator's trusted inline scripts are built from `\`-continued
    // Rust strings, where a JS `//` comment silently swallows the rest of
    // the script (the newline is stripped at compile time). Guard the whole
    // class: balanced braces, no line comments outside string literals,
    // and the theme toggle's wiring intact.
    let b = fixture("scripts", false);
    run(&b, None);
    let page = b.page("policies/travel.html");
    for script in page
        .split("<script>")
        .skip(1)
        .map(|s| s.split("</script>").next().unwrap_or_default())
    {
        let mut depth = 0i32;
        let mut in_str: Option<char> = None;
        let mut in_comment = false;
        let mut prev = '\0';
        for c in script.chars() {
            if in_comment {
                if prev == '*' && c == '/' {
                    in_comment = false;
                    prev = '\0';
                    continue;
                }
                prev = c;
                continue;
            }
            match in_str {
                Some(q) if c == q => in_str = None,
                Some(_) => {}
                None if c == '\'' || c == '"' => in_str = Some(c),
                None if c == '{' => depth += 1,
                None if c == '}' => depth -= 1,
                None if c == '/' && prev == '/' => {
                    // Line comments swallow the rest of the script because the
                    // Rust `\` line-continuations strip every newline.
                    panic!("line comment in trusted script: {script}");
                }
                None if c == '*' && prev == '/' => in_comment = true,
                _ => {}
            }
            prev = c;
        }
        assert_eq!(depth, 0, "unbalanced braces in trusted script: {script}");
        assert!(
            in_str.is_none(),
            "unterminated string literal in trusted script: {script}"
        );
    }
    assert!(page.contains("root.dispatchEvent(new CustomEvent('okf-theme-change'))"));
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
        title: None,
    })
    .unwrap();
    assert!(out.join("index.html").is_file());
    let _ = Path::new(&b.root);
}

#[test]
fn nav_folders_are_titled_without_a_trailing_slash() {
    let b = TestBundle::new("nav-folders");
    b.write("index.md", "---\nokf_version: \"0.2\"\n---\n\n# Index\n");
    b.write(
        "log.md",
        "# Update Log\n\n## 2026-08-20\n* **Update**: init.\n",
    );
    // Folders whose labels come from the segment name.
    b.write(
        "foo-kebab/a.md",
        "---\ntype: Concept\ntitle: A\n---\n\n# A\n",
    );
    b.write(
        "foo_snake/a.md",
        "---\ntype: Concept\ntitle: A\n---\n\n# A\n",
    );
    b.write(
        "foo space/a.md",
        "---\ntype: Concept\ntitle: A\n---\n\n# A\n",
    );

    run(&b, None);
    let dash = b.page("index.html");

    // Segment-derived labels: capitalized, delimiters become spaces, no `/`.
    assert!(
        dash.contains(r#"<a class="dir" href="foo-kebab/index.html">Foo Kebab</a>"#),
        "kebab folder label: {dash}"
    );
    assert!(
        dash.contains(r#"<a class="dir" href="foo_snake/index.html">Foo Snake</a>"#),
        "snake folder label: {dash}"
    );
    assert!(
        dash.contains(r#"<a class="dir" href="foo space/index.html">Foo Space</a>"#),
        "space folder label: {dash}"
    );
    // The old trailing-slash rendering is gone.
    assert!(
        !dash.contains("/</a>"),
        "no folder link should end in a slash: {dash}"
    );

    // The folder's own index page heading is the capitalized name, not "foo-kebab/".
    let dir_page = b.page("foo-kebab/index.html");
    assert!(
        dir_page.contains("<h1>Foo Kebab</h1>"),
        "dir heading: {dir_page}"
    );
    assert!(
        dir_page.contains("<title>Foo Kebab</title>"),
        "dir title: {dir_page}"
    );
}

#[test]
fn site_title_lives_in_the_header_and_is_configurable() {
    let b = fixture("site-title", false);

    // Default: "okf site" in the header, not in the nav sidebar.
    run(&b, None);
    let dash = b.page("index.html");
    assert!(
        dash.contains(r#"<header class="site-header">"#),
        "header present: {dash}"
    );
    let header = &dash[dash.find("<header").unwrap()..dash.find("</header>").unwrap()];
    assert!(
        header.contains(r#"<a class="site-name" href="index.html">okf site</a>"#),
        "default title in header: {header}"
    );
    assert!(
        !dash.contains(r#"class="site-title""#),
        "old nav site-title is gone: {dash}"
    );

    // Custom title via SiteOptions.title.
    generate(SiteOptions {
        root: b.root.clone(),
        out_dir: b.site(),
        today: None,
        title: Some("Foo Site".to_string()),
    })
    .unwrap();
    let dash = b.page("index.html");
    assert!(
        dash.contains(r#"<a class="site-name" href="index.html">Foo Site</a>"#),
        "custom title in header: {dash}"
    );
    // A nested page links back to the header title with the right prefix.
    let travel = b.page("policies/travel.html");
    assert!(
        travel.contains(r#"<a class="site-name" href="../index.html">Foo Site</a>"#),
        "custom title with prefix on nested page: {travel}"
    );
}

#[test]
fn nav_marks_the_current_page_active() {
    let b = fixture("nav-active", false);
    run(&b, None);

    // Concept page: its own leaf link is active; the dashboard link is not.
    let travel = b.page("policies/travel.html");
    assert!(
        travel.contains(r#"<a class="active" href="../policies/travel.html">Travel policy</a>"#),
        "current concept is active: {travel}"
    );
    assert!(
        travel.contains(r#"<a href="../index.html">Dashboard</a>"#),
        "dashboard is not active on a concept page: {travel}"
    );

    // Dashboard page: the Dashboard special link is active.
    let dash = b.page("index.html");
    assert!(
        dash.contains(r#"<a class="active" href="index.html">Dashboard</a>"#),
        "dashboard link is active on the dashboard: {dash}"
    );

    // Directory index page: the folder's own dir link is active.
    let pol = b.page("policies/index.html");
    assert!(
        pol.contains(r#"<a class="dir active" href="../policies/index.html">Policies</a>"#),
        "directory link is active on its index page: {pol}"
    );
}

/// Collapsible nav submenus: every directory row ships a caret toggle,
/// renders expanded, and is wired to the persisted closed set. The
/// stylesheet half of the contract is asserted too — the toggle only does
/// anything because the compiled `site.css` collapses on its
/// `aria-expanded` state.
#[test]
fn nav_submenus_are_collapsible_and_default_to_expanded() {
    let b = TestBundle::new("nav-collapse");
    b.write("index.md", "---\nokf_version: \"0.2\"\n---\n\n# Index\n");
    b.write(
        "log.md",
        "# Update Log\n\n## 2026-08-20\n* **Update**: init.\n",
    );
    b.write(
        "policies/travel.md",
        "---\ntype: Policy\ntitle: Travel\n---\n\n# Travel\n",
    );
    b.write(
        "policies/nested/deep.md",
        "---\ntype: Policy\ntitle: Deep\n---\n\n# Deep\n",
    );
    run(&b, None);
    let dash = b.page("index.html");
    let nav = &dash[dash.find(r#"<nav id="site-nav""#).unwrap()..dash.find("</nav>").unwrap()];

    // The heading row: link, then the caret button on the far right. The
    // `li`'s `data-dir` is the bundle path the scripts persist.
    assert!(
        nav.contains(
            r#"<li data-dir="policies"><div class="dir-row"><a class="dir" href="policies/index.html">Policies</a><button class="dir-toggle" type="button" aria-expanded="true" aria-label="Toggle Policies"><svg class="dir-caret""#
        ),
        "directory row carries its toggle: {nav}"
    );
    // Nested directories key on their full path, not the segment.
    assert!(
        nav.contains(r#"<li data-dir="policies/nested">"#),
        "nested dir keys on its full path: {nav}"
    );
    // One toggle per directory — leaf links get none.
    assert_eq!(
        nav.matches(r#"class="dir-toggle""#).count(),
        2,
        "one toggle per directory: {nav}"
    );
    // Server-rendered state is expanded, so a no-JS page shows everything.
    assert!(
        !nav.contains(r#"aria-expanded="false""#),
        "submenus render expanded: {nav}"
    );

    // The collapse rules the toggle drives, from the committed site.css
    // (inlined into every page). Regenerate with `cargo xtask tailwind`.
    assert!(
        dash.contains(".tree li:has(>.dir-row>.dir-toggle[aria-expanded=false])>ul{display:none}"),
        "stylesheet collapses closed submenus; regenerate site.css"
    );
    assert!(
        dash.contains(
            ".tree .dir-toggle[aria-expanded=false] .dir-caret{transform:rotate(-90deg)}"
        ),
        "stylesheet turns the caret right when closed; regenerate site.css"
    );

    // Persistence: the pre-paint boot reads the closed set and the toggle
    // script writes it back, dropping the boot style once aria is authoritative.
    assert!(
        dash.contains("localStorage.getItem('okf-nav-closed')"),
        "boot reads the closed set: {dash}"
    );
    assert!(
        dash.contains("localStorage.setItem(KEY, JSON.stringify(set))")
            && dash.contains("localStorage.removeItem(KEY)"),
        "toggle persists the closed set: {dash}"
    );
    assert!(
        dash.contains("document.getElementById('okf-nav-closed-style')"),
        "toggle script drops the pre-paint style: {dash}"
    );
}

/// The search asset: index shape, XSS-escaped producer content, heading
/// anchors that match the emitted ids, and lazy-load wiring in the header.
#[test]
fn search_index_asset_is_emitted_and_escaped() {
    let b = fixture("search-index", false);
    // A concept whose title would break out of a JS string if unescaped.
    b.write(
        "evil.md",
        "---\ntype: Note\ntitle: \"</script><script>alert(1)</script>\"\n---\n\n# Evil\n",
    );
    run(&b, None);

    let js = b.page("assets/search-index.js");
    assert!(
        js.starts_with("window.okfSearchIndex={"),
        "index header: {js}"
    );
    // Every < is escaped, so the literal `</script>` sequence that would
    // close an inline script tag can never occur in the file.
    assert!(
        js.contains(r"u003c/script>\u003cscript>alert(1)"),
        "escaped < in title: {}",
        &js[js.find("alert").map_or(0, |i| i.saturating_sub(80))..]
    );
    assert!(
        !js.contains("</script>\""),
        "no literal </script> anywhere the file could close a script tag: {js}"
    );
    // The client ships in the same file, after the index assignment.
    assert!(js.contains("okf site search client"), "client appended");
    // Bodies ride along for the client's substring search.
    assert!(js.contains("reimbursed at approved rates"));
}

#[test]
fn search_header_input_wires_the_lazy_boot() {
    let b = fixture("search-header", false);
    run(&b, None);

    let dash = b.page("index.html");
    assert!(
        dash.contains(r#"data-asset="assets/search-index.js""#),
        "{dash}"
    );
    assert!(dash.contains("data-prefix=\"\""), "{dash}");
    let travel = b.page("policies/travel.html");
    assert!(
        travel.contains(r#"data-asset="../assets/search-index.js""#),
        "nested prefix: {travel}"
    );
    assert!(travel.contains(r#"data-prefix="../""#));
    // The arming boot ships inline on every page.
    assert!(dash.contains("input.dataset.armed"), "{dash}");
}

#[test]
fn search_index_heading_anchors_match_emitted_ids() {
    let b = fixture("search-anchors", false);
    // Two same-text headings force -1 disambiguation, matching the writer.
    b.write(
        "dupes.md",
        "---\ntype: Note\ntitle: Duplicate headings\n---\n\n\
         # Rates\n\nFirst.\n\n## Rates\n\nSecond.\n",
    );
    run(&b, None);

    let js = b.page("assets/search-index.js");
    let idx = &js["window.okfSearchIndex=".len()..];
    // Parse the JSON tail back out: cut at the `;` that ends the assignment.
    let json = &idx[..idx.find(';').unwrap()];
    let val: serde_json::Value = serde_json::from_str(json).unwrap();

    // dupes.md: the writer emits id="rates" and id="rates-1".
    let dupes = val["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "dupes")
        .unwrap();
    let anchors: Vec<&str> = dupes["headings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["anchor"].as_str().unwrap())
        .collect();
    assert_eq!(anchors, ["rates", "rates-1"]);

    // The page's emitted ids agree with the index anchors.
    let page = b.page("dupes.html");
    assert!(page.contains("<h1 id=\"rates\""), "{page}");
    assert!(page.contains("id=\"rates-1\""), "{page}");

    // travel's heading also agrees.
    let travel = val["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "policies/travel")
        .unwrap();
    assert_eq!(travel["headings"][0]["anchor"], "travel-policy");
    let travel_page = b.page("policies/travel.html");
    assert!(travel_page.contains("<h1 id=\"travel-policy\""));
}

/// Generates with an explicit `--title`-equivalent override.
fn run_titled(b: &TestBundle, title: Option<&str>) -> okf_web::SiteSummary {
    generate(SiteOptions {
        root: b.root.clone(),
        out_dir: b.site(),
        today: None,
        title: title.map(str::to_string),
    })
    .unwrap()
}

/// The header title's precedence chain: `SiteOptions::title` (the CLI's
/// `--title`) over `site.title` in `.okf/config.yaml` over the default.
#[test]
fn config_title_is_used_and_the_option_overrides_it() {
    let b = fixture("config-title", false);
    b.write(".okf/config.yaml", "site:\n  title: Travel knowledge\n");

    run_titled(&b, None);
    let dash = b.page("index.html");
    assert!(
        dash.contains(r#"<a class="site-name" href="index.html">Travel knowledge</a>"#),
        "config title in header: {dash}"
    );

    run_titled(&b, Some("Flag Wins"));
    let dash = b.page("index.html");
    assert!(
        dash.contains(r#"<a class="site-name" href="index.html">Flag Wins</a>"#),
        "option overrides config: {dash}"
    );
}

/// Strict tooling: an unknown *key* fails the build with the section's known
/// keys, an unknown *section* is only noted, and a malformed file reports the
/// YAML line. Bundle content stays permissive — only the config is strict.
#[test]
fn config_problems_fail_the_build_or_are_noted() {
    let b = fixture("config-strict", false);

    b.write(".okf/config.yaml", "site:\n  titel: Oops\n");
    let err = generate(SiteOptions {
        root: b.root.clone(),
        out_dir: b.site(),
        today: None,
        title: None,
    })
    .unwrap_err();
    assert!(matches!(err, okf_web::SiteError::Config(_)), "{err:?}");
    let message = err.to_string();
    assert!(message.contains("unknown key `titel`"), "{message}");
    assert!(message.contains("known keys: title, fonts"), "{message}");
    assert!(message.contains(".okf/config.yaml"), "{message}");

    b.write(".okf/config.yaml", "site:\n  title: [oops\n");
    let message = generate(SiteOptions {
        root: b.root.clone(),
        out_dir: b.site(),
        today: None,
        title: None,
    })
    .unwrap_err()
    .to_string();
    assert!(message.contains("line 2"), "yaml line reported: {message}");

    b.write(
        ".okf/config.yaml",
        "studio:\n  theme: dark\nsite:\n  title: Docs\n",
    );
    let summary = run_titled(&b, None);
    assert_eq!(summary.notes.len(), 1, "{:?}", summary.notes);
    assert!(
        summary.notes[0].contains("unknown section `studio`"),
        "{:?}",
        summary.notes
    );
}

/// Fonts are opt-in: with no config the build writes no font stylesheet and
/// links none, so the committed inline CSS stays the only typography source.
#[test]
fn a_bundle_without_config_gets_no_font_assets() {
    let b = fixture("fonts-absent", true);
    run(&b, None);

    assert!(!b.site().join("assets/fonts.css").exists());
    assert!(!b.site().join("assets/fonts").exists());
    for page in ["index.html", "policies/travel.html", "__okf/graph.html"] {
        let html = b.page(page);
        assert!(
            !html.contains("<link rel=\"stylesheet\""),
            "no stylesheet link on {page}: {html}"
        );
        assert!(!html.contains("fonts.css"), "no fonts.css on {page}");
    }
}

/// Family settings become a `:root` token override in `assets/fonts.css`,
/// linked after the inline stylesheet at every page depth, and the mermaid
/// boot reads the same token so diagrams follow the prose.
#[test]
fn font_families_emit_a_linked_token_override() {
    let b = fixture("fonts-families", true);
    b.write(
        ".okf/config.yaml",
        "site:\n  fonts:\n    body: \"Newsreader, Georgia, serif\"\n    \
         code: \"JetBrains Mono, ui-monospace, monospace\"\n",
    );
    run(&b, None);

    let css = b.page("assets/fonts.css");
    assert!(
        css.contains(
            ":root {\n  --font-body: \"Newsreader\", \"Georgia\", serif;\n  \
                      --font-code: \"JetBrains Mono\", ui-monospace, monospace;\n}\n"
        ),
        "token override: {css}"
    );
    assert!(!css.contains("@font-face"), "no faces configured: {css}");

    // Linked after the inline <style>, with a depth-correct href.
    let dash = b.page("index.html");
    let style_end = dash.find("</style>").expect("inline stylesheet");
    let link = dash
        .find(r#"<link rel="stylesheet" href="assets/fonts.css">"#)
        .expect("root page links fonts.css");
    assert!(link > style_end, "link follows the inline stylesheet");
    assert!(
        b.page("policies/travel.html")
            .contains(r#"<link rel="stylesheet" href="../assets/fonts.css">"#),
        "nested page climbs one level"
    );
    assert!(
        b.page("__okf/graph.html")
            .contains(r#"<link rel="stylesheet" href="../assets/fonts.css">"#),
        "graph page climbs one level"
    );

    // Diagrams follow the body token rather than a hard-coded family.
    let graph = b.page("__okf/graph.html");
    assert!(
        graph.contains("getComputedStyle(document.documentElement)")
            && graph.contains("getPropertyValue('--font-body')")
            && graph.contains("cfg.fontFamily = font"),
        "mermaid reads the token: {graph}"
    );
}

/// Self-hosted faces: the named files are copied into `assets/fonts/` and
/// described by `@font-face` rules whose `url()` is stylesheet-relative, so
/// one href works at every page depth (and over `file://`).
#[test]
fn font_files_are_copied_and_described() {
    let b = fixture("fonts-files", false);
    b.write(".okf/fonts/newsreader-400.woff2", "wOF2-not-really");
    b.write(".okf/fonts/newsreader-700i.otf", "OTTO-not-really");
    b.write(
        ".okf/config.yaml",
        "site:\n  fonts:\n    body: Newsreader, serif\n    files:\n      \
         - family: Newsreader\n        file: newsreader-400.woff2\n        weight: 400\n      \
         - family: Newsreader\n        file: newsreader-700i.otf\n        weight: 700\n        \
         style: italic\n",
    );
    run(&b, None);

    assert_eq!(
        b.page("assets/fonts/newsreader-400.woff2"),
        "wOF2-not-really"
    );
    assert_eq!(
        b.page("assets/fonts/newsreader-700i.otf"),
        "OTTO-not-really"
    );
    let css = b.page("assets/fonts.css");
    assert!(
        css.contains(
            "@font-face {\n  font-family: \"Newsreader\";\n  \
             src: url(\"fonts/newsreader-400.woff2\") format(\"woff2\");\n  \
             font-style: normal;\n  font-weight: 400;\n  font-display: swap;\n}\n"
        ),
        "{css}"
    );
    assert!(
        css.contains("src: url(\"fonts/newsreader-700i.otf\") format(\"opentype\");")
            && css.contains("font-style: italic;"),
        "{css}"
    );

    // A face naming a file the bundle does not carry is a config error.
    b.write(
        ".okf/config.yaml",
        "site:\n  fonts:\n    files:\n      - family: Ghost\n        file: ghost.woff2\n",
    );
    let err = generate(SiteOptions {
        root: b.root.clone(),
        out_dir: b.site(),
        today: None,
        title: None,
    })
    .unwrap_err();
    assert!(matches!(err, okf_web::SiteError::Config(_)), "{err:?}");
    let message = err.to_string();
    assert!(message.contains("ghost.woff2"), "{message}");
    assert!(message.contains(".okf/fonts"), "{message}");
}
