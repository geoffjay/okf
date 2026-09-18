---
type: Decision
title: Site themes are two attributes over a token catalog, not a second stylesheet
description: Decision record on how runtime theme switching works in okf site - a theme is a set of CSS custom-property values rather than a set of rules, carried by data-scheme (the light/dark base) and data-theme (palette overrides) on <html>, over a catalog compiled into site.css and extended per bundle through a generated theme.css.
tags:
  - okf-web
  - theming
  - configuration
  - adr
status: stable
generated:
  by: omp/main
  at: "2026-09-16T18:00:00Z"
stale_after: "2026-12-31T00:00:00Z"
sources:
  - id: okf-repo
    resource: https://github.com/W4G1/okf
    title: okf workspace repository
  - id: themes-plan
    resource: docs/knowledgebase/plans/okf_site_themes_plan.md
    title: Runtime theme switching plan, with the stylesheet audit behind this record
  - id: tailwind-source
    resource: crates/okf-web/assets/tailwind.css
    title: The hand-authored stylesheet holding the token blocks and built-in palettes
  - id: render-rs
    resource: crates/okf-web/src/render.rs
    title: The theme catalog, the pre-paint boot, the picker, and the mermaid boot
  - id: config-adr
    resource: docs/knowledgebase/decisions/okf_bundle_tool_config.md
    title: Per-bundle tool configuration decision record
  - id: shiki-dual-themes
    resource: https://shiki.style/guide/dual-themes
    title: Shiki dual- and multi-theme rendering with CSS variables
  - id: wcag-contrast
    resource: https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
    title: WCAG 2.2 - Understanding contrast (minimum)
---

# Decision: `data-scheme` + `data-theme` over a token catalog

Context: the site's stylesheet is compiled ahead of time by Tailwind and
committed, which looks like it forecloses letting a reader change theme.
It does not. An audit of the compiled output found 51 of the 62
color-bearing declarations inside the three token blocks, the remainder
being Tailwind's own `--color-violet-*` variables, two transparent
form-control backgrounds, and one panel shadow[^themes-plan]. Every
semantic color already resolved through `var(--token)`, and custom
properties re-resolve at computed-value time — so **a theme is a set of
values, and the ahead-of-time build only fixes the set of rules**. What was
missing was the N-theme shape.

## Two independent axes, because "is this dark?" is not a theme

Four behaviors are not tokens and cannot follow a palette on their own:
`color-scheme`, which shiki variable resolves, which mermaid mode renders,
and which glyph the theme button shows. With exactly two themes they could
be inferred from a `.dark`/`.light` class; with a catalog they cannot, so
the appearance became a declared property — and, once declared, obviously
orthogonal to the palette:

- `data-scheme` (`light`/`dark`, absent = follow the system) is the
  appearance. It selects the base palette and all four derived behaviors.
- `data-theme` (a *family* id, absent = the built-in `default` family)
  layers that family's overrides for the current appearance.

The first cut conflated them — the menu listed `auto`, `light`, `dark`,
`sepia`, `contrast` as one axis and every palette was pinned to a scheme —
and it broke on first contact with a real bundle: a Nord theme could only
be dark, and a stale `okf-theme` holding `light` silently outranked a
bundle's pinned palette. A family is the theme; light and dark are its
variants.

### A family has up to two variants, and a missing one falls through

Every variant is scoped to one appearance, explicitly
(`[data-theme="nord"][data-scheme="dark"]`) and through the matching media
query for the attribute-less `auto` case. A variant a family does not
define is simply absent from the stylesheet, so the base palette shows: a
light-only `sepia` under a dark appearance renders the standard dark
palette rather than a warm theme with unreadable ink. The menu says
"light only" next to such a family, because an invisible rule looks like a
bug.

Three properties follow from the split, and each is why it exists:

- **No-JS and first paint are unchanged.** Neither attribute present is
  precisely the pre-theme stylesheet: light base, dark under
  `prefers-color-scheme`. A bundle that pins a family gets that attribute
  rendered into the markup, so a reader without JavaScript sees it too.
- **Variants may be partial.** A variant naming four colors inherits the
  rest from its scheme's base block, which it outranks by specificity
  (0-3-0 against 0-2-0) regardless of order.
- **Derived behaviors need no per-theme CSS.** One shiki ladder, one glyph
  rule, one mermaid branch cover every palette — including palettes the
  stylesheet has never seen, which is the only way a bundle-defined family
  can get them right at all.

## The catalog is compiled in; bundles extend it

Built-in families (`default`, `sepia` light-only, `contrast` in both
appearances) live in `tailwind.css` and ship inside the inline stylesheet:
no fetch, no generated bytes. Bundle families arrive from
`.okf/config.yaml`[^config-adr] as `assets/theme.css`, written only when
`site.themes` exists — the same opt-in shape as `fonts.css`. One config
entry is one *variant*: entries sharing an id merge into a family, and
`(id, scheme)` is what must be unique.

The catalog exists twice, as a Rust table (`render::BUILTIN_THEMES`, which
the picker and the boot's allowlist are generated from) and as CSS. Drift
would be silent — a menu entry whose variant has no rules is
indistinguishable from a deliberately missing one — so a unit test asserts
the table and the compiled stylesheet agree variant by variant.

A bundle family may take a built-in id: the cascade then overrides that
family's tokens, and a variant the built-in lacks completes it, so adding a
dark `sepia` turns the light-only entry into a two-variant one. That is not
a special case in the code; it is what "later sheet, higher specificity"
already means.

`auto`, `light`, and `dark` are reserved against family ids. They name
appearances, and a theme called `light` would make both the menu and the
stored state ambiguous.

## Two keys, and one migration

`okf-theme` now holds the family and `okf-scheme` the appearance; absent
means the build's default, which is `default`/`auto` unless `site.theme`,
`site.scheme`, `okf site --theme`, or `--scheme` pins otherwise. A stored
family this build does not define falls back to that default rather than
leaving `<html>` carrying an unmatched `data-theme`.

The one migration: before families, `okf-theme` held `light`/`dark` — an
appearance. A stored value naming an appearance is therefore read as one,
which is exactly why those ids are reserved. It also fixes the reported
symptom that prompted the rework: a reader whose old toggle choice was
`light` no longer has it outrank a bundle's pinned theme, because the two
are no longer the same axis.

## A menu with two groups, not a toggle

One control, two radio groups: Appearance, then Theme. Flipping light↔dark
costs two clicks instead of one, which is the price of an N-way choice next
to an axis; a toggle plus a picker puts two controls in a 4 rem header, and
click-to-cycle hides the current selection. `aria-checked` is the selection
state and the stylesheet draws the tick from it, so the accessible state
cannot drift from the rendered one — the same rule the nav's caret follows.

## Code colors stay on the light/dark axis

Shiki can emit `--shiki-<key>` for any number of themes[^shiki-dual-themes],
but adding one means editing `shiki-entry.mjs` and re-running the offline
esbuild bundle by hand on a 9.6 MB asset. Palettes therefore select the
light or the dark side of the vendored GitHub pair, while `--code-bg` keeps
the card itself on the palette. Revisit when someone wants code colors that
differ from the page by more than the card.

## Diagrams go through the browser's color parser

Mermaid honors `themeVariables` wholesale only under its `base` theme, so a
successful token read selects `base` and passes the palette. The tokens are
authored in `oklch()`, which mermaid's color library (khroma) cannot parse
— and one unparseable color rejects the whole render, leaving every diagram
blank. Rather than ship a color-space conversion, the boot paints one pixel
on a canvas and reads it back: the browser's own parser, any CSS color the
stylesheet can hold, eight lines. Diagram nodes take `--surface` while the
canvas takes `--mermaid-bg`, so a node never matches the card it sits on.

## Contrast is measured, not eyeballed

`contrast` clears WCAG AAA in both variants: 21:1 body text on black with
its weakest badge at 10.9:1, and 21:1 on white with its weakest badge at
9.07:1 once the accents move to the 900 steps[^wcag-contrast]. `sepia`
drops its amber and green accents one Tailwind step: the light base's 700s
measure 4.30 and 4.21 against warm paper — under AA for badge text — and
the 800s restore 6.07 and 6.04. The light base's own `tier-human` sits at
4.44 on its surface; that predates this work and is left alone
deliberately, recorded here so it is not rediscovered as a regression.

[^okf-repo]: okf workspace repository
[^themes-plan]: Runtime theme switching plan, with the stylesheet audit behind this record
[^tailwind-source]: The hand-authored stylesheet holding the token blocks and built-in palettes
[^render-rs]: The theme catalog, the pre-paint boot, the picker, and the mermaid boot
[^config-adr]: Per-bundle tool configuration decision record
[^shiki-dual-themes]: Shiki dual- and multi-theme rendering with CSS variables
[^wcag-contrast]: WCAG 2.2 - Understanding contrast (minimum)
