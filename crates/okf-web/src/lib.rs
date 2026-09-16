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
//!     theme: None,
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
//! - `assets/shiki.min.js`, a vendored shiki highlighter bundle, written only
//!   when some page carries a fenced code block with a language tag; pages
//!   highlight client-side in GitHub light/dark themes, resolved by the
//!   site's `data-scheme` attribute.
//! - `assets/fonts.css` plus `assets/fonts/`, written only when the bundle
//!   configures fonts, and linked after the inline stylesheet so its token
//!   overrides win.
//! - `assets/theme.css`, written only when the bundle defines its own
//!   palettes, and linked after `fonts.css` for the same reason.
//!
//! ## Themes
//!
//! A visitor picks a palette from the header menu; the choice lives in
//! `localStorage` under `okf-theme` and is applied to `<html>` before first
//! paint as two attributes — `data-scheme` (`light`/`dark`, absent = follow
//! the system) for the base palette and everything else on the light/dark
//! axis, and `data-theme` for the palette's token overrides. The catalog is
//! [`render::BUILTIN_THEMES`] plus the bundle's own `site.themes`, and
//! [`SiteOptions::theme`] or `site.theme` pins what a first visit sees.
//!
//! ## Per-bundle configuration
//!
//! A bundle may pin its site settings in `.okf/config.yaml`, read by
//! [`config::SiteConfig`] — a header title, the typography tokens, with
//! self-hosted font files under `.okf/fonts/`, and its own themes. The
//! directory is invisible to every OKF walker, so configuration never
//! becomes content. Caller-supplied options win over the file:
//! [`SiteOptions::title`] overrides `site.title` and [`SiteOptions::theme`]
//! overrides `site.theme`. A bundle with no `.okf/` generates the same
//! output a bundle that configures nothing does.
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

pub mod config;
pub mod markdown;
pub mod render;
pub mod search;

use config::{Fonts, SiteConfig, Themes};
use okf_core::{Bundle, BundleError, ConceptId, Date};
use render::{BUILTIN_THEMES, SiteChrome, SitePage, ThemeEntry, write_page};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The vendored mermaid.min.js (12.0.0, MIT; see `assets/vendor/README.md`).
const MERMAID_JS: &[u8] = include_bytes!("../assets/vendor/mermaid.min.js");

/// The vendored shiki.min.js (3.23.0, MIT; see `assets/vendor/README.md`) —
/// an esbuild IIFE of shiki's Oniguruma engine with inlined WASM and every
/// bundled language, exposing `window.okfShiki { load, highlight }`.
const SHIKI_JS: &[u8] = include_bytes!("../assets/vendor/shiki.min.js");

/// Options for one `generate` run.
#[derive(Clone, Debug)]
pub struct SiteOptions {
    /// The bundle directory to read.
    pub root: PathBuf,
    /// The directory to write the site into (created if missing).
    pub out_dir: PathBuf,
    /// The date staleness is evaluated against; `None` uses the system clock.
    pub today: Option<Date>,
    /// The site title shown in the header. `None` falls back to the bundle's
    /// `site.title` (see [`config::SiteConfig`]), then to
    /// [`DEFAULT_SITE_TITLE`].
    pub title: Option<String>,
    /// The theme a first-time visitor sees, by id. `None` falls back to the
    /// bundle's `site.theme`, then to [`config::AUTO_THEME_ID`] — following
    /// the system light/dark preference. An id the site's catalog does not
    /// define is an error, not a silent fallback.
    pub theme: Option<String>,
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
}

/// The `okf site` result reported to the caller.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SiteSummary {
    /// Pages written, including directory indexes and special pages.
    pub pages: usize,
    /// Concepts whose body carries at least one mermaid block.
    pub mermaid_pages: usize,
    /// Concepts whose body carries at least one fenced code block with a
    /// language tag — the pages shiki highlights.
    pub code_pages: usize,
    /// The `.okf/config.yaml` this build read, absent when the bundle has
    /// none — the fastest way to tell a configuration that took effect from
    /// one the build never saw.
    pub config: Option<PathBuf>,
    /// Which `site:` settings that file supplied, in schema order (`title`,
    /// `fonts.body`, `fonts.code`, `fonts.files`).
    pub config_settings: Vec<&'static str>,
    /// Notes from reading `.okf/config.yaml` — one per section this version
    /// of `okf` ignored, so a config written for a newer release degrades
    /// visibly rather than silently.
    pub notes: Vec<String>,
}

/// Generates the site for the bundle at `options.root` into `options.out_dir`.
///
/// Bundle problems are *not* errors here, matching okf-core's permissive
/// loader: parse errors and broken links render as visible content. The
/// bundle's own `.okf/config.yaml` is held to the opposite standard: a typo
/// there fails the build rather than silently doing nothing.
///
/// # Errors
///
/// Returns an error if the bundle root cannot be loaded (see
/// [`Bundle::load`]), if `.okf/config.yaml` exists and cannot be used (see
/// [`config::SiteConfig::load`]), if the requested default theme is not one
/// the site defines, or if any output file cannot be written.
pub fn generate(options: SiteOptions) -> Result<SiteSummary, SiteError> {
    let today = options.effective_today();
    let SiteOptions {
        root,
        out_dir,
        title,
        theme,
        ..
    } = options;
    let config = SiteConfig::load(&root)?;
    let config_settings = config.settings();
    let config_path = config.path;
    // Precedence: the caller's title (the CLI's `--title`) over the bundle's
    // `site.title` over the built-in default.
    let site_title = title
        .or(config.title)
        .unwrap_or_else(|| DEFAULT_SITE_TITLE.to_string());
    let fonts = config.fonts;
    let themes = config.themes;
    let catalog = theme_catalog(&themes);
    // Same precedence for the theme, with one difference: an id nothing
    // defines is an error rather than a fallback, because a default theme
    // that silently does nothing is exactly the CI-hostile case the config
    // is strict about.
    let default_theme = resolve_default_theme(&root, theme, config.theme, &catalog)?;
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
    let code_pages = pages
        .iter()
        .filter(|p| p.has_shiki)
        .filter(|p| matches!(p.rel_path, PagePath::Concept(_)))
        .count();

    // The search index + client ship unconditionally (the header input is
    // on every page) but are *fetched* lazily on first keystroke; mermaid
    // and shiki ship only when some page needs them, so clean bundles
    // never download what they do not use.
    let bundle_has_mermaid = pages.iter().any(|p| p.has_mermaid);
    let bundle_has_code = pages.iter().any(|p| p.has_shiki);

    let chrome = SiteChrome {
        title: &site_title,
        has_mermaid: bundle_has_mermaid,
        has_fonts: !fonts.is_empty(),
        has_theme_css: !themes.is_empty(),
        themes: &catalog,
        default_theme: &default_theme,
    };

    fs::create_dir_all(&out_dir).map_err(|e| SiteError::Io(e, out_dir.clone()))?;

    for page in &pages {
        write_page(page, &bundle, &out_dir, chrome)?;
    }

    {
        let assets_dir = out_dir.join("assets");
        fs::create_dir_all(&assets_dir).map_err(|e| SiteError::Io(e, assets_dir.clone()))?;
        let search_path = assets_dir.join("search-index.js");
        let search_js = crate::search::search_index_js(&bundle, Some(today));
        fs::write(&search_path, search_js).map_err(|e| SiteError::Io(e, search_path.clone()))?;
        if bundle_has_mermaid {
            let mermaid_path = assets_dir.join("mermaid.min.js");
            fs::write(&mermaid_path, MERMAID_JS)
                .map_err(|e| SiteError::Io(e, mermaid_path.clone()))?;
        }
        if bundle_has_code {
            let shiki_path = assets_dir.join("shiki.min.js");
            fs::write(&shiki_path, SHIKI_JS).map_err(|e| SiteError::Io(e, shiki_path.clone()))?;
        }
        // Fonts are opt-in: a bundle that configures none gets no stylesheet
        // and no `<link>`, leaving the committed inline CSS the only source
        // of typography.
        if !fonts.is_empty() {
            let fonts_css = assets_dir.join("fonts.css");
            fs::write(&fonts_css, fonts.to_css())
                .map_err(|e| SiteError::Io(e, fonts_css.clone()))?;
            copy_font_files(&root, &assets_dir, &fonts)?;
        }
        // Themes are opt-in the same way: the built-in palettes are already
        // in the inline stylesheet, so a bundle that defines none writes no
        // second file and links nothing.
        if !themes.is_empty() {
            let theme_css = assets_dir.join("theme.css");
            fs::write(&theme_css, themes.to_css())
                .map_err(|e| SiteError::Io(e, theme_css.clone()))?;
        }
    }

    Ok(SiteSummary {
        pages: pages.len(),
        mermaid_pages,
        code_pages,
        config: config_path,
        config_settings,
        notes: config.notes,
    })
}

/// The site's theme catalog: the built-in palettes, then the bundle's own.
///
/// A bundle theme whose id matches a built-in is *not* a second menu entry:
/// its `[data-theme]` block lands in `assets/theme.css`, which is linked
/// after the inline stylesheet, so it overrides that palette's tokens and
/// the entry keeps its built-in label and scheme. That is the cascade doing
/// the work, and it is the natural way to say "keep the standard themes,
/// restyle them".
fn theme_catalog(themes: &Themes) -> Vec<ThemeEntry<'_>> {
    let mut catalog: Vec<ThemeEntry<'_>> = BUILTIN_THEMES.to_vec();
    for theme in themes {
        if catalog.iter().any(|entry| entry.id == theme.id()) {
            continue;
        }
        catalog.push(ThemeEntry {
            id: theme.id(),
            label: theme.label(),
            scheme: Some(theme.scheme()),
        });
    }
    catalog
}

/// Resolves the theme a first visit gets: the caller's `--theme` over the
/// bundle's `site.theme` over [`config::AUTO_THEME_ID`].
///
/// An id outside the catalog fails the build, and the two sources report
/// differently on purpose: a bad `site.theme` is a configuration error
/// carrying the file's path, a bad `--theme` is the caller's mistake.
fn resolve_default_theme(
    root: &Path,
    requested: Option<String>,
    configured: Option<String>,
    catalog: &[ThemeEntry<'_>],
) -> Result<String, SiteError> {
    let known = || {
        catalog
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let (id, from_caller) = match (requested, configured) {
        (Some(id), _) => (id, true),
        (None, Some(id)) => (id, false),
        (None, None) => return Ok(config::AUTO_THEME_ID.to_string()),
    };
    if catalog.iter().any(|entry| entry.id == id) {
        return Ok(id);
    }
    Err(if from_caller {
        SiteError::Theme(format!(
            "`{id}` is not one of this site's themes: {}",
            known()
        ))
    } else {
        SiteError::Config(config::ConfigError::invalid(
            root,
            format!(
                "`site.theme` names `{id}`, which is not one of this site's themes: {}",
                known()
            ),
        ))
    })
}

/// Copies the configured faces from `.okf/fonts/` into `assets/fonts/`.
///
/// Names are deduplicated, so two `@font-face` entries cut from one file
/// copy it once. A name the bundle does not actually carry is a
/// *configuration* error rather than an I/O one: the config promised the
/// file, and the name it used is what the report has to point at.
fn copy_font_files(root: &Path, assets_dir: &Path, fonts: &Fonts) -> Result<(), SiteError> {
    let names: std::collections::BTreeSet<&str> =
        fonts.files.iter().map(config::FontFile::file).collect();
    if names.is_empty() {
        return Ok(());
    }
    let src_dir = config::fonts_dir(root);
    let dest_dir = assets_dir.join("fonts");
    fs::create_dir_all(&dest_dir).map_err(|e| SiteError::Io(e, dest_dir.clone()))?;
    for name in names {
        let src = src_dir.join(name);
        let dest = dest_dir.join(name);
        match fs::copy(&src, &dest) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(SiteError::Config(config::ConfigError::invalid(
                    root,
                    format!(
                        "`site.fonts.files` names `{name}`, which is not in \
                         `{}/`",
                        config::FONTS_DIR
                    ),
                )));
            }
            Err(e) => return Err(SiteError::Io(e, src)),
        }
    }
    Ok(())
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
    /// The bundle's `.okf/config.yaml` could not be used.
    Config(config::ConfigError),
    /// The caller asked for a default theme the site does not define.
    Theme(String),
    /// An output file could not be written.
    Io(io::Error, PathBuf),
}

impl std::fmt::Display for SiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bundle(e) => write!(f, "bundle: {e}"),
            Self::Config(e) => write!(f, "config: {e}"),
            Self::Theme(message) => write!(f, "theme: {message}"),
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

impl From<config::ConfigError> for SiteError {
    fn from(e: config::ConfigError) -> Self {
        Self::Config(e)
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
