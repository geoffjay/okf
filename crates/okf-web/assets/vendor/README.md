# Vendored third-party assets

## mermaid.min.js

- What: mermaid.js diagram renderer, minified self-contained IIFE build.
- Version: 12.0.0 (npm `mermaid` package, `dist/mermaid.min.js`).
- Source: https://www.npmjs.com/package/mermaid/v/12.0.0
- SHA-256 of mermaid.min.js:
  28fca7ae6ebc7ed7bb63bde63136a74bfef14f296a57e403657eeb8b32836073
- License: MIT, see `mermaid-LICENSE.txt` (Copyright (c) 2014-2022 Knut
  Sveidqvist and mermaid.js contributors).
- Vendored as a plain static asset — not a crate or npm dependency — because
  there is no production-grade pure-Rust mermaid renderer, and executing
  mermaid at build time would put a JS runtime in the build chain. The bundle
  is embedded into the `okf-web` crate with `include_bytes!` and written into
  the generated site's `assets/` directory only when at least one page
  contains a diagram.

To update: download the new `dist/mermaid.min.js`, replace the file, update
the version and SHA-256 above, and re-copy the upstream LICENSE.
