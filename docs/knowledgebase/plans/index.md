# Plan

* [First-class search - one engine in okf-core, three surfaces](okf_search_plan.md) - Plan for search as a workspace capability: extract the studio's fuzzy engine and query syntax into a std-only okf-core search module, add an okf search CLI subcommand with body-text hits, cut the studio over to the shared engine, and make the site's dead header input work via a build-time JSON index plus a small first-party vanilla-JS client - no pagefind, no WASM.
* [okf-web as a maud + pulldown-cmark static site generator](okf_web_plan.md) - Plan for okf-web, the static site generator crate: pure-Rust build pipeline over okf-core and okf-validator, maud templates, pulldown-cmark rendering, mermaid.js vendored as an asset, and no WASM framework.
