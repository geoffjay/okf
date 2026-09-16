//! Maud templates: page layout, concept pages, the dashboard, the graph
//! page, directory indexes, and the nav tree.
//!
//! Every dynamic value passes through maud's escaping; the only
//! [`PreEscaped`] content is markdown bodies (escaped by pulldown-cmark) and
//! our own vendored-mermaid `<script>` block.

use crate::PagePath;
use crate::config::{self, Scheme, SchemePref};
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
    /// Whether the page carries a fenced code block with a language tag —
    /// the pages the shiki bootstrap highlights.
    pub has_shiki: bool,
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

/// One entry in the site's theme catalog: a *family*, offered once in the
/// header menu and rendered in whichever variant the appearance calls for.
///
/// The catalog is the built-in table below plus whatever the bundle defines
/// in `.okf/config.yaml`, merged by [`crate::generate`]. The id becomes
/// `data-theme` on `<html>` — except [`config::DEFAULT_THEME_ID`], which is
/// the *absence* of the attribute — and the appearance is the independent
/// `data-scheme` axis. A family that defines only one variant leaves the
/// other appearance on the built-in base palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeEntry<'a> {
    /// The `data-theme` value, and the string persisted under `okf-theme`.
    pub id: &'a str,
    /// The menu label. Escaped like every other interpolation.
    pub label: &'a str,
    /// Whether the family carries a light variant.
    pub light: bool,
    /// Whether the family carries a dark variant.
    pub dark: bool,
}

impl ThemeEntry<'_> {
    /// Whether the family covers `scheme` itself rather than falling
    /// through to the base palette.
    #[must_use]
    pub const fn has(&self, scheme: Scheme) -> bool {
        match scheme {
            Scheme::Light => self.light,
            Scheme::Dark => self.dark,
        }
    }
}

/// The families compiled into [`SITE_CSS`], in menu order.
///
/// `default` is the base palette itself — the `:root` and `[data-scheme]`
/// blocks — so it has no `[data-theme]` block; every other variant here
/// does, and `builtin_themes_have_css_blocks` holds the table and the
/// stylesheet together, because an entry whose palette was deleted would
/// render a menu item that silently does nothing.
pub const BUILTIN_THEMES: [ThemeEntry<'static>; 3] = [
    ThemeEntry {
        id: config::DEFAULT_THEME_ID,
        label: "Default",
        light: true,
        dark: true,
    },
    ThemeEntry {
        id: "sepia",
        label: "Sepia",
        light: true,
        dark: false,
    },
    ThemeEntry {
        id: "contrast",
        label: "High contrast",
        light: true,
        dark: true,
    },
];

/// The appearance choices the menu offers, in menu order. `Auto` sets no
/// attribute, leaving the stylesheet's `prefers-color-scheme` rules.
const SCHEME_CHOICES: [(SchemePref, &str); 3] = [
    (SchemePref::Auto, "Auto (system)"),
    (SchemePref::Light, "Light"),
    (SchemePref::Dark, "Dark"),
];

/// The build-wide chrome every page shares: values that come from the site
/// build rather than from the page's own concept.
#[derive(Clone, Copy, Debug)]
pub struct SiteChrome<'a> {
    /// The header title, linking back to the dashboard.
    pub title: &'a str,
    /// Whether this build wrote `assets/mermaid.min.js`; a diagram page in a
    /// bundle whose asset was skipped must not link it.
    pub has_mermaid: bool,
    /// Whether this build wrote `assets/fonts.css`. Pages link it after the
    /// inline stylesheet, so its `:root` token overrides win the cascade tie.
    pub has_fonts: bool,
    /// Whether this build wrote `assets/theme.css` — the bundle's own
    /// families, linked after `fonts.css` for the same reason.
    pub has_theme_css: bool,
    /// The catalog the header menu offers, built-ins first.
    pub themes: &'a [ThemeEntry<'a>],
    /// The family a visitor with no stored choice gets; an id from `themes`.
    pub default_theme: &'a str,
    /// The appearance a visitor with no stored choice gets.
    pub default_scheme: SchemePref,
}

/// Writes one page into the output tree.
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] (with its path) on write failure.
pub fn write_page(
    page: &SitePage,
    bundle: &Bundle,
    out_dir: &std::path::Path,
    chrome: SiteChrome<'_>,
) -> Result<(), crate::SiteError> {
    use std::fs;

    let rel = page.rel_path.rel();
    let dest = out_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| crate::SiteError::Io(e, parent.to_path_buf()))?;
    }
    let document = layout(page, bundle, chrome);
    fs::write(&dest, document.into_string()).map_err(|e| crate::SiteError::Io(e, dest.clone()))
}

/// The full HTML document for one page.
fn layout(page: &SitePage, bundle: &Bundle, chrome: SiteChrome<'_>) -> Markup {
    let SitePage {
        rel_path,
        title,
        body_html,
        has_mermaid,
        has_shiki,
        meta_rows,
    } = page;
    let prefix = prefix_for(rel_path);
    let current_rel = rel_path.rel();
    let wants_mermaid = *has_mermaid && chrome.has_mermaid;
    let wants_shiki = *has_shiki;
    // The build's own defaults, rendered into the markup rather than left to
    // the boot: a reader without JavaScript still gets the bundle's pinned
    // theme, and the boot only has to *change* attributes when a stored
    // choice differs. `default`/`auto` are the absence of their attribute.
    let pinned_theme =
        (chrome.default_theme != config::DEFAULT_THEME_ID).then_some(chrome.default_theme);
    let pinned_scheme = chrome.default_scheme.scheme().map(Scheme::as_css);
    html! {
        (DOCTYPE)
        html lang="en" data-theme=[pinned_theme] data-scheme=[pinned_scheme] {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                // Pre-render theme application: reads localStorage before
                // first paint to avoid a flash of the wrong theme. Our own
                // trusted inline script, like mermaid_boot.
                script {
                    (PreEscaped(theme_boot(
                        chrome.themes,
                        chrome.default_theme,
                        chrome.default_scheme,
                    )))
                }
                // PreEscaped: SITE_CSS is trusted compile-time content; maud's
                // default escaping would turn `>` combinators and quoted
                // font-family names into `&gt;`/`&quot;` and break the rules.
                style { (PreEscaped(SITE_CSS)) }
                // The generated stylesheets, after the inline one so their
                // token overrides win: external `<link>`s are fine here
                // because the files are ours and page-depth-relative, and a
                // failed load leaves the committed defaults in place. Fonts
                // first, palettes second; the two token sets are disjoint,
                // so the order is for determinism rather than cascade.
                @if chrome.has_fonts {
                    link rel="stylesheet" href=(format!("{prefix}assets/fonts.css"));
                }
                @if chrome.has_theme_css {
                    link rel="stylesheet" href=(format!("{prefix}assets/theme.css"));
                }
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
                    a class="site-name" href=(format!("{prefix}index.html")) { (chrome.title) }
                    div class="site-search" {
                        // data-asset: the lazy fetch target for the search
                        // index + client; data-prefix: the depth-correct
                        // ../ chain the client prefixes result hrefs with.
                        input type="search" name="q" placeholder="Search" aria-label="Search"
                            data-asset=(format!("{prefix}assets/search-index.js"))
                            data-prefix=(prefix) {}
                    }
                    (theme_picker(chrome.themes, chrome.default_theme, chrome.default_scheme))
                }
                div class="layout" {
                    nav id="site-nav" class="tree" aria-label="bundle contents" {
                        (nav_tree(bundle, &prefix, &current_rel))
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
                script { (PreEscaped(NAV_TOGGLE_BOOT)) }
                script { (PreEscaped(NAV_SUBMENU_BOOT)) }
                script { (PreEscaped(SEARCH_ARM_BOOT)) }
                script { (PreEscaped(THEME_MENU_BOOT)) }
                @if wants_mermaid {
                    // Classic scripts, not modules: the vendored build is a
                    // self-contained IIFE that sets a global, and classic
                    // scripts also work over file:// (no CORS on modules).
                    script src=(format!("{prefix}assets/mermaid.min.js")) {}
                    script { (PreEscaped(mermaid_boot())) }
                }
                @if wants_shiki {
                    // Same shape as the mermaid pair: the vendored IIFE sets
                    // window.okfShiki, and the boot swaps highlighted HTML in.
                    script src=(format!("{prefix}assets/shiki.min.js")) {}
                    script { (PreEscaped(shiki_boot())) }
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

/// The header's theme control: the resolved-appearance glyph, and behind it
/// a menu with one radio group per axis — appearance, then theme family.
///
/// `aria-checked` is the selection state — the stylesheet draws the tick
/// from it — and the items carry the attribute values the menu boot
/// applies, so the page needs no second copy of the catalog in script form.
/// A page is served with the build defaults checked; [`THEME_MENU_BOOT`]
/// re-marks them from `localStorage` on load, the same values the pre-paint
/// boot already applied to `<html>`.
///
/// A family with one variant says so next to its name, because the
/// fallback is otherwise invisible: choosing `sepia` and then switching to
/// dark lands on the base dark palette by design, not by failure.
fn theme_picker(
    themes: &[ThemeEntry<'_>],
    default_theme: &str,
    default_scheme: SchemePref,
) -> Markup {
    html! {
        div class="theme-picker" {
            button id="theme-btn" class="theme-btn" type="button" title="Theme"
                aria-label="Theme" aria-haspopup="true" aria-expanded="false" {
                // Sun shows in dark schemes, moon in light ones; visibility
                // is driven purely by `data-scheme` in the stylesheet.
                span class="icon-sun" { (PreEscaped(SUN_ICON)) }
                span class="icon-moon" { (PreEscaped(MOON_ICON)) }
            }
            div class="theme-menu" role="menu" aria-labelledby="theme-btn"
                data-default-theme=(default_theme)
                data-default-scheme=(default_scheme.as_str()) hidden {
                p class="theme-group" id="theme-group-scheme" { "Appearance" }
                ul role="group" aria-labelledby="theme-group-scheme" {
                    @for (scheme, label) in SCHEME_CHOICES {
                        li role="none" {
                            button class="theme-item" type="button" role="menuitemradio"
                                data-scheme-id=(scheme.as_str())
                                aria-checked=(checked(scheme == default_scheme)) {
                                (label)
                            }
                        }
                    }
                }
                p class="theme-group" id="theme-group-theme" { "Theme" }
                ul role="group" aria-labelledby="theme-group-theme" {
                    @for theme in themes {
                        li role="none" {
                            button class="theme-item" type="button" role="menuitemradio"
                                data-theme-id=(theme.id)
                                aria-checked=(checked(theme.id == default_theme)) {
                                (theme.label)
                                @if theme.light != theme.dark {
                                    span class="theme-variant" {
                                        (if theme.light { "light only" } else { "dark only" })
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The `aria-checked` value for a radio item.
const fn checked(is_checked: bool) -> &'static str {
    if is_checked { "true" } else { "false" }
}

/// `true` for an id safe to write into a JavaScript string literal and a CSS
/// attribute selector: the charset `config` validates bundle themes against.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Applies stored site state before first paint, so nothing flashes.
///
/// Four keys, read synchronously in `<head>`: the theme family
/// (`okf-theme`) and the appearance (`okf-scheme`), applied as
/// `data-theme`/`data-scheme` on `<html>`, plus the collapsed nav
/// (`okf-nav`) and the closed nav submenus (`okf-nav-closed`). The markup
/// already carries the build's pinned defaults, so this script only has to
/// *change* the attributes when a stored choice differs — including
/// removing them, which is what `default` and `auto` mean.
///
/// The catalog is inlined as a family-id set because `<head>` runs before
/// the menu exists. It doubles as the allowlist: a stored id this build
/// does not define falls back to the build default rather than leaving
/// `<html>` carrying a `data-theme` no block matches. Ids outside
/// `[a-z0-9-]` are skipped, so the emitted literal cannot be escaped.
///
/// One migration lives here. Before themes had variants, `okf-theme` held
/// `light`/`dark` — an appearance, not a family — so a stored value that
/// names an appearance is read as one. Those ids are reserved against
/// bundle themes precisely so this stays unambiguous.
///
/// The submenu state cannot be an attribute or class here — the nav does
/// not exist yet — so it is applied as an injected stylesheet keyed on
/// `li[data-dir]`, mirroring the `aria-expanded` rules in `site.css`. It
/// outranks them on specificity (`#site-nav`) and `NAV_SUBMENU_BOOT` drops
/// it once the real aria state is on the toggles. Stored paths go through
/// `CSS.escape` (an unquoted attribute value is an identifier), so a
/// hand-written key cannot inject rules.
#[must_use]
pub fn theme_boot(
    themes: &[ThemeEntry<'_>],
    default_theme: &str,
    default_scheme: SchemePref,
) -> String {
    let mut families = String::from("{");
    for theme in themes.iter().filter(|theme| is_safe_id(theme.id)) {
        if families.len() > 1 {
            families.push(',');
        }
        let _ = write!(families, "'{}':1", theme.id);
    }
    families.push('}');
    let default_theme = if is_safe_id(default_theme) {
        default_theme
    } else {
        config::DEFAULT_THEME_ID
    };
    let mut js = String::with_capacity(THEME_BOOT_BODY.len() + families.len() + 80);
    js.push_str("(function () {  var F = ");
    js.push_str(&families);
    js.push_str(", DT = '");
    js.push_str(default_theme);
    js.push_str("', DS = '");
    js.push_str(default_scheme.as_str());
    js.push_str("';");
    js.push_str(THEME_BOOT_BODY);
    js
}

/// Everything in [`theme_boot`] after the catalog literal it is emitted
/// with.
const THEME_BOOT_BODY: &str = "\
  var root = document.documentElement;\
  var theme = DT, scheme = DS;\
  try {\
    var t = localStorage.getItem('okf-theme');\
    var s = localStorage.getItem('okf-scheme');\
    if (!s && (t === 'light' || t === 'dark')) { s = t; t = null; }\
    if (t && Object.prototype.hasOwnProperty.call(F, t)) theme = t;\
    if (s === 'auto' || s === 'light' || s === 'dark') scheme = s;\
  } catch (e) { /* no localStorage (e.g. privacy mode): the build defaults */ }\
  if (theme && theme !== 'default') root.setAttribute('data-theme', theme);\
  else root.removeAttribute('data-theme');\
  if (scheme === 'light' || scheme === 'dark') root.setAttribute('data-scheme', scheme);\
  else root.removeAttribute('data-scheme');\
  try {\
    if (localStorage.getItem('okf-nav') === 'collapsed') {\
      root.classList.add('nav-hidden');\
    }\
    var closed = JSON.parse(localStorage.getItem('okf-nav-closed') || '[]');\
    if (Array.isArray(closed) && closed.length) {\
      var hide = [], turn = [];\
      for (var i = 0; i < closed.length; i++) {\
        var sel = '#site-nav li[data-dir=' + CSS.escape(String(closed[i])) + ']';\
        hide.push(sel + '>ul');\
        turn.push(sel + '>.dir-row .dir-caret');\
      }\
      var st = document.createElement('style');\
      st.id = 'okf-nav-closed-style';\
      st.textContent = hide.join(',') + '{display:none}'\
        + turn.join(',') + '{transform:rotate(-90deg)}';\
      document.head.appendChild(st);\
    }\
  } catch (e) { /* no localStorage: the expanded nav */ }\
})()";

/// Wires the header's theme menu: opens and closes it, applies a chosen
/// family or appearance to `<html>`, persists it, and keeps `aria-checked`
/// accurate on both groups.
///
/// Everything it needs is already in the DOM — each item carries the axis
/// it belongs to and its value, the menu carries the build defaults — so
/// this script is the same bytes on every page. The two axes are
/// independent: changing appearance keeps the chosen family and simply
/// renders its other variant, or the base palette when it has none.
///
/// The keys are not namespaced per site, so several okf sites on one origin
/// share the reader's choices; the `storage` listener makes that visible
/// live in an already-open tab instead of only on its next load.
const THEME_MENU_BOOT: &str = "\
(function () {\
  var root = document.documentElement;\
  var picker = document.querySelector('.theme-picker');\
  if (!picker) return;\
  var btn = picker.querySelector('.theme-btn');\
  var menu = picker.querySelector('.theme-menu');\
  if (!btn || !menu) return;\
  var items = Array.prototype.slice.call(menu.querySelectorAll('.theme-item'));\
  if (!items.length) return;\
  var axes = {\
    theme: {\
      attr: 'data-theme-id',\
      key: 'okf-theme',\
      bare: 'default',\
      fallback: menu.getAttribute('data-default-theme')\
    },\
    scheme: {\
      attr: 'data-scheme-id',\
      key: 'okf-scheme',\
      bare: 'auto',\
      fallback: menu.getAttribute('data-default-scheme')\
    }\
  };\
  var group = function (axis) {\
    return items.filter(function (entry) { return entry.hasAttribute(axis.attr); });\
  };\
  var item = function (axis, value) {\
    var found = group(axis).filter(function (entry) {\
      return entry.getAttribute(axis.attr) === value;\
    });\
    return found.length ? found[0] : null;\
  };\
  var mark = function (axis, value) {\
    group(axis).forEach(function (entry) {\
      entry.setAttribute('aria-checked',\
        entry.getAttribute(axis.attr) === value ? 'true' : 'false');\
    });\
  };\
  var stored = function (key) {\
    try { return localStorage.getItem(key); } catch (e) { return null; }\
  };\
  var apply = function (axis, value, persist) {\
    var chosen = item(axis, value) || item(axis, axis.fallback) || group(axis)[0];\
    if (!chosen) return;\
    var picked = chosen.getAttribute(axis.attr);\
    var attribute = axis.attr === 'data-theme-id' ? 'data-theme' : 'data-scheme';\
    if (picked === axis.bare) root.removeAttribute(attribute);\
    else root.setAttribute(attribute, picked);\
    mark(axis, picked);\
    if (persist) {\
      try { localStorage.setItem(axis.key, picked); } catch (e) { /* ignore */ }\
    }\
    /* Diagrams re-render with the matching palette when either axis moves. */\
    root.dispatchEvent(new CustomEvent('okf-theme-change'));\
  };\
  var axisOf = function (entry) {\
    return entry.hasAttribute('data-theme-id') ? axes.theme : axes.scheme;\
  };\
  var open = function (yes) {\
    menu.hidden = !yes;\
    btn.setAttribute('aria-expanded', yes ? 'true' : 'false');\
    if (yes) {\
      var current = menu.querySelector('.theme-item[aria-checked=\"true\"]');\
      (current || items[0]).focus();\
    }\
  };\
  btn.addEventListener('click', function (e) {\
    e.stopPropagation();\
    open(menu.hidden);\
  });\
  items.forEach(function (entry, ix) {\
    entry.addEventListener('click', function () {\
      var axis = axisOf(entry);\
      apply(axis, entry.getAttribute(axis.attr), true);\
      open(false);\
      btn.focus();\
    });\
    entry.addEventListener('keydown', function (e) {\
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {\
        e.preventDefault();\
        var step = e.key === 'ArrowDown' ? 1 : items.length - 1;\
        items[(ix + step) % items.length].focus();\
      } else if (e.key === 'Home') {\
        e.preventDefault();\
        items[0].focus();\
      } else if (e.key === 'End') {\
        e.preventDefault();\
        items[items.length - 1].focus();\
      }\
    });\
  });\
  document.addEventListener('keydown', function (e) {\
    if (e.key === 'Escape' && !menu.hidden) {\
      open(false);\
      btn.focus();\
    }\
  });\
  document.addEventListener('click', function (e) {\
    if (!menu.hidden && !picker.contains(e.target)) open(false);\
  });\
  menu.addEventListener('focusout', function (e) {\
    if (!menu.hidden && !picker.contains(e.relatedTarget)) open(false);\
  });\
  window.addEventListener('storage', function (e) {\
    if (e.key === 'okf-theme') apply(axes.theme, e.newValue || axes.theme.fallback, false);\
    else if (e.key === 'okf-scheme') apply(axes.scheme, e.newValue || axes.scheme.fallback, false);\
  });\
  /* The markup ships the build defaults checked and the head boot has\
     already applied the stored choices; mirror them onto the menu, taking\
     the same legacy reading of an `okf-theme` that names an appearance. */\
  var theme = stored('okf-theme');\
  var scheme = stored('okf-scheme');\
  if (!scheme && (theme === 'light' || theme === 'dark')) { scheme = theme; theme = null; }\
  mark(axes.theme, item(axes.theme, theme) ? theme : axes.theme.fallback);\
  mark(axes.scheme, item(axes.scheme, scheme) ? scheme : axes.scheme.fallback);\
})()";

/// Wires the hamburger's checkbox to the persisted nav state: on load it
/// syncs the checkbox with the boot-applied `.nav-hidden` class; on change
/// it flips the class and stores `okf-nav` (`collapsed`/removed).
const NAV_TOGGLE_BOOT: &str = "\
(function () {\
  var root = document.documentElement;\
  var box = document.getElementById('nav-toggle');\
  if (!box) return;\
  box.checked = root.classList.contains('nav-hidden');\
  box.addEventListener('change', function () {\
    root.classList.toggle('nav-hidden', box.checked);\
    try {\
      if (box.checked) localStorage.setItem('okf-nav', 'collapsed');\
      else localStorage.removeItem('okf-nav');\
    } catch (e) { /* ignore */ }\
  });\
})()";

/// Wires the nav's per-directory collapse toggles. `aria-expanded` on each
/// `.dir-toggle` is the collapse state; the stylesheet turns the caret and
/// hides the submenu from it. On load the stored closed set is applied and
/// `THEME_BOOT`'s pre-paint style is dropped; each click flips one
/// directory and rewrites `okf-nav-closed` — a JSON array of the closed
/// directory paths, absent when every submenu is open (the default). The
/// set is re-read before each write so two tabs merge instead of
/// clobbering.
const NAV_SUBMENU_BOOT: &str = "\
(function () {\
  var nav = document.getElementById('site-nav');\
  if (!nav) return;\
  var KEY = 'okf-nav-closed';\
  function closed() {\
    try {\
      var v = JSON.parse(localStorage.getItem(KEY) || '[]');\
      return Array.isArray(v) ? v : [];\
    } catch (e) { return []; }\
  }\
  var stored = closed();\
  nav.querySelectorAll('.dir-toggle').forEach(function (btn) {\
    var li = btn.closest('li[data-dir]');\
    if (!li) return;\
    var dir = li.getAttribute('data-dir');\
    btn.setAttribute('aria-expanded', stored.indexOf(dir) === -1 ? 'true' : 'false');\
    btn.addEventListener('click', function () {\
      var open = btn.getAttribute('aria-expanded') !== 'true';\
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');\
      var set = closed();\
      var at = set.indexOf(dir);\
      if (open) { if (at !== -1) set.splice(at, 1); }\
      else if (at === -1) set.push(dir);\
      try {\
        if (set.length) localStorage.setItem(KEY, JSON.stringify(set));\
        else localStorage.removeItem(KEY);\
      } catch (e) { /* ignore */ }\
    });\
  });\
  var pre = document.getElementById('okf-nav-closed-style');\
  if (pre) pre.remove();\
})()";

/// Arms the header search input: on the first keystroke it injects the
/// search asset (the generated index + client, a classic script so it works
/// over `file://`); the client then binds the input and serves the query
/// that triggered the load. Until then, no search bytes are fetched — a
/// clean bundle pays nothing.
const SEARCH_ARM_BOOT: &str = "\
(function () {\
  var input = document.querySelector('.site-search input[type=\"search\"]');\
  if (!input || input.dataset.armed) return;\
  input.dataset.armed = '1';\
  function arm() {\
    if (window.okfSearchIndex !== undefined) return;\
    var s = document.createElement('script');\
    s.src = input.dataset.asset;\
    s.onerror = function () { input.disabled = true; input.placeholder = 'Search unavailable'; };\
    document.head.appendChild(s);\
  }\
  input.addEventListener('input', function () {\
    if (input.value.trim()) arm();\
  });\
  input.addEventListener('keydown', function (e) {\
    if (e.key === 'Enter') arm();\
  });\
})()";

/// The hamburger icon: a plain inline SVG whose stroke inherits the button's
/// current color, so light/dark schemes both work with no custom CSS.
const MENU_ICON: &str = "\
<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" \
stroke-width=\"2\" stroke-linecap=\"round\" width=\"20\" height=\"20\" \
aria-hidden=\"true\"><path d=\"M4 6h16M4 12h16M4 18h16\"/></svg>";

/// The submenu caret: one chevron pointing down, which the stylesheet
/// rotates to point right when its toggle reports `aria-expanded="false"`.
/// Stroke inherits the button's color like the other glyphs.
const CARET_ICON: &str = "\
<svg class=\"dir-caret\" viewBox=\"0 0 24 24\" fill=\"none\" \
stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" \
stroke-linejoin=\"round\" width=\"14\" height=\"14\" aria-hidden=\"true\">\
<path d=\"M6 9l6 6 6-6\"/></svg>";

/// The mermaid bootstrap, a classic inline script placed at the end of
/// `<body>` so `.mermaid` nodes already exist. Strict security level (the
/// default) sanitizes the diagram source; `startOnLoad: false` plus an
/// explicit `run` keeps the render observable and marks processed nodes.
/// The scheme comes from `<html data-scheme>` (falling back to the system
/// preference when no theme is chosen) and diagrams re-render on
/// `okf-theme-change`, so labels and edges stay legible on every card.
///
/// Diagrams follow the palette, not just the axis: `fontFamily` and the
/// mapped `themeVariables` are read from the computed tokens, so any
/// palette — built-in or one a bundle defined in its `.okf/config.yaml` —
/// reaches diagrams with no Rust-side knowledge of it. `base` is the only
/// mermaid theme that honors `themeVariables` wholesale, so a successful
/// token read selects it and `darkMode` tells it which way to derive the
/// shades it computes itself; an empty read falls back to mermaid's own
/// `default`/`dark`.
///
/// Token values are converted to hex first, by painting one pixel and
/// reading it back. Mermaid derives its shades with khroma, which parses
/// hex, `rgb()`, and `hsl()` but not `oklch()` — the syntax this site's
/// palette is authored in — and a color it cannot parse rejects the whole
/// `run`, leaving every diagram unrendered. The canvas is the browser's own
/// parser, so it converts any CSS color the stylesheet can hold, including
/// a bundle's, without shipping a color-space implementation.
const fn mermaid_boot() -> &'static str {
    "\
if (window.mermaid) {\
  var isDark = function () {\
    var scheme = document.documentElement.getAttribute('data-scheme');\
    if (scheme) return scheme === 'dark';\
    return window.matchMedia('(prefers-color-scheme: dark)').matches;\
  };\
  var nodes = document.querySelectorAll('pre.mermaid');\
  /* Cache each diagram's source before the first render replaces it with\
     SVG, so a theme change can rebuild them with the matching palette. */\
  nodes.forEach(function (el) { el.dataset.diagram = el.textContent; });\
  var renderDiagrams = function (dark) {\
    nodes.forEach(function (el) {\
      el.textContent = el.dataset.diagram;\
      el.removeAttribute('data-processed');\
    });\
    var style = getComputedStyle(document.documentElement);\
    var canvas = document.createElement('canvas');\
    canvas.width = canvas.height = 1;\
    var pixel = canvas.getContext('2d', { willReadFrequently: true });\
    var token = function (name) { return style.getPropertyValue(name).trim(); };\
    var hex = function (name) {\
      var value = token(name);\
      if (!value || !pixel) return '';\
      pixel.clearRect(0, 0, 1, 1);\
      pixel.fillStyle = value;\
      pixel.fillRect(0, 0, 1, 1);\
      var rgb = pixel.getImageData(0, 0, 1, 1).data;\
      return '#' + [rgb[0], rgb[1], rgb[2]].map(function (c) {\
        return ('0' + c.toString(16)).slice(-2);\
      }).join('');\
    };\
    var cfg = {\
      securityLevel: 'strict',\
      startOnLoad: false,\
      theme: dark ? 'dark' : 'default'\
    };\
    var font = token('--font-body');\
    if (font) cfg.fontFamily = font;\
    var bg = hex('--mermaid-bg');\
    var ink = hex('--ink');\
    var edge = hex('--edge');\
    if (bg && ink && edge) {\
      /* The canvas is the card the diagram sits on, so nodes take the page\
         surface instead: same two tokens the rest of the page layers, and\
         a node that matched its card would read as flat. */\
      var node = hex('--surface') || bg;\
      cfg.theme = 'base';\
      cfg.themeVariables = {\
        darkMode: dark,\
        background: bg,\
        primaryColor: node,\
        mainBkg: node,\
        secondaryColor: hex('--code-bg') || node,\
        tertiaryColor: hex('--panel-bg') || node,\
        primaryTextColor: ink,\
        secondaryTextColor: ink,\
        tertiaryTextColor: ink,\
        textColor: ink,\
        nodeTextColor: ink,\
        lineColor: edge,\
        primaryBorderColor: edge,\
        secondaryBorderColor: edge,\
        tertiaryBorderColor: edge,\
        nodeBorder: edge\
      };\
    }\
    mermaid.initialize(cfg);\
    mermaid.run({ querySelector: 'pre.mermaid', suppressErrors: true })\
      .then(function () {\
        nodes.forEach(function (el) { el.dataset.processed = '1'; });\
      })\
      .catch(function (e) { console.warn('okf-web: mermaid render failed:', e); });\
  };\
  renderDiagrams(isDark());\
  document.documentElement.addEventListener('okf-theme-change', function () {\
    renderDiagrams(isDark());\
  });\
}"
}

/// The shiki bootstrap, a classic inline script placed at the end of
/// `<body>`, like the mermaid one. The writer emits every fenced block as
/// `<pre class="md-pre"><code class="md-code language-X">escaped</code></pre>`;
/// the boot harvests those languages (lowercased: fence info strings are
/// case-insensitive, shiki's registry is not), registers them from the
/// vendored bundle, and swaps each `<pre>` wholesale for shiki's
/// dual-theme HTML — the vendored `highlight` already restyles the `<pre>`
/// with the site's `md-pre` class. Colors travel as `--shiki-light`/
/// `--shiki-dark` CSS variables, so the theme toggle needs no re-render
/// here. The vendored bundle assigns `window.okfShikiReady` synchronously
/// (a promise; `window.okfShiki` appears only after the inlined-WASM
/// highlighter initializes), so the boot chains on that instead of
/// guarding on `okfShiki` — a synchronous guard would race the WASM init
/// and lose. The escaped source stays until the swap, so no-JS and load
/// failures degrade to the plain monospace block; unknown languages keep
/// the `<pre>` as-is: a lang the bundle does not carry renders unhighlighted,
/// never broken.
const fn shiki_boot() -> &'static str {
    "\
if (window.okfShikiReady) {\
  var langOf = function (code) {\
    for (var i = 0; i < code.classList.length; i++) {\
      var m = /^language-(.+)$/.exec(code.classList[i]);\
      if (m) return m[1].toLowerCase();\
    }\
    return null;\
  };\
  var langs = [];\
  document.querySelectorAll('article pre.md-pre > code').forEach(function (c) {\
    var lang = langOf(c);\
    if (lang && langs.indexOf(lang) === -1) langs.push(lang);\
  });\
  window.okfShikiReady.then(function () {\
    return window.okfShiki.load(langs);\
  }).then(function () {\
    document.querySelectorAll('article pre.md-pre > code').forEach(function (c) {\
      var lang = langOf(c);\
      if (lang === null) return;\
      var html = window.okfShiki.highlight(c.textContent, lang);\
      if (html === null) return;\
      c.parentNode.outerHTML = html;\
    });\
  });\
}"
}

/// The nav tree: the bundle's directory structure with links to every
/// concept page, plus the special pages.
#[must_use]
pub fn nav_tree(bundle: &Bundle, prefix: &str, current_rel: &str) -> Markup {
    // Build a nested tree from concept ids, then render as nested <ul>. Each
    // link is marked `.active` when its target matches the current page's
    // root-relative path (`PagePath::rel`), which is the href minus `prefix`.
    let mut root = NavNode::default();
    for concept in bundle.concepts() {
        let mut segments = concept.id.segments().to_vec();
        segments.pop(); // drop the leaf name; the chain that remains is directories
        let mut node = &mut root;
        for seg in &segments {
            node = node.children.entry(seg.clone()).or_default();
        }
        node.leaves
            .push((concept.id.clone(), concept.display_title()));
    }
    html! {
        div class="special" {
            a class=[(current_rel == "index.html").then_some("active")] href=(format!("{prefix}index.html")) { "Dashboard" }
            a class=[(current_rel == "__okf/graph.html").then_some("active")] href=(format!("{prefix}__okf/graph.html")) { "Graph" }
        }
        (root.render(prefix, "", current_rel))
    }
}

/// A directory node in the nav tree.
#[derive(Default)]
struct NavNode {
    children: BTreeMap<String, Self>,
    leaves: Vec<(ConceptId, String)>,
}

impl NavNode {
    /// `prefix` is the page's `../`-chain to the site root; `dir` is this
    /// node's `/`-joined path from the bundle root (empty at the top).
    ///
    /// Directory items render as a `.dir-row` (the link plus its collapse
    /// toggle) over the nested `<ul>`. `data-dir` carries the node's bundle
    /// path: it keys both the persisted closed set and the pre-paint style
    /// the boot script injects. Every submenu renders expanded, so a no-JS
    /// page shows the whole tree.
    fn render(&self, prefix: &str, dir: &str, current_rel: &str) -> Markup {
        html! {
            ul {
                @for (id, title) in &self.leaves {
                    @let active = format!("{id}.html") == current_rel;
                    li {
                        a class=[active.then_some("active")] href=(format!("{prefix}{id}.html")) { (title) }
                    }
                }
                @for (seg, child) in &self.children {
                    @let child_dir = if dir.is_empty() { seg.clone() } else { format!("{dir}/{seg}") };
                    @let active = format!("{child_dir}/index.html") == current_rel;
                    @let label = segment_title(seg);
                    li data-dir=(child_dir) {
                        div class="dir-row" {
                            a class=(if active { "dir active" } else { "dir" }) href=(format!("{prefix}{child_dir}/index.html")) { (label) }
                            // `aria-expanded` is the only collapse state:
                            // the stylesheet derives both the caret's
                            // direction and the submenu's visibility from
                            // it, so the accessible state cannot drift from
                            // the rendered one.
                            button class="dir-toggle" type="button" aria-expanded="true"
                                aria-label=(format!("Toggle {label}")) {
                                (PreEscaped(CARET_ICON))
                            }
                        }
                        (child.render(prefix, &child_dir, current_rel))
                    }
                }
            }
        }
    }
}

/// Formats a directory segment into a display title: splits on `-`, `_`, and
/// whitespace, then capitalizes the first letter of each word. `"foo-kebab"`
/// becomes `"Foo Kebab"`; `"foo space"` becomes `"Foo Space"`.
fn segment_title(seg: &str) -> String {
    let mut words = Vec::new();
    for part in seg.split(|c: char| c == '-' || c == '_' || c.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        let first = chars.next().unwrap_or_default().to_uppercase().to_string();
        words.push(format!("{first}{}", chars.as_str()));
    }
    if words.is_empty() {
        seg.to_string()
    } else {
        words.join(" ")
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
        has_shiki: body.has_shiki,
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
        has_shiki: false,
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
        p class="md-p" { "Every concept and cross-link in the bundle. Dashed edges mark broken links (permitted by the spec: they may be not-yet-written knowledge)." }
        pre class="mermaid" { (src) }
    };

    SitePage {
        rel_path: PagePath::Graph,
        title: "Cross-link graph".to_string(),
        body_html: body.into_string(),
        has_mermaid: true,
        has_shiki: false,
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
        // Same md-* component classes the markdown writer emits, so the
        // generated index pages share the site's prose styles.
        let section = html! {
            @if !first { hr class="md-hr" {}
            }
            h2 class="md-h2" { (type_) }
            ul class="md-list" {
                @for (title, id, description) in &entries {
                    li class="md-li" {
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
        title: segment_title(dir.name()),
        body_html: body,
        has_mermaid: false,
        has_shiki: false,
        meta_rows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in variant must have a block in the compiled stylesheet.
    ///
    /// The catalog is knowledge held twice — once as a Rust table, once as
    /// CSS — and the failure mode of drift is silent: a menu entry whose
    /// variant has no rules looks like the base palette, which is also what
    /// a *deliberately* missing variant looks like. `default` is the base
    /// palette itself, so it is the one family with no `[data-theme]`
    /// block. The Tailwind minifier drops the quotes from attribute values,
    /// so both spellings count.
    #[test]
    fn builtin_themes_have_css_blocks() {
        for theme in BUILTIN_THEMES {
            assert!(is_safe_id(theme.id), "unsafe built-in id `{}`", theme.id);
            assert!(
                theme.light || theme.dark,
                "`{}` defines no variant at all",
                theme.id
            );
            if theme.id == config::DEFAULT_THEME_ID {
                assert!(theme.light && theme.dark, "the base family covers both");
                continue;
            }
            for scheme in [Scheme::Light, Scheme::Dark] {
                let quoted = format!(
                    "[data-theme=\"{}\"][data-scheme=\"{}\"]",
                    theme.id,
                    scheme.as_css()
                );
                let bare = format!("[data-theme={}][data-scheme={}]", theme.id, scheme.as_css());
                let present = SITE_CSS.contains(&quoted) || SITE_CSS.contains(&bare);
                assert_eq!(
                    present,
                    theme.has(scheme),
                    "`{}` {} variant: table and site.css disagree; run `cargo xtask tailwind`",
                    theme.id,
                    scheme.as_css()
                );
            }
        }
    }

    /// The scheme ladders every palette relies on must survive a recompile.
    #[test]
    fn site_css_keys_schemes_on_the_data_attribute() {
        for selector in [
            "[data-scheme=dark]",
            "[data-scheme=light]",
            ":not([data-scheme])",
        ] {
            assert!(SITE_CSS.contains(selector), "site.css lost `{selector}`");
        }
        assert!(
            !SITE_CSS.contains(":root.dark"),
            "site.css still carries the pre-attribute theme classes"
        );
    }

    /// The boot inlines the family catalog as its allowlist, and only ids
    /// that can be written into a JavaScript string literal reach it.
    #[test]
    fn theme_boot_inlines_the_catalog() {
        let themes = [
            ThemeEntry {
                id: config::DEFAULT_THEME_ID,
                label: "Default",
                light: true,
                dark: true,
            },
            ThemeEntry {
                id: "nord",
                label: "Nord",
                light: false,
                dark: true,
            },
            ThemeEntry {
                id: "no'quotes",
                label: "Hostile",
                light: true,
                dark: false,
            },
        ];
        let js = theme_boot(&themes, "nord", SchemePref::Dark);
        assert!(js.contains("var F = {'default':1,'nord':1}"), "{js}");
        assert!(js.contains("DT = 'nord', DS = 'dark'"), "{js}");
        assert!(!js.contains("no'quotes"), "unsafe id reached the literal");

        // An id the build does not define cannot become the default: the
        // fallback is the base family, which sets no attribute at all.
        let js = theme_boot(&themes, "not an id", SchemePref::Auto);
        assert!(js.contains("DT = 'default', DS = 'auto'"), "{js}");
    }
}
