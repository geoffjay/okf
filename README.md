<div align="center">

# okf

A **pure-Rust** implementation, library and CLI toolkit for the [Open Knowledge Format (OKF) v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md): Google's open, human- and agent-friendly format for representing knowledge as a directory of Markdown files with YAML frontmatter.

[![crates.io](https://img.shields.io/crates/v/okf.svg?label=okf)](https://crates.io/crates/okf)
[![crates.io](https://img.shields.io/crates/v/okf-core.svg?label=okf-core)](https://crates.io/crates/okf-core)
[![docs.rs](https://img.shields.io/docsrs/okf)](https://docs.rs/okf)
[![CI](https://github.com/W4G1/okf/actions/workflows/rust.yml/badge.svg)](https://github.com/W4G1/okf/actions/workflows/rust.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](#license)
[![Deps.rs Crate Dependencies (latest)](https://img.shields.io/deps-rs/okf/latest)](https://crates.io/crates/okf/dependencies)

<br/>
<br/>

<img src="assets/okf_studio.png" alt="okf studio preview" width="100%"/>

</div>

---

## Table of Contents

<details>
<summary>Click to expand</summary>

- [What is OKF?](#what-is-okf)
- [Hands-on quickstart (60 seconds)](#hands-on-quickstart-60-seconds)
  - [1. Install the CLI](#1-install-the-cli)
  - [2. Initialize a new bundle](#2-initialize-a-new-bundle)
  - [3. Create concepts](#3-create-concepts)
  - [4. Check conformance and auto-fix issues](#4-check-conformance-and-auto-fix-issues)
  - [5. Inspect trust and visualize the graph](#5-inspect-trust-and-visualize-the-graph)
  - [6. Explore with OKF Studio](#6-explore-with-okf-studio)
- [Interactive studio: `okf studio`](#interactive-studio-okf-studio)
- [Static site: `okf site`](#static-site-okf-site)
- [Anatomy of an OKF bundle](#anatomy-of-an-okf-bundle)
  - [Directory structure](#directory-structure)
  - [Concept document example](#concept-document-example)
  - [Attested computation example](#attested-computation-example)
- [Core concepts](#core-concepts)
  - [Trust tiers](#trust-tiers)
  - [Freshness and staleness](#freshness-and-staleness)
  - [Provenance and footnote attribution](#provenance-and-footnote-attribution)
  - [Cross-links, heading links, and anchors](#cross-links-heading-links-and-anchors)
  - [Attested computations](#attested-computations)
- [CLI reference and workflows](#cli-reference-and-workflows)
  - [Interactive studio: studio](#interactive-studio-studio)
  - [Scaffolding: init and new](#scaffolding-init-and-new)
  - [Quality gate: validate and lint](#quality-gate-validate-and-lint)
  - [Auditing trust: trust and info](#auditing-trust-trust-and-info)
  - [Searching: search](#searching-search)
  - [Link graph and discovery: links and graph](#link-graph-and-discovery-links-and-graph)
  - [Listing computations: computations](#listing-computations-computations)
  - [Semantic diffs: diff](#semantic-diffs-diff)
  - [Formatting and indexing: fmt, index, and parse](#formatting-and-indexing-fmt-index-and-parse)
  - [Universal JSON output: --json / -j](#universal-json-output---json---j)
- [CI/CD integration](#cicd-integration)
- [Using as a Rust library](#using-as-a-rust-library)
  - [1. Loading and validating a bundle](#1-loading-and-validating-a-bundle)
  - [2. Inspecting attested computations](#2-inspecting-attested-computations)
  - [3. Multi-language syntax checking](#3-multi-language-syntax-checking)
- [Workspace crates](#workspace-crates)
- [Design choices](#design-choices)
- [Mapping to the spec](#mapping-to-the-spec)
- [License](#license)

</details>

---

## What is OKF?

The **Open Knowledge Format (OKF)** is a specification from Google for representing written knowledge as a directory of Markdown files with structured YAML frontmatter.

An OKF bundle is plain text in a folder. There is no database, no external service, and no schema registry. If you can read a text file, you can read OKF.

- **Concepts**: Individual Markdown documents (`.md`), each containing one piece of knowledge with YAML frontmatter.
- **Bundles**: A directory tree of related concepts. An `index.md` file acts as the directory listing, and `log.md` records revision history.
- **Trust & verification**: Frontmatter tracks who authored a concept (`generated: { by, at }`) and who or what verified it (`verified: [{ by, at }]`). Trust tiers (`unverified`, `machine-confirmed`, `human-reviewed`) are derived dynamically from these events.
- **Freshness & lifecycle**: Concepts define status (`draft`, `stable`, `deprecated`) and explicit expiration dates (`stale_after`).
- **Provenance**: Sources list where knowledge originated, who wrote it, and when it changed, with footnote citations linking claims directly to source IDs.
- **Attested computations**: Executable contracts specifying parameters, runtimes (SQL, Python, dbt), execution receipts, and deterministic verification attesters.

The `okf` crate is a pure-Rust implementation, validator, linter, and CLI toolkit for working with OKF v0.2 bundles.

---

## Hands-on quickstart (60 seconds)

### 1. Install the CLI

Install `okf` from crates.io:

```sh
cargo install okf
```

*(Or install as a Cargo plugin via `cargo install cargo-okf`, which lets you run `cargo okf <command>`)*

### 2. Initialize a new bundle

Create an OKF bundle with a root `index.md`, audit `log.md`, and an initial concept:

```sh
okf init company_knowledge --title "Company policies and operations"
cd company_knowledge
```

### 3. Create concepts

Scaffold new concepts with standardized frontmatter:

```sh
# Create a policy concept
okf new policies/travel_expenses --type Policy --title "Travel and expense policy" --description "Rules and reimbursement rates for business travel."

# Create an Attested Computation contract
okf new computations/mileage_calc --attested --title "Mileage reimbursement calculator"
```

### 4. Check conformance and auto-fix issues

Run `validate` and `lint` to audit your bundle:

```sh
# Validate strict OKF v0.2 conformance
okf validate .

# Run opinionated hygiene checks and automatically remediate fixable issues
okf lint . --fix
```

### 5. Inspect trust and visualize the graph

```sh
# View trust tiers and staleness status
okf trust .

# Generate a Mermaid graph of concept relationships (renders directly in GitHub Markdown)
okf graph . --format mermaid
```

### 6. Explore with OKF Studio

```sh
okf studio .
```

---

## Interactive studio: okf studio

`okf studio` is an interactive terminal interface for exploring and managing OKF bundles.

Unlike one-off commands that run once and exit, the studio stays open and automatically updates when files are edited on disk.

- **Omnisearch (`/`)**: Fuzzy search across ids, titles, tags, headings, and
  body text with the shared filter syntax (`#tag`, `tier:`, `is:stale`,
  `is:broken`) — the same engine as `okf search` and the site.
- **Browse and read**: Navigate directories, read rendered Markdown, follow links, and inspect metadata and revision history.
- **Audit bundle health**: View trust levels, freshness, and concepts that need attention.
- **Computations**: Inspect Attested Computation contracts and test parameter inputs.
- **Refactor safely**: Move, rename, merge, split, or delete concepts with preview confirmation before changes are saved.

```sh
okf studio [bundle]
```

---

## Static site: okf site

`okf site` generates a deployable static website from a bundle — the same
engine as `okf studio`, but rendered to a directory of HTML instead of a live
terminal. Where `okf graph --format mermaid` prints diagram source and the
studio shows fenced `mermaid` blocks as boxed code, the site renders those
diagrams for real, in the browser.

```sh
okf site [bundle]            # writes <bundle>/site by default
okf site . --out public      # choose the output directory
okf site . --today 2026-07-01  # pin staleness for reproducible builds
okf site . --title "Docs"    # header title; overrides .okf/config.yaml
okf site . --theme sepia     # theme a first visit gets; overrides .okf/config.yaml
```

- **Pure build-time generation.** A Rust binary in, a directory of HTML out —
  no server, no WASM, no JavaScript framework. Deploys to GitHub Pages or any
  static host; relative URLs mirror the bundle layout, so it works under any
  base path.
- **Tailwind-styled, zero external CSS.** Pages inline one stylesheet compiled
  from [Tailwind CSS v4](https://tailwindcss.com); the site loads no external
  stylesheet (the vendored mermaid.js and shiki.js are the only external
  assets, each only on pages that use them).
- **Rendered diagrams.** Fenced ` ```mermaid ` blocks render client-side via a
  vendored [mermaid.js](https://mermaid.js.org/) (shipped only when a page has
  a diagram). Diagram source is escaped into a `<pre>` fallback that survives
  render failures and no-JS.
- **Syntax-highlighted code.** Fenced blocks with a language tag highlight
  client-side via a vendored [shiki](https://shiki.style) — every shiki
  language, GitHub light/dark themes resolved as CSS variables (theme
  toggling is free), shipped only when a page has code, with the escaped
  source kept as the no-JS fallback.
- **Themes a reader can change.** The header's theme menu offers `auto`
  (follow the system), `light`, `dark`, `sepia`, and `contrast`, plus any
  palette the bundle defines. The choice persists in `localStorage` and is
  applied to `<html>` before first paint, so reloading never flashes: two
  attributes, `data-scheme` (`light`/`dark`, absent = follow the system) for
  the base palette and `data-theme` for its overrides. Every color in the
  stylesheet resolves through custom properties, so switching is instant and
  needs no second stylesheet, no recompile, and no page reload — code
  highlighting follows the scheme through shiki's CSS variables, and
  diagrams re-render on the palette's own tokens.
- **Parity panels.** Every concept page shows what the studio inspector does:
  trust/status/staleness badges, backlinks, sources with footnote→source
  attribution, a headings table of contents, and validator/lint findings.
- **Search that works anywhere.** Every page's header search runs the same
  engine as `okf search` and the studio palette: `assets/search-index.js`
  (build-time JSON index of metadata, heading anchors, and bodies, with
  HTML-breaking bytes JS-escaped) plus a first-party vanilla-JS client,
  lazy-fetched only on the first keystroke — no external search binary, no
  WASM. Filters (`#tag`, `tier:`, `is:stale`, …) compose exactly as in the
  CLI.
- **Dashboard and graph.** A trust dashboard (tier distribution, attention
  queue, actor stats — the numbers agree with `okf info` / `okf trust`) plus a
  bundle-wide mermaid cross-link graph.
- **Per-bundle settings in the bundle.** A bundle may pin its site settings in
  `.okf/config.yaml` — a dot-directory every OKF walker ignores, so
  configuration never becomes content and never shows up in `okf validate`,
  `okf lint`, or the link graph:

  ```yaml
  # .okf/config.yaml
  site:
    title: "Travel knowledge"      # `okf site --title` still overrides this
    theme: nord                    # first-visit theme; `--theme` overrides it
    themes:                        # extra palettes, offered in the header menu
      - id: nord
        label: "Nord"
        scheme: dark               # base palette, code colors, diagram mode
        colors:                    # closed token set; every key optional
          surface: "#2e3440"
          ink: "#eceff4"
          edge: "#4c566a"
          link: "#88c0d0"
    fonts:
      body: "Newsreader, Georgia, serif"
      code: "JetBrains Mono, ui-monospace, monospace"
      files:                       # self-hosted: .okf/fonts/<file>
        - family: Newsreader
          file: newsreader-400.woff2
          weight: 400
        - family: Newsreader
          file: newsreader-700i.woff2
          weight: 700
          style: italic
  ```

  Fonts resolve through two CSS custom properties (`--font-body`,
  `--font-code`) and palettes through nineteen more, so a configured bundle
  gets generated `assets/fonts.css` and `assets/theme.css` — `@font-face`
  rules for the copied files, `:root[data-theme="…"]` blocks for the
  palettes — linked after the inline stylesheet. A palette overrides only
  the tokens it names; its `scheme` supplies the rest, and taking a built-in
  id (`dark`) restyles that theme instead of adding a menu entry. Font files
  are copied to `assets/fonts/`, never fetched from a CDN, so the site still
  deploys offline and opens over `file://`; mermaid diagrams pick up the body
  family and the palette too. Bundles that configure nothing write neither
  generated file, and content stays permissive while the config is strict: an
  unknown key, a bad color, a missing font file, or a `theme:` naming a theme
  nothing defines fails the build (an unknown *section* is only reported, so a
  config written for a newer `okf` still builds). Each build prints the file
  it read and the settings it supplied — `read docs/.okf/config.yaml (title,
  fonts.body, theme, themes)` — so a config the build never saw (a `.okf/`
  above the bundle root, an older `okf`) shows up as a missing line instead of
  a mystery.

`okf site` is on by default; opt out with `--no-default-features` (see
[Using as a Rust library](#using-as-a-rust-library)). Built on the
[`okf-web`](https://crates.io/crates/okf-web) crate, whose `generate` entry
point can be called directly.

---

## Anatomy of an OKF bundle

### Directory structure

A typical OKF bundle repository looks like this:

```text
company_knowledge/
├── index.md                      # Root table of contents (declares okf_version: "0.2")
├── log.md                        # Audit log of changes grouped by ISO-8601 date
├── policies/
│   ├── index.md                  # Subdirectory index (auto-generated)
│   ├── travel_expenses.md        # Concept document (Policy)
│   └── paid_time_off.md          # Concept document (Policy)
├── computations/
│   ├── index.md                  # Subdirectory index (auto-generated)
│   └── mileage_calc.md           # Attested Computation concept
└── references/
    ├── skills/submit_expense.md  # Execution instructions
    └── attesters/verify_rate.py  # Deterministic verifier
```

### Concept document example

`policies/travel_expenses.md`:

```markdown
---
type: Policy
title: Travel and expense policy
description: Rules and standard per-mile reimbursement rates for employee travel.
tags: [hr, finance, travel, expenses]
status: stable
generated:
  by: reference_agent/gemini-3.7-flash
  at: 2026-06-20T22:53:05Z
verified:
  by: human:sarah_hr
  at: 2026-06-25T09:00:00Z
stale_after: 2026-12-31T00:00:00Z
sources:
  - id: mileage-guide
    resource: https://example.com/finance/mileage-guide
    title: Standard mileage reimbursement guidelines
---

# Travel and expense policy

Employees traveling on company business are reimbursed for personal vehicle usage at standard approved rates.[^mileage-guide]

Total reimbursement is calculated using the [Mileage reimbursement calculator](../computations/mileage_calc.md).

[^mileage-guide]: Standard mileage reimbursement guidelines
```

### Attested computation example

`computations/mileage_calc.md`:

````markdown
---
type: Attested Computation
title: Mileage reimbursement calculator
description: Sanctioned computation to calculate employee vehicle travel reimbursement.
status: stable
runtime: python
parameters:
  - { name: miles, type: number, required: true }
  - { name: rate_per_mile, type: number, required: false }
executor:
  resource: references/skills/submit_expense.md
  receipt: [report_id, calculated_amount, status]
attester:
  resource: references/attesters/verify_rate.py
generated:
  by: human:alex_finance
  at: 2026-06-15T10:00:00Z
verified:
  by: process:ci-nightly
  at: 2026-06-20T00:00:00Z
---

# Mileage reimbursement calculator

# Computation

```python
def calculate_reimbursement(miles: float, rate_per_mile: float = 0.67) -> float:
    return round(miles * rate_per_mile, 2)
```
````

---

## Core concepts

### Trust tiers

In a corpus where both humans and AI agents write documents, trust is critical. OKF derives trust tiers dynamically from verification events rather than storing a subjective score:

| Trust tier | Meaning | Verification condition |
|------------|---------|------------------------|
| **`human-reviewed`** | Highest confidence. Verified by a human. | At least one `verified.by` starts with `human:` (e.g., `human:alice`). |
| **`machine-confirmed`** | Moderate confidence. Checked by automated process or test suite. | Verified by a process (e.g., `process:nightly-ci` or `agent/v1`), with no human review. |
| **`unverified`** | Baseline draft or unreviewed agent output. | No `verified` entries present. |

### Freshness and staleness

Knowledge decays over time. The `stale_after: YYYY-MM-DD` field gives documents an explicit expiration date.

- `okf trust .` flags stale concepts in terminal output.
- `okf validate . --today 2026-07-01` allows pinning a date in CI for deterministic staleness checks.

### Provenance and footnote attribution

OKF documents record origin and credibility signals under `sources`:

```yaml
sources:
  - id: mileage-guide
    resource: https://example.com/finance/mileage-guide
    title: Standard Mileage Reimbursement Guidelines
    author: human:finance_team
    last_modified: 2026-04-01T00:00:00Z
    usage_count: 1200
```

Inline claims reference sources via standard Markdown footnotes keyed to `sources[].id` (e.g., `According to company guidelines...[^mileage-guide]`).

### Cross-links, heading links, and anchors

Concepts link to each other with standard Markdown links (spec §6.1), either bundle-absolute (`/tables/customers.md`, recommended) or relative (`./other.md`). A link may address a specific **section** of its target by appending the heading's anchor, which every renderer derives as a GitHub-style slug:

```markdown
See the [join keys](/tables/orders.md#join-keys) and [Enterprise tier](#enterprise-tier).
```

For convenience, `okf` also expands `[[target]]` **heading-link shorthand** while rendering, so producers that write wiki-style links get working links everywhere (`okf site` pages, the studio viewer, the link graph):

| Shorthand | Expands to | Points at |
|-----------|------------|-----------|
| `[[Pricing Tiers]]` | `[Pricing Tiers](#pricing-tiers)` | The `## Pricing Tiers` heading of the same document |
| `[[#Pricing Tiers]]` | `[Pricing Tiers](#pricing-tiers)` | Same, written as an explicit anchor |
| `[[plans/rollout]]` | `[plans/rollout](plans/rollout)` | Another concept |
| `[[plans/rollout#Phase 2]]` | `[Phase 2](plans/rollout#phase-2)` | A heading in another concept |

Fragments are slugified (the transform is idempotent, so already-slug targets are unchanged), and wikilinks inside fenced code blocks or inline code stay verbatim. `okf validate` reports anchors — shorthand or authored — that match no heading in the target document.

### Attested computations

An `Attested Computation` defines a contract for executing deterministic calculations:
1. **`runtime`**: Environment (e.g., `python`, `bigquery`, `dbt`, `snowflake`).
2. **`parameters`**: Typed arguments required for execution.
3. **`# Computation`**: The code or query (inline or referenced).
4. **`executor`**: Resource that executes the logic and returns a receipt.
5. **`attester`**: Deterministic script that verifies the receipt output.

> **Note**: `okf` parses and validates attestation contracts; executing computation and attestation is a consumer-side runtime responsibility.

---

## CLI reference and workflows

```text
okf <command> [options] [arguments]
```

### Interactive studio: studio

Open the interactive terminal UI:

```sh
# Open the current directory in studio
okf studio

# Open a specific bundle and start on the graph tab
okf studio ./company_knowledge --tab graph

# Check staleness against a pinned date and set an author identity
okf studio ./company_knowledge --today 2026-12-01 --author "human:sarah"
```

### Scaffolding: init and new

```sh
# Initialize a new bundle in the current directory
okf init . --title "Company policies"

# Initialize a bare bundle without sample concept
okf init ./company_knowledge --bare

# Create a new concept with title and description
okf new policies/travel_expenses --type Policy --title "Travel and expense policy" --description "Rules and reimbursement rates for business travel"

# Create an Attested Computation concept
okf new computations/mileage_calc --attested --title "Mileage reimbursement calculator"
```

### Bundle manipulation: mv, rm, split, and merge

Manipulating and refactoring a bundle without breaking cross-links is a first-class capability in `okf`:

```sh
# Move or rename a concept (rewrites all backlinks across the bundle + rebases outgoing links and anchors)
okf mv auth/token security/jwt --bundle ./company_knowledge

# Preview rename without writing to disk
okf mv auth/token security/jwt --dry-run --json

# Rename a section/heading in-place and rewrite all internal and bundle-wide anchor links
okf mv billing/pricing#pricing-tiers billing/pricing#subscription-plans

# Safely remove a concept (fails if incoming links point to it)
okf rm legacy/old_policy

# Re-route all backlinks pointing to the removed concept to a replacement
okf rm legacy/old_policy --redirect-to policies/new_policy

# Unlink backlinks into plain text
okf rm legacy/old_policy --unlink

# Extract a section/heading into a new concept document
okf split billing/pricing billing/enterprise --section "Enterprise Tier" --title "Enterprise Pricing"

# Consolidate two concepts into one (merges sources, footnotes, verified events, and backlinks)
okf merge billing/discounts billing/pricing
```

### Quality gate: validate and lint

`okf validate` verifies strict OKF v0.2 specification conformance, checking schema validity, broken cross-links, missing attestation resources, and broken section anchors (exits with non-zero code on errors):

```sh
# Conformance check
okf validate ./company_knowledge

# Check conformance against a specific evaluation date
okf validate ./company_knowledge --today 2026-12-01

# Automatically fix conformant issues (e.g., migrate legacy v0.1 fields)
okf validate ./company_knowledge --fix
```

`okf lint` evaluates 13 opinionated hygiene rules (missing headings, orphan concepts, key ordering, heading hierarchy, whitespace issues):

```sh
# Lint bundle
okf lint ./company_knowledge

# Automatically apply fixes across all files (adds titles, headings, formats keys, fixes whitespace)
okf lint ./company_knowledge --fix
```

### Auditing trust: trust and info

```sh
# View per-concept trust tier, verification history, and staleness
okf trust ./company_knowledge
```

*Example Output:*
```text
policies/travel_expenses [stable] human-reviewed
  generated: reference_agent/gemini-3.7-flash at 2026-06-20T22:53:05Z
  verified:  human:sarah_hr at 2026-06-25T09:00:00Z
  stale_after: 2026-12-31
  source:    [mileage-guide] Standard mileage reimbursement guidelines
computations/mileage_calc [stable] machine-confirmed
  generated: human:alex_finance at 2026-06-15T10:00:00Z
  verified:  process:ci-nightly at 2026-06-20T00:00:00Z

2 concept(s):
     1  human-reviewed
     1  machine-confirmed
```

```sh
# Summarize bundle statistics, types, and health
okf info ./company_knowledge
```

### Searching: search

`okf search` runs the same engine the studio palette uses over concept
metadata (ids, titles, descriptions, tags, headings) and body text, with a
composable filter syntax shared by every search surface:

| Term          | Meaning                          | Matches                        |
|---------------|----------------------------------|--------------------------------|
| `#tag`        | frontmatter tag                  | `tags` (case-insensitive)      |
| `type:X`      | concept type                     | `type`                         |
| `tier:X`      | trust tier                       | `human-reviewed`, `machine-confirmed`, `unverified` |
| `status:X`    | lifecycle status                 | `status`                       |
| `is:stale`    | stale on `today` (or `--today`)  | derived flag                   |
| `is:broken`   | has broken outgoing links        | derived flag                   |
| anything else | free text                        | fuzzy metadata match; body substring |

```sh
# Free text: fuzzy over ids/titles/tags/headings, grep-style body hits
okf search --bundle ./company_knowledge "mileage"

# Filters compose: stale, unreviewed concepts — an attention queue
okf search --bundle ./company_knowledge "is:stale tier:unverified" --json

# Pin staleness for deterministic output
okf search --bundle ./company_knowledge "is:stale" --today 2026-12-01 --limit 50
```

Metadata hits print as `id [status] title` (heading hits indented beneath),
body hits as `id:line  snippet…`, and `--json` emits
`{ query, hits, body_hits }` for pipelines and agents.

### Link graph and discovery: links and graph

```sh
# Inspect all internal and broken cross-links
okf links ./company_knowledge

# Check only for broken links (fails in CI if broken links exist)
okf links ./company_knowledge --broken --check

# Export cross-links in JSON format
okf links ./company_knowledge --format json

# Render link graph as Mermaid (ideal for GitHub READMEs or PR summaries)
okf graph ./company_knowledge --format mermaid --sources

# Export full dependency graph as JSON
okf graph ./company_knowledge --format json
```

### Listing computations: computations

Inspect and list all `Attested Computation` contracts declared in the bundle:

```sh
# List all attested computation contracts
okf computations ./company_knowledge
```

### Semantic diffs: diff

Perform semantic comparison between two OKF bundles (or two git worktrees):

```sh
okf diff ./bundle_v1 ./bundle_v2
```

*Example Output:*
```text
added (1):
  + policies/paid_time_off
removed (0):
renamed (1):
  ~ policies/old_travel -> policies/travel_expenses
content (1):
  ~ policies/travel_expenses (body)
trust (1):
  policies/travel_expenses: tier unverified -> human-reviewed
added links (1):
  + policies/travel_expenses -> computations/mileage_calc
```

### Formatting and indexing: fmt, index, and parse

```sh
# Dry-run format check for CI (exits with non-zero code if files need formatting)
okf fmt ./company_knowledge --check

# Format frontmatter and body in place across all markdown files
okf fmt ./company_knowledge -w

# Regenerate all index.md table-of-contents files across the directory tree
okf index ./company_knowledge

# Inspect AST and parsed frontmatter structure of a single document
okf parse ./company_knowledge/policies/travel_expenses.md
```

### Universal JSON output: `--json` / `-j`

Every CLI subcommand supports machine-readable JSON output via `--json` (or `-j` / `--format json`) for automated pipelines and AI agent tool calling:

```sh
okf validate ./company_knowledge --json
okf lint ./company_knowledge --json
okf info ./company_knowledge --json
okf trust ./company_knowledge --json
okf search --bundle ./company_knowledge "is:stale tier:unverified" --json
okf fmt ./company_knowledge --check --json
okf diff ./bundle_v1 ./bundle_v2 --json
```

## CI/CD integration

Add `okf` to your GitHub Actions workflow to automatically check every pull request:

`.github/workflows/okf.yml`:

```yaml
name: Bundle CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  validate:
    name: Conformance and lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install okf
        run: cargo install okf

      - name: Check formatting
        run: okf fmt ./company_knowledge --check

      - name: Validate OKF conformance
        run: okf validate ./company_knowledge

      - name: Check broken links
        run: okf links ./company_knowledge --broken --check

      - name: Lint bundle
        run: okf lint ./company_knowledge
```

---

## Using as a Rust library

Add `okf` or `okf-core` to your `Cargo.toml`:

```sh
cargo add okf
```

The `okf` crate's features let you take only what you need. `validator`,
`studio`, and `site` are on by default, as are the four language parsers
behind the validator's syntax checks: `python`, `javascript`, `rust`, and
`sql`. Each parser can be dropped independently. This matters under a strict dependency
or licence policy: the Python parser (`rustpython-parser`) depends on the
LGPL-3.0-only `malachite` crates, which an allow-list policy such as
`cargo deny` will reject. Conformance validation and linting need none of
the parsers, so a licence-clean dependency that still checks SQL is:

```toml
[dependencies]
okf = { version = "0.4", default-features = false, features = ["validator", "sql"] }
```

Or, without any of the validator, `okf-core` alone for a zero-dependency
model and parser. See the [`okf-validator` README](crates/okf-validator/README.md#cargo-features)
for the full feature table.

### 1. Loading and validating a bundle

```rust,no_run
use okf::{Bundle, ConceptId, Date, TrustTier, validate_bundle};

// Load bundle from disk
let bundle = Bundle::load("./company_knowledge")?;
println!("Loaded {} concepts", bundle.len());

// Conformance check
let report = validate_bundle(&bundle);
if report.is_conformant() {
    println!("Conformant with OKF v{}", okf::OKF_VERSION);
}

// Traverse cross-links and backlinks
let policy_id = ConceptId::parse("policies/travel_expenses")?;
for link in bundle.links_from(&policy_id) {
    println!("{} -> {} (exists: {})", policy_id, link.target, link.exists);
}
for backlink in bundle.backlinks(&policy_id) {
    println!("Referenced by backlink: {backlink}");
}

// Check trust and staleness
let today = Date::today_utc().unwrap();
for concept in bundle.concepts() {
    if concept.trust_tier() < TrustTier::HumanReviewed && concept.is_stale_on(today) {
        println!("Warning: {} is stale or unreviewed", concept.id);
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 2. Inspecting attested computations

```rust
use okf::Document;

let doc = Document::parse(
    "---\n\
     type: Attested Computation\n\
     runtime: python\n\
     parameters:\n\
     \x20 - { name: miles, type: number, required: true }\n\
     executor:\n\
     \x20 resource: references/skills/submit_expense.md\n\
     \x20 receipt: [report_id, calculated_amount, status]\n\
     ---\n\n# Computation\n\n\
     \x20   def calculate_reimbursement(miles: float, rate_per_mile: float = 0.67) -> float:\n\
     \x20       return round(miles * rate_per_mile, 2)\n",
)?;

let contract = doc.attested_computation().unwrap();
assert_eq!(contract.runtime.as_deref(), Some("python"));
assert_eq!(contract.required_parameters().count(), 1);
assert!(contract.computation.code().unwrap().contains("calculate_reimbursement"));
# Ok::<(), okf::DocumentError>(())
```

---

### 3. Multi-language syntax checking

```rust,no_run
use okf::{Bundle, check_syntax, lint_bundle};

let bundle = Bundle::load("./company_knowledge")?;
let report = lint_bundle(&bundle);

for diagnostic in report.diagnostics {
    println!("{diagnostic}");
}

// Check syntax directly for any supported language
assert!(check_syntax("python", "def calculate(total):
    return total * 0.2
").is_ok());
assert!(check_syntax("typescript", "const add = (a: number, b: number): number => a + b;").is_ok());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Python, JavaScript/TypeScript, Rust, and SQL checking each sit behind a
cargo feature of the same name (all on by default; see above). With a
feature disabled, `check_syntax` accepts that language unchecked and
`Language::is_supported` tells you so. JSON, YAML, and Bash are always
checked.

---

## Workspace crates

This repository is structured as a multi-crate Rust workspace:

| Crate | Description | Documentation |
| [`okf-web`](https://crates.io/crates/okf-web) | The `okf site` static site generator: maud templates, pulldown-cmark rendering, Tailwind v4 styling, vendored mermaid.js diagrams, and vendored shiki code highlighting — a deployable HTML view of a bundle. | [![docs.rs](https://img.shields.io/docsrs/okf-web)](https://docs.rs/okf-web) |
| [`okf`](https://crates.io/crates/okf) | CLI binary and re-exports of all core and validator APIs. | [![docs.rs](https://img.shields.io/docsrs/okf)](https://docs.rs/okf) |
| [`okf-core`](https://crates.io/crates/okf-core) | Pure-Rust OKF engine (YAML subset parser, AST, link graphs, diff, fix engine). | [![docs.rs](https://img.shields.io/docsrs/okf-core)](https://docs.rs/okf-core) |
| [`okf-validator`](https://crates.io/crates/okf-validator) | Conformance validator, multi-language syntax checker, and 13 opinionated linting rules. | [![docs.rs](https://img.shields.io/docsrs/okf-validator)](https://docs.rs/okf-validator) |
| [`okf-studio`](https://crates.io/crates/okf-studio) | The `okf studio` interactive terminal UI: explorer, graph, mission control, computations playground, and live refactoring. | [![docs.rs](https://img.shields.io/docsrs/okf-studio)](https://docs.rs/okf-studio) |
| [`okf-web`](https://crates.io/crates/okf-web) | The `okf site` static site generator: maud templates, pulldown-cmark rendering, Tailwind v4 styling, and vendored mermaid.js diagrams — a deployable HTML view of a bundle. | [![docs.rs](https://img.shields.io/docsrs/okf-web)](https://docs.rs/okf-web) |
| [`cargo-okf`](https://crates.io/crates/cargo-okf) | Cargo plugin wrapper allowing `cargo okf <cmd>`. | [![docs.rs](https://img.shields.io/docsrs/cargo-okf)](https://docs.rs/cargo-okf) |

---

## Design choices

- **Full frontmatter preservation:** Rather than deserializing into rigid structs (which would drop custom or extension keys), `Frontmatter` maintains an order-preserving map and layers typed accessors on top. Unknown keys survive round-trips untouched.
- **Computed, not stored, trust signals:** Trust tiers and credibility signals are derived at query time from verified actors. Storing a subjective trust number is fragile and non-portable.
- **Permissive and resilient loading:** `Bundle::load` never crashes on a single broken file; parse errors and broken links are collected as diagnostic graph items so you can inspect and fix them.
- **Deterministic by default:** Staleness checks are opt-in (`--today`) so validation remains reproducible across different execution environments.

---

## Mapping to the spec

| Spec section | Responsibility | Module |
|--------------|----------------|--------|
| §2 Terminology / Concept ID | Identifier normalization & path resolution | [`concept_id::ConceptId`](https://docs.rs/okf/latest/okf/concept_id/struct.ConceptId.html) |
| §3 Bundle structure | Directory traversal & reserved files | [`bundle::Bundle`](https://docs.rs/okf/latest/okf/bundle/struct.Bundle.html) |
| §4 Concept documents | Document AST, YAML frontmatter, body | [`document::Document`](https://docs.rs/okf/latest/okf/document/struct.Document.html), [`frontmatter::Frontmatter`](https://docs.rs/okf/latest/okf/frontmatter/struct.Frontmatter.html) |
| §5.1 Provenance | Sources, credibility signals, footnotes | [`provenance::Source`](https://docs.rs/okf/latest/okf/provenance/struct.Source.html), [`provenance::attributions`](https://docs.rs/okf/latest/okf/provenance/fn.attributions.html) |
| §5.2 Trust | `generated`, `verified` actors & timestamps | [`trust::Generated`](https://docs.rs/okf/latest/okf/trust/struct.Generated.html), [`trust::Verification`](https://docs.rs/okf/latest/okf/trust/struct.Verification.html) |
| §5.3 Trust tiers | `unverified`, `machine-confirmed`, `human-reviewed` | [`trust::TrustTier`](https://docs.rs/okf/latest/okf/trust/enum.TrustTier.html) |
| §5.4 / §5.5 Lifecycle | `status: draft\|stable\|deprecated`, `stale_after` | [`trust::Status`](https://docs.rs/okf/latest/okf/trust/enum.Status.html), [`trust::is_stale_on`](https://docs.rs/okf/latest/okf/trust/fn.is_stale_on.html) |
| §6 Cross-linking and paths | Relative link parsing, targets, and backlinks | [`links`](https://docs.rs/okf/latest/okf/links/) |
| §7 Actor convention | `human:<id>`, `process:<id>`, `<producer>/<ver>` | [`actor::Actor`](https://docs.rs/okf/latest/okf/actor/struct.Actor.html) |
| §8 Index files | Auto-generation of directory `index.md` listings | [`index::regenerate_indexes`](https://docs.rs/okf/latest/okf/index/fn.regenerate_indexes.html) |
| §9 Log files | Parsing and formatting `log.md` histories | [`log::Log`](https://docs.rs/okf/latest/okf/log/struct.Log.html) |
| §10 Attested computations | Contract models, parameters, and inline/external script syntax validation | [`computation::AttestedComputation`](https://docs.rs/okf/latest/okf/computation/struct.AttestedComputation.html), [`syntax::check_syntax`](https://docs.rs/okf/latest/okf/syntax/fn.check_syntax.html) |
| §11 Conformance | Conformance testing engine & diagnostic reporting | [`validate::validate_bundle`](https://docs.rs/okf/latest/okf/validate/fn.validate_bundle.html) |

---

## License

Licensed under the **Apache License, Version 2.0**, matching the upstream [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format) project. See [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE) for details.

`okf-core` has no dependencies. The default build of `okf` and `okf-validator` pulls in third-party language parsers, one of which (`rustpython-parser`, behind the `python` feature) depends on LGPL-3.0-only crates. Consumers with a strict licence policy can disable that feature; see [Using as a Rust library](#using-as-a-rust-library).

*Disclaimer: This is an independent open-source implementation and is not affiliated with or endorsed by Google.*
