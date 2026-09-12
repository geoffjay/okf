# Log

## 2026-09-11
* **Creation**: Initialized OKF bundle.

## 2026-09-11
* Added [okf-web static site generator plan](plans/okf_web_plan.md) and its [architecture decision record](architecture/okf_web_static_site.md): maud + pulldown-cmark build-time pipeline, mermaid.js vendored as an asset, `okf site` subcommand behind a feature flag.

## 2026-09-11
* **Scaffold**: Augmented the okf knowledge base at `docs/knowledgebase/` using the `okf-ify` skill. Added the agent policy section to `index.md` and the concept directories `concepts`, `decisions`, `patterns`, `references` (alongside the existing `architecture` and `plans`), plus [references/okf-spec.md](references/okf-spec.md). Wired Claude Code (`.claude/hooks/`), opencode (`.opencode/opencode.jsonc`) and oh-my-pi (`.omp/extensions/kb-hooks.ts`) to consult and update the KB. OKF v0.2 conformant.
