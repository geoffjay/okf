---
type: Plan
title: okf-web as a maud + pulldown-cmark static site generator
description: "Plan for okf-web, the static site generator crate: pure-Rust build pipeline over okf-core and okf-validator, maud templates, pulldown-cmark rendering, mermaid.js vendored as an asset, and no WASM framework."
status: stable
generated:
  by: human:w4g1
  at: "2026-09-11T00:00:00Z"
stale_after: "2026-10-31T00:00:00Z"
tags:
  - okf-web
  - ssg
  - architecture
  - planning
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: maud
    resource: https://docs.rs/maud
    title: maud compile-time HTML templating
  - id: pulldown-cmark
    resource: https://docs.rs/pulldown-cmark
    title: pulldown-cmark markdown parser
  - id: mermaid
    resource: https://mermaid.js.org/
    title: mermaid.js diagram renderer
---

# okf-web: a pure-Rust static site generator for OKF bundles

## Motivation

`okf studio` renders markdown bodies as styled terminal text; fenced
`mermaid` blocks appear as boxed code, never rendered. The goal is a
static website view of a bundle where mermaid diagrams actually render, plus
trust badges, staleness, backlinks, and the cross-link graph — deployable to
any static host (GitHub Pages included), no server, no WASM.

The full decision trail — why not okf-studio as a dependency, why not
Yew/Leptos/Dioxus/Topcoat, link-rewriting and escaping strategy — is recorded
in [Decision: okf-site vs okf-web naming and dependency choice](../decisions/okf_web_static_site.md).

## Scope

In scope:

- New crate `crates/okf-web/` replacing the placeholder at
  `crates/reserved/okf-web/`, wired as `okf site` behind a `site` cargo
  feature on the `okf` binary (mirroring how `studio` is wired).
- Pure build-time generation: Rust binary in, directory of HTML out.
- Mermaid diagram rendering client-side via vendored mermaid.js assets.

Out of scope (v1):

- Search (pagefind or other post-build indexers).
- Interactive graph exploration (force/radial layout, pan, zoom, focus).
- Any client-side Rust/WASM, any JS framework, any server.

## Architecture

Zola is the template: a build-time generator, not an app. There is no
client-side state to be reactive about, so no reactive framework — Yew
(CSR-only, empty-div runtime rendering), Leptos (real SSG but drags SSR
machinery and hydration runtime for output with no event handlers), Dioxus
(prerenders by running the app and curling itself), and Topcoat (a server
framework, pre-1.0, wrong shape for static hosting) are all the wrong tool.
Maud[^maud] supplies the component-style `html!` ergonomics with zero
runtime; Yew experience transfers directly. Diagrams render with
mermaid.js[^mermaid], and markdown rendering uses
pulldown-cmark[^pulldown-cmark].

### Crate layout

```text
crates/okf-web/src/
├── lib.rs          SiteOptions { root, out_dir, today } → pub fn generate()
│                   (entry point mirrors okf_studio::run)
├── render.rs       maud templates: page layout, concept page, dashboard,
│                   graph page, nav tree
└── markdown.rs     body → HTML: rewrite links → pulldown-cmark event filter
                    → intercept mermaid blocks → push_html
```

### Build-time pipeline

```text
Bundle::load (okf-core, permissive: parse errors become marked nodes)
  → per concept:
      okf_core::markdown::rewrite_markdown_links  (.md targets → output URLs,
                                                    skips code fences/inline code)
      → pulldown_cmark::Parser event stream (Options::ENABLE_TABLES)
      → intercept ```mermaid``` blocks → <pre class="mermaid">src</pre>
        + has_mermaid page flag
      → pulldown_cmark::html::push_html (handles all text/code escaping)
  → maud page: nav tree, frontmatter panel (trust tier, status, staleness
    badge), backlinks (Bundle::backlinks), sources panel (FootnoteRef,
    ResolvedSource), headings TOC (extract_headings, heading_slug)
  → write page file at the concept's bundle-relative path with .html
```

Health badges come from `okf_validator::{validate_bundle_at, lint_bundle_at}`
— the same calls `okf_studio::snapshot::Snapshot::build` makes — with the
language-check features forwarded the same way okf-studio forwards them.

### Mermaid strategy

The primary motivation, and the one place "only Rust dependencies" cannot
hold: there is no production-grade pure-Rust mermaid renderer, and executing
mermaid.js at build time would put a JS runtime in the build chain — a worse
violation than vendoring.

- Vendor mermaid as a static asset in the generated site's `assets/`
  directory (like the repo's `assets/okf_studio.png`), not a crate
  dependency. **Implemented as the single-file `dist/mermaid.min.js`** (a
  self-contained ~5 MB IIFE), embedded into the `okf-web` binary with
  `include_bytes!` and written to `assets/mermaid.min.js`. This supersedes
  the original "vendor the ESM `dist/` tree" plan: the real mermaid 12 ESM
  tree is ~63 MB across hundreds of chunk files, and ESM module scripts are
  blocked by CORS over `file://`, so the site would not open without a
  server. A classic `<script src>` is smaller, simpler, and opens directly
  from disk.
- Emit the mermaid `<script>` only on pages flagged `has_mermaid`, and write
  the asset only when some page carries a diagram, so clean bundles never
  download or ship it.
- Initialize with `securityLevel: 'strict'` (the default), which sanitizes
  diagram source, plus `mermaid.run({ suppressErrors: true })` so a malformed
  diagram leaves its escaped source visible instead of throwing.
- Escape the diagram source into the `<pre class="mermaid">` so the raw text
  survives render failures and noscript.
- The bundle-wide graph page reuses the `okf graph --format mermaid` flowchart
  emitter logic (flowchart LR, `mermaid_label` escaping, phantom nodes for
  broken links) rendered by the same vendored mermaid.

### Security posture

- No raw HTML passthrough from markdown bodies: drop `Html` events before
  `push_html` (pulldown-cmark `unsafe` option stays off).
- Maud escapes every frontmatter interpolation (title, actors, tags) — a
  `<script>` in a `title:` is a real XSS vector; verified in a prototype
  that maud escapes and pulldown escapes code correctly.
- External `sources[].resource` URLs get `rel="noopener noreferrer"`.

### Determinism

`SiteOptions.today: Option<Date>` pins the build date for staleness badges
("deterministic by default" is the workspace's documented design choice);
the emitted dashboard carries an "as of" date. Output paths and relative
URLs mirror the bundle layout so the site deploys under any base path
without a `--base-url` flag.

## Wiring into the workspace

Mirror the studio precedent exactly (`crates/okf/Cargo.toml`):

1. New workspace member `crates/okf-web/` (replace the reserved placeholder)
   with `okf-core` + `okf-validator` as dependencies; no `okf-studio`
   dependency — the TUI snapshot drags ratatui/crossterm and is shaped for
   panes, not pages. Shared derivations belong in `okf-core` if overlap
   grows.
2. `okf` crate: `site = ["validator", "dep:okf-web"]` feature, default-on
   like `studio`; `Commands::Site(SiteArgs)` behind `#[cfg(feature = "site")]`
   with a thin `cmd_site` wrapper. `cargo-okf` inherits for free.
3. Page map from the studio tabs: Explorer → concept pages + directory
   indexes; Graph → mermaid overview page; Trust → static dashboard (tier
   distribution, attention queue, actor stats); Computations → contract
   pages with syntax-check badges.

## Phasing

1. **Milestone 1 — walking skeleton.** Crate skeleton with
   `generate(SiteOptions) -> io::Result<()>`; load bundle, emit one HTML
   page per concept with maud layout and rewritten links; mermaid blocks
   intercepted with vendored mermaid; wire `okf site` subcommand. Acceptance:
   `okf site .` on a bundle containing a mermaid diagram produces a site
   that renders the diagram in a browser with no console errors.
2. **Milestone 2 — parity panels.** Frontmatter panel with trust/status/
   staleness badges, backlinks, sources panel with OKF footnote→source
   linking, headings TOC. Acceptance: every concept page shows what the
   studio Explorer inspector shows.
3. **Milestone 3 — dashboard + graph.** Trust dashboard with attention queue
   and the bundle-wide mermaid graph page. Acceptance: dashboard numbers
   match `okf trust .` / `okf info .` output for the same `--today`.
4. **Milestone 4 — release.** Docs (README section + workspace crate table),
   feature-gating tests in CI matrix, changelog, version bump per repo
   release process.

Deferred to a v2 decision: interactive graph exploration. The trigger would
be pan/zoom/egocentric focus that mermaid cannot deliver; the vehicle would
be Leptos islands reusing `okf-studio`'s std-only graph model and layout
(`graph/model.rs`, `graph/layout.rs` — deterministic FNV-seeded
Fruchterman–Reingold, no ratatui imports, compiles to WASM as-is) as one
island on otherwise-static pages.

## Risks

- Mermaid render failures on malformed diagrams: mitigated by keeping the
  escaped source visible in the `<pre>` fallback.
- Large-bundle graph pages: mitigated by the v1 node cap / per-directory
  subgraphs.
- Upstream (W4G1/okf) direction drift on the `okf-web` name: the reserved
  crate exists precisely for "web-based bundle viewing"; if the fork and
  upstream diverge, rename to `okf-site` at the cost of the natural
  upstream-accept story.

## Verification

- Milestone 1: generated site opened in a browser; mermaid renders; links
  navigate between generated pages.
- `okf validate` and `okf lint` run against this knowledgebase after every
  edit, and must pass with no errors.
- Escape tests: frontmatter containing `<script>` produces escaped output
  (prototype-verified with maud + pulldown-cmark; port into unit tests).

[^okf-repo]: okf workspace repository
[^maud]: maud compile-time HTML templating
[^pulldown-cmark]: pulldown-cmark markdown parser
[^mermaid]: mermaid.js diagram renderer
