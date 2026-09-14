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

## shiki.min.js

- What: shiki syntax highlighter, an esbuild IIFE bundle of the npm package
  (`shiki-entry.mjs` in this directory is the entry; the `window.okfShiki
  { load, highlight }` API and its design notes are documented there).
- Version: 3.23.0 (npm `shiki`, `@shikijs/*` 3.23.0,
  `@shikijs/vscode-textmate` 10.0.2; Oniguruma WASM inlined from
  `shiki/wasm`, which re-exports `@shikijs/engine-oniguruma/wasm-inlined`).
- Source: https://www.npmjs.com/package/shiki/v/3.23.0
- SHA-256 of shiki.min.js:
  cd65dcdd6d67b7c9ba019d41039afdd430e488cfea4de5cc3e9064471ce93f8b
- License: MIT, see `shiki-LICENSE.txt` (Copyright (c) 2021 Pine Wu,
  Copyright (c) 2023 Anthony Fu).
- Vendored for the same reason as mermaid: highlighting at build time would
  put a JS runtime in the build chain, and no pure-Rust TextMate highlighter
  exists. shiki publishes ESM only (no IIFE build since v1), so the entry is
  bundled offline with esbuild into the single classic script this site needs
  — self-contained and `file://`-openable. The Oniguruma engine is chosen
  over shiki's JavaScript regex engine because the latter's language set
  excludes `rust`, `toml`, `ini`, `diff`, `console`, and `go`; the WASM is
  inlined as a data URL, so nothing is fetched at runtime either. The bundle
  embeds every shiki language and registers the page's languages lazily; it
  is written into the generated site's `assets/` directory only when at
  least one page carries a fenced code block with a language tag.

To update: in a scratch dir run `npm install shiki@<new> esbuild`, replace
`shiki-entry.mjs` here if the API changed, run
`esbuild shiki-entry.mjs --bundle --minify --format=iife --platform=browser
--outfile=shiki.min.js`, update the version and SHA-256 above, and re-copy
the upstream LICENSE.