# okf-web

`okf site` — a pure-Rust static site generator for
[Open Knowledge Format (OKF)](https://github.com/W4G1/okf) bundles.

Where `okf studio` is the resident, interactive view of a bundle, `okf site`
is the deployable one: a build-time pipeline that turns a bundle into a
directory of HTML — no server, no WASM, no client-side framework.

```text
okf site [bundle]   # --today, --out DIR, --title; writes <bundle>/site by default
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
  themes, colors as CSS variables so the theme toggle is free — shipped
  only when some page has code. Highlighter directives after the language
  are ignored, so the rustdoc form (` ```rust,no_run `) highlights like a
  bare ` ```rust `. The escaped source stays in the `<pre>` until the
  highlight swaps it in, so no-JS degrades gracefully.
- Frontmatter panels with trust/status/staleness badges, backlinks, sources
  with OKF footnote→source attribution, headings TOC, and validator/lint
  findings — everything the studio Explorer inspector shows.
- A single inline stylesheet compiled from Tailwind CSS v4, so pages fetch no
  external CSS (mermaid and shiki are the only external assets, and only on
  pages that use them).
- A generated `assets/fonts.css` (plus `assets/fonts/`) when the bundle
  configures fonts — see below — linked after the inline stylesheet so its
  token overrides win.

Per-bundle configuration: a bundle may pin its site settings in
`.okf/config.yaml`, read by `okf_web::config::SiteConfig` with okf-core's
std-only YAML parser (no new dependency). `.okf/` is invisible to every OKF
walker, so configuration never becomes content.

```yaml
site:
  title: "Travel knowledge"      # `okf site --title` overrides this
  fonts:
    body: "Newsreader, Georgia, serif"
    code: "JetBrains Mono, ui-monospace, monospace"
    files:                       # self-hosted: .okf/fonts/<file>
      - family: Newsreader
        file: newsreader-400.woff2
        weight: 400
```

Typography resolves through the `--font-body`/`--font-code` custom
properties, so a font setting is a token override in the generated
stylesheet — `site.css` stays universal and no bundle needs a Tailwind
recompile. Faces are copied to `assets/fonts/` and described with
`@font-face` (stylesheet-relative `url()`, so one href works at every page
depth and over `file://`); mermaid reads the body token for diagram labels.
A bundle with no `.okf/` emits byte-identical output to a pre-configuration
build. Content stays permissive, configuration is strict: unknown keys, bad
values, and missing font files fail the build, while an unknown top-level
section is only reported (`SiteSummary::notes`). CSS is constructed rather
than interpolated — quoted family segments, closed vocabularies for
weight/style, flat ASCII file names — so no configured string can inject
CSS or escape `.okf/fonts/`.

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