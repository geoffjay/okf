# okf-web

`okf site` — a pure-Rust static site generator for
[Open Knowledge Format (OKF)](https://github.com/W4G1/okf) bundles.

Where `okf studio` is the resident, interactive view of a bundle, `okf site`
is the deployable one: a build-time pipeline that turns a bundle into a
directory of HTML — no server, no WASM, no client-side framework.

```text
okf site [bundle]   # --today, --out DIR, --title, --theme, --scheme; writes <bundle>/site
```

What it emits:

- One page per concept at the concept's bundle-relative path (`.md` → `.html`),
  with relative links so the site deploys under any base path.
- A nav tree mirroring the bundle layout — every directory is a collapsible
  submenu with a caret toggle (down when open, right when closed), expanded
  by default and remembered in `localStorage` — plus a trust dashboard
  (`__okf/dashboard.html`) and a bundle-wide mermaid cross-link graph
  (`__okf/graph.html`).
- Rendered mermaid diagrams: ` ```mermaid ` blocks become client-side
  diagrams via a vendored `mermaid.min.js` (MIT; see
  `assets/vendor/README.md`), shipped only when some page has a diagram.
- Syntax-highlighted code blocks: fenced blocks with a language tag are
  highlighted client-side by a vendored `shiki.min.js` (MIT; see
  `assets/vendor/README.md`) — every shiki language, GitHub light/dark
  themes, colors as CSS variables resolved from the page's `data-scheme`, so
  switching theme costs no re-render — shipped only when some page has code.
  Highlighter directives after the language are ignored, so the rustdoc form
  (` ```rust,no_run `) highlights like a bare ` ```rust `. The escaped source
  stays in the `<pre>` until the highlight swaps it in, so no-JS degrades
  gracefully. Every block — fenced or indented, highlighted or not — renders
  inside the same bordered, padded card a diagram gets, on the code surface
  token.
- A theme menu in the header with two axes: an appearance (`auto`, `light`,
  `dark`) and a theme family (`default`, `sepia`, `contrast`, plus the
  bundle's own). See below.
- Frontmatter panels with trust/status/staleness badges, backlinks, sources
  with OKF footnote→source attribution, headings TOC, and validator/lint
  findings — everything the studio Explorer inspector shows.
- A single inline stylesheet compiled from Tailwind CSS v4, so pages fetch no
  external CSS (mermaid and shiki are the only external assets, and only on
  pages that use them).
- A generated `assets/fonts.css` (plus `assets/fonts/`) when the bundle
  configures fonts, and a generated `assets/theme.css` when it defines
  themes — see below — linked after the inline stylesheet so their token
  overrides win.

Themes are values, not rules, which is why they switch at runtime against
an ahead-of-time stylesheet: every color resolves through a `:root` custom
property. Two independent attributes carry the choice on `<html>`, applied
from `localStorage` (`okf-scheme`, `okf-theme`) before first paint so a
reload never flashes:

- `data-scheme` — `light` or `dark`, absent to follow the system. It is the
  appearance: the base palette, `color-scheme`, which shiki variable
  resolves, which mermaid mode renders, which glyph the theme button shows.
- `data-theme` — a *family* id whose variant for the current appearance
  layers on top. Absent means the built-in `default` family.

A family holds up to two variants, so flipping the appearance switches
between a theme's light and dark forms. A variant a family does not define
is absent from the stylesheet, so that appearance falls through to the base
palette — `sepia` is light-only, and the menu says so rather than leaving
the fallback to be discovered.

With neither attribute the page is exactly what it was before themes
existed: a light stylesheet with a `prefers-color-scheme: dark` override.
A bundle that pins a theme gets the attribute rendered into the markup, so
that reaches a reader with no JavaScript too.

Per-bundle configuration: a bundle may pin its site settings in
`.okf/config.yaml`, read by `okf_web::config::SiteConfig` with okf-core's
std-only YAML parser (no new dependency). `.okf/` is invisible to every OKF
walker, so configuration never becomes content.

```yaml
site:
  title: "Travel knowledge"      # `okf site --title` overrides this
  theme: nord                    # first-visit theme; `okf site --theme` overrides it
  scheme: auto                   # first-visit appearance; `--scheme` overrides it
  themes:                        # one entry per variant, merged by id
    - id: nord                   # [a-z][a-z0-9-]{0,31}; may take a built-in id
      label: "Nord"              # menu text; defaults to the id
      scheme: dark               # optional, defaults to light
      colors:                    # closed token set; every key optional
        surface: "#2e3440"
        ink: "#eceff4"
        edge: "#4c566a"
        link: "#88c0d0"
    - id: nord                   # same id = this family's other variant
      scheme: light
      colors:
        surface: "#eceff4"
        ink: "#2e3440"
  fonts:
    body: "Newsreader, Georgia, serif"
    code: "JetBrains Mono, ui-monospace, monospace"
    files:                       # self-hosted: .okf/fonts/<file>
      - family: Newsreader
        file: newsreader-400.woff2
        weight: 400
```

Typography resolves through the `--font-body`/`--font-code` custom
properties and palettes through the nineteen color tokens
(`config::THEME_TOKENS`), so every setting is a token override in a
generated stylesheet — `site.css` stays universal and no bundle needs a
Tailwind recompile. Faces are copied to `assets/fonts/` and described with
`@font-face` (stylesheet-relative `url()`, so one href works at every page
depth and over `file://`); variants become
`:root[data-theme="…"][data-scheme="…"]` blocks in `assets/theme.css`
(each with a `prefers-color-scheme` twin for the attribute-less `auto`
case), linked last so they win the cascade tie. A variant overrides only
what it names — the appearance's base palette seeds the rest — and a family
that takes a built-in id restyles that theme rather than adding a second
menu entry, including supplying a variant the built-in lacks. `auto`,
`light`, and `dark` are reserved ids: they name appearances, so they belong
in `scheme:`. Mermaid reads the body font and the palette tokens, so
diagrams follow both.

A bundle with no `.okf/` writes neither generated stylesheet. Content stays
permissive, configuration is strict: unknown keys, bad values, missing font
files, a duplicate `(id, scheme)` variant, conflicting labels for one
family, and a `theme:` naming a theme nothing defines all fail the build,
while an unknown top-level section is only reported
(`SiteSummary::notes`). `SiteSummary` also names the file the build read
(`config`) and the settings it supplied (`config_settings`), which is what
separates a configuration that took effect from one the build never saw.
CSS is constructed rather than interpolated — quoted family segments,
closed vocabularies for weight/style and for color syntax, flat ASCII file
names and theme ids — so no configured string can inject CSS or escape
`.okf/fonts/`.

Family lists name families the *viewer's* browser resolves; a family that is
neither installed there nor listed under `files:` falls back to the next
segment, so `files:` is the only way to guarantee a face. Names must match
the family exactly (`Maple Mono`, not `MapleMono`).

Security: no raw HTML passthrough from markdown bodies (pulldown-cmark's
`unsafe` stays off), maud escapes every frontmatter interpolation, and
external source URLs carry `rel="noopener noreferrer"`.

Regenerating the CSS: styling lives in `assets/tailwind.css`, the only
hand-authored stylesheet; run `cargo xtask tailwind` to recompile the vendored
`assets/site.css` with the Tailwind v4 standalone CLI (fetched on first use, or
set `$TAILWIND_BIN` to a local binary). `okf site` itself needs no toolchain —
it embeds the committed `site.css` at build time.

This crate is a library with a single entry point (`okf_web::generate`); the
`okf` binary's `site` subcommand is a thin wrapper around it. See
`docs/knowledgebase/decisions/okf_web_static_site.md` in the repository
for the design record.