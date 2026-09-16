# Decisions

* [Search lives in okf-core and the site ships a first-party client, not pagefind](okf_search_engine.md) - Decision record on making search a workspace capability - extracting the studio engine into a std-only okf-core module rather than keeping it in a consumer, and building a small first-party vanilla-JS site client against a build-time JSON index rather than adopting pagefind or porting the whole SearchIndex to WASM.

* [okf-site vs okf-web naming and dependency choice for the site generator](okf_web_static_site.md) - Decision record on naming the static-site crate okf-web, depending on okf-core instead of okf-studio, and rejecting WASM frameworks in favor of a build-time pipeline.

* [Site themes are two attributes over a token catalog, not a second stylesheet](okf_site_theme_model.md) - Decision record on runtime theme switching in `okf site`: a theme is a set of CSS custom-property values rather than rules, carried by `data-scheme` (light/dark base) and `data-theme` (palette overrides) on `<html>`, over a catalog compiled into `site.css` and extended per bundle through a generated `theme.css`.

* [Per-bundle tool configuration lives in .okf/config.yaml, read only by its consumer](okf_bundle_tool_config.md) - Decision record on giving bundles a tool-configuration file: a dot-directory invisible to every OKF walker, YAML parsed by okf-core's std-only subset parser rather than a new TOML dependency, a `site:` mapping rather than an array of tables, strict keys against permissive content, and CLI-over-config-over-default precedence.
