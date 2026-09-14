# Decisions

* [Search lives in okf-core and the site ships a first-party client, not pagefind](okf_search_engine.md) - Decision record on making search a workspace capability - extracting the studio engine into a std-only okf-core module rather than keeping it in a consumer, and building a small first-party vanilla-JS site client against a build-time JSON index rather than adopting pagefind or porting the whole SearchIndex to WASM.

* [okf-site vs okf-web naming and dependency choice for the site generator](okf_web_static_site.md) - Decision record on naming the static-site crate okf-web, depending on okf-core instead of okf-studio, and rejecting WASM frameworks in favor of a build-time pipeline.
