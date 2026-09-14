//! Fuzzy matching, the omnisearch index, and the shared query syntax.
//!
//! One scorer serves every search surface — the studio's omnisearch
//! palette, command mode, refactor form completion, tree type-ahead, and
//! graph filtering, the `okf search` subcommand, and the generated site's
//! client-side search. The syntax layer adds the cheap, composable filters
//! (`#tag`, `type:Policy`, `tier:unverified`, `is:stale`, `is:broken`)
//! reused by every filterable view, and [`search_bodies`] layers
//! grep-style body hits on top.
//!
//! Everything here is std-only over the permissive [`Bundle`]: parse
//! errors, broken links, and malformed frontmatter never abort a search —
//! they are simply not in the model.

use crate::{Bundle, ConceptId, Date, Status, TrustTier};
use std::str::FromStr;

/// One concept's searchable representation, precomputed at index build.
#[derive(Clone, Debug)]
pub struct SearchEntry {
    /// The concept id.
    pub id: ConceptId,
    /// Display title.
    pub title: String,
    /// One-line description (may be empty).
    pub description: String,
    /// Frontmatter tags.
    pub tags: Vec<String>,
    /// Body headings, for heading-level hits.
    pub headings: Vec<String>,
    /// The concept `type`.
    pub type_: String,
    /// Trust tier, for `tier:` filters.
    pub tier: TrustTier,
    /// Lifecycle status, for `status:` filters.
    pub status: Status,
    /// Whether the concept is stale today, for `is:stale`.
    pub stale: bool,
    /// Whether the concept has broken outgoing links, for `is:broken`.
    pub broken: bool,
}

/// The precomputed omnisearch index over a bundle's concepts.
#[derive(Clone, Debug, Default)]
pub struct SearchIndex {
    /// One entry per concept, in bundle order.
    pub entries: Vec<SearchEntry>,
}

/// A single omnisearch result.
#[derive(Clone, Debug)]
pub struct SearchHit {
    /// The concept the hit points at.
    pub id: ConceptId,
    /// The heading within the concept, when the hit is heading-level.
    pub heading: Option<String>,
    /// Fuzzy score (higher is better).
    pub score: i32,
    /// Char indices of the query match within [`SearchHit::label`].
    pub indices: Vec<usize>,
    /// The text the match was scored against.
    pub label: String,
}

/// A single body-text hit from [`search_bodies`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyHit {
    /// The concept whose body matched.
    pub id: ConceptId,
    /// The 1-based body line that matched.
    pub line: usize,
    /// The matching line, windowed around the match for display.
    pub snippet: String,
    /// Byte offset of the match within the snippet, for highlighting.
    pub start: usize,
}

/// A structured filter parsed from the shared query syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    /// `#tag`
    Tag(String),
    /// `type:Policy`
    Type(String),
    /// `tier:unverified`
    Tier(TrustTier),
    /// `status:draft`
    Status(String),
    /// `is:stale`
    Stale,
    /// `is:broken`
    Broken,
}

/// A parsed query: free text plus zero or more filters.
#[derive(Clone, Debug, Default)]
pub struct Query {
    /// The fuzzy free-text part.
    pub text: String,
    /// The structured filters.
    pub filters: Vec<Filter>,
}

impl Query {
    /// Parses the shared query syntax: whitespace-separated terms, where
    /// `#x`, `type:x`, `tier:x`, `status:x`, `is:stale`, and `is:broken`
    /// become filters and everything else joins the fuzzy text.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let mut text_terms: Vec<&str> = Vec::new();
        let mut filters = Vec::new();
        for term in raw.split_whitespace() {
            if let Some(tag) = term.strip_prefix('#') {
                if !tag.is_empty() {
                    filters.push(Filter::Tag(tag.to_string()));
                    continue;
                }
            } else if let Some(t) = term.strip_prefix("type:") {
                filters.push(Filter::Type(t.to_string()));
                continue;
            } else if let Some(t) = term.strip_prefix("tier:") {
                if let Ok(tier) = TrustTier::from_str(t) {
                    filters.push(Filter::Tier(tier));
                    continue;
                }
            } else if let Some(s) = term.strip_prefix("status:") {
                filters.push(Filter::Status(s.to_string()));
                continue;
            } else if term == "is:stale" {
                filters.push(Filter::Stale);
                continue;
            } else if term == "is:broken" {
                filters.push(Filter::Broken);
                continue;
            }
            text_terms.push(term);
        }
        Self {
            text: text_terms.join(" "),
            filters,
        }
    }

    /// Whether an entry passes every filter.
    #[must_use]
    pub fn matches_filters(&self, entry: &SearchEntry) -> bool {
        self.filters.iter().all(|f| match f {
            Filter::Tag(tag) => entry.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)),
            Filter::Type(t) => entry.type_.eq_ignore_ascii_case(t),
            Filter::Tier(tier) => entry.tier == *tier,
            Filter::Status(s) => entry.status.as_str().eq_ignore_ascii_case(s),
            Filter::Stale => entry.stale,
            Filter::Broken => entry.broken,
        })
    }
}

impl SearchIndex {
    /// Builds the index over every concept in `bundle`.
    ///
    /// `today` is the date staleness is evaluated against (`None` means the
    /// current UTC date, with the same fallback the studio snapshot uses).
    /// Every field is derived directly from the bundle, so this is the one
    /// canonical index — the studio snapshot, the `okf search` subcommand,
    /// and the generated site's client all consume it.
    #[must_use]
    pub fn build(bundle: &Bundle, today: Option<Date>) -> Self {
        let today = today.or_else(Date::today_utc).unwrap_or(Date {
            year: 2026,
            month: 1,
            day: 1,
        });
        let entries = bundle
            .concepts()
            .iter()
            .map(|concept| {
                let links = bundle.links_from(&concept.id);
                SearchEntry {
                    id: concept.id.clone(),
                    title: concept.display_title(),
                    description: concept
                        .document
                        .frontmatter
                        .description()
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or_default(),
                    tags: concept.document.frontmatter.tags(),
                    headings: crate::extract_headings(&concept.document.body)
                        .iter()
                        .map(|h| h.text.to_string())
                        .collect(),
                    type_: concept
                        .type_()
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or_default(),
                    tier: concept.trust_tier(),
                    status: concept.status(),
                    stale: concept.is_stale_on(today),
                    broken: links.iter().any(|l| !l.exists),
                }
            })
            .collect();
        Self { entries }
    }

    /// Runs a query over the index, returning at most `limit` hits, best
    /// first. An empty free-text query returns every entry passing the
    /// filters, in index order.
    #[must_use]
    pub fn search(&self, raw_query: &str, limit: usize) -> Vec<SearchHit> {
        let query = Query::parse(raw_query);
        let mut hits: Vec<SearchHit> = Vec::new();
        for entry in &self.entries {
            if !query.matches_filters(entry) {
                continue;
            }
            if query.text.is_empty() {
                hits.push(SearchHit {
                    id: entry.id.clone(),
                    heading: None,
                    score: 0,
                    indices: Vec::new(),
                    label: entry.id.to_string(),
                });
                continue;
            }
            // Concept-level hit: best score across id, title, description,
            // and tags.
            let id_str = entry.id.to_string();
            let mut best: Option<SearchHit> = None;
            let candidates: Vec<&str> = std::iter::once(id_str.as_str())
                .chain(std::iter::once(entry.title.as_str()))
                .chain(std::iter::once(entry.description.as_str()))
                .chain(entry.tags.iter().map(String::as_str))
                .collect();
            for hay in candidates {
                if let Some((score, indices)) = fuzzy_match(&query.text, hay)
                    && best.as_ref().is_none_or(|b| score > b.score)
                {
                    best = Some(SearchHit {
                        id: entry.id.clone(),
                        heading: None,
                        score,
                        indices,
                        label: hay.to_string(),
                    });
                }
            }
            if let Some(hit) = best {
                hits.push(hit);
            }
            // Heading-level hits are separate result rows.
            for heading in &entry.headings {
                if let Some((score, indices)) = fuzzy_match(&query.text, heading) {
                    hits.push(SearchHit {
                        id: entry.id.clone(),
                        heading: Some(heading.clone()),
                        // Slightly discounted so the concept row leads.
                        score: score - 1,
                        indices,
                        label: heading.clone(),
                    });
                }
            }
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
        hits.truncate(limit);
        hits
    }
}

/// The window of context shown around a body match: characters of the
/// matching line kept on each side of the match.
const SNIPPET_CONTEXT: usize = 60;

/// Searches every concept's body text for the free-text part of the query,
/// returning at most `limit` hits, in bundle order then line order.
///
/// The filter terms of the shared query syntax (`#tag`, `type:`, …) are
/// honored: they narrow which concepts are scanned, and the remaining free
/// text is matched as a smart-case substring per line — uppercase in the
/// query forces an exact match, mirroring [`fuzzy_match`]. An empty
/// free-text query scans nothing, matching the index search's behavior.
///
/// Bodies are scanned rather than indexed: they are already resident in the
/// loaded [`Bundle`], so precomputing would only copy memory.
#[must_use]
pub fn search_bodies(bundle: &Bundle, raw_query: &str, limit: usize) -> Vec<BodyHit> {
    search_bodies_indexed(bundle, &SearchIndex::build(bundle, None), raw_query, limit)
}

/// [`search_bodies`] over a prebuilt index, for callers that already hold one
/// (the studio snapshot, the `okf search` subcommand, the site generator) so
/// per-keystroke queries never re-derive the index.
#[must_use]
pub fn search_bodies_indexed(
    bundle: &Bundle,
    index: &SearchIndex,
    raw_query: &str,
    limit: usize,
) -> Vec<BodyHit> {
    let query = Query::parse(raw_query);
    if query.text.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for entry in &index.entries {
        if !query.matches_filters(entry) {
            continue;
        }
        let Some(concept) = bundle.get(&entry.id) else {
            continue;
        };
        for (line_no, line) in concept.document.body.lines().enumerate() {
            if let Some(start) = find_substring_smart_case(&query.text, line) {
                let trimmed = line.trim_start();
                let lead = line.len() - trimmed.len();
                let start_in_trimmed = start.saturating_sub(lead);
                let (snippet, offset) = window(trimmed, start_in_trimmed, query.text.len());
                hits.push(BodyHit {
                    id: entry.id.clone(),
                    line: line_no + 1,
                    snippet,
                    start: offset,
                });
                if hits.len() >= limit {
                    return hits;
                }
            }
        }
    }
    hits
}

/// Smart-case substring search: the query matches case-insensitively unless
/// it contains uppercase, in which case it must match exactly (mirroring
/// [`fuzzy_match`]'s smart case). Returns the byte offset of the match.
fn find_substring_smart_case(query: &str, line: &str) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    if query.chars().any(char::is_uppercase) {
        line.find(query)
    } else {
        line.to_lowercase()
            .find(&query.to_lowercase())
            .and_then(|byte| line.get(..byte).map(str::len))
    }
}

/// Windows `line` around the match at `start` (byte offset), returning the
/// snippet and the match's byte offset within it. Leading indentation is
/// already trimmed by the caller.
fn window(line: &str, start: usize, len: usize) -> (String, usize) {
    let half = SNIPPET_CONTEXT;
    let from = start.saturating_sub(half);
    let to = (start + len + half).min(line.len());
    // Snap to char boundaries so slicing cannot panic on multibyte text.
    let mut from = from.min(line.len());
    while !line.is_char_boundary(from) {
        from += 1;
    }
    let mut to = to.min(line.len());
    while to < line.len() && !line.is_char_boundary(to) {
        to += 1;
    }
    let mut prefix = String::new();
    if from > 0 {
        prefix.push('…');
    }
    let mut suffix = String::new();
    if to < line.len() {
        suffix.push('…');
    }
    let offset = prefix.len() + (start - from);
    (
        format!(
            "{prefix}{}{suffix}",
            line[from..to].trim_matches(|c| c == '\t' || c == ' ')
        ),
        offset,
    )
}

const BONUS_BOUNDARY: i32 = 16;
const BONUS_CAMEL: i32 = 12;
const BONUS_CONSECUTIVE: i32 = 8;
const BONUS_FIRST_CHAR: i32 = 20;
const PENALTY_GAP_START: i32 = -3;
const PENALTY_GAP_EXTEND: i32 = -1;
const MATCH_SCORE: i32 = 16;

/// Scores `query` against `haystack` with a Smith-Waterman-style alignment.
///
/// Subsequence match with affine gap penalties, plus bonuses at the start
/// of the haystack, after `/ _ - . :` separators and whitespace, and at
/// camelCase boundaries — tuned so `pte` finds `policies/travel_expenses`
/// via segment initials.
///
/// Case-insensitive by default; a query char written in uppercase must match
/// exactly (smart-case). Returns the score and the matched char indices, or
/// `None` when `query` is not a subsequence of `haystack`.
#[must_use]
pub fn fuzzy_match(query: &str, haystack: &str) -> Option<(i32, Vec<usize>)> {
    const NEG: i32 = i32::MIN / 4;

    let query_chars: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
    let haystack_chars: Vec<char> = haystack.chars().collect();
    if query_chars.is_empty() {
        return Some((0, Vec::new()));
    }
    if query_chars.len() > haystack_chars.len() {
        return None;
    }

    let eq = |qc: char, hc: char| {
        if qc.is_uppercase() {
            qc == hc
        } else {
            qc.to_lowercase().eq(hc.to_lowercase())
        }
    };
    let bonus_at = |idx: usize| -> i32 {
        if idx == 0 {
            return BONUS_FIRST_CHAR;
        }
        let prev = haystack_chars[idx - 1];
        if matches!(prev, '/' | '_' | '-' | '.' | ':') || prev.is_whitespace() {
            BONUS_BOUNDARY
        } else if prev.is_lowercase() && haystack_chars[idx].is_uppercase() {
            BONUS_CAMEL
        } else {
            0
        }
    };

    let (q_len, h_len) = (query_chars.len(), haystack_chars.len());
    // dp[i][j]: best score with query_chars[i] aligned at haystack_chars[j]; parent[i][j] is the
    // position of query_chars[i-1] on that best path.
    let mut dp = vec![vec![NEG; h_len]; q_len];
    let mut parent = vec![vec![usize::MAX; h_len]; q_len];

    for (j, &hc) in haystack_chars.iter().enumerate() {
        if eq(query_chars[0], hc) {
            // A leading gap is free: matching later in the haystack is not
            // penalized, only rewarded less when it lacks a boundary bonus.
            dp[0][j] = MATCH_SCORE + bonus_at(j);
        }
    }
    for i in 1..q_len {
        // best_prev = max over k <= j-2 of dp[i-1][k] plus the affine gap
        // cost of the cells between k and the current j; every candidate
        // decays at the same rate, so a running max suffices.
        let mut best_prev = NEG;
        let mut best_prev_j = usize::MAX;
        for j in i..h_len {
            if best_prev > NEG {
                best_prev += PENALTY_GAP_EXTEND;
            }
            if j >= 2 && dp[i - 1][j - 2] > NEG {
                let candidate = dp[i - 1][j - 2] + PENALTY_GAP_START;
                if candidate > best_prev {
                    best_prev = candidate;
                    best_prev_j = j - 2;
                }
            }
            if !eq(query_chars[i], haystack_chars[j]) {
                continue;
            }
            let consecutive = if dp[i - 1][j - 1] > NEG {
                dp[i - 1][j - 1] + MATCH_SCORE + BONUS_CONSECUTIVE + bonus_at(j)
            } else {
                NEG
            };
            let gapped = if best_prev > NEG {
                best_prev + MATCH_SCORE + bonus_at(j)
            } else {
                NEG
            };
            if consecutive >= gapped {
                if consecutive > NEG {
                    dp[i][j] = consecutive;
                    parent[i][j] = j - 1;
                }
            } else {
                dp[i][j] = gapped;
                parent[i][j] = best_prev_j;
            }
        }
    }

    let (mut best_j, mut best_score) = (usize::MAX, NEG);
    for (j, &score) in dp[q_len - 1].iter().enumerate() {
        if score > best_score {
            best_score = score;
            best_j = j;
        }
    }
    if best_j == usize::MAX {
        return None;
    }
    let mut indices = vec![0usize; q_len];
    let mut cursor = best_j;
    for i in (0..q_len).rev() {
        indices[i] = cursor;
        if i > 0 {
            cursor = parent[i][cursor];
        }
    }
    Some((best_score, indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_segment_initials() {
        let (score, idx) = fuzzy_match("pte", "policies/travel_expenses").unwrap();
        assert!(score > 0);
        assert_eq!(idx, vec![0, 9, 16]);
    }

    #[test]
    fn prefers_boundary_matches() {
        let (loose, _) = fuzzy_match("te", "notes").unwrap();
        let (boundary, _) = fuzzy_match("te", "travel_expenses").unwrap();
        assert!(boundary > loose);
    }

    #[test]
    fn non_subsequence_is_none() {
        assert!(fuzzy_match("xyz", "policies").is_none());
        assert!(fuzzy_match("aa", "a").is_none());
    }

    #[test]
    fn smart_case() {
        assert!(fuzzy_match("Pol", "policies").is_none());
        assert!(fuzzy_match("pol", "Policies").is_some());
    }

    #[test]
    fn query_syntax_parses_filters() {
        let q = Query::parse("trav #hr type:Policy tier:unverified is:stale is:broken");
        assert_eq!(q.text, "trav");
        assert_eq!(q.filters.len(), 5);
        assert!(q.filters.contains(&Filter::Tag("hr".into())));
        assert!(q.filters.contains(&Filter::Type("Policy".into())));
        assert!(q.filters.contains(&Filter::Tier(TrustTier::Unverified)));
        assert!(q.filters.contains(&Filter::Stale));
        assert!(q.filters.contains(&Filter::Broken));
    }

    #[test]
    fn smart_case_substring_finds_exact_for_uppercase_query() {
        assert_eq!(
            find_substring_smart_case("Mileage", "the Mileage rate"),
            Some(4)
        );
        assert_eq!(
            find_substring_smart_case("mileage", "The MILEAGE rate"),
            Some(4)
        );
        // Lowercase query, no match.
        assert_eq!(find_substring_smart_case("xyz", "the Mileage rate"), None);
    }

    #[test]
    fn window_snips_long_lines_with_ellipses() {
        let line = "a".repeat(300);
        let (snippet, offset) = window(&line, 150, 1);
        assert!(snippet.starts_with('…'));
        assert!(snippet.ends_with('…'));
        assert_eq!(offset, 63);
        // Short line: no ellipses, offset preserved.
        let (snippet, offset) = window("short line", 6, 4);
        assert_eq!(snippet, "short line");
        assert_eq!(offset, 6);
    }

    #[test]
    fn window_snaps_to_char_boundaries() {
        // Multibyte: start 2 bytes in is not a boundary; must not panic.
        let line = "héllo wörld ünïcode";
        let (snippet, offset) = window(line, 1, 1);
        assert!(!snippet.is_empty());
        assert!(offset < snippet.len());
    }
}
