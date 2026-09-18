//! The site's search index: `assets/search-index.js` built from the shared
//! `okf-core` engine, plus the first-party `assets/search.js` client that
//! queries it from the header input.
//!
//! The index is the same `SearchIndex::build` result the `okf search`
//! subcommand and the studio palette consume — one engine, three
//! projections. Bodies ride along because the client's body search
//! (smart-case substring, line numbers) mirrors the CLI's `search_bodies`.

use okf_core::{Bundle, ConceptId, Date, SearchIndex};
use std::collections::HashMap;

/// The vendored client script (first-party vanilla JS, the fuzzy scorer and
/// query syntax ported from okf-core; see the keep-in-sync comment inside).
const SEARCH_JS: &str = include_str!("../assets/search.js");

/// One entry of the serialized index: the searchable fields the client
/// needs, in the shape its JS consumes.
#[derive(serde::Serialize)]
struct IndexEntry {
    id: String,
    title: String,
    description: String,
    tags: Vec<String>,
    headings: Vec<HeadingAnchor>,
    #[serde(rename = "type")]
    type_: String,
    tier: String,
    status: String,
    stale: bool,
    broken: bool,
}

/// A body heading with its page anchor, so heading hits deep-link.
#[derive(serde::Serialize, Clone)]
struct HeadingAnchor {
    text: String,
    anchor: String,
}

/// One serialized body, for the client's smart-case substring search.
#[derive(serde::Serialize)]
struct BodyEntry {
    id: String,
    body: String,
}

/// The shape serialized into `window.okfSearchIndex`.
#[derive(serde::Serialize)]
struct IndexJson {
    entries: Vec<IndexEntry>,
    bodies: Vec<BodyEntry>,
}

/// Builds `window.okfSearchIndex = {json};` plus the search client as one
/// JS file the pages can load lazily on first keystroke.
///
/// The JSON is post-processed to replace `<`, `&`, U+2028, and U+2029 with
/// their JS escapes so the file is valid standalone JavaScript in every
/// browser and inert if ever inlined into HTML — a `<script>` in a concept
/// title lands as `\u003c` text, never as markup.
#[must_use]
pub fn search_index_js(bundle: &Bundle, today: Option<Date>) -> String {
    let index = SearchIndex::build(bundle, today);
    let anchors = heading_anchors(bundle);
    let mut json = IndexJson {
        entries: Vec::with_capacity(index.entries.len()),
        bodies: Vec::with_capacity(index.entries.len()),
    };
    for entry in index.entries {
        let headings = anchors.get(&entry.id).cloned().unwrap_or_default();
        json.entries.push(IndexEntry {
            id: entry.id.to_string(),
            title: entry.title,
            description: entry.description,
            tags: entry.tags,
            headings,
            type_: entry.type_,
            tier: entry.tier.to_string(),
            status: entry.status.as_str().to_string(),
            stale: entry.stale,
            broken: entry.broken,
        });
        if let Some(concept) = bundle.get(&entry.id) {
            json.bodies.push(BodyEntry {
                id: entry.id.to_string(),
                body: concept.document.body.clone(),
            });
        }
    }
    let mut text = serde_json::to_string(&json).unwrap_or_default();
    escape_js_unsafe(&mut text);
    format!("window.okfSearchIndex={text};\n{SEARCH_JS}")
}

/// Per-concept heading anchors computed by the same slug allocator the page
/// writer uses, so index anchors match the emitted `id=` attributes.
/// Disambiguation is per-page, matching GitHub anchors and the writer.
fn heading_anchors(bundle: &Bundle) -> HashMap<ConceptId, Vec<HeadingAnchor>> {
    let mut map = HashMap::new();
    for concept in bundle.concepts() {
        let mut allocator = okf_core::SlugAllocator::default();
        let rows: Vec<HeadingAnchor> = okf_core::extract_headings(&concept.document.body)
            .into_iter()
            .map(|heading| HeadingAnchor {
                text: heading.text.to_string(),
                anchor: allocator.allocate(heading.text),
            })
            .collect();
        map.insert(concept.id.clone(), rows);
    }
    map
}

/// Replaces the bytes that would break a JS string context or an HTML
/// parser with their JS escapes: `<`, `&`, U+2028, and U+2029. JSON's own
/// escaping covers quotes, backslashes, and control characters; these four
/// are the ones that matter once the JSON lands inside a `<script>` or a
/// `.js` file.
fn escape_js_unsafe(json: &mut String) {
    *json = json
        .replace('<', "\\u003c")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html_breaking_bytes() {
        let mut s = String::from("a<b&c\u{2028}d\u{2029}e");
        escape_js_unsafe(&mut s);
        assert_eq!(s, "a\\u003cb\\u0026c\\u2028d\\u2029e");
        assert!(!s.contains('<'));
        assert!(!s.contains('&'));
    }
}
