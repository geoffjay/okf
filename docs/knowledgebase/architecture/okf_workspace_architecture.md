---
type: Concept
title: okf workspace architecture
description: The crate-level architecture of the okf project — okf-core as the model, okf-validator as the judge, okf-studio as the resident view, okf-web as the deployable view, and the okf/cargo-okf entry points.
tags: [architecture, okf-core, okf-web]
---

# OKF Workspace Architecture

The okf project is a Cargo workspace of six crates with one rule at its
center: **okf-core models the bundle; every other crate consumes it.** No
crate re-parses markdown or YAML on its own — they all share okf-core's
permissive loader, link graph, and frontmatter accessors.

## Workspace layout

```mermaid
flowchart TD
    subgraph entry ["entry points"]
        OKF["okf<br/>(CLI binary)"]
        COKF["cargo-okf<br/>(cargo subcommand)"]
    end
    subgraph consumers ["consumers"]
        STUDIO["okf-studio<br/>(terminal UI)"]
        WEB["okf-web<br/>(static site generator)"]
        VALIDATOR["okf-validator<br/>(conformance + lint)"]
    end
    CORE["okf-core<br/>(bundle model, YAML, links, markdown)"]

    OKF --> STUDIO
    OKF --> WEB
    OKF --> VALIDATOR
    COKF --> OKF
    STUDIO --> CORE
    WEB --> CORE
    WEB --> VALIDATOR
    VALIDATOR --> CORE
    STUDIO --> VALIDATOR
```

Dependency direction only ever points inward. okf-core depends on the
standard library alone (its own YAML-subset parser, markdown link scanner,
directory walker). okf-validator adds conformance checking, linting, and
optional multi-language syntax parsers (oxc for JavaScript, RustPython for
Python). okf-studio renders the model interactively in the terminal;
okf-web renders it once at build time into static HTML. The `okf` binary
is a thin clap shell over all of them; `cargo okf` delegates to it.

## The one model, three views

okf-core's [`Bundle::load`] is permissive by design: parse errors, broken
links, and malformed frontmatter never abort a load — they are recorded on
the bundle and rendered as visible content. That single decision shapes
every consumer:

```mermaid
flowchart LR
    B["bundle on disk<br/>(markdown + YAML frontmatter)"]
    L["Bundle::load<br/>(permissive)"]
    M["Bundle model<br/>concepts · links · backlinks<br/>sources · parse errors"]
    V["okf-validator<br/>validate + lint"]
    S["okf studio<br/>live, interactive"]
    W["okf site<br/>static HTML"]

    B --> L --> M
    M --> V
    M --> S
    M --> W
    V -. "reports" .-> S
    V -. "reports" .-> W
```

- **okf studio** holds one resident process: an Elm-style unidirectional
  loop (message → update → snapshot → view) over a continuously
  re-validated model, rendering with ratatui.
- **okf site** is the build-time pipeline: pulldown-cmark event stream →
  classed-HTML writer → maud page templates → one flat directory of HTML
  with inlined Tailwind CSS and vendored mermaid.js. No server, no WASM.
- **okf validator** is the only crate that judges; both views display its
  reports but never duplicate its rules.

## CLI surface

The `okf` binary maps every model capability onto a subcommand; the
interactive studio and the site generator are just the two "view"
subcommands at the ends of the list:

```mermaid
flowchart LR
    subgraph authoring ["authoring"]
        A1["init · new"]
        A2["mv · rm"]
        A3["split · merge"]
    end
    subgraph quality ["quality"]
        Q1["validate · lint · fmt"]
        Q2["fix (via --fix)"]
    end
    subgraph introspection ["introspection"]
        I1["info · trust · links"]
        I2["graph · computations · diff"]
        I3["parse · index"]
    end
    views["views"]
    V1["studio (interactive)"]
    V2["site (static HTML)"]

    authoring --> CORE2["okf-core"]
    quality --> CORE2
    introspection --> CORE2
    views --> CORE2
```

Every subcommand shares the same guarantee: bundle problems are reported,
not fatal. `--json` exists on nearly all of them so agents (the "a" in
OKF's human- *and* agent-friendly design) can drive the same surface
people do.

## Data flow through the site generator

As the newest consumer, okf-web illustrates the permissive pipeline end
to end:

```mermaid
flowchart LR
    subgraph load ["load (okf-core)"]
        FS["*.md walk"] --> PARSE["parse<br/>frontmatter + body"]
        PARSE --> GRAPH["link graph<br/>outbound · backlinks"]
    end
    subgraph render ["render (okf-web)"]
        REWRITE["link rewrite<br/>.md → .html"] --> MD["markdown events<br/>(GFM: tables, task lists,<br/>footnotes, strikethrough)"]
        MD --> WRITER["classed-HTML writer<br/>md-* component classes"]
        WRITER --> TPL["maud layout<br/>header · nav · panels"]
    end
    CSS["site.css<br/>(Tailwind, inlined)"]
    MERMAID["mermaid.min.js<br/>(vendored, only if needed)"]
    OUT["site/<br/>flat HTML tree"]

    load --> render --> OUT
    CSS --> OUT
    MERMAID -. "diagram pages only" .-> OUT
```

The stylesheet is compiled once with the Tailwind standalone CLI
(`cargo xtask tailwind`) and committed, so `okf site` itself needs no
toolchain; the generator embeds it with `include_str!` at build time.

## Design rules worth preserving

1. **One model.** New capability lands in okf-core (or okf-validator for
   judgment), never duplicated in a consumer.
2. **Permissive in, safe out.** Producer mistakes surface as content;
   producer HTML is dropped at render time, never passed through.
3. **Minimal dependencies.** okf-core stays std-only; heavier parsers in
   okf-validator are optional features; okf-web carries only maud and
   pulldown-cmark.
4. **Two entry points, one binary.** `cargo okf` wraps `okf`; no logic
   lives in either shell.
