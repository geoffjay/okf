---
okf_version: "0.2"
---

# okf knowledge base

This is the working knowledge base for the okf project, conforming to the
[Open Knowledge Format (OKF) v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md).

It consolidates working knowledge about the project: what okf is, how it is
structured, decisions and their rationale, recurring patterns, and plans.
It is authored by people and agents and meant to be read by both.

## For agents (policy)

This section is the single source of truth for how agents should use this knowledge
base. Tooling injects it into context automatically, so it does not depend on
`CLAUDE.md`/`AGENTS.md` being picked up.

Wired agents: **Claude Code** via a `SessionStart` hook (`.claude/hooks/kb-inject.py`,
with `PostToolUse`/`Stop` reminders in `.claude/hooks/kb-reminder.py`), **opencode**
via the `instructions` array in `.opencode/opencode.jsonc`, and **oh-my-pi** via the
`.omp/extensions/kb-hooks.ts` extension.

**Consult before acting.** Before working on a task, scan the entries below and
read any concept/decision/pattern doc relevant to what you are about to change.
Prefer the recorded decision or pattern over re-deriving one. This index is the
map; read the specific doc on demand rather than guessing.

**Update after acting.** Update the knowledge base when a change would make an
existing entry wrong or leave a new fact unrecorded. In particular:

* A new architectural decision → add or update a [decision](decisions/index.md)
  or an [architecture](architecture/index.md) record and relevant concept docs.
* A new recurring convention → add a [pattern](patterns/index.md).
* A new concept or architectural understanding → add a [concept](concepts/index.md).
* A forward-looking plan or roadmap item → add a [plan](plans/index.md).
* An external source or spec referenced by the KB → add a [reference](references/index.md).

Concept docs require YAML frontmatter with a `type` field; `index.md` and
`log.md` are reserved. When you add a doc, add a one-line pointer to the matching
category index below and a line to [`log.md`](log.md). If you deliberately decide
*not* to record a change, that is fine — the policy is judgement, not a mandate
to touch the KB on every edit.

# Concept

* [Overview](overview.md) - Initial overview concept for this bundle.

# Subdirectories

* [architecture](architecture/index.md) - Decision record on naming the static-site crate okf-web vs okf-site, depending on okf-core instead of okf-studio, and rejecting WASM frameworks in favor of a build-time pipeline.
* [concepts](concepts/index.md) - Concepts: what okf is and how it is structured. _(empty)_
* [decisions](decisions/index.md) - Decisions and their rationale. _(empty)_
* [patterns](patterns/index.md) - Recurring conventions and patterns. _(empty)_
* [plans](plans/index.md) - Plan for okf-web, the static site generator crate: pure-Rust build pipeline over okf-core and okf-validator, maud templates, pulldown-cmark rendering, mermaid.js vendored as an asset, and no WASM framework.
* [references](references/index.md) - External sources and specs referenced by this knowledge base, including the OKF spec.
