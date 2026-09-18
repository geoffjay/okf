---
type: Decision
title: Per-bundle tool configuration lives in .okf/config.yaml, read only by its consumer
description: Decision record on giving bundles a tool-configuration file - a dot-directory invisible to every OKF walker, YAML parsed by okf-core's std-only subset parser rather than a new TOML dependency, a `site:` mapping rather than an array of tables, strict keys against permissive content, and CLI-over-config-over-default precedence.
status: stable
generated:
  by: human:geoff
  at: "2026-09-15T00:00:00Z"
stale_after: "2026-12-31T00:00:00Z"
tags:
  - okf-web
  - configuration
  - typography
  - adr
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: fonts-plan
    resource: docs/knowledgebase/plans/okf_site_fonts_plan.md
    title: Per-bundle site configuration and font support plan
  - id: config-loader
    resource: crates/okf-web/src/config.rs
    title: The okf-web configuration loader and CSS emitter
  - id: web-adr
    resource: docs/knowledgebase/decisions/okf_web_static_site.md
    title: okf-web naming and dependency decision record
  - id: search-adr
    resource: docs/knowledgebase/decisions/okf_search_engine.md
    title: Search engine decision record, which records the dependency ethos
---

# Decision: `.okf/config.yaml` as the per-bundle tool-configuration file

Context: nothing about a site build was configurable from inside a bundle.
`okf site --title` was a flag that persisted nowhere, and typography was
fixed in the committed stylesheet, so a project could not say "this bundle
renders in a serif face." Fonts needed a home first; see
[the plan](../plans/okf_site_fonts_plan.md) for the execution and
`crates/okf-web/src/config.rs` for the loader[^config-loader].

## `.okf/` is the home, because it is invisible to content

A dot-directory is skipped by nothing and *collected* by nothing: the
bundle loader gathers `*.md` files only. Configuration and font binaries in
`.okf/` therefore cannot become concepts, cannot appear in
`okf validate`/`okf lint`/`okf index` output, and cannot enter the link
graph. The file is tool configuration — the same category as `.git/` — and
the alternative that was rejected outright is frontmatter on `index.md`:
that smuggles tooling keys into a spec-reserved document and invites
conformance thinking about keys the specification does not define.

## YAML, not TOML

The candidate was `.okf/config.toml` with the `toml` crate. Rejected: it
would add this workspace's first foreign-format parser for a file
`okf_core::yaml::{Value, Mapping}` — public, std-only — already parses. The
dependency ethos is on the record[^search-adr]; the search work's single
addition (`serde_json`) was called out as a cost. Every authored config
artifact in this repository is already YAML: frontmatter, generated blocks,
this file's own header. One format everywhere beats ecosystem fashion. If
TOML is ever insisted on, the schema transfers unchanged and only the
parser call moves.

## `site:` is a mapping, not an array of tables

One bundle has one site. `[[site]]` (or a YAML sequence) invites
merge-order and multi-config questions that have no consumer. Future tools
get sibling sections — a `studio:` section, say — and an unknown top-level
section is *ignored with a build note* rather than rejected, so a config
written for a newer `okf` degrades instead of breaking.

## Permissive content, strict tooling

Bundles stay permissive: a frontmatter parse error or a broken link renders
as visible content, and that never changes. Configuration is held to the
opposite standard, because a typo that silently does nothing is hostile in
CI.

| Situation | Behaviour |
|-----------|-----------|
| No `.okf/config.yaml` | Default config; output byte-identical to a pre-config build |
| Unreadable or malformed file | Build error carrying the YAML line |
| Unknown key in a known section | Build error naming the key and the section's known keys |
| Unknown top-level section | Ignored, reported as a build note |
| Bad value (weight, style, family, file name) | Build error naming the configuration path |

`SiteError` gained a `Config` variant, distinct from `Io`, because
`generate()`'s contract documents which failures are which.

## Precedence: CLI over config over default

`--title` overrides `site.title`, which overrides `DEFAULT_SITE_TITLE`. The
merge happens inside `generate()`, where the bundle root is known — the same
shape as `Bundle::load` reading `index.md` without a caller-provided path. A
library caller who wants no file involvement keeps that control by pointing
`SiteOptions::root` at a directory with no `.okf/`. The old
`effective_title` helper was removed rather than left to claim an
effectiveness it no longer had.

## Fonts ride CSS custom properties, not a stylesheet overlay

Typography moved onto two tokens in the hand-authored Tailwind source,
`--font-body` and `--font-code`, defaulting to the values the sheet already
computed. Any font setting is then a token override in a generated
`assets/fonts.css`, linked *after* the inline `<style>`, so `site.css` stays
universal and no bundle triggers a per-project Tailwind recompile. The
stylesheet and its `assets/fonts/` copies are written only when a bundle
configures fonts, which keeps default output untouched by construction.

Rejected alternatives:

- **An arbitrary `.okf/site.css` overlay** — unbounded blast radius over the
  `md-*`/theme/shiki design contracts. Token overrides are bounded and
  reviewable; revisit only if a real demand appears.
- **Configurable CDN font URLs (Google Fonts etc.)** — one external fetch
  per page view against a documented zero-fetch, `file://`-openable deploy
  property. Self-hosted files only.
- **`--font` CLI flags only** — not per-project, not persisted. The CLI is
  the override layer, not the storage layer.

## CSS is constructed, never interpolated

No configured byte reaches the stylesheet verbatim, so no configuration can
inject CSS:

- family lists are split on commas, trimmed, and re-emitted — generic
  keywords from a closed list bare, family names double-quoted after a
  charset check (letters, digits, spaces, `-`, `_`, `.`, `+`);
- `weight` is a number (1–1000) and `style` a closed vocabulary;
- `url()` targets are generator-built paths of files the generator itself
  copied, named by a flat ASCII charset with no separators, no `..`, and no
  leading dot — so path traversal is a parse error, not a runtime check;
- the `@font-face` `format()` hint is derived from the extension, not from
  configuration.

Diagram labels follow the prose by reading the computed `--font-body` at
`mermaid.initialize` time, so the boot script stays static JS with no
Rust-side parameterization.

## Fork containment

okf-web exists upstream only as a reserved placeholder[^web-adr], so the
whole site generator is fork-owned territory. This decision touches no
shared crate: the loader and the CSS emitter live in
`crates/okf-web/src/config.rs`[^config-loader], and nothing in okf-core,
okf-validator, or the shared CLI knows the file exists. If upstream ever
lands its own configuration convention, adaptation is a rename at one
file's boundary rather than a model change.

[^okf-repo]: okf workspace repository
[^fonts-plan]: Per-bundle site configuration and font support plan
[^config-loader]: The okf-web configuration loader and CSS emitter
[^web-adr]: okf-web naming and dependency decision record
[^search-adr]: Search engine decision record, which records the dependency ethos
