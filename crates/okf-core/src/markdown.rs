//! Markdown scanning, link rewriting, heading extraction, and anchor slugs.
//!
//! OKF documents use Markdown with YAML frontmatter. This module provides pure-Rust
//! Markdown utilities for inspecting and transforming Markdown content:
//!
//! - [`heading_slug`]: Generates kebab-case URL anchor slugs matching GitHub/CommonMark conventions.
//! - [`extract_headings`]: Extracts all headings in document order, skipping fenced code blocks.
//! - [`rewrite_markdown_links`]: Rewrites inline links while ignoring code blocks and inline code.
//! - [`expand_wikilinks`]: Expands `[[heading]]` shorthand into standard anchor links.

use crate::links::Link;
use std::fmt::Write as _;

/// A parsed markdown heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownHeading<'a> {
    /// Heading level (1 for `#`, 2 for `##`, up to 6).
    pub level: usize,
    /// The trimmed heading text after `#...# `.
    pub text: &'a str,
    /// 1-based line number within the markdown text.
    pub line_num: usize,
    /// 0-based line index within lines.
    pub line_index: usize,
}

impl MarkdownHeading<'_> {
    /// Derives a standard Markdown anchor slug from this heading's text.
    #[must_use]
    pub fn slug(&self) -> String {
        heading_slug(self.text)
    }
}

/// Derives a standard Markdown anchor slug from a heading text.
///
/// Example: `"## Pricing Tiers"` -> `"pricing-tiers"`
#[must_use]
pub fn heading_slug(text: &str) -> String {
    let clean = text.trim().trim_start_matches('#').trim();
    let mut slug = String::with_capacity(clean.len());
    for c in clean.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if (c.is_whitespace() || c == '-' || c == '_') && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// A per-document slug allocator that disambiguates duplicate heading
/// slugs with `-1`, `-2`, … suffixes, matching GitHub's anchor behavior.
///
/// One allocator must own every heading of a document (page renderers and
/// search indexes alike) so emitted `id` attributes and index anchors
/// cannot drift apart.
#[derive(Clone, Debug, Default)]
pub struct SlugAllocator {
    used: std::collections::HashSet<String>,
}

impl SlugAllocator {
    /// Allocates the slug for `text`, suffixing when the base slug is
    /// already taken. An empty heading text yields an empty slug that is
    /// never registered, mirroring the permissive handling of headings
    /// with no anchorable characters.
    #[must_use]
    pub fn allocate(&mut self, text: &str) -> String {
        let base = heading_slug(text);
        if base.is_empty() {
            return base;
        }
        let mut slug = base.clone();
        let mut n: usize = 0;
        while self.used.contains(&slug) {
            n += 1;
            slug = format!("{base}-{n}");
        }
        self.used.insert(slug.clone());
        slug
    }

    /// The already-allocated slug for `text` without registering a new one:
    /// the suffix a *second* allocator would hand out for the same text.
    /// Useful for index builders that must mirror a page's allocations
    /// without mutating them.
    #[must_use]
    pub fn peek_duplicate(&self, text: &str) -> String {
        let base = heading_slug(text);
        let mut slug = base.clone();
        let mut n: usize = 0;
        while self.used.contains(&slug) {
            n += 1;
            slug = format!("{base}-{n}");
        }
        slug
    }
}

/// Parses a single line as an ATX heading (`# ` through `###### `).
///
/// Returns `(level, text)` if the line is a heading, or `None` otherwise.
#[must_use]
pub fn parse_heading_line(line: &str) -> Option<(usize, &str)> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let count = t.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&count) && t[count..].starts_with(' ') {
        Some((count, t[count..].trim()))
    } else {
        None
    }
}

/// Extracts all markdown headings in document order, ignoring headings inside fenced code blocks.
#[must_use]
pub fn extract_headings(markdown: &str) -> Vec<MarkdownHeading<'_>> {
    let mut headings = Vec::new();
    let mut fence: Option<char> = None;

    for (i, line) in markdown.lines().enumerate() {
        let trimmed_start = line.trim_start();
        if let Some(f) = fence {
            if trimmed_start.starts_with(&f.to_string().repeat(3)) {
                fence = None;
            }
            continue;
        }
        if trimmed_start.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if trimmed_start.starts_with("~~~") {
            fence = Some('~');
            continue;
        }

        if let Some((level, text)) = parse_heading_line(line) {
            headings.push(MarkdownHeading {
                level,
                text,
                line_num: i + 1,
                line_index: i,
            });
        }
    }

    headings
}

/// Checks whether a heading text matches a search query by title, slug, or normalized spacing.
#[must_use]
pub fn matches_heading(heading_text: &str, query: &str) -> bool {
    let clean_query = query.trim().trim_start_matches('#').trim();
    let query_slug = heading_slug(clean_query);
    let title = heading_text.trim().trim_start_matches('#').trim();

    title.eq_ignore_ascii_case(clean_query)
        || heading_slug(title) == query_slug
        || title
            .replace(['-', '_'], " ")
            .eq_ignore_ascii_case(&clean_query.replace(['-', '_'], " "))
}

/// Removes an optional `"title"` (or `'title'`) suffix from a link destination.
#[must_use]
pub fn strip_title(dest: &str) -> String {
    let d = dest.trim();
    if let Some(idx) = d.find([' ', '\t']) {
        let (url, rest) = d.split_at(idx);
        let rest = rest.trim_start();
        if rest.starts_with('"') || rest.starts_with('\'') {
            return url.to_string();
        }
    }
    d.to_string()
}

/// Normalizes a raw link destination.
///
/// Unwraps the `CommonMark` `<...>` form, which is how a destination is allowed
/// to contain spaces, and otherwise removes an optional title suffix.
#[must_use]
pub fn clean_destination(dest: &str) -> String {
    let d = dest.trim();
    if let Some(rest) = d.strip_prefix('<')
        && let Some(end) = rest.find('>')
    {
        return rest[..end].to_string();
    }
    strip_title(d)
}

/// Extracts the title suffix part (e.g. ` "My Title"`) from a destination string.
#[must_use]
pub fn extract_title_suffix(dest: &str) -> String {
    let d = dest.trim();
    let after_dest = if let Some(rest) = d.strip_prefix('<')
        && let Some(end) = rest.find('>')
    {
        &rest[end + 1..]
    } else if let Some(idx) = d.find([' ', '\t']) {
        &d[idx..]
    } else {
        ""
    };
    let trimmed_suffix = after_dest.trim();
    if trimmed_suffix.starts_with('"') || trimmed_suffix.starts_with('\'') {
        format!(" {trimmed_suffix}")
    } else {
        String::new()
    }
}

/// Parses `[[target]]` starting at `start` (the first `[`). Returns the inner
/// target and the index just past the closing `]]`.
///
/// The target must be non-empty (after trimming) and contain no `[`, `]`, or
/// `\` — any of those means something else was meant (a nested link, a
/// reference-style definition, an escaped bracket), and the construct is
/// left for other markdown rules. Keeping targets bracket-free also means an
/// expansion can never produce unbalanced link text.
#[must_use]
pub fn parse_wikilink(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start + 1) != Some(&'[') {
        return None;
    }
    let mut i = start + 2;
    let mut end = None;
    while i + 1 < chars.len() {
        match chars[i] {
            ']' if chars[i + 1] == ']' => {
                end = Some(i);
                break;
            }
            '[' | '\\' | ']' => return None,
            _ => i += 1,
        }
    }
    let end = end?;
    let target: String = chars[start + 2..end].iter().collect();
    (!target.trim().is_empty()).then_some((target, end + 2))
}

/// Expands `[[target]]` wikilinks into standard markdown links.
///
/// OKF bodies use standard markdown links (spec §6.1); `[[target]]` is
/// producer-side shorthand this toolchain expands, so every consumer sees
/// one link syntax:
///
/// - A bare target names a **heading of the current document**:
///   `[[Pricing Tiers]]` becomes `[Pricing Tiers](#pricing-tiers)`, the
///   same slug `heading_slug` produces, so emitted heading ids resolve.
/// - `[[#Pricing Tiers]]` is the explicit-anchor spelling of the same.
/// - A target that already names a destination — an external URL, an
///   anchor-bearing or rooted path, an `.md` file — is linked verbatim, with
///   any fragment slugified, so the ordinary link machinery resolves it:
///   `[[concepts/plan#Milestone 2]]` becomes
///   `[concepts/plan#Milestone 2](concepts/plan#milestone-2)`.
///
/// Fragments are slugified rather than kept literal because page renderers
/// derive heading ids from `heading_slug`; the transform is idempotent, so
/// an author who already wrote `[[#some-heading]]` gets the same link.
///
/// Fenced code blocks and inline code spans are left untouched, matching
/// [`rewrite_markdown_links`]. Returns the expanded text and the number of
/// wikilinks expanded.
///
/// # Examples
///
/// ```
/// # use okf_core::markdown::expand_wikilinks;
/// let (out, n) = expand_wikilinks("See [[Pricing Tiers]] and [[#Details]].");
/// assert_eq!(out, "See [Pricing Tiers](#pricing-tiers) and [Details](#details).");
/// assert_eq!(n, 2);
/// ```
#[must_use]
pub fn expand_wikilinks(body: &str) -> (String, usize) {
    let mut out_lines = Vec::new();
    let mut total = 0;
    let mut fence: Option<char> = None;

    for line in body.lines() {
        let trimmed_start = line.trim_start();
        if let Some(f) = fence {
            if trimmed_start.starts_with(&f.to_string().repeat(3)) {
                fence = None;
            }
            out_lines.push(line.to_string());
            continue;
        }
        if trimmed_start.starts_with("```") {
            fence = Some('`');
            out_lines.push(line.to_string());
            continue;
        }
        if trimmed_start.starts_with("~~~") {
            fence = Some('~');
            out_lines.push(line.to_string());
            continue;
        }

        let (new_line, count) = expand_wikilinks_in_line(line);
        total += count;
        out_lines.push(new_line);
    }

    let mut result = out_lines.join("\n");
    if body.ends_with('\n') {
        result.push('\n');
    }
    (result, total)
}

/// Expands the wikilinks in one (non-fenced) line. Inline code spans are
/// blanked in a *detection copy* (same length, same positions) so wikilinks
/// inside them are left verbatim, while the output keeps the original text.
/// Lines carrying no `[[` at all pass through without per-char work:
/// renderers call this per body, every frame.
fn expand_wikilinks_in_line(line: &str) -> (String, usize) {
    if !line.contains("[[") {
        return (line.to_string(), 0);
    }
    let chars: Vec<char> = line.chars().collect();
    let blank_chars: Vec<char> = blank_inline_code(line).chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut count = 0;
    let mut i = 0;

    while i < chars.len() {
        if blank_chars[i] == '['
            && !is_escaped(&blank_chars, i)
            && let Some((target, next)) = parse_wikilink(&blank_chars, i)
        {
            out.push_str(&expand_one_wikilink(&target));
            count += 1;
            i = next;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }

    (out, count)
}

/// The markdown link one `[[target]]` expands to.
///
/// - A bare target names a heading of the current document:
///   `[[Pricing Tiers]]` -> `[Pricing Tiers](#pricing-tiers)`.
/// - `[[#fragment]]` is the explicit spelling of the same.
/// - A target with a path part (`[[concepts/plan]]`,
///   `[[concepts/plan#Milestone 2]]`, `[[other.md]]`) is a concept link;
///   the path is linked verbatim and the fragment slugified, and the
///   ordinary link machinery resolves it.
/// - An external URL is linked verbatim, untouched.
fn expand_one_wikilink(target: &str) -> String {
    let trimmed = target.trim();
    if crate::links::is_external(trimmed) {
        return format!("[{trimmed}]({trimmed})");
    }
    let (dest_body, display) = if let Some((path_part, fragment)) = trimmed.split_once('#') {
        // The fragment is a heading name, slugified so the link lands on
        // the id a page renderer emits; already-slug input is unchanged.
        let display = fragment.trim().to_string();
        let dest_body = if path_part.is_empty() {
            format!("#{}", heading_slug(fragment))
        } else {
            format!("{path_part}#{}", heading_slug(fragment))
        };
        (dest_body, display)
    } else if trimmed.contains('/') || trimmed.contains(".md") {
        // A concept path as written; resolution is the link machinery's job.
        (trimmed.to_string(), trimmed.to_string())
    } else {
        (format!("#{}", heading_slug(trimmed)), trimmed.to_string())
    };
    // A destination carrying spaces needs the `<...>` form to survive
    // markdown parsing, matching `rewrite_markdown_links`.
    if dest_body.chars().any(char::is_whitespace) {
        format!("[{display}](<{dest_body}>)")
    } else {
        format!("[{display}]({dest_body})")
    }
}

/// Whether the character at `index` is preceded by an odd number of
/// backslashes, and is therefore escaped in Markdown.
#[must_use]
pub const fn is_escaped(chars: &[char], index: usize) -> bool {
    let mut backslashes = 0;
    let mut i = index;
    while i > 0 && chars[i - 1] == '\\' {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1
}

/// Attempts to parse `[text](dest)` starting at `start` (the `[`). Returns the
/// text, destination, and index just past the closing `)`.
#[must_use]
pub fn parse_inline_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let mut i = start + 1;
    let mut depth = 1;
    let text_start = i;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1, // skip escaped char
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 || i >= chars.len() {
        return None;
    }
    let text: String = chars[text_start..i].iter().collect();

    let mut j = i + 1;
    if j >= chars.len() || chars[j] != '(' {
        return None;
    }
    j += 1;
    let dest_start = j;
    let mut paren = 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 1,
            '(' => paren += 1,
            ')' => {
                paren -= 1;
                if paren == 0 {
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    if paren != 0 || j >= chars.len() {
        return None;
    }
    let dest: String = chars[dest_start..j].iter().collect();
    Some((text, dest, j + 1))
}

/// Replaces inline code spans (backtick-delimited) with spaces so links/footnotes inside
/// them are not extracted.
#[must_use]
pub fn blank_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Returns the body's lines, as `(1-based line number, text)`, with fenced
/// code blocks removed and inline code spans blanked out.
#[must_use]
pub fn code_free_lines(body: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    for (i, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(f) = fence {
            if trimmed.starts_with(&f.to_string().repeat(3)) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        out.push((i + 1, blank_inline_code(line)));
    }
    out
}

/// Action to perform on a detected markdown link during rewrite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkRewriteAction {
    /// Keep the link destination as-is.
    Keep,
    /// Rewrite the destination URL to a new target (preserving title and brackets).
    Rewrite(String),
    /// Unlink: replace `[Text](dest)` with plain `Text`.
    Unlink,
}

/// Rewrites inline markdown links in a document body using a callback function.
///
/// Preserves fenced code blocks and inline code spans untouched.
pub fn rewrite_markdown_links<F>(body: &str, mut rewrite_fn: F) -> (String, usize)
where
    F: FnMut(&Link, &str) -> LinkRewriteAction,
{
    let mut out_lines = Vec::new();
    let mut total_rewritten = 0;
    let mut fence: Option<char> = None;

    for line in body.lines() {
        let trimmed_start = line.trim_start();
        if let Some(f) = fence {
            if trimmed_start.starts_with(&f.to_string().repeat(3)) {
                fence = None;
            }
            out_lines.push(line.to_string());
            continue;
        }
        if trimmed_start.starts_with("```") {
            fence = Some('`');
            out_lines.push(line.to_string());
            continue;
        }
        if trimmed_start.starts_with("~~~") {
            fence = Some('~');
            out_lines.push(line.to_string());
            continue;
        }

        let (new_line, count) = rewrite_line_links(line, &mut rewrite_fn);
        total_rewritten += count;
        out_lines.push(new_line);
    }

    let mut result = out_lines.join("\n");
    if body.ends_with('\n') {
        result.push('\n');
    }
    (result, total_rewritten)
}

fn rewrite_line_links<F>(line_text: &str, rewrite_fn: &mut F) -> (String, usize)
where
    F: FnMut(&Link, &str) -> LinkRewriteAction,
{
    let chars: Vec<char> = line_text.chars().collect();
    let mut out = String::with_capacity(line_text.len());
    let mut count = 0;
    let mut i = 0;
    let mut in_inline_code = false;

    while i < chars.len() {
        if chars[i] == '`' && !is_escaped(&chars, i) {
            in_inline_code = !in_inline_code;
            out.push('`');
            i += 1;
            continue;
        }

        if !in_inline_code
            && chars[i] == '['
            && !is_escaped(&chars, i)
            && let Some((text, dest_raw, next_i)) = parse_inline_link(&chars, i)
        {
            let target = clean_destination(&dest_raw);
            let link = Link {
                text: text.clone(),
                kind: Link::classify(&target),
                target,
            };

            match rewrite_fn(&link, &dest_raw) {
                LinkRewriteAction::Keep => {
                    let raw_slice: String = chars[i..next_i].iter().collect();
                    out.push_str(&raw_slice);
                }
                LinkRewriteAction::Rewrite(new_dest) => {
                    let title_suffix = extract_title_suffix(&dest_raw);
                    let formatted_dest = if new_dest.contains(' ') && !new_dest.starts_with('<') {
                        format!("<{new_dest}>")
                    } else {
                        new_dest
                    };
                    let _ = write!(out, "[{text}]({formatted_dest}{title_suffix})");
                    count += 1;
                }
                LinkRewriteAction::Unlink => {
                    out.push_str(&text);
                    count += 1;
                }
            }
            i = next_i;
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_slug() {
        assert_eq!(heading_slug("Pricing Tiers"), "pricing-tiers");
        assert_eq!(heading_slug("## Deep Heading!"), "deep-heading");
        assert_eq!(
            heading_slug("Special_Chars & Symbols"),
            "special-chars-symbols"
        );
    }

    #[test]
    fn test_extract_headings() {
        let md = "\
# Top Heading

Some content.

```python
# Not a heading
pass
```

## Sub Heading

~~~bash
# Also not a heading
~~~

### Third Heading
";
        let headings = extract_headings(md);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Top Heading");
        assert_eq!(headings[0].slug(), "top-heading");

        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Sub Heading");
        assert_eq!(headings[1].slug(), "sub-heading");

        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[2].text, "Third Heading");
        assert_eq!(headings[2].slug(), "third-heading");
    }

    #[test]
    fn test_rewrite_markdown_links() {
        let body = "\
# Title

See [User Guide](../guides/user.md) and [Profile](/users/profile.md#info).
Also `[Code Link](../not/a/link.md)` should not change.

```python
# [Python Link](../ignored.md)
pass
```
";

        let (rewritten, count) = rewrite_markdown_links(body, |link, _| {
            if link.target.starts_with("../guides/user.md") {
                LinkRewriteAction::Rewrite("../../docs/user.md".to_string())
            } else if link.target.starts_with("/users/profile.md") {
                LinkRewriteAction::Unlink
            } else {
                LinkRewriteAction::Keep
            }
        });

        assert_eq!(count, 2);
        assert!(rewritten.contains("[User Guide](../../docs/user.md)"));
        assert!(rewritten.contains("and Profile."));
        assert!(rewritten.contains("`[Code Link](../not/a/link.md)`"));
        assert!(rewritten.contains("# [Python Link](../ignored.md)"));
    }

    #[test]
    fn test_expand_wikilinks() {
        let body = "# Title\n\nSee [[Pricing Tiers]] and [[#Explicit]] next.\n\n`[[not a link]]`\n\n```python\n[[in a fence]]\n```\n";
        let (out, count) = expand_wikilinks(body);
        assert_eq!(count, 2);
        assert!(out.contains("[Pricing Tiers](#pricing-tiers)"));
        assert!(out.contains("[Explicit](#explicit)"));
        assert!(out.contains("`[[not a link]]`"));
        assert!(out.contains("[[in a fence]]"));
    }

    #[test]
    fn wikilinks_expand_to_heading_anchors() {
        // Every accepted form, one line each.
        let (out, n) = expand_wikilinks(
            "[[Pricing]] [[#Notes]] [[plans/roadmap]] [[plans/roadmap#Milestone 2]] \
             [[plans/roadmap.md#Deploy]] [[https://example.com/a]]",
        );
        assert_eq!(n, 6);
        assert!(out.contains("[Pricing](#pricing)"));
        assert!(out.contains("[Notes](#notes)"));
        assert!(out.contains("[plans/roadmap](plans/roadmap)"));
        // A path-plus-fragment target displays the heading name, like the
        // `[Foo](foo.md#foo-heading)` authored form it stands for.
        assert!(
            out.contains("[Milestone 2](plans/roadmap#milestone-2)"),
            "{out}"
        );
        assert!(out.contains("[Deploy](plans/roadmap.md#deploy)"), "{out}");
        assert!(out.contains("[https://example.com/a](https://example.com/a)"));
    }

    #[test]
    fn wikilinks_reject_brackets_and_escaped_openers() {
        // Nested brackets, a stray closing bracket, and an escaped `[[` are
        // not wikilinks and stay verbatim.
        let (out, n) = expand_wikilinks("[[a [b]] [[a] [[b \\[[escaped]]");
        assert_eq!(n, 0);
        assert_eq!(out, "[[a [b]] [[a] [[b \\[[escaped]]");
    }

    #[test]
    fn wikilink_heading_targets_survive_reparse() {
        // The expansion must be a link `clean_destination` can parse back,
        // classified as an Anchor — the property every consumer relies on.
        let body = "# Pricing\n\nSee [[Pricing]] above.\n";
        let (expanded, _) = expand_wikilinks(body);
        let links = crate::links::extract_links(&expanded);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "Pricing");
        assert_eq!(links[0].target, "#pricing");
        assert_eq!(links[0].kind, crate::LinkKind::Anchor);
    }
}
