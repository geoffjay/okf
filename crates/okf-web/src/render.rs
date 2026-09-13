//! Maud templates: page layout, concept pages, the dashboard, the graph
//! page, directory indexes, and the nav tree.
//!
//! Every dynamic value passes through maud's escaping; the only
//! [`PreEscaped`] content is markdown bodies (escaped by pulldown-cmark) and
//! our own vendored-mermaid `<script>` block.

use crate::PagePath;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use okf_core::{Bundle, Concept, ConceptId, Date, Status, TrustTier};
use okf_validator::{Diagnostic, Report, Severity};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One page ready to write.
#[derive(Clone, Debug)]
pub struct SitePage {
    /// Where the page lands in the output tree.
    pub rel_path: PagePath,
    /// The page `<title>`.
    pub title: String,
    /// The rendered body HTML (already escaped by the pipeline).
    pub body_html: String,
    /// Whether the page carries `<pre class="mermaid">` content.
    pub has_mermaid: bool,
    /// Extra `<link>`/`<meta>` rows for the frontmatter panel, as
    /// `(label, rendered-value HTML)`; the values are maud-escaped here.
    pub meta_rows: Vec<(String, Markup)>,
}

/// Badge colors for the trust tiers, mirroring the studio's theme roles.
const fn tier_class(tier: TrustTier) -> &'static str {
    match tier {
        TrustTier::Unverified => "badge tier-unverified",
        TrustTier::MachineConfirmed => "badge tier-machine",
        TrustTier::HumanReviewed => "badge tier-human",
    }
}

/// Badge classes for lifecycle statuses.
const fn status_class(status: &Status) -> &'static str {
    match status {
        Status::Draft => "badge status-draft",
        Status::Stable => "badge status-stable",
        Status::Deprecated => "badge status-deprecated",
        Status::Other(_) => "badge status-other",
    }
}

/// The badge for a validator/lint finding.
const fn severity_class(sev: Severity) -> &'static str {
    match sev {
        Severity::Info => "badge sev-info",
        Severity::Warning => "badge sev-warning",
        Severity::Error => "badge sev-error",
    }
}

/// The `../` prefix from a page to the site root.
fn prefix_for(path: &PagePath) -> String {
    "../".repeat(path.depth())
}

/// A concept id linked to its generated page.
fn concept_link(bundle: &Bundle, prefix: &str, id: &ConceptId) -> Markup {
    let title = bundle
        .get(id)
        .map_or_else(|| id.name().to_string(), Concept::display_title);
    let exists = bundle.contains(id);
    html! {
        @if exists {
            a href=(format!("{prefix}{id}.html")) { (title) }
        } @else {
            span class="broken-link" title="broken link" { (title) }
        }
    }
}

/// The site-wide CSS: Tailwind v4 output compiled from `assets/tailwind.css`
/// by `cargo xtask tailwind` and vendored as `assets/site.css`. Inlined into
/// each page so the site deploys as a flat directory tree with zero external
/// fetches beyond mermaid; regenerate it after editing the source stylesheet.
const SITE_CSS: &str = include_str!("../assets/site.css");

// (The mermaid bootstrap lives in `mermaid_boot`, emitted per page with a
// depth-correct asset path; no module-level constant.)

/// Writes one page into the output tree.
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] (with its path) on write failure.
pub fn write_page(
    page: &SitePage,
    bundle: &Bundle,
    out_dir: &std::path::Path,
    bundle_has_mermaid: bool,
) -> Result<(), crate::SiteError> {
    use std::fs;

    let rel = page.rel_path.rel();
    let dest = out_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| crate::SiteError::Io(e, parent.to_path_buf()))?;
    }
    let document = layout(page, bundle, bundle_has_mermaid);
    fs::write(&dest, document.into_string()).map_err(|e| crate::SiteError::Io(e, dest.clone()))
}

/// The full HTML document for one page.
fn layout(page: &SitePage, bundle: &Bundle, bundle_has_mermaid: bool) -> Markup {
    let SitePage {
        rel_path,
        title,
        body_html,
        has_mermaid,
        meta_rows,
    } = page;
    let prefix = prefix_for(rel_path);
    let wants_mermaid = *has_mermaid && bundle_has_mermaid;
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                // Pre-render theme application: reads localStorage before
                // first paint to avoid a flash of the wrong theme. Our own
                // trusted inline script, like mermaid_boot.
                script { (PreEscaped(THEME_BOOT)) }
                // PreEscaped: SITE_CSS is trusted compile-time content; maud's
                // default escaping would turn `>` combinators and quoted
                // font-family names into `&gt;`/`&quot;` and break the rules.
                style { (PreEscaped(SITE_CSS)) }
            }
            body {
                header class="site-header" {
                    // CSS-only collapse: a visually-hidden checkbox toggles
                    // the nav via `body:has(:checked)` selectors in the
                    // stylesheet — no script needed for the nav.
                    // The `<label>` is the hamburger button.
                    input id="nav-toggle" class="nav-toggle-input" type="checkbox" {}
                    label class="menu-btn" for="nav-toggle" title="Toggle navigation" {
                        // Inline SVG: hamburger glyph whose stroke inherits
                        // currentColor, so light/dark schemes both work.
                        (PreEscaped(MENU_ICON))
                    }
                    div class="site-search" {
                        input type="search" name="q" placeholder="Search" aria-label="Search" {}
                    }
                    button class="theme-btn" type="button" title="Toggle theme"
                        aria-label="Toggle dark mode" {
                        // Sun shows in dark mode (click → light); moon shows
                        // in light mode (click → dark). Visibility is driven
                        // purely by the html theme class in the stylesheet.
                        span class="icon-sun" { (PreEscaped(SUN_ICON)) }
                        span class="icon-moon" { (PreEscaped(MOON_ICON)) }
                    }
                }
                div class="layout" {
                    nav id="site-nav" class="tree" aria-label="bundle contents" {
                        (nav_tree(bundle, &prefix))
                    }
                    main {
                        h1 { (title) }
                        @for (label, value) in meta_rows {
                            div class="panel" {
                                h3 { (label) }
                                div { (value) }
                            }
                        }
                        article {
                            (PreEscaped(body_html))
                        }
                    }
                }
                script { (PreEscaped(THEME_TOGGLE_BOOT)) }
                @if wants_mermaid {
                    // Classic scripts, not modules: the vendored build is a
                    // self-contained IIFE that sets a global, and classic
                    // scripts also work over file:// (no CORS on modules).
                    script src=(format!("{prefix}assets/mermaid.min.js")) {}
                    script { (PreEscaped(mermaid_boot())) }
                }
            }
        }
    }
}

/// The sun glyph, shown in dark mode (clicking switches to light).
const SUN_ICON: &str = "\
<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" \
stroke-width=\"2\" stroke-linecap=\"round\" width=\"20\" height=\"20\" \
aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"4\"/>\
<path d=\"M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41\
M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41\"/></svg>";

/// The moon glyph, shown in light mode (clicking switches to dark).
const MOON_ICON: &str = "\
<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" \
stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" \
width=\"20\" height=\"20\" aria-hidden=\"true\">\
<path d=\"M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z\"/></svg>";

/// Applies the stored theme before first paint: sets `.dark`/`.light` on
/// `<html>` from localStorage (`okf-theme`), leaving both classes off when
/// the user never chose (system preference then drives the media query).
/// Runs synchronously in `<head>` to avoid a flash of the wrong theme.
const THEME_BOOT: &str = "\
(function () {\
  try {\
    var t = localStorage.getItem('okf-theme');\
    if (t === 'dark' || t === 'light') {\
      document.documentElement.classList.add(t);\
    }\
  } catch (e) { /* no localStorage (e.g. privacy mode): system theme */ }\
})()";

/// Wires the header's theme button: flips the html class, persists the
/// choice, and keeps `aria-pressed` accurate. `okf-theme` is also keyed so
/// multiple okf sites on the same origin share the choice.
const THEME_TOGGLE_BOOT: &str = "\
(function () {\
  var btn = document.querySelector('.theme-btn');\
  if (!btn) return;\
  var root = document.documentElement;\
  function apply(t) {\
    root.classList.remove('dark', 'light');\
    if (t) root.classList.add(t);\
    btn.setAttribute('aria-pressed', t === 'dark' ? 'true' : 'false');\
    try {\
      if (t) localStorage.setItem('okf-theme', t);\
      else localStorage.removeItem('okf-theme');\
    } catch (e) { /* ignore */ }\
  }\
  btn.addEventListener('click', function () {\
    var dark = root.classList.contains('dark')\
      || (!root.classList.contains('light')\
          && window.matchMedia('(prefers-color-scheme: dark)').matches);\
    apply(dark ? 'light' : 'dark');\
  });\
  // Reflect the initial state on the button.
  var dark = root.classList.contains('dark')\
    || (!root.classList.contains('light')\
        && window.matchMedia('(prefers-color-scheme: dark)').matches);\
  btn.setAttribute('aria-pressed', dark ? 'true' : 'false');\
})()";

/// The hamburger icon: a plain inline SVG whose stroke inherits the button's
/// current color, so light/dark schemes both work with no custom CSS.
const MENU_ICON: &str = "\
<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" \
stroke-width=\"2\" stroke-linecap=\"round\" width=\"20\" height=\"20\" \
aria-hidden=\"true\"><path d=\"M4 6h16M4 12h16M4 18h16\"/></svg>";

/// The mermaid bootstrap, a classic inline script placed at the end of
/// `<body>` so `.mermaid` nodes already exist. Strict security level (the
/// default) sanitizes the diagram source; `startOnLoad: false` plus an
/// explicit `run` keeps the render observable and marks processed nodes.
const fn mermaid_boot() -> &'static str {
    "\
if (window.mermaid) {\
  mermaid.initialize({ securityLevel: 'strict', startOnLoad: false });\
  mermaid.run({ querySelector: 'pre.mermaid', suppressErrors: true })\
    .then(function () {\
      document.querySelectorAll('pre.mermaid').forEach(function (el) { el.dataset.processed = '1'; });\
    })\
    .catch(function (e) { console.warn('okf-web: mermaid render failed:', e); });\
}"
}

/// The nav tree: the bundle's directory structure with links to every
/// concept page, plus the special pages.
#[must_use]
pub fn nav_tree(bundle: &Bundle, prefix: &str) -> Markup {
    // Build a nested tree from concept ids, then render as nested <ul>.
    let mut root = NavNode::default();
    for concept in bundle.concepts() {
        let mut segments = concept.id.segments().to_vec();
        let name = segments.pop().unwrap_or_default();
        let mut node = &mut root;
        for seg in &segments {
            node = node.children.entry(seg.clone()).or_default();
        }
        node.leaves
            .push((name, concept.id.clone(), concept.display_title()));
    }
    html! {
        div class="site-title" {
            a href=(format!("{prefix}index.html")) { "okf site" }
        }
        div class="special" {
            a href=(format!("{prefix}index.html")) { "Dashboard" }
            a href=(format!("{prefix}__okf/graph.html")) { "Graph" }
        }
        (root.render(prefix, ""))
    }
}

/// A directory node in the nav tree.
#[derive(Default)]
struct NavNode {
    children: BTreeMap<String, Self>,
    leaves: Vec<(String, ConceptId, String)>,
}

impl NavNode {
    /// `prefix` is the page's `../`-chain to the site root; `dir` is this
    /// node's `/`-joined path from the bundle root (empty at the top).
    fn render(&self, prefix: &str, dir: &str) -> Markup {
        html! {
            ul {
                @for (name, id, title) in &self.leaves {
                    li {
                        a href=(format!("{prefix}{id}.html")) { (title) }
                        " " code { (name) }
                    }
                }
                @for (seg, child) in &self.children {
                    @let child_dir = if dir.is_empty() { seg.clone() } else { format!("{dir}/{seg}") };
                    li {
                        a class="dir" href=(format!("{prefix}{child_dir}/index.html")) { (seg) "/" }
                        (child.render(prefix, &child_dir))
                    }
                }
            }
        }
    }
}

/// A row helper: renders a `String` value into the meta grid.
fn meta_row(label: &str, value: &str) -> (String, Markup) {
    (label.to_string(), html! { (value) })
}

/// Builds the concept page for one concept.
#[must_use]
#[allow(clippy::too_many_lines)] // one cohesive page: frontmatter + panels.
pub fn concept_page(
    bundle: &Bundle,
    concept: &Concept,
    today: Date,
    validation: &Report,
    lint: &Report,
) -> SitePage {
    let id = &concept.id;
    let path = PagePath::Concept(id.clone());
    let prefix = prefix_for(&path);
    let fm = &concept.document.frontmatter;

    let body = crate::markdown::render(
        bundle,
        id,
        &concept.document.body,
        &prefix,
        &|target: &ConceptId| format!("{target}.html"),
    );

    // --- Frontmatter panel ---------------------------------------------
    let mut rows: Vec<(String, Markup)> = Vec::new();
    if let Some(type_) = concept.type_() {
        rows.push(meta_row("type", &type_));
    }
    rows.push((
        "status".into(),
        html! { span class=(status_class(&concept.status())) { (concept.status().as_str()) } },
    ));
    rows.push((
        "trust".into(),
        html! { span class=(tier_class(concept.trust_tier())) { (concept.trust_tier().as_str()) } },
    ));
    if let Some(stale_after) = fm.stale_after() {
        let stale = concept.is_stale_on(today);
        rows.push((
            "fresh".into(),
            html! {
                span class=(if stale { "badge stale" } else { "badge fresh" }) {
                    @if stale { "stale since " (stale_after.raw) }
                    @else { "fresh until " (stale_after.raw) }
                }
            },
        ));
    }
    if let Some(generated) = fm.generated() {
        let by = generated.by.as_ref().map_or("", |b| b.as_str());
        let at = generated.at.as_ref().map_or("", |a| a.raw.as_str());
        rows.push((
            "generated".into(),
            html! { (by) " " span class="num" { (at) } },
        ));
    }
    let verified = fm.verified();
    if !verified.is_empty() {
        rows.push((
            "verified".into(),
            html! {
                ul style="margin:0;padding-left:1rem" {
                    @for v in &verified {
                        li { (v.by.as_ref().map_or("", |b| b.as_str())) " " span class="num" { (v.at.as_ref().map_or("", |a| a.raw.as_str())) } }
                    }
                }
            },
        ));
    }
    let tags = fm.tags();
    if !tags.is_empty() {
        rows.push((
            "tags".into(),
            html! {
                @for tag in &tags { span class="badge" { (tag) } " " }
            },
        ));
    }
    let out_degree = bundle.links_from(id).len();
    let in_degree = bundle.backlinks(id).len();
    rows.push((
        "links".into(),
        html! { span class="num" { (out_degree) } " out · " span class="num" { (in_degree) } " in" },
    ));

    // --- Backlinks -------------------------------------------------------
    let backlinks = bundle.backlinks(id);
    let backlinks_panel = if backlinks.is_empty() {
        None
    } else {
        Some(html! {
            div class="panel" {
                h3 { "Backlinks" }
                ul {
                    @for bl in backlinks {
                        li { (concept_link(bundle, &prefix, bl)) }
                    }
                }
            }
        })
    };

    // --- Outgoing links ---------------------------------------------------
    let links = bundle.links_from(id);
    let links_panel = if links.is_empty() {
        None
    } else {
        Some(html! {
            div class="panel" {
                h3 { "Links" }
                ul {
                    @for link in links {
                        li {
                            @if link.exists {
                                (concept_link(bundle, &prefix, &link.target))
                            } @else {
                                span class="broken-link" title="broken link" { (link.text) }
                                " → " code { (link.target.to_string()) }
                            }
                        }
                    }
                }
            }
        })
    };

    // --- Sources with footnote attribution --------------------------------
    let sources = bundle.sources_of(id);
    let attributions = concept.document.attributions();
    let sources_panel = if sources.is_empty() {
        None
    } else {
        Some(html! {
            div class="panel" {
                h3 { "Sources" }
                ul {
                    @for resolved in sources {
                        @let source = &resolved.source;
                        li {
                            @let cited = source.id.as_ref().is_some_and(|sid| attributions.iter().any(|a| &a.label == sid && a.references > 0));
                            @match (source.resource_kind(), resolved.concept.as_ref()) {
                                (okf_core::ResourceKind::Url, _) => {
                                    a href=(source.resource.clone().unwrap_or_default()) rel="noopener noreferrer" { (source.label()) }
                                }
                                (okf_core::ResourceKind::Path, Some(target)) => {
                                    (concept_link(bundle, &prefix, target))
                                    " " code { (source.label()) }
                                }
                                (okf_core::ResourceKind::Path, None) => {
                                    code { (source.resource.clone().unwrap_or_default()) }
                                }
                                _ => { (source.label()) }
                            }
                            @if let Some(author) = &source.author { " — " (author.as_str()) }
                            @if let Some(count) = source.usage_count {
                                " used " span class="num" { (count) } "×"
                            }
                            @if cited { " " span class="badge status-stable" { "cited" } }
                            @else if source.id.is_some() { " " span class="badge sev-warning" { "uncited" } }
                        }
                    }
                }
            }
        })
    };

    // --- Diagnostics -------------------------------------------------------
    let diags: Vec<&Diagnostic> = validation
        .diagnostics
        .iter()
        .chain(lint.diagnostics.iter())
        .filter(|d| d.concept.as_ref() == Some(id) || d.path.as_ref() == Some(&concept.path))
        .collect();
    let diags_panel = if diags.is_empty() {
        None
    } else {
        Some(html! {
            div class="panel" {
                h3 { "Findings" }
                ul {
                    @for d in diags {
                        li { span class=(severity_class(d.severity)) { (d.severity.to_string()) } " " (d.message) }
                    }
                }
            }
        })
    };

    // --- TOC ---------------------------------------------------------------
    let headings = body.headings.clone();
    let toc_panel = if headings.len() < 2 {
        None
    } else {
        Some(html! {
            div class="panel" {
                h3 { "Contents" }
                div class="toc-columns" {
                    @for (level, text, slug) in &headings {
                        div style=(format!("padding-left:{}.2rem", level - 1)) {
                            a href=(format!("#{slug}")) { (text) }
                        }
                    }
                }
            }
        })
    };

    // Assemble the panels into the meta area: frontmatter facts first, then
    // the side panels.
    let mut meta_html = html! {
        dl class="meta-grid" {
            @for (label, value) in &rows {
                dt { (label) }
                dd { (value) }
            }
        }
    };
    for panel in [
        toc_panel,
        backlinks_panel,
        links_panel,
        sources_panel,
        diags_panel,
    ]
    .into_iter()
    .flatten()
    {
        meta_html = html! { (meta_html) (panel) };
    }

    SitePage {
        rel_path: path,
        title: concept.display_title(),
        body_html: body.html,
        has_mermaid: body.has_mermaid,
        meta_rows: vec![("Overview".to_string(), meta_html)],
    }
}

/// The trust dashboard: tier distribution, attention queue, actor stats.
#[must_use]
#[allow(clippy::too_many_lines)] // one cohesive dashboard of derived stats.
pub fn dashboard_page(
    bundle: &Bundle,
    today: Date,
    validation: &Report,
    lint: &Report,
) -> SitePage {
    let _ = (validation, lint);
    let mut tier_counts = [0usize; 3];
    let mut status_counts = [0usize; 4];
    let mut stale = 0usize;
    let mut stale_soon = 0usize;
    let mut actors: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut types: BTreeMap<String, usize> = BTreeMap::new();

    for concept in bundle.concepts() {
        let fm = &concept.document.frontmatter;
        tier_counts[match concept.trust_tier() {
            TrustTier::Unverified => 0,
            TrustTier::MachineConfirmed => 1,
            TrustTier::HumanReviewed => 2,
        }] += 1;
        status_counts[match concept.status() {
            Status::Draft => 0,
            Status::Stable => 1,
            Status::Deprecated => 2,
            Status::Other(_) => 3,
        }] += 1;
        let stale_after = fm.stale_after();
        let is_stale = concept.is_stale_on(today);
        if is_stale {
            stale += 1;
        } else if let Some(f) = stale_after {
            let effective = f
                .datetime
                .map(|dt| dt.utc_date())
                .or_else(|| Date::parse(f.raw.trim().get(..10).unwrap_or("")));
            if let Some(date) = effective {
                let delta = date.days_since_epoch() - today.days_since_epoch();
                if (0..=30).contains(&delta) {
                    stale_soon += 1;
                }
            }
        }
        let type_name = concept
            .type_()
            .map_or_else(|| "(untyped)".to_string(), std::borrow::Cow::into_owned);
        *types.entry(type_name).or_default() += 1;
        if let Some(by) = fm.generated().and_then(|g| g.by) {
            actors.entry(by.as_str().to_string()).or_default().0 += 1;
        }
        for verification in fm.verified() {
            if let Some(by) = verification.by {
                actors.entry(by.as_str().to_string()).or_default().1 += 1;
            }
        }
    }

    // Attention queue: same transparent risk score the studio uses.
    let mut attention: Vec<(ConceptId, i64, Vec<String>)> = Vec::new();
    for concept in bundle.concepts() {
        let id = &concept.id;
        let mut risk: i64 = 0;
        let mut reasons: Vec<String> = Vec::new();
        let is_stale = concept.is_stale_on(today);
        let in_degree = bundle.backlinks(id).len();
        if is_stale {
            risk += 40;
            reasons.push("stale".to_string());
        }
        match concept.trust_tier() {
            TrustTier::Unverified => {
                risk += 25;
                reasons.push("unverified".to_string());
            }
            TrustTier::MachineConfirmed => {
                risk += 10;
                reasons.push("machine-confirmed".to_string());
            }
            TrustTier::HumanReviewed => reasons.push("human-reviewed".to_string()),
        }
        if concept.status().is_deprecated() && in_degree > 0 {
            risk += 20;
            reasons.push(format!("deprecated · {in_degree} incoming links remain"));
        }
        let diag_count = validation
            .diagnostics
            .iter()
            .chain(lint.diagnostics.iter())
            .filter(|d| d.concept.as_ref() == Some(id) || d.path.as_ref() == Some(&concept.path))
            .count();
        if diag_count > 0 {
            risk += i64::try_from(diag_count).unwrap_or(i64::MAX).min(30);
            reasons.push(format!("{diag_count} finding(s)"));
        }
        let needs_attention = is_stale
            || concept.trust_tier() == TrustTier::Unverified
            || (concept.status().is_deprecated() && in_degree > 0)
            || diag_count > 0;
        if needs_attention {
            attention.push((id.clone(), risk, reasons));
        }
    }
    attention.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let prefix = prefix_for(&PagePath::Dashboard);
    let body = html! {
        div class="pagerow" {
            section {
                div class="panel" {
                    h3 { "Trust" }
                    dl class="meta-grid" {
                        dt { "unverified" } dd class="num" { (tier_counts[0]) }
                        dt { "machine-confirmed" } dd class="num" { (tier_counts[1]) }
                        dt { "human-reviewed" } dd class="num" { (tier_counts[2]) }
                    }
                }
                div class="panel" {
                    h3 { "Status" }
                    dl class="meta-grid" {
                        dt { "draft" } dd class="num" { (status_counts[0]) }
                        dt { "stable" } dd class="num" { (status_counts[1]) }
                        dt { "deprecated" } dd class="num" { (status_counts[2]) }
                        dt { "other" } dd class="num" { (status_counts[3]) }
                    }
                }
                div class="panel" {
                    h3 { "Freshness (as of " (today) ")" }
                    dl class="meta-grid" {
                        dt { "stale" } dd class="num" { (stale) }
                        dt { "stale within 30 days" } dd class="num" { (stale_soon) }
                    }
                }
                div class="panel" {
                    h3 { "Types" }
                    dl class="meta-grid" {
                        @for (type_, n) in &types {
                            dt { (type_) } dd class="num" { (n) }
                        }
                    }
                }
            }
            section {
                div class="panel" {
                    h3 { "Attention queue" }
                    table {
                        thead { tr { th { "risk" } th { "concept" } th { "why" } } }
                        tbody {
                            @for (id, risk, reasons) in &attention {
                                tr {
                                    td class="num" { (risk) }
                                    td { (concept_link(bundle, &prefix, id)) }
                                    td { (reasons.join(" · ")) }
                                }
                            }
                        }
                    }
                }
                div class="panel" {
                    h3 { "Actors" }
                    table {
                        thead { tr { th { "actor" } th { "generated" } th { "verified" } } }
                        tbody {
                            @for (actor, (generated, verified)) in &actors {
                                tr {
                                    td { (actor) }
                                    td class="num" { (generated) }
                                    td class="num" { (verified) }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    SitePage {
        rel_path: PagePath::Dashboard,
        title: format!("Dashboard — {} concept(s)", bundle.len()),
        body_html: body.into_string(),
        has_mermaid: false,
        meta_rows: Vec::new(),
    }
}

/// Assigns `id` a stable Mermaid node name, remembering declaration order —
/// the same interning `okf graph --format mermaid` uses.
fn intern_node(
    id: &str,
    node_id: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
    next: &mut usize,
) {
    if node_id.contains_key(id) {
        return;
    }
    node_id.insert(id.to_string(), format!("n{next}"));
    *next += 1;
    order.push(id.to_string());
}

/// The bundle-wide graph page: the `okf graph --format mermaid` flowchart,
/// rendered client-side by the vendored mermaid.
#[must_use]
pub fn graph_page(bundle: &Bundle) -> SitePage {
    // Flowchart source, mirroring the CLI's print_graph_mermaid: stable node
    // ids, phantom nodes for broken links, escaped labels.
    let mut src = String::from("flowchart LR\n");
    let mut node_id: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut next: usize = 0;
    for concept in bundle.concepts() {
        intern_node(&concept.id.to_string(), &mut node_id, &mut order, &mut next);
        for link in bundle.links_from(&concept.id) {
            intern_node(
                &link.target.to_string(),
                &mut node_id,
                &mut order,
                &mut next,
            );
        }
    }
    for id in &order {
        let name = &node_id[id];
        let _ = writeln!(src, "  {name}[\"{}\"]", mermaid_label(id));
    }
    for concept in bundle.concepts() {
        let from = node_id[&concept.id.to_string()].clone();
        for link in bundle.links_from(&concept.id) {
            let target = node_id[&link.target.to_string()].clone();
            if link.exists {
                let _ = writeln!(src, "  {from} --> {target}");
            } else {
                let _ = writeln!(src, "  {from} -.->|broken| {target}");
            }
        }
    }

    let body = html! {
        p { "Every concept and cross-link in the bundle. Dashed edges mark broken links (permitted by the spec: they may be not-yet-written knowledge)." }
        pre class="mermaid" { (src) }
    };

    SitePage {
        rel_path: PagePath::Graph,
        title: "Cross-link graph".to_string(),
        body_html: body.into_string(),
        has_mermaid: true,
        meta_rows: Vec::new(),
    }
}

/// A label safe inside a Mermaid `["..."]` node declaration, the same
/// escaping as `okf graph --format mermaid`.
fn mermaid_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "#quot;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
}

/// A directory index page: every concept under one directory, grouped by
/// type the way `index.md` files are.
#[must_use]
pub fn directory_page(bundle: &Bundle, dir: &ConceptId) -> SitePage {
    let prefix = prefix_for(&PagePath::Directory(dir.clone()));
    let under = format!("{dir}/");

    // Group concepts under this directory by type, sorted like index.md.
    let mut groups: BTreeMap<String, Vec<(String, ConceptId, String)>> = BTreeMap::new();
    for concept in bundle.concepts() {
        if !concept.id.to_string().starts_with(&under) {
            continue;
        }
        let type_ = concept
            .type_()
            .map_or_else(|| "Other".to_string(), std::borrow::Cow::into_owned);
        groups.entry(type_).or_default().push((
            concept.display_title(),
            concept.id.clone(),
            concept
                .document
                .frontmatter
                .description()
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_default(),
        ));
    }

    let mut body = String::new();
    let mut first = true;
    for (type_, mut entries) in groups {
        entries.sort_by_key(|(title, _, _)| title.to_lowercase());
        let section = html! {
            @if !first { hr; }
            h2 { (type_) }
            ul {
                @for (title, id, description) in &entries {
                    li {
                        (title)
                        " "
                        (concept_link(bundle, &prefix, id))
                        @if !description.is_empty() { " — " (description) }
                    }
                }
            }
        };
        body.push_str(&section.into_string());
        first = false;
    }

    SitePage {
        rel_path: PagePath::Directory(dir.clone()),
        title: format!("{dir}/"),
        body_html: body,
        has_mermaid: false,
        meta_rows: Vec::new(),
    }
}
