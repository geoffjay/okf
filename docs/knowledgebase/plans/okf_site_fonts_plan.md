---
type: Plan
title: Per-bundle site configuration and font support for okf site
description: "Plan for a per-bundle configuration mechanism (.okf/config.yaml with a site section, parsed by okf-core's std-only YAML) plus font support in okf site: typography refactored onto CSS custom-property tokens, per-bundle font families and self-hosted font files emitted as an opt-in generated fonts.css layered over the committed stylesheet - no new dependencies, no upstream-facing surface outside okf-web."
status: draft
generated:
  by: omp/main
  at: "2026-09-14T19:30:00Z"
stale_after: "2026-12-31T00:00:00Z"
tags:
  - okf-web
  - configuration
  - typography
  - planning
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: tailwind-source
    resource: crates/okf-web/assets/tailwind.css
    title: The site's hand-authored Tailwind source, where typography currently lives
  - id: web-plan
    resource: docs/knowledgebase/plans/okf_web_plan.md
    title: okf-web static site generator plan
  - id: web-adr
    resource: docs/knowledgebase/decisions/okf_web_static_site.md
    title: okf-web naming and dependency decision record
  - id: search-adr
    resource: docs/knowledgebase/decisions/okf_search_engine.md
    title: Search engine decision record, which records the dependency ethos
  - id: mdn-font-face
    resource: https://developer.mozilla.org/en-US/docs/Web/CSS/@font-face
    title: MDN @font-face reference
---

# Per-bundle site configuration and fonts for `okf site`

## Motivation

The site's typography is fixed at generator build time. `tailwind.css`
sets `font: 16px/1.6 system-ui, sans-serif` on `body`[^tailwind-source],
and `code`/`pre` carry no `font-family` at all — code blocks ride the
browser's default monospace. `site.css` is compiled once, committed, and
`include_str!`'d, so a bundle has no way to say "this project renders in
a serif face" or "this project ships its own brand font."

The missing capability is really two things:

1. **A per-bundle override mechanism.** Nothing about a site build is
   configurable from inside the bundle today — `--title` is a CLI flag,
   nothing persists. A project should be able to pin its site settings
   the same way it pins its content, and different bundles should get
   different answers.
2. **Fonts as the first resident setting.** Font families and
   self-hosted font files, resolved at build time.

The second needs the first, so this plan builds the mechanism and moves
fonts in as its first (and title as its second, trivial) residents.

## Scope

In scope:

- `.okf/config.yaml`: a per-bundle tool-configuration file, invisible to
  every existing walker, parsed with `okf_core::yaml` — zero new
  dependencies.
- A `site:` section for okf-web: `title` and a `fonts` group (body and
  code family lists, plus self-hosted font files under `.okf/fonts/`).
- Typography refactored onto CSS custom properties so any setting is a
  token override layered over the committed stylesheet — no per-project
  Tailwind recompile, `site.css` stays universal.
- A generated `assets/fonts.css`, written and linked only when a bundle
  configures fonts, so default bundles stay byte-identical.

Out of scope (v1):

- Any okf-core, okf-validator, or shared-CLI surface: the config is
  consumed only by okf-web.
- External font CDNs (Google Fonts etc.): the site must stay deployable
  offline and over `file://`, so fonts are self-hosted files only.
- Arbitrary user-CSS passthrough (a `.okf/site.css` overlay): it would
  let bundles break the `md-*`/theme/shiki design contracts; token
  overrides cover the actual need. Revisit only if a real demand appears.
- Type-scale/line-height knobs: the token set starts at two family
  tokens; more tokens are additive config keys later.
- Per-*directory* (sub-bundle) scoping: one bundle, one config.

## Decisions

### `.okf/config.yaml`, YAML, mapping not array

The candidate was `.okf/config.toml` with a `[[site]]` section. Three
adjustments, each for a recorded reason:

- **`.okf/` as the home** — kept from the original idea. A dot-directory
  is invisible to every walker in the workspace: `Bundle::load` collects
  only `*.md` files, and the remediation/index walkers skip dot-directories
  outright, so config and font files can never become concepts, never
  appear in `okf validate`/`lint`/`index` output, and never pollute the
  link graph. The file is *tool config*, not content — the same category
  as `.git/`.
- **YAML, not TOML** — TOML would add this workspace's first
  foreign-format parser to okf-web's six dependencies, while
  `okf_core::yaml::{Value, Mapping}` is already public, std-only API
  (the search work's only dependency addition was `serde_json`, and even
  that was called out in the log[^search-adr]). Every authored config
  artifact in this repo is already YAML — frontmatter, generated blocks.
  One format everywhere beats ecosystem fashion here. If TOML is ever
  insisted on, the schema below transfers unchanged; only the parser swap
  moves.
- **`site:` as a mapping, not `[[site]]`** — one bundle has one site; an
  array-of-tables invites merge-order and multi-config questions nobody
  has. Future tools (a `studio:` section, say) get sibling sections;
  unknown top-level sections are ignored with a build note, so a config
  written for a newer okf degrades rather than breaks.

### Fork deviation is structurally contained

The upstream concern is real but already answered by shape: okf-web
exists upstream only as a reserved placeholder[^web-adr], so *all* of the
site generator is fork-owned territory. This plan touches no shared
crate — the loader lives in `crates/okf-web/src/config.rs`, the emission
in `render.rs`/`lib.rs`. If upstream ever lands its own config
convention, adaptation is a rename at this one file's boundary, not a
model change.

### Permissive content, strict tooling

Bundles stay permissive — parse errors and broken links render as
content, and that never changes. But a config typo that silently does
nothing is hostile in CI, so the config is strict:

- Unreadable or malformed file → build error citing `YamlError`'s line.
- Unknown key inside `site:` → build error naming the key and the
  section's known keys.
- Missing file → the default config; output is **byte-identical** to a
  pre-config world (this becomes a regression test).

`SiteError` gains a `Config` kind — distinct from `Io`, since the
generate() contract documents which failures are which.

### Precedence: CLI over config over default

`--title` (and later flags) override config, config overrides the
built-in default. The merge happens inside `generate()`, where the
bundle root is known — the same shape as `Bundle::load` reading
`index.md`'s `okf_version` without a caller-provided path. A library
caller who wants no file involvement keeps that control by pointing
`SiteOptions::root` at a directory without a `.okf/`.

### Fonts: tokens now, override layer later

The enabler is a small refactor: `tailwind.css` learns two custom
properties, `--font-body` and `--font-code`, defaulting to today's
values. The `body` `font:` shorthand splits so the family component can
be `var(--font-body)`, and `code`/`pre` finally get an explicit mono
stack via `var(--font-code)` (today they ride the UA default — a latent
gap this fixes). `site.css` is recompiled once and stays universal.

Every font setting then becomes a generated override **after** the inline
stylesheet:

```text
.okf/config.yaml  ──parse──▶  SiteConfig
.okf/fonts/*      ──copy───▶  assets/fonts/*
SiteConfig        ──emit──▶  assets/fonts.css   (@font-face + :root token override)
page head:  <style>site.css</style> then <link href="{prefix}assets/fonts.css">
```

- **`fonts.css` is written and `<link>`ed only when fonts are
  configured** — opt-in by construction, so the committed stylesheet and
  default output are untouched. The `<link>` sits after the inline
  `<style>`, so it wins cascade ties; the per-page `{prefix}` chain the
  mermaid/shiki assets already use makes the href depth-correct.
- **CSS is constructed, never interpolated.** Family lists are parsed as
  comma-separated segments, trimmed, and re-emitted as quoted strings
  (generic keywords like `serif` unquoted); `url()` targets are always
  generator-generated asset paths of files the generator itself copied;
  `weight`/`style` come from closed vocabularies. No config string
  reaches the CSS verbatim, so no config can inject CSS.
- **Self-hosted only.** `site.fonts.files` names files under
  `.okf/fonts/` (missing file → build error). The generator copies them
  to `assets/fonts/` and emits `@font-face` with the format derived from
  the extension (`woff2`/`woff`/`ttf`/`otf`)[^mdn-font-face]. File names
  are restricted to a flat, conservative charset — no separators, no
  `..`, no leading dot — making path traversal a parse error rather than
  a runtime check.
- **Diagrams follow the prose.** The mermaid boot's
  `mermaid.initialize` gains `fontFamily` read from the computed
  `--font-body` token at init time — the boot script is static JS, so
  this needs no Rust-side parameterization, and diagram labels match the
  body font without a re-render.

### Schema

```yaml
# .okf/config.yaml - per-bundle tool configuration, read at `okf site`.
site:
  # Header title. `okf site --title` still overrides this.
  title: "Travel knowledge"

  fonts:
    # CSS font-family lists for the two typography tokens.
    body: "Newsreader, Georgia, serif"
    code: "JetBrains Mono, ui-monospace, monospace"

    # Self-hosted faces: .okf/fonts/<file> -> assets/fonts/<file>.
    files:
      - family: Newsreader
        file: newsreader-400.woff2
        weight: 400
      - family: Newsreader
        file: newsreader-700i.woff2
        weight: 700
        style: italic
```

## Rejected alternatives

- **`.okf/config.toml` + the `toml` crate** — first foreign-format
  dependency for a file `okf_core::yaml` already parses; see above.
- **`[[site]]` array-of-tables** — multi-config ambiguity with no
  consumer; YAGNI.
- **Configuring CDN font URLs** — one external fetch per page view
  against a documented zero-fetch, `file://`-openable deploy property.
- **Arbitrary CSS overlay file** — unbounded blast radius over the
  design system; token overrides are bounded and reviewable.
- **`--font` CLI flags only** — not per-project, not persisted; the CLI
  stays the override layer, not the storage layer.
- **Frontmatter on `index.md`** — smuggles tooling keys into a
  spec-reserved document and trips `KNOWN_FRONTMATTER_KEYS`-style
  conformance thinking; tool config does not belong in content.

## Wiring

1. `crates/okf-web/src/config.rs` (new): `SiteConfig::load(&root)` →
   `Option<SiteSection>` via `okf_core::yaml`; strict-key validation;
   the flat-name charset check; a `Display`-able error carrying the
   YAML line. Unit tests over tempdir configs.
2. `crates/okf-web/src/lib.rs`: `generate()` discovers and merges config
   (title precedence; fonts collected); `SiteError::Config`; write
   `assets/fonts.css` + copy `assets/fonts/` when configured; extend
   `SiteSummary` with nothing (font presence is visible in the output
   tree, not a count worth reporting).
3. `crates/okf-web/src/render.rs`: emit the `fonts.css` `<link>` with the
   page's `{prefix}` when the bundle configures fonts; extend the
   mermaid boot's `initialize` with the token-read `fontFamily`.
4. `assets/tailwind.css`: the two tokens and the `body`/`code`/`pre`
   refactor; `cargo xtask tailwind` recompile; no visual change.
5. No CLI changes beyond what falls out of the title precedence
   (`--title` keeps its current shape and help text gains a "overrides
   config" clause).

## Phasing

1. **Typography tokens.** Refactor + recompile; explicit mono on
   `code`/`pre`. Acceptance: existing suite green; a default bundle's
   rendered pages unchanged except the new `font-family` declarations;
   browser spot-check light/dark.
2. **Config loader + title.** `.okf/config.yaml` parsing, strictness,
   `SiteError::Config`, title precedence `--title > site.title >
   DEFAULT_SITE_TITLE`. Acceptance: parse/strict-key/charset unit tests;
   integration test — config title renders, `--title` wins, no-config
   bundle byte-identical to before.
3. **Font families.** `fonts.body`/`fonts.code` → `fonts.css` with the
   `:root` override, linked on every page with a depth-correct prefix;
   mermaid `fontFamily` follows the token. Acceptance: integration test
   asserts the override and the link; no-config bundle still
   byte-identical; browser shows the chosen family.
4. **Font files.** `fonts.files` → copy + `@font-face`; traversal-shaped
   names and missing files are build errors. Acceptance: files copied,
   `@font-face` correct, `document.fonts` confirms the face loads in a
   browser, site still opens over `file://`.
5. **Docs + records.** README `okf site` section (config file, fonts),
   okf-web README, changelog; promote the config mechanism from this
   plan's Decisions section to a `decisions/` ADR; update this bundle's
   own KB (`okf validate` + `okf lint` must pass after every edit).

## Risks

- **CSS injection via config** — mitigated by constructed emission:
  quoted segments, closed vocabularies, generator-owned `url()`s.
- **Path traversal via `file:` entries** — mitigated at parse time:
  flat names, no separators, conservative charset.
- **Upstream divergence** — contained: fork-local crate only; one file
  to rename if upstream lands a convention.
- **Stale `fonts.css` after removing config** — same class as a stale
  `mermaid.min.js` today (generate never cleans `out_dir`); the link
  disappears from pages, which is the visible contract.
- **Determinism** — preserved: emission is a pure function of config and
  font bytes; no timestamps, no clock.

## Verification

- `cargo test -p okf-web`, then the workspace suite; `okf validate .` and
  `okf lint .` on this knowledge base after the doc edits.
- The byte-identical default: generate a fixture bundle with no config
  before and after every milestone — outputs must match exactly.
- End-to-end browser check (the web plan's acceptance bar[^web-plan]):
  build this KB with a serif body + one self-hosted face; confirm
  computed font, the face in `document.fonts`, diagrams following the
  token, no console errors, and `file://` still opening.

[^okf-repo]: okf workspace repository
[^tailwind-source]: The site's hand-authored Tailwind source, where typography currently lives
[^web-plan]: okf-web static site generator plan
[^web-adr]: okf-web naming and dependency decision record
[^search-adr]: Search engine decision record, which records the dependency ethos
[^mdn-font-face]: MDN @font-face reference