// Vendored okf shiki entry — the source of the committed shiki.min.js.
//
// Built with esbuild (see assets/vendor/README.md):
//   esbuild shiki-entry.mjs --bundle --minify --format=iife --platform=browser
//
// Design, mirroring the vendored mermaid.min.js precedent:
// - Every shiki language registers lazily from the bundled map, so the file
//   is self-contained (no fetches; the site opens from file://).
// - The Oniguruma engine with inlined WASM, because the JavaScript regex
//   engine's language set excludes rust, toml, ini, diff, console, and go —
//   languages OKF knowledge bases actually fence. The WASM is a data URL
//   inside the bundle, so nothing is fetched at runtime either.
// - One dual-theme render: `defaultColor: false` emits only
//   `--shiki-light`/`--shiki-dark` CSS variables, which the site's
//   stylesheet resolves under its .dark/.light theme classes — a theme
//   toggle costs nothing (no re-render, unlike mermaid).
// - A transformer rewrites the outer <pre> to the site's md-pre component
//   class (dropping shiki's inline background/color), so the swap keeps the
//   site's code-block styling and the boot can replace the <pre> wholesale.
//
// window.okfShikiReady (a promise) is assigned synchronously; the per-page
// boot script chains on it and then drives window.okfShiki
// { load(langs), highlight(code, lang) }, collecting languages from the
// `code.language-{lang}` classes the markdown writer emits.

import { createHighlighter, bundledLanguages } from 'shiki';

// The handle is assigned synchronously so the boot script can chain on it:
// the highlighter itself (WASM init) resolves later, and `window.okfShiki`
// only exists once it has.
window.okfShikiReady = createHighlighter({
  themes: ['github-light', 'github-dark'],
}).then(function (hl) {
  window.okfShiki = {
    // Registers languages from the bundled map (aliases included as their
    // own keys). Unknown names are skipped; loadLanguage rejections on
    // grammars that need the wasm engine later are swallowed per-language.
    load: function (langs) {
      return Promise.all(
        langs
          .filter(function (l) {
            return !!bundledLanguages[l];
          })
          .map(function (l) {
            return hl.loadLanguage(l).catch(function () {});
          })
      );
    },
    // Returns highlighted HTML for the code element, or null when the
    // language is not loaded (the boot leaves the plain <pre> in place).
    highlight: function (code, lang) {
      if (hl.getLoadedLanguages().indexOf(lang) === -1) return null;
      return hl.codeToHtml(code, {
        lang: lang,
        themes: { light: 'github-light', dark: 'github-dark' },
        defaultColor: false,
        transformers: [
          {
            pre: function (node) {
              node.properties.class = ['md-pre', 'shiki'];
              delete node.properties.style;
            },
          },
        ],
      });
    },
  };
});
// A failed init leaves okfShiki undefined; the boot's chain then runs with
// nothing to highlight and the plain <pre>s stay as the fallback.
window.okfShikiReady.catch(function (err) {
  console.warn('okf-web: shiki failed to initialize:', err);
});