//! # okf-web: a static site generator for OKF bundles
//!
//! A pure-Rust, build-time generator: an OKF bundle in, a directory of HTML
//! out. No server, no WASM, no client-side framework — the same relationship
//! to `okf studio` that Zola has to an interactive CMS.
//!
//! ```no_run
//! use okf_web::{SiteOptions, generate};
//!
//! generate(SiteOptions {
//!     root: "./my_bundle".into(),
//!     out_dir: "./site".into(),
//!     today: None,
//!     title: None,
//! })?;
//! # Ok::<(), okf_web::SiteError>(())
//! ```
//!
//! ## What it emits
//!
//! - One page per concept, at the concept's bundle-relative path with `.html`
//!   substituted for `.md`, so relative links keep working under any base path.
//! - A nav tree mirroring the bundle layout, a dashboard
//!   (`__okf/dashboard.html`), and a bundle-wide mermaid graph page
//!   (`__okf/graph.html`).
//! - A single inline stylesheet compiled from Tailwind CSS v4 (source in
//!   `assets/tailwind.css`, vendored as `assets/site.css`), embedded in every
//!   page so the site fetches no external CSS.
//! - `assets/mermaid.min.js`, a vendored mermaid build, written only when some
//!   page carries a diagram, so clean bundles never download it.
//!
//! ## Security posture
//!
//! - No raw HTML passthrough from markdown bodies: `Html` events are dropped
//!   before `push_html`, and pulldown-cmark's `unsafe` option stays off.
//! - Every frontmatter interpolation goes through maud, which escapes
//!   unconditionally — a `<script>` in a `title:` is inert.
//! - External `sources[].resource` URLs are linked with
//!   `rel="noopener noreferrer"`.
//!
//! ## Determinism
//!
//! [`SiteOptions::today`] pins the build date for staleness badges, matching
//! the workspace's "deterministic by default" choice; `None` means the system
//! clock. Output paths and relative URLs mirror the bundle layout so the site
//! deploys under any base path without a `--base-url` flag.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic, clippy::nursery)]

pub mod markdown;
pub mod render;

use okf_core::{Bundle, BundleError, ConceptId, Date};
use render::{SitePage, write_page};
use std::fs;
use std::io;
use std::path::PathBuf;

/// The vendored mermaid.min.js (12.0.0, MIT; see `assets/vendor/README.md`).
const MERMAID_JS: &[u8] = include_bytes!("../assets/vendor/mermaid.min.js");

/// Options for one `generate` run.
#[derive(Clone, Debug)]
pub struct SiteOptions {
    /// The bundle directory to read.
    pub root: PathBuf,
    /// The directory to write the site into (created if missing).
    pub out_dir: PathBuf,
    /// The date staleness is evaluated against; `None` uses the system clock.
    pub today: Option<Date>,
    /// The site title shown in the header; `None` uses [`DEFAULT_SITE_TITLE`].
    pub title: Option<String>,
}

/// The site title used when [`SiteOptions::title`] is `None`.
pub const DEFAULT_SITE_TITLE: &str = "okf site";

impl SiteOptions {
    /// The effective build date, from `today` or the system clock.
    ///
    /// Falls back to a fixed date only if the system clock reports a time
    /// before the Unix epoch, matching `okf_studio::snapshot::Snapshot::build`.
    #[must_use]
    pub fn effective_today(&self) -> Date {
        self.today.or_else(Date::today_utc).unwrap_or(Date {
            year: 1970,
            month: 1,
            day: 1,
        })
    }

    /// The effective site title, from `title` or [`DEFAULT_SITE_TITLE`].
    #[must_use]
    pub fn effective_title(&self) -> &str {
        self.title.as_deref().unwrap_or(DEFAULT_SITE_TITLE)
    }
}

/// The `okf site` result reported to the caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiteSummary {
    /// Pages written, including directory indexes and special pages.
    pub pages: usize,
    /// Concepts whose body carries at least one mermaid block.
    pub mermaid_pages: usize,
}

/// Generates the site for the bundle at `options.root` into `options.out_dir`.
///
/// Bundle problems are *not* errors here, matching okf-core's permissive
/// loader: parse errors and broken links render as visible content. Only
/// I/O failures (an unreadable bundle root, an unwritable output directory)
/// are returned as errors.
///
/// # Errors
///
/// Returns an error if the bundle root cannot be loaded (see
/// [`Bundle::load`]) or if any output file cannot be written.
pub fn generate(options: SiteOptions) -> Result<SiteSummary, SiteError> {
    let today = options.effective_today();
    let site_title = options.effective_title().to_string();
    let SiteOptions { root, out_dir, .. } = options;
    let bundle = Bundle::load(&root)?;

    // Health badges from the same calls the studio snapshot makes.
    let validation = okf_validator::validate_bundle_at(&bundle, Some(today));
    let lint = okf_validator::lint_bundle_at(&bundle, Some(today));

    let mut pages: Vec<SitePage> = Vec::new();
    for concept in bundle.concepts() {
        pages.push(render::concept_page(
            &bundle,
            concept,
            today,
            &validation,
            &lint,
        ));
    }
    pages.push(render::dashboard_page(&bundle, today, &validation, &lint));
    pages.push(render::graph_page(&bundle));

    // Directory index pages: one per subdirectory that holds a concept,
    // mirroring the bundle's index.md files.
    for dir in directories_with_concepts(&bundle) {
        pages.push(render::directory_page(&bundle, &dir));
    }

    let mermaid_pages = pages
        .iter()
        .filter(|p| p.has_mermaid)
        .filter(|p| matches!(p.rel_path, PagePath::Concept(_)))
        .count();

    // The mermaid asset ships only when some page embeds a diagram, so clean
    // bundles never download it.
    let bundle_has_mermaid = pages.iter().any(|p| p.has_mermaid);

    fs::create_dir_all(&out_dir).map_err(|e| SiteError::Io(e, out_dir.clone()))?;

    for page in &pages {
        write_page(page, &bundle, &out_dir, bundle_has_mermaid, &site_title)?;
    }

    if bundle_has_mermaid {
        let assets_dir = out_dir.join("assets");
        fs::create_dir_all(&assets_dir).map_err(|e| SiteError::Io(e, assets_dir.clone()))?;
        let mermaid_path = assets_dir.join("mermaid.min.js");
        fs::write(&mermaid_path, MERMAID_JS).map_err(|e| SiteError::Io(e, mermaid_path.clone()))?;
    }

    Ok(SiteSummary {
        pages: pages.len(),
        mermaid_pages,
    })
}

/// Where a generated page lives in the output tree.
///
/// The output mirrors the bundle layout so relative links deploy under any
/// base path. Collision-proofing relies on the reserved `index.md`/`log.md`
/// rule: no concept id ever ends in `index`, so `index.html` and
/// `<dir>/index.html` never clash with a concept page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PagePath {
    /// A concept page: `<concept path>.html`, mirroring the bundle layout.
    Concept(ConceptId),
    /// The trust dashboard and site landing page: `index.html`.
    Dashboard,
    /// The bundle-wide mermaid graph: `__okf/graph.html`.
    Graph,
    /// A subdirectory listing page: `<dir>/index.html`.
    Directory(ConceptId),
}

impl PagePath {
    /// The output path relative to the site root, `/`-separated.
    #[must_use]
    pub fn rel(&self) -> String {
        match self {
            Self::Concept(id) => format!("{id}.html"),
            Self::Dashboard => "index.html".to_string(),
            Self::Graph => "__okf/graph.html".to_string(),
            Self::Directory(id) => format!("{id}/index.html"),
        }
    }

    /// How deep the page sits below the site root: the number of directory
    /// levels a relative link from this page must climb with `../`.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Self::Concept(id) => id.segments().len().saturating_sub(1),
            Self::Directory(id) => id.segments().len(),
            Self::Graph => 1,
            Self::Dashboard => 0,
        }
    }
}

/// An error from [`generate`].
#[derive(Debug)]
pub enum SiteError {
    /// The bundle could not be loaded.
    Bundle(BundleError),
    /// An output file could not be written.
    Io(io::Error, PathBuf),
}

impl std::fmt::Display for SiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bundle(e) => write!(f, "bundle: {e}"),
            Self::Io(e, path) => write!(f, "{}: {e}", path.display()),
        }
    }
}

impl std::error::Error for SiteError {}

impl From<BundleError> for SiteError {
    fn from(e: BundleError) -> Self {
        Self::Bundle(e)
    }
}

/// Every subdirectory (by `/`-joined path) that contains a concept, directly
/// or through a child directory, in deterministic (sorted) order. The bundle
/// root is served by the dashboard, so it is not returned here.
fn directories_with_concepts(bundle: &Bundle) -> Vec<ConceptId> {
    let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for concept in bundle.concepts() {
        // A concept `a/b/c.md` makes directories `a` and `a/b` exist in the
        // site. Depth 0 (the empty root path) is skipped.
        let segments = concept.id.segments();
        for depth in 1..segments.len() {
            if let Ok(id) = ConceptId::new(segments[..depth].to_vec()) {
                dirs.insert(id.to_string());
            }
        }
    }
    dirs.into_iter()
        .filter_map(|k| ConceptId::parse(&k).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_paths_mirror_the_bundle_layout() {
        let id = ConceptId::parse("tables/orders").unwrap();
        assert_eq!(PagePath::Concept(id.clone()).rel(), "tables/orders.html");
        assert_eq!(PagePath::Concept(id).depth(), 1);
        assert_eq!(
            PagePath::Concept(ConceptId::parse("overview").unwrap()).depth(),
            0
        );
        assert_eq!(PagePath::Graph.rel(), "__okf/graph.html");
    }
}
