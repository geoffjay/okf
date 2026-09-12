---
okf_version: "0.2"
---

# Concept

* [Overview](overview.md) - Initial overview concept for this bundle.

# Subdirectories

* [architecture](architecture/index.md) - Decision record on naming the static-site crate okf-web vs okf-site, depending on okf-core instead of okf-studio, and rejecting WASM frameworks in favor of a build-time pipeline.
* [plans](plans/index.md) - Plan for okf-web, the static site generator crate: pure-Rust build pipeline over okf-core and okf-validator, maud templates, pulldown-cmark rendering, mermaid.js vendored as an asset, and no WASM framework.
