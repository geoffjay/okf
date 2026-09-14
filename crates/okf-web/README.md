# okf-web

`okf site` — a pure-Rust static site generator for
[Open Knowledge Format (OKF)](https://github.com/W4G1/okf) bundles.

Where `okf studio` is the resident, interactive view of a bundle, `okf site`
is the deployable one: a build-time pipeline that turns a bundle into a
directory of HTML — no server, no WASM, no client-side framework.

```text
okf site [bundle]   # --today, --out DIR; writes <bundle>/site by default
```

What it emits:

- One page per concept at the concept's bundle-relative path (`.md` → `.html`),
  with relative links so the site deploys under any base path.
- A nav tree mirroring the bundle layout, a trust dashboard
  (`__okf/dashboard.html`), and a bundle-wide mermaid cross-link graph
  (`__okf/graph.html`).
- Rendered mermaid diagrams: ` ```mermaid ` blocks become client-side
  diagrams via a vendored `mermaid.min.js` (MIT; see
  `assets/vendor/README.md`), shipped only when some page has a diagram.
- Frontmatter panels with trust/status/staleness badges, backlinks, sources
  with OKF footnote→source attribution, headings TOC, and validator/lint
  findings — everything the studio Explorer inspector shows.
- A single inline stylesheet compiled from Tailwind CSS v4, so pages fetch no
  external CSS (mermaid is the only external asset, and only on diagram pages).

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