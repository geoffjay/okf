---
type: Decision
title: Search lives in okf-core and the site ships a first-party client, not pagefind
description: Decision record on making search a workspace capability - extracting the studio engine into a std-only okf-core module rather than keeping it in a consumer, and building a small first-party vanilla-JS site client against a build-time JSON index rather than adopting pagefind or porting the whole SearchIndex to WASM.
status: stable
generated:
  by: human:geoff
  at: "2026-09-14T18:09:26Z"
stale_after: "2026-12-31T00:00:00Z"
tags:
  - search
  - okf-core
  - okf-web
  - adr
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: studio-search
    resource: crates/okf-studio/src/search.rs
    title: The studio's fuzzy matcher, omnisearch index, and shared query syntax
  - id: web-adr
    resource: docs/knowledgebase/decisions/okf_web_static_site.md
    title: okf-web naming and dependency decision record
  - id: pagefind
    resource: https://pagefind.app/
    title: pagefind static site search
---

# Decision: search in okf-core, first-party site client

Context: the workspace has three views of one bundle — `okf` CLI,
`okf studio`, `okf site` — and search exists in one of them, inside the
TUI. The CLI has no search verb; the site renders a dead search input in
every page header (deferred in [the web plan](../plans/okf_web_plan.md)).
See [the search plan](../plans/okf_search_plan.md) for how this executes.
This record supersedes one line of the earlier web decision
record[^web-adr]: "No search in v1 (post-build pagefind later, not porting
the fuzzy SearchIndex)."

## Search is core capability, not a view feature

The workspace's first rule is "okf-core models the bundle; every other
crate consumes it" — new capability lands in okf-core, never duplicated in
a consumer. The studio's engine (Smith-Waterman-style fuzzy matcher with
smart-case and boundary bonuses, plus the composable filter syntax:
`#tag`, `type:`, `tier:`, `status:`, `is:stale`, `is:broken`) is pure
data over the permissive `Bundle`: it needs no ratatui, no crossterm, no
validator — nothing outside std. Keeping it in okf-studio while the CLI
and site grow their own would create the exact duplication the rule
exists to prevent; three consumers is the trigger the web ADR itself
named: "if shared derivations grow, the clean move is extracting them
into okf-core."

Chosen: move `okf-studio/src/search.rs` to `okf-core/src/search.rs`
verbatim, add `SearchIndex::build(&Bundle, Option<Date>)` and
`search_bodies` (grep-style body hits) so any consumer builds the index
without studio machinery, keep okf-core std-only. The studio re-exports
`okf_core::search` — its palette, graph filter, and refactor completion
keep working through the same module path.

Rejected: a shared helper crate (`okf-search`) — one more crate name,
version, and feature matrix for what is two source files of std-only
code; and leaving the engine in studio with the CLI depending on
`okf-studio` — which drags ratatui into every CLI install.

## Site: first-party client over a build-time index, not pagefind

The web ADR deferred search to "post-build pagefind later." Pagefind
solves a real problem — indexing arbitrary static HTML without any
knowledge of content structure — but that is not this problem. The site
generator *is* the build step; it already holds the parsed `Bundle` with
typed frontmatter (trust tiers, staleness, tags, types, headings), and
adopting an external Rust binary would mean: a post-build step in every
deploy pipeline (against the "Rust binary in, directory of HTML out"
contract), a second index format whose fragments and metadata model
cannot express `tier:unverified AND is:stale` — the queries this corpus
actually answers — and the workspace's first non-Rust build dependency
sitting in the critical path.

Chosen: `okf site` serializes the same `SearchIndex::build` result (plus
bodies) to `assets/search-index.js` at build time — deterministically,
`--today`-pinned — and a ~150-line first-party vanilla-JS client
(`search.js`, `include_str!`-vendored like mermaid/shiki but always
shipped, lazily fetched) ports `fuzzy_match` and `Query::parse` to make
the existing header input work. The port is small, deliberate, and
testable against shared fixtures; the asset is valid standalone JS
(`<`/`&`/U+2028 escaped), inert, and fetched only on first keystroke.

Rejected alternatives:

- **Port the whole engine to WASM.** The web ADR already rejected WASM
  frameworks for pages with no interactivity; the same logic holds for a
  search box. A ~150-line scorer port is reviewable; a wasm-bindgen
  toolchain for one function is not, and it reintroduces what the
  decision record calls "client-side state machinery" for a filterable
  list.
- **A JS fuzzy library (fzf.js, Fuse.js).** Vendoring a scorer this
  simple is cheaper than vendoring a library whose ranking semantics
  then diverge from the studio's; one engine means one notion of a hit,
  so studio and site agree on what `pte` matches.
- **Server-side search.** Out of scope by the site's founding contract:
  no server, deployable to any static host.

## What this buys

- `okf search` becomes an agent-scriptable verb (`--json`) with the same
  query syntax the studio palette already teaches.
- Studio and site results agree by construction — same index, same
  scorer, same filters.
- No dependency changes except `serde_json` in okf-web (for index
  serialization; hand-rolled JSON escaping is a bug farm).

[^okf-repo]: okf workspace repository
[^studio-search]: The studio's fuzzy matcher, omnisearch index, and shared query syntax
[^web-adr]: okf-web naming and dependency decision record
[^pagefind]: pagefind static site search
