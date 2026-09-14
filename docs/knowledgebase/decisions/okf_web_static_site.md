---
type: Decision
title: okf-site vs okf-web naming and dependency choice for the site generator
description: Decision record on naming the static-site crate okf-web vs okf-site, depending on okf-core instead of okf-studio, and rejecting WASM frameworks in favor of a build-time pipeline.
status: stable
generated:
  by: human:w4g1
  at: "2026-09-11T00:00:00Z"
stale_after: "2026-10-31T00:00:00Z"
tags:
  - okf-web
  - architecture
  - adr
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: reserved-crate
    resource: crates/reserved/okf-web/README.md
    title: Reserved okf-web crate placeholder
  - id: tokio-blog
    resource: https://tokio.rs/blog/2026-07-22-announcing-topcoat
    title: Announcing Topcoat
  - id: leptos-ssg
    resource: https://github.com/leptos-rs/leptos/pull/1649
    title: Leptos static site generation PR
---

# Decision: okf-web vs okf-site, and what it builds on

Context: extend the okf toolkit with a static site view of a bundle. Primary
driver: view rendered content — mermaid diagrams — not terminal text. See
[the implementation plan](../plans/okf_web_plan.md) for how this executes.

## Naming: okf-web

`crates/reserved/okf-web/`[^reserved-crate] already reserves the crate name
with the stated purpose "future web-based bundle viewing for the Open
Knowledge Format" — the author's own slot for exactly this feature. Taking
that slot makes the work a natural upstream contribution. `okf-site` is the
fallback if the fork must stay independent of upstream's release cadence.
`okf-visualize` was rejected: visualization is `okf graph`'s job; a site is
a renderer with navigation and health views, which is broader.

## Dependency base: okf-core + okf-validator, not okf-studio

`okf_studio::snapshot::Snapshot` is public, pure data, and tempting. Rejected:

- Drags `ratatui` + `crossterm` into a website generator, against the
  workspace's documented minimal-dependency ethos.
- Shaped for the TUI: `SearchIndex` (command-palette fuzzy matcher),
  pane-oriented `FileTree`, mission-control attention ordering. A site wants
  a nav tree of `<a>` elements, not those.
- The studio's markdown renderer is styled-terminal-lines with a link focus
  map — a different problem from markdown→HTML; not reusable.
- If shared derivations (trust stats, per-concept meta) grow, the clean move
  is extracting them into `okf-core`, which also serves the reserved
  `okf-server` idea; site→studio coupling serves neither.

Everything the site needs is already public API in okf-core:
`Bundle::load`/`parse_errors` (permissive), `Frontmatter` typed accessors
(`trust_tier`, `status`, `generated`, `verified`, staleness),
`Bundle::backlinks`, `footnotes` + `provenance` (OKF footnotes key to
`sources[].id`, not in-body defs — the part generic generators get wrong),
`markdown::{extract_headings, heading_slug, rewrite_markdown_links}` (the
link-rewriting hook, fence/inline-code aware), `AttestedComputation`.
Health badges: `okf_validator::{validate_bundle_at, lint_bundle_at}` — the
same calls the studio snapshot makes.

## Why no WASM framework (Yew / Leptos / Dioxus / Topcoat)

The common thread: every Rust→WASM web framework exists to solve client-side
state. Bundle pages are markdown + frontmatter — pure build-time data. The
site is closer to Zola (a pure-Rust SSG with no WASM anywhere) than to an app.

- **Yew**: CSR-only; no static-export story; ships WASM and renders into an
  empty `<div>` — bad SEO, no content without JS.
- **Leptos**: real SSG (`build_static_routes`)[^leptos-ssg] but prerenders
  routes of an *SSR app* — server machinery, hydration runtime,
  cargo-leptos/trunk toolchain to emit HTML with no event handlers. Islands
  mode is the theoretically right fit but experimental.
- **Dioxus**: SSG by running the app and curling itself to snapshot pages —
  a runtime-prerender hack, plus `dx` in the build.
- **Topcoat**: a *server* framework from the Tokio/Axum ecosystem, own README
  says "Early-stage and experimental. Expect breaking changes."[^tokio-blog]
  Wrong shape for static hosting.

Chosen instead: `maud` (compile-time `html!` templates — the same component
ergonomics as Yew, zero runtime, auto-escaping) + `pulldown-cmark` (event
stream makes the mermaid intercept trivial; `ENABLE_TABLES`; lighter than
comrak). Build deps stay pure Rust; mermaid.js is vendored as a static asset,
not a dependency. Prototype verified: maud escapes frontmatter, pulldown
escapes code, tables render, mermaid blocks intercept cleanly.

## Non-goals

- No search in v1 (post-build pagefind later, not porting the fuzzy
  SearchIndex).
- No refactor verbs or fix engine on the site — a static site is read-only
  by construction; those stay in the TUI.
- No interactive graph in v1; revisit with Leptos islands only if pan/zoom/
  egocentric exploration is actually requested.

[^okf-repo]: okf workspace repository
[^reserved-crate]: Reserved okf-web crate placeholder
[^leptos-ssg]: Leptos static site generation PR
[^tokio-blog]: Announcing Topcoat
