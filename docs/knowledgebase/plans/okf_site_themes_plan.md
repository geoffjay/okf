---
type: Plan
title: Runtime theme switching for okf site
description: "Plan for visitor-selectable themes in the generated site: the compiled Tailwind stylesheet already routes every semantic color through :root custom properties, so a theme is a set of token values, not a set of rules. Splits today's .dark/.light class into two attributes (data-scheme for the light/dark axis, data-theme for palette overrides), replaces the toggle with a header picker over a compiled-in catalog, and lets a bundle add its own palettes through .okf/config.yaml as a generated assets/theme.css - no per-bundle Tailwind recompile."
tags:
  - okf-web
  - theming
  - configuration
  - planning
status: stable
generated:
  by: omp/main
  at: "2026-09-16T12:00:00Z"
stale_after: "2026-12-31T00:00:00Z"
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: tailwind-source
    resource: crates/okf-web/assets/tailwind.css
    title: The site's hand-authored Tailwind source, where the palette tokens live
  - id: site-css
    resource: crates/okf-web/assets/site.css
    title: The committed compiled stylesheet, audited for color literals
  - id: render-rs
    resource: crates/okf-web/src/render.rs
    title: The page shell, theme bootstrap, and the mermaid/shiki boots
  - id: fonts-plan
    resource: docs/knowledgebase/plans/okf_site_fonts_plan.md
    title: Per-bundle site configuration and font support plan
  - id: config-adr
    resource: docs/knowledgebase/decisions/okf_bundle_tool_config.md
    title: Per-bundle tool configuration decision record
  - id: web-adr
    resource: docs/knowledgebase/decisions/okf_web_static_site.md
    title: okf-web naming and dependency decision record
  - id: mdn-custom-properties
    resource: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_cascading_variables/Using_CSS_custom_properties
    title: MDN - Using CSS custom properties
  - id: mdn-color-scheme
    resource: https://developer.mozilla.org/en-US/docs/Web/CSS/color-scheme
    title: MDN - color-scheme
  - id: shiki-dual-themes
    resource: https://shiki.style/guide/dual-themes
    title: Shiki dual- and multi-theme rendering with CSS variables
  - id: wcag-contrast
    resource: https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
    title: WCAG 2.2 - Understanding contrast (minimum)
---

# Runtime theme switching for `okf site`

## Verdict

Feasible, and cheap — the ahead-of-time Tailwind build is not the
constraint people expect it to be.

An audit of the committed stylesheet[^site-css] makes it concrete. Of its
536 declarations, 62 carry a color literal, and 51 of those sit inside the
three token blocks (`:root`, the `prefers-color-scheme: dark` media block,
and `:root.dark`). The other 11 are Tailwind's own `--color-violet-300`
and `--color-violet-400` theme variables (which `--link` points at), two
preflight `background-color: #0000` rules on form controls, and one
`box-shadow: 0 10px 30px #00000026` on the search results panel. Every
*semantic* color in the site — surface, ink, edge, link, panel, code
background, trust tiers, statuses, severities — already resolves through
`var(--token)`. The search client injects no inline colors either: it
styles results with the `.site-search-results` classes the stylesheet
owns.

That is the whole feasibility answer. Custom properties are resolved at
computed-value time on every element[^mdn-custom-properties], so
re-assigning them at runtime repaints the site with no recompile. A theme
is a set of *values*; the precompiled CSS fixes the set of *rules*.
Tailwind has to be re-run only when a theme needs a rule that does not
exist yet — a new token or a new selector — which is a one-time,
universal change, never per-bundle and never per-theme.

Runtime switching already works for exactly two themes: `THEME_BOOT`
applies `.dark`/`.light` from `localStorage` before first paint, and the
header button flips the class and dispatches
`okf-theme-change`[^render-rs]. What is missing is the *N-theme* shape,
and four things that are not tokens and therefore do not follow a palette
automatically:

1. `color-scheme`, which drives UA widgets and scrollbars[^mdn-color-scheme].
2. The shiki ladder, which resolves `--shiki-light` or `--shiki-dark`.
3. The mermaid boot's theme name (`'dark'` vs `'default'`).
4. The sun/moon icon visibility rules.

All four are keyed on "is this dark?", derived today from the presence of
`.dark`/`.light` or the media query. With more than two themes that
question stops being answerable from the theme's identity, so the plan's
central move is to make it a declared property instead of an inference.

## Scope

In scope:

- A two-axis state model on `<html>`: `data-scheme` (`light`/`dark`,
  absent = follow the system) and `data-theme` (a palette id, absent =
  the scheme's base palette). Replaces `.dark`/`.light` outright.
- A compiled-in theme catalog: `auto`, `light`, `dark`, plus two built-in
  palettes (`sepia`, `contrast`) that exist to make the picker worth
  having and to prove the mechanism carries a real palette.
- A header theme picker replacing the two-state button, keyboard
  operable, persisted in the existing `okf-theme` key, applied pre-paint.
- Bundle-defined palettes: `site.themes` in `.okf/config.yaml` emitted as
  a generated `assets/theme.css`, following the `fonts.css`
  precedent[^fonts-plan] — opt-in, constructed rather than interpolated,
  no Tailwind toolchain for the bundle author.
- `site.theme` (plus `okf site --theme`) to pin the theme a first-time
  visitor sees.
- Diagrams that follow arbitrary palettes, via mermaid `themeVariables`
  read from the computed tokens.

Out of scope (v1):

- **Per-theme code colors.** Shiki can emit `--shiki-<key>` for any
  number of themes[^shiki-dual-themes], but adding one means editing
  `shiki-entry.mjs` and re-running the offline esbuild bundle — a manual
  step outside `cargo xtask`, on a 9.6 MB asset. Themes select the light
  or the dark side of the existing GitHub pair; `--code-bg` still
  follows the palette, so a sepia page gets a sepia code card with
  GitHub-light tokens inside it.
- Arbitrary user CSS. Same reasoning as the fonts plan: token overrides
  are bounded and reviewable, a stylesheet overlay is not.
- Per-theme fonts. `fonts.css` already owns the typography tokens, and
  the two token sets stay disjoint by validation.
- Theming `okf studio`. Its `Theme` is a `NO_COLOR` switch over ratatui
  styles; nothing here transfers.
- A no-JS theme selector. Today's toggle already requires JS; a
  `:checked`-driven CSS-only picker cannot persist or pre-paint (see
  rejected alternatives).

## Decisions

### Two attributes, two axes

`data-scheme` carries everything that is really a light/dark question:
the base palette, `color-scheme`, the shiki side, the mermaid base theme,
and which glyph the picker button shows. `data-theme` carries palette
overrides on top of it.

```css
:root                        { /* light base: today's :root */ }
@media (prefers-color-scheme: dark) {
  :root:not([data-scheme])   { /* today's media block, verbatim */ }
}
:root[data-scheme="light"]   { color-scheme: light; }
:root[data-scheme="dark"]    { /* today's :root.dark, verbatim */ }
:root[data-theme="sepia"]    { /* palette overrides only */ }
```

Three properties fall out of this split, and each one is why the split
exists rather than a flat `data-theme` ladder:

- **No-JS and first paint are unchanged.** Neither attribute present is
  exactly today's behavior — light base, dark under the media query — so
  a visitor without JavaScript, or with `localStorage` unavailable, sees
  what they see now.
- **Partial themes work.** A bundle theme that sets four colors inherits
  the rest from its declared scheme's base block. Equal specificity
  (`:root[data-scheme="dark"]` and `:root[data-theme="nord"]` are both
  0-2-0) means source order decides, and the generated `theme.css` is
  linked after the inline stylesheet, so overrides win. Without the
  scheme axis, a dark palette that forgot `--edge` would inherit the
  *light* edge from `:root`.
- **The derived behaviors need no per-theme CSS.** The shiki ladder
  collapses from four selector groups to two (`:root[data-scheme="dark"]`
  plus the `:not([data-scheme])` media case), and every new theme —
  built-in or bundle-defined — is covered by construction.

A bundle theme may reuse a built-in id (`dark`), in which case it
overrides that palette instead of adding a menu entry. This is a
consequence of the cascade, not a special case, and it is the natural way
to say "keep the standard two themes, restyle them."

### The catalog is compiled in; bundles extend it

Two sources, mirroring the fonts split:

1. **Built-ins** live in `tailwind.css` and ship inside `site.css`. Zero
   fetch, zero generated bytes, available to every bundle. `auto`,
   `light`, `dark` are the current behavior renamed; `sepia` (light
   scheme, warm surfaces) and `contrast` (dark scheme, maximal ink/surface
   separation) are the new ones.
2. **Bundle themes** come from `.okf/config.yaml` and are emitted as
   `assets/theme.css`, linked after `fonts.css`. The token sets are
   disjoint, so link order between the two generated sheets is not load
   bearing, but it is fixed anyway for determinism.

The Rust side needs the catalog too — the picker markup and the boot's
id allowlist are generated from it — so `render.rs` gains a
`BUILTIN_THEMES: &[ThemeEntry]` table of `{ id, label, scheme }`. That is
a duplication of knowledge with the CSS, and duplication that can drift
silently is not acceptable: a unit test asserts every built-in id appears
as a `[data-theme="<id>"]` selector in `SITE_CSS`, so deleting a palette
block without deleting its entry fails the suite.

### Persisted state does not change shape

`okf-theme` keeps holding a single string. The legacy values `'dark'` and
`'light'` are valid ids in the new catalog, so no migration code exists:
a visitor who toggled to dark last week keeps dark. Absent key = `auto`.

A stored id the page does not know (a bundle theme removed since, or a
key hand-written into the origin's storage) must not leave the page in a
half-state — with `data-theme` set but no matching block, the site would
render on the light base even under a dark system preference. The boot
therefore validates the stored value against the catalog the generator
rendered into it, and falls back to the build's default theme, then to
`auto`.

### The picker replaces the toggle

One control, not two. The header button becomes a menu trigger
(`aria-haspopup`, `aria-expanded`) that still shows the sun/moon glyph
for the resolved scheme; the panel lists `Auto (system)` first, then every
catalog entry as a `role="menuitemradio"` with `aria-checked`. Escape,
click-outside, and arrow-key roving focus are ~30 lines in the existing
boot style.

The cost is real and worth naming: flipping light↔dark becomes two clicks
instead of one. The alternative — keeping a toggle *and* adding a picker —
puts two controls with overlapping meaning in a 4 rem header, and
click-to-cycle hides the current selection and scales badly past three
themes. A menu is the honest shape for an N-way choice.

A `storage` event listener applies a change made in another tab, which
also makes the existing "multiple okf sites on one origin share the
choice" comment true while a second tab is open.

### Configuration is strict and CSS is constructed

Unchanged ethos from the config ADR[^config-adr]: content is permissive,
tooling is strict. Unknown keys, unknown color tokens, malformed colors,
a missing `scheme`, a duplicate theme id, and a `site.theme` naming a
theme that does not exist are all build errors naming the offending key.
An unknown top-level section stays a note.

No configured byte reaches the stylesheet verbatim:

- **Ids** match `[a-z][a-z0-9-]{0,31}`, so an id can never close an
  attribute selector or a block.
- **Labels** are HTML, not CSS, and go through maud's escaping like every
  other frontmatter interpolation.
- **Color tokens** come from a closed set — the documented token names,
  minus the font tokens — so a theme cannot reach `--font-body`, and
  cannot invent a property.
- **Color values** parse against a closed grammar (`#rgb`, `#rgba`,
  `#rrggbb`, `#rrggbbaa`, and `oklch()`/`oklab()`/`rgb()`/`hsl()` with
  numeric, percentage, `none`, and `/`-alpha arguments only) and are
  re-emitted from the parse, exactly as `FamilyList` re-emits font
  families. Function nesting, `var()`, `url()`, and `;` are parse errors,
  not filtered strings.

### Diagrams follow the tokens

The mermaid boot already reads `--font-body` from the computed style to
set `fontFamily`. The same call site gains `themeVariables` for the
handful of mermaid variables that map onto site tokens
(`background`/`primaryColor` from `--mermaid-bg`, `primaryTextColor` and
`textColor` from `--ink`, `lineColor`/`primaryBorderColor` from `--edge`),
with the base theme still chosen by scheme. This is what makes a bundle
palette reach diagrams without any Rust-side knowledge of the palette,
and it costs nothing at build time.

## Schema

```yaml
# .okf/config.yaml
site:
  # The theme a first-time visitor gets; `okf site --theme` overrides it.
  # Omitted = `auto` (follow the system preference), today's behavior.
  theme: nord

  themes:
    - id: nord              # [a-z][a-z0-9-]{0,31}; unique; may shadow a built-in
      label: "Nord"         # menu text; defaults to the id
      scheme: dark          # required: base palette, color-scheme, shiki side
      colors:               # closed token set; every key optional
        surface: "#2e3440"
        ink: "#eceff4"
        edge: "#4c566a"
        link: "#88c0d0"
        panel-bg: "#3b4252"
        code-bg: "#3b4252"
        mermaid-bg: "#3b4252"
        tier-human: "#a3be8c"
```

Token keys accepted under `colors`: `surface`, `ink`, `edge`, `link`,
`shadow`, `panel-bg`, `code-bg`, `mermaid-bg`, `tier-unverified`,
`tier-machine`, `tier-human`, `status-draft`, `status-stable`,
`status-deprecated`, `status-other`, `sev-info`, `sev-warning`,
`sev-error`, `danger`.

## Rejected alternatives

- **A CSS-only picker (radio inputs plus `body:has(:checked)`).** The nav
  collapse proves the trick works, and custom properties set on `body` do
  cascade. But it cannot persist a choice, cannot apply before first
  paint, and would need its own emission path parallel to the attribute
  one. The existing toggle already requires JS; two mechanisms for one
  feature is the worse trade.
- **A flat `data-theme` ladder with no scheme axis.** Every theme would
  have to restate all 19 tokens (bundle themes could not be partial), and
  the shiki/mermaid/icon rules would need a per-theme entry — meaning a
  bundle theme could not get them right at all, because those rules live
  in the precompiled stylesheet.
- **Recompiling Tailwind per bundle.** Puts the standalone CLI (a
  downloaded binary) into every `okf site` run, breaks the "the generator
  needs no toolchain" property, and buys nothing: themes change values,
  and values are already tokens.
- **Shipping every shiki theme.** The bundle is 9.6 MB with two; the
  update path is a manual esbuild invocation documented in the vendor
  README. Deferred until someone asks for code colors that differ from
  the page palette by more than the card background.
- **`prefers-color-scheme`-only, no override.** That is the pre-toggle
  world the repo already moved past.
- **A `--theme` flag with no config key.** Not per-project and not
  persisted; the CLI stays the override layer, matching `--title`.

## Wiring

1. `crates/okf-web/assets/tailwind.css`: rewrite the three token blocks
   and the two derived ladders (shiki, theme icons) onto `data-scheme`;
   add `--shadow` and point the `.site-search-results` box-shadow at it;
   add the `sepia` and `contrast` palette blocks; add `.theme-menu`
   component styles (panel on `--panel-bg`, `--edge` border, `--shadow`,
   checked-item accent). Recompile with `cargo xtask tailwind`.
2. `crates/okf-web/src/render.rs`: `ThemeEntry` + `BUILTIN_THEMES`;
   `SiteChrome` gains `themes` and `default_theme`; `THEME_BOOT` becomes
   `theme_boot(&[ThemeEntry], Option<&str>)` emitting the id allowlist and
   setting both attributes pre-paint; `THEME_TOGGLE_BOOT` becomes
   `THEME_MENU_BOOT` (open/close, Escape, outside click, roving focus,
   apply/persist, `okf-theme-change`, `storage` sync); the menu markup;
   `mermaid_boot` reads the scheme attribute and the palette tokens.
3. `crates/okf-web/src/config.rs`: `Theme`, `ThemeColors`, `Scheme`, and
   a `Color` newtype with the closed grammar; `site.theme` and
   `site.themes` in `read_site`'s `KNOWN`; `Themes::to_css()` beside the
   validation it depends on, as `Fonts::to_css` is; `settings()` gains
   `theme` and `themes`.
4. `crates/okf-web/src/lib.rs`: `SiteOptions::theme`; precedence
   `--theme > site.theme > auto` resolved against the merged catalog
   (unknown id is an error naming the known ids); write `assets/theme.css`
   and set `SiteChrome::has_theme_css` when the bundle defines themes;
   extend the doc comment's asset list.
5. `crates/okf/src/cli.rs`: `--theme` on `SiteArgs`, the module doc's
   `site` line, and a `note: --theme overrode \`site.theme\`` line
   mirroring the title note.
6. Docs: `crates/okf-web/README.md` (state model, catalog, config keys),
   the root `README.md` `okf site` section, and this bundle's KB.

## Phasing

1. **Two-axis refactor.** Attributes replace classes across
   `tailwind.css`, `render.rs`, and the boots; no new capability, no
   visible change. Acceptance: existing `tests/site.rs` assertions
   updated to the attribute model and green; toggling still works in a
   browser with no flash on reload; a bundle built before and after
   differs only in the expected attribute/selector strings.
2. **Catalog and picker.** `ThemeEntry`, the parameterized boot, the menu
   markup and behavior, the `sepia` and `contrast` palettes, the
   drift test. Acceptance: every catalog entry applies in a browser and
   survives reload; keyboard-only operation works; `auto` tracks a
   system-preference change live; body text meets WCAG AA 4.5:1 in every
   built-in palette and AAA in `contrast`[^wcag-contrast].
3. **Bundle themes.** Config parsing and validation, `theme.css`
   emission, `site.theme`/`--theme` precedence, `SiteSummary` reporting.
   Acceptance: unit tests for the grammar, the closed token set, id
   charset, duplicate ids, missing `scheme`, and an unknown default;
   integration tests that a bundle without `site.themes` writes no
   `assets/theme.css` and links none, that a configured bundle writes it
   with the `<link>` after `fonts.css`, and that a config theme shadowing
   `dark` overrides rather than appends.
4. **Diagram fidelity.** `themeVariables` from the computed tokens.
   Acceptance: a diagram page under `sepia` and under a config theme
   shows matching canvas, text, and edge colors, and re-renders on
   switch with no console errors.
5. **Docs and records.** READMEs, changelog, `log.md`, and an ADR
   promoting the two-axis state model out of this plan;
   `okf validate`/`okf lint` clean.

## Risks

- **Silent drift between the Rust catalog and the CSS blocks** — the
  `SITE_CSS` selector test makes it loud.
- **A stored id that no longer exists** — allowlist validation in the
  boot, falling back to the build default and then `auto`.
- **CSS injection through config colors** — closed grammar, re-emission
  from the parse, closed token key set, id charset.
- **Contrast regressions in new palettes** — measured, not eyeballed;
  ratios are an acceptance criterion, and the 19 tokens include every
  badge color, which is where a hand-tuned palette usually fails first.
- **Stale `assets/theme.css` after removing config** — same class as the
  stale `fonts.css`/`mermaid.min.js` case: `generate` never cleans
  `out_dir`, and the `<link>` disappearing from the pages is the visible
  contract.
- **The "no external CSS" property gets a second exception** — already
  true for `fonts.css`; the README sentence needs updating rather than
  defending, and both files stay same-origin, page-depth-relative, and
  `file://`-openable.
- **Default output is no longer byte-identical to a pre-theme build** —
  by design: the shell gains a picker. The invariant that survives is the
  config one: a bundle that defines no themes writes no generated
  stylesheet and links nothing.

## Verification

- `cargo test -p okf-web`, then the workspace suite.
- Browser matrix on the generated site for this knowledge base, over
  `file://` and over a server: each catalog entry × (a diagram page, a
  code page, the search panel, the dashboard, the graph) — tokens
  applied, no flash on reload, no console errors.
- Contrast measured per palette against WCAG AA for body text and
  badges[^wcag-contrast].
- A bundle with no `.okf/config.yaml` generated before and after
  milestone 3: identical output.
- `okf validate .` and `okf lint .` on this knowledge base after the doc
  edits.

[^okf-repo]: okf workspace repository
[^tailwind-source]: The site's hand-authored Tailwind source, where the palette tokens live
[^site-css]: The committed compiled stylesheet, audited for color literals
[^render-rs]: The page shell, theme bootstrap, and the mermaid/shiki boots
[^fonts-plan]: Per-bundle site configuration and font support plan
[^config-adr]: Per-bundle tool configuration decision record
[^web-adr]: okf-web naming and dependency decision record
[^mdn-custom-properties]: MDN - Using CSS custom properties
[^mdn-color-scheme]: MDN - color-scheme
[^shiki-dual-themes]: Shiki dual- and multi-theme rendering with CSS variables
[^wcag-contrast]: WCAG 2.2 - Understanding contrast (minimum)
