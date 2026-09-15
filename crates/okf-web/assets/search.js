/* okf site search client.
 *
 * First-party vanilla JS. The fuzzy scorer and query syntax below are a
 * MANUAL PORT of okf-core's `search` module (crates/okf-core/src/search.rs)
 * — keep the scoring constants in sync with it. The index is generated at
 * build time by okf-web into this same file (prepended above this comment
 * as `window.okfSearchIndex = {...};`).
 *
 * Behavior mirrors the studio palette: type in the header input, results
 * drop down, ↑/↓ select, Enter navigates, Esc closes. Metadata hits first
 * (id, title, description, tags, headings), body hits after (smart-case
 * substring, one row per line). All rows are built with createElement +
 * textContent only — bundle content is producer data and never touches
 * innerHTML.
 */
(function () {
  'use strict';

  /* ---- Scoring constants (keep in sync with okf-core search.rs) ---- */
  var BONUS_BOUNDARY = 16;
  var BONUS_CAMEL = 12;
  var BONUS_CONSECUTIVE = 8;
  var BONUS_FIRST_CHAR = 20;
  var PENALTY_GAP_START = -3;
  var PENALTY_GAP_EXTEND = -1;
  var MATCH_SCORE = 16;

  /* Smith-Waterman-style alignment, ported from okf-core's fuzzy_match. */
  function fuzzyMatch(query, haystack) {
    var q = query.replace(/\s+/g, '');
    if (!q) return null;
    if (q.length > haystack.length) return null;
    var h = haystack;
    var eq = function (qc, hc) {
      if (qc !== qc.toLowerCase()) return qc === hc;
      return qc.toLowerCase() === hc.toLowerCase();
    };
    var bonusAt = function (i) {
      if (i === 0) return BONUS_FIRST_CHAR;
      var prev = h[i - 1];
      if ('/_-.:'.indexOf(prev) >= 0 || /\s/.test(prev)) return BONUS_BOUNDARY;
      if (prev === prev.toLowerCase() && h[i] !== h[i].toLowerCase()) return BONUS_CAMEL;
      return 0;
    };
    var n = q.length, m = h.length;
    var dp = [], parent = [];
    for (var i = 0; i < n; i++) { dp.push(new Array(m).fill(-Infinity)); parent.push(new Array(m).fill(-1)); }
    for (var j = 0; j < m; j++) if (eq(q[0], h[j])) dp[0][j] = MATCH_SCORE + bonusAt(j);
    for (var i2 = 1; i2 < n; i2++) {
      var bestPrev = -Infinity, bestPrevJ = -1;
      for (var j2 = i2; j2 < m; j2++) {
        if (bestPrev > -Infinity) bestPrev += PENALTY_GAP_EXTEND;
        if (j2 >= 2 && dp[i2 - 1][j2 - 2] > -Infinity) {
          var cand = dp[i2 - 1][j2 - 2] + PENALTY_GAP_START;
          if (cand > bestPrev) { bestPrev = cand; bestPrevJ = j2 - 2; }
        }
        if (!eq(q[i2], h[j2])) continue;
        var consecutive = dp[i2 - 1][j2 - 1] > -Infinity
          ? dp[i2 - 1][j2 - 1] + MATCH_SCORE + BONUS_CONSECUTIVE + bonusAt(j2) : -Infinity;
        var gapped = bestPrev > -Infinity ? bestPrev + MATCH_SCORE + bonusAt(j2) : -Infinity;
        if (consecutive >= gapped) {
          if (consecutive > -Infinity) { dp[i2][j2] = consecutive; parent[i2][j2] = j2 - 1; }
        } else { dp[i2][j2] = gapped; parent[i2][j2] = bestPrevJ; }
      }
    }
    var bestJ = -1, bestScore = -Infinity;
    for (var j3 = 0; j3 < m; j3++) if (dp[n - 1][j3] > bestScore) { bestScore = dp[n - 1][j3]; bestJ = j3; }
    if (bestJ < 0) return null;
    var indices = new Array(n), cursor = bestJ;
    for (var i3 = n - 1; i3 >= 0; i3--) { indices[i3] = cursor; if (i3 > 0) cursor = parent[i3][cursor]; }
    return { score: bestScore, indices: indices };
  }

  /* ---- Query syntax (keep in sync with okf-core Query::parse) ---- */
  function parseQuery(raw) {
    var text = [], filters = [];
    raw.split(/\s+/).forEach(function (term) {
      if (!term) return;
      if (term[0] === '#' && term.length > 1) { filters.push({ kind: 'tag', value: term.slice(1) }); return; }
      if (term.indexOf('type:') === 0) { filters.push({ kind: 'type', value: term.slice(5) }); return; }
      if (term.indexOf('tier:') === 0) { filters.push({ kind: 'tier', value: term.slice(5) }); return; }
      if (term.indexOf('status:') === 0) { filters.push({ kind: 'status', value: term.slice(7) }); return; }
      if (term === 'is:stale') { filters.push({ kind: 'stale' }); return; }
      if (term === 'is:broken') { filters.push({ kind: 'broken' }); return; }
      text.push(term);
    });
    return { text: text.join(' '), filters: filters };
  }

  function matchesFilters(entry, filters) {
    return filters.every(function (f) {
      switch (f.kind) {
        case 'tag': return entry.tags.some(function (t) { return t.toLowerCase() === f.value.toLowerCase(); });
        case 'type': return entry.type.toLowerCase() === f.value.toLowerCase();
        case 'tier': return entry.tier.toLowerCase() === f.value.toLowerCase();
        case 'status': return entry.status.toLowerCase() === f.value.toLowerCase();
        case 'stale': return entry.stale;
        case 'broken': return entry.broken;
      }
      return false;
    });
  }

  /* Smart-case substring search (keep in sync with find_substring_smart_case). */
  function findSubstring(query, line) {
    if (!query) return -1;
    if (query.toLowerCase() !== query) return line.indexOf(query);
    return line.toLowerCase().indexOf(query.toLowerCase());
  }

  var MAX_ROWS = 14;
  var SNIPPET_CONTEXT = 60;
  var BODY_LIMIT = 50;

  function searchIndex(idx, rawQuery) {
    var q = parseQuery(rawQuery);
    var hits = [];
    idx.entries.forEach(function (entry) {
      if (!matchesFilters(entry, q.filters)) return;
      if (!q.text) {
        hits.push({ entry: entry, heading: null, anchor: '', score: 0, label: entry.id, indices: [] });
        return;
      }
      var candidates = [entry.id, entry.title, entry.description].concat(entry.tags);
      var best = null, bestLabel = '';
      candidates.forEach(function (c) {
        var r = fuzzyMatch(q.text, c);
        if (r && (!best || r.score > best.score)) { best = r; bestLabel = c; }
      });
      if (best) hits.push({ entry: entry, heading: null, anchor: '', score: best.score, label: bestLabel, indices: best.indices });
      entry.headings.forEach(function (hd) {
        var r = fuzzyMatch(q.text, hd.text);
        if (r) hits.push({ entry: entry, heading: hd.text, anchor: hd.anchor, score: r.score - 1, label: hd.text, indices: r.indices });
      });
    });
    hits.sort(function (a, b) { return b.score - a.score || (a.entry.id < b.entry.id ? -1 : 1); });
    var bodyHits = [];
    if (q.text) {
      idx.bodies.forEach(function (b) {
        var entry = null;
        for (var i = 0; i < idx.entries.length; i++) if (idx.entries[i].id === b.id) { entry = idx.entries[i]; break; }
        if (!entry || !matchesFilters(entry, q.filters)) return;
        b.body.split('\n').forEach(function (line, lineNo) {
          if (bodyHits.length >= BODY_LIMIT) return;
          var at = findSubstring(q.text, line);
          if (at >= 0) {
            var t = line.replace(/^\s+/, '');
            var from = Math.max(0, at - SNIPPET_CONTEXT);
            var to = Math.min(t.length, at + q.text.length + SNIPPET_CONTEXT);
            bodyHits.push({
              entry: entry,
              snippet: (from > 0 ? '\u2026' : '') + t.slice(from, to) + (to < t.length ? '\u2026' : ''),
              line: lineNo + 1
            });
          }
        });
      });
    }
    return { hits: hits.slice(0, MAX_ROWS), bodyHits: bodyHits.slice(0, MAX_ROWS) };
  }

  /* ---- DOM wiring ----
   *
   * The page ships a tiny inline boot (see okf-web render.rs) that arms the
   * input and injects this file on the first keystroke; once this script
   * runs, we bind the input, run any query already typed, and own all later
   * keystrokes.
   */
  function makeRow(kind, text, href, selected) {
    var a = document.createElement('a');
    a.className = 'search-row' + (selected ? ' selected' : '') + (kind ? ' ' + kind : '');
    a.href = href;
    a.appendChild(document.createTextNode(text));
    return a;
  }

  function boot(input) {
    if (!input || input.dataset.okfSearch === '1' || !window.okfSearchIndex) return;
    input.dataset.okfSearch = '1';
    var box = null, sel = 0, rows = [];
    var prefix = input.dataset.prefix || '';

    function render(results, query) {
      closeBox();
      var hits = results.hits, bodyHits = results.bodyHits;
      if (!query.trim() || (!hits.length && !bodyHits.length)) return;
      box = document.createElement('div');
      box.className = 'site-search-results';
      hits.forEach(function (hit) {
        var href = prefix + hit.entry.id + '.html' + (hit.anchor ? '#' + hit.anchor : '');
        var text = hit.heading
          ? hit.entry.id + ' \u00bb ' + hit.heading
          : hit.entry.id + (hit.entry.title && hit.label !== hit.entry.id ? ' \u00b7 ' + hit.entry.title : '');
        rows.push({ el: makeRow('meta', text, href, rows.length === 0), href: href });
      });
      bodyHits.forEach(function (hit) {
        var href = prefix + hit.entry.id + '.html';
        var text = hit.entry.id + ':' + hit.line + '  ' + hit.snippet;
        rows.push({ el: makeRow('body', text, href, rows.length === 0), href: href });
      });
      sel = 0;
      rows.forEach(function (r) { box.appendChild(r.el); });
      input.parentNode.appendChild(box);
    }

    function closeBox() {
      if (box) { box.remove(); box = null; }
      rows = [];
      sel = 0;
    }

    function move(delta) {
      if (!rows.length) return;
      rows[sel].el.classList.remove('selected');
      sel = Math.min(Math.max(sel + delta, 0), rows.length - 1);
      rows[sel].el.classList.add('selected');
      rows[sel].el.scrollIntoView({ block: 'nearest' });
    }

    function run() {
      render(searchIndex(window.okfSearchIndex, input.value), input.value);
    }

    input.addEventListener('input', run);
    input.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowDown') { e.preventDefault(); move(1); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); move(-1); }
      else if (e.key === 'Escape') { closeBox(); }
      else if (e.key === 'Enter') {
        if (rows.length && rows[sel]) {
          e.preventDefault();
          window.location.assign(rows[sel].href);
        }
      }
    });
    document.addEventListener('click', function (e) {
      if (box && !box.contains(e.target) && e.target !== input) closeBox();
    });
    // The keystroke that triggered the lazy load happened before this
    // script ran; serve it now.
    if (input.value.trim()) run();
  }

  function init() {
    document.querySelectorAll('.site-search input[type="search"]').forEach(boot);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();