//! Markdown body → HTML: link rewriting, the mermaid intercept, and event
//! filtering before the classed-HTML writer.
//!
//! The pipeline, per concept body:
//!
//! 1. [`okf_core::markdown::rewrite_markdown_links`] maps `.md` targets to
//!    the generated site's `.html` URLs (relative, so the site deploys under
//!    any base path), leaving code fences and inline code untouched.
//! 2. A [`pulldown_cmark::Parser`] event stream (GitHub-flavoured markdown:
//!    tables, strikethrough, task lists, footnotes; raw `Html` events
//!    dropped — pulldown's `unsafe` option stays off) intercepts
//!    ` ```mermaid ` code blocks, emitting `<pre class="mermaid">` with the
//!    escaped source as a render-failure/noscript fallback.
//! 3. [`write_classed_html`] renders everything else, emitting a `md-*`
//!    class on every element so the Tailwind-compiled stylesheet (whose
//!    `source(none)` mode only defines classes named in `tailwind.css`)
//!    styles markdown bodies through component classes, like the rest of
//!    the site.

use okf_core::markdown::{LinkRewriteAction, rewrite_markdown_links};
use okf_core::{Bundle, ConceptId, LinkKind};
use pulldown_cmark::{Alignment, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::{HashMap, VecDeque};

/// What [`render`] produced for one body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderedBody {
    /// The rendered HTML.
    pub html: String,
    /// Whether at least one mermaid block was intercepted.
    pub has_mermaid: bool,
    /// Body headings with the ids injected into the HTML, for the TOC:
    /// `(level, text, id)`.
    pub headings: Vec<(usize, String, String)>,
}

/// Renders a concept body to HTML.
///
/// `prefix` is the `../`-chain from the page to the site root; `out_of` maps
/// a concept id to its output URL relative to the site root.
pub fn render(
    bundle: &Bundle,
    id: &ConceptId,
    body: &str,
    prefix: &str,
    out_of: &dyn Fn(&ConceptId) -> String,
) -> RenderedBody {
    // (1) Rewrite concept links to output URLs. The callback mirrors
    // okf-core's own resolution: prefer whichever percent-decoding reading
    // names a concept that exists, else the literal one (broken link).
    let rewritten = rewrite_markdown_links(body, |link, _| {
        let candidates = link.resolve_all(id);
        let target = candidates
            .iter()
            .find(|t| bundle.contains(t))
            .or_else(|| candidates.first());
        match (target, link.kind) {
            (Some(target), LinkKind::Absolute | LinkKind::Relative) => {
                LinkRewriteAction::Rewrite(format!("{prefix}{}", out_of(target)))
            }
            // External links, anchors, and malformed targets pass through.
            _ => LinkRewriteAction::Keep,
        }
    })
    .0;
    // GFM parity: tables, strikethrough, task lists, and footnotes. Raw HTML
    // stays off (see `Intercept`) so enabling these is safe.
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut intercept = Intercept::new(Parser::new_ext(&rewritten, opts));
    let mut html_out = String::new();
    write_classed_html(&mut html_out, intercept.by_ref());
    RenderedBody {
        html: html_out,
        has_mermaid: intercept.has_mermaid,
        headings: intercept.headings,
    }
}

/// The event adapter: drops raw `Html`/`InlineHtml` events (no passthrough
/// from bodies), rewrites ` ```mermaid ` code blocks into
/// `<pre class="mermaid">` elements, and injects GitHub-style heading ids
/// so the TOC anchors resolve.
///
/// Mermaid output is emitted through [`Event::Html`] — deliberate, since the
/// `<pre>` is *our* markup with *our* escaping, not producer HTML.
struct Intercept<'a, I> {
    iter: I,
    /// Events staged for the next `next()` calls.
    queued: VecDeque<Event<'a>>,
    /// Whether any mermaid block was seen.
    has_mermaid: bool,
    /// Headings with their injected ids, in document order.
    headings: Vec<(usize, String, String)>,
    /// Slugs already used, for GitHub-style `-1` disambiguation.
    used_slugs: std::collections::HashSet<String>,
}

impl<'a, I: Iterator<Item = Event<'a>>> Intercept<'a, I> {
    fn new(iter: I) -> Self {
        Self {
            iter,
            queued: VecDeque::new(),
            has_mermaid: false,
            headings: Vec::new(),
            used_slugs: std::collections::HashSet::new(),
        }
    }

    /// Whether the code-block info string names mermaid.
    fn is_mermaid(kind: &pulldown_cmark::CodeBlockKind) -> bool {
        matches!(kind, pulldown_cmark::CodeBlockKind::Fenced(info) if info
            .split(' ')
            .next()
            .unwrap_or("")
            .eq_ignore_ascii_case("mermaid"))
    }

    /// Buffers a heading's events to compute its slug, then re-emits the
    /// `Start` with the id set. Duplicate headings get GitHub's `-1`, `-2`,
    /// … disambiguation so every TOC anchor points at exactly one heading.
    fn inject_heading_id(&mut self, tag: Tag<'a>) -> Event<'a> {
        let Tag::Heading {
            level,
            id,
            classes,
            attrs,
        } = tag
        else {
            return Event::Start(tag);
        };
        let mut buffer: Vec<Event<'a>> = Vec::new();
        let mut text = String::new();
        for event in self.iter.by_ref() {
            match event {
                Event::End(TagEnd::Heading(end_level)) if end_level == level => {
                    // Re-queue the End: the writer (and any downstream
                    // consumer) still needs it to close the tag.
                    buffer.push(Event::End(TagEnd::Heading(end_level)));
                    break;
                }
                Event::Text(t) => {
                    text.push_str(&t);
                    buffer.push(Event::Text(t));
                }
                Event::Html(_) | Event::InlineHtml(_) => {}
                other => buffer.push(other),
            }
        }
        let mut slug = okf_core::markdown::heading_slug(&text);
        let mut n: usize = 0;
        while self.used_slugs.contains(&slug) && !slug.is_empty() {
            n += 1;
            slug = format!("{}-{n}", okf_core::markdown::heading_slug(&text));
        }
        if !slug.is_empty() {
            self.used_slugs.insert(slug.clone());
        }
        self.headings.push((level as usize, text, slug.clone()));
        self.queued.extend(buffer);
        Event::Start(Tag::Heading {
            level,
            id: (!slug.is_empty()).then_some(slug.into()).or(id),
            classes,
            attrs,
        })
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for Intercept<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.queued.pop_front() {
            return Some(event);
        }
        match self.iter.next()? {
            // No raw HTML passthrough from markdown bodies. HtmlBlock
            // start/end tags are push_html no-ops, so nothing else is needed.
            Event::Html(_) | Event::InlineHtml(_) => self.next(),
            Event::Start(Tag::CodeBlock(kind)) if Self::is_mermaid(&kind) => {
                // Drain the block's text and re-emit it as <pre class="mermaid">.
                let mut source = String::new();
                for event in self.iter.by_ref() {
                    match event {
                        Event::Text(t) => source.push_str(&t),
                        Event::Html(_) | Event::InlineHtml(_) => {}
                        Event::End(TagEnd::CodeBlock) => break,
                        other => self.queued.push_back(other),
                    }
                }
                self.has_mermaid = true;
                Some(Event::Html(
                    format!(r#"<pre class="mermaid">{}</pre>"#, escape_pre(&source)).into(),
                ))
            }
            Event::Start(tag @ Tag::Heading { .. }) => Some(self.inject_heading_id(tag)),
            other => Some(other),
        }
    }
}

/// Escapes `&`, `<`, `>` so diagram source survives inside the `<pre>` as
/// both the mermaid input and the render-failure/noscript fallback.
fn escape_pre(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// The classed-HTML writer: pulldown-cmark's `push_html` emits bare element
/// tags, but this site's stylesheet is compiled with Tailwind `source(none)`
/// — only classes `@apply`'d in `tailwind.css` exist as CSS. So the writer
/// emits a semantic `md-*` class on every markdown element (mirroring the
/// `site-header`/`panel` component idiom) and `tailwind.css` defines each
/// class with `@apply`. Output shape otherwise matches `push_html`: same
/// tags, ids, and attribute escaping, with raw `Html` events already
/// dropped upstream by [`Intercept`].
///
/// The `html` feature of pulldown-cmark is therefore unused; only the
/// parser's event stream matters.
#[allow(clippy::too_many_lines)] // one cohesive element writer; see above.
fn write_classed_html<'a, I>(out: &mut String, mut events: I)
where
    I: Iterator<Item = Event<'a>>,
{
    use std::fmt::Write as _;

    /// Escapes text for HTML body nodes (&, <, >).
    fn esc(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
    /// Escapes for quoted attributes (adds `"` and `'`).
    fn esc_attr(s: &str) -> String {
        esc(s).replace('"', "&quot;").replace('\'', "&#x27;")
    }

    let mut footnote_numbers: HashMap<CowStr<'a>, usize> = HashMap::new();
    let mut table_alignments: Vec<Alignment> = Vec::new();
    let mut table_cell_index = 0;
    let mut in_table_head = false;

    while let Some(event) = events.next() {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => out.push_str("<p class=\"md-p\">"),
                Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                } => {
                    // The writer emits only our md- classes; producer-side
                    // heading classes/attrs never appear in output.
                    let _ = (classes, attrs);
                    write!(out, "<{level}").unwrap();
                    if let Some(id) = id {
                        write!(out, " id=\"{}\"", esc_attr(&id)).unwrap();
                    }
                    write!(out, " class=\"md-{level}\">").unwrap();
                }
                Tag::BlockQuote(_) => out.push_str("<blockquote class=\"md-quote\">"),
                Tag::CodeBlock(kind) => match kind {
                    CodeBlockKind::Fenced(info) => {
                        let lang = info.split(' ').next().unwrap_or("");
                        if lang.is_empty() {
                            out.push_str("<pre class=\"md-pre\"><code class=\"md-code\">");
                        } else {
                            write!(
                                out,
                                "<pre class=\"md-pre\"><code class=\"md-code language-{}\">",
                                esc_attr(lang)
                            )
                            .unwrap();
                        }
                    }
                    CodeBlockKind::Indented => {
                        out.push_str("<pre class=\"md-pre\"><code class=\"md-code\">");
                    }
                },
                Tag::List(Some(start)) => {
                    write!(out, "<ol class=\"md-list\" start=\"{start}\">").unwrap();
                }
                Tag::List(None) => out.push_str("<ul class=\"md-list\">"),
                Tag::Item => out.push_str("<li class=\"md-li\">"),
                Tag::Table(alignments) => {
                    table_alignments = alignments;
                    out.push_str("<table class=\"md-table\">");
                }
                Tag::TableHead => {
                    in_table_head = true;
                    table_cell_index = 0;
                    out.push_str("<thead><tr>");
                }
                Tag::TableRow => {
                    table_cell_index = 0;
                    out.push_str("<tr>");
                }
                Tag::TableCell => {
                    let (tag, cls) = if in_table_head {
                        ("th", "md-th")
                    } else {
                        ("td", "md-td")
                    };
                    let align = match table_alignments.get(table_cell_index) {
                        Some(Alignment::Left) => " text-left",
                        Some(Alignment::Center) => " text-center",
                        Some(Alignment::Right) => " text-right",
                        _ => "",
                    };
                    table_cell_index += 1;
                    write!(out, "<{tag} class=\"{cls}{align}\">").unwrap();
                }
                Tag::Emphasis => out.push_str("<em class=\"md-em\">"),
                Tag::Superscript => out.push_str("<sup>"),
                Tag::Subscript => out.push_str("<sub>"),
                Tag::Strong => out.push_str("<strong class=\"md-strong\">"),
                Tag::Strikethrough => out.push_str("<del class=\"md-del\">"),
                Tag::Link {
                    dest_url, title, ..
                } => {
                    let title_attr = if title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", esc_attr(&title))
                    };
                    write!(
                        out,
                        "<a class=\"md-a\" href=\"{}\"{title_attr}>",
                        esc_attr(&dest_url)
                    )
                    .unwrap();
                }
                Tag::Image {
                    dest_url, title, ..
                } => {
                    // alt text: drain events until End(Image), matching
                    // push_html's raw_text (escaped for an attribute).
                    let mut alt = String::new();
                    for inner in events.by_ref() {
                        match inner {
                            Event::End(TagEnd::Image) => break,
                            Event::Text(t) | Event::Code(t) => alt.push_str(&esc_attr(&t)),
                            Event::SoftBreak | Event::HardBreak => alt.push(' '),
                            _ => {}
                        }
                    }
                    let title_attr = if title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", esc_attr(&title))
                    };
                    write!(
                        out,
                        "<img class=\"md-img\" src=\"{}\" alt=\"{alt}\"{title_attr} />",
                        esc_attr(&dest_url)
                    )
                    .unwrap();
                }
                Tag::FootnoteDefinition(name) => {
                    let len = footnote_numbers.len() + 1;
                    let number = *footnote_numbers.entry(name.clone()).or_insert(len);
                    // The label links back to its reference so the citation
                    // is a working link, and it closes immediately so the
                    // number and the body text sit on one line.
                    write!(
                        out,
                        "<div class=\"md-footnote\" id=\"{}\">\
                         <a class=\"md-footnote-backref\" href=\"#fnref-{}\">{number}</a> ",
                        esc_attr(&name),
                        esc_attr(&name)
                    )
                    .unwrap();
                }
                Tag::DefinitionList
                | Tag::DefinitionListTitle
                | Tag::DefinitionListDefinition
                | Tag::MetadataBlock(_)
                | Tag::HtmlBlock => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => out.push_str("</p>\n"),
                TagEnd::Heading(level) => writeln!(out, "</{level}>").unwrap(),
                TagEnd::BlockQuote(_) => out.push_str("</blockquote>\n"),
                TagEnd::CodeBlock => out.push_str("</code></pre>\n"),
                TagEnd::List(true) => out.push_str("</ol>\n"),
                TagEnd::List(false) => out.push_str("</ul>\n"),
                TagEnd::Item => out.push_str("</li>\n"),
                TagEnd::Table => out.push_str("</tbody></table>\n"),
                TagEnd::TableHead => {
                    in_table_head = false;
                    out.push_str("</tr></thead><tbody>\n");
                }
                TagEnd::TableRow => out.push_str("</tr>\n"),
                TagEnd::TableCell => {
                    out.push_str(if in_table_head { "</th>" } else { "</td>" });
                }
                TagEnd::Emphasis => out.push_str("</em>"),
                TagEnd::Strong => out.push_str("</strong>"),
                TagEnd::Strikethrough => out.push_str("</del>"),
                TagEnd::Link => out.push_str("</a>"),
                TagEnd::FootnoteDefinition => out.push_str("</div>\n"),
                TagEnd::Image | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) => {}
                TagEnd::DefinitionList => out.push_str("</dl>\n"),
                TagEnd::DefinitionListTitle => out.push_str("</dt>\n"),
                TagEnd::DefinitionListDefinition => out.push_str("</dd>\n"),
                TagEnd::Subscript => out.push_str("</sub>"),
                TagEnd::Superscript => out.push_str("</sup>"),
            },
            Event::Text(t) => out.push_str(&esc(&t)),
            Event::Code(t) => write!(out, "<code class=\"md-code\">{}</code>", esc(&t)).unwrap(),
            // Html reaching the writer is only Intercept's own escaped
            // output (the mermaid <pre>); producer HTML is dropped upstream
            // in `Intercept::next`.
            Event::Html(t) => out.push_str(&t),
            Event::InlineHtml(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {}
            Event::SoftBreak => out.push('\n'),
            Event::HardBreak => out.push_str("<br class=\"md-br\" />\n"),
            Event::Rule => out.push_str("<hr class=\"md-hr\" />\n"),
            Event::FootnoteReference(name) => {
                let len = footnote_numbers.len() + 1;
                let number = *footnote_numbers.entry(name.clone()).or_insert(len);
                write!(
                    out,
                    "<sup class=\"md-footnote-ref\" id=\"fnref-{}\">\
                     <a class=\"md-a\" href=\"#{}\">{number}</a></sup>",
                    esc_attr(&name),
                    esc_attr(&name)
                )
                .unwrap();
            }
            Event::TaskListMarker(checked) => {
                writeln!(
                    out,
                    "<input type=\"checkbox\" class=\"md-task\" disabled=\"\"{} />",
                    if checked { " checked=\"\"" } else { "" }
                )
                .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use okf_core::{Bundle, ConceptId};

    fn bundle_with(body: &str) -> Bundle {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "okf-web-md-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("index.md"),
            "---\nokf_version: \"0.2\"\n---\n\n# Index\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("doc.md"),
            format!("---\ntype: Doc\ntitle: Doc\n---\n\n{body}"),
        )
        .unwrap();
        let bundle = Bundle::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        bundle
    }

    fn out_of(id: &ConceptId) -> String {
        format!("{id}.html")
    }

    #[test]
    fn relative_links_from_nested_pages_get_prefix() {
        let dir = std::env::temp_dir().join(format!("okf-web-md-nest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(
            dir.join("index.md"),
            "---\nokf_version: \"0.2\"\n---\n\n# Index\n",
        )
        .unwrap();
        std::fs::write(dir.join("root.md"), "---\ntype: Doc\n---\n\n# Root\n").unwrap();
        std::fs::write(
            dir.join("a/b/leaf.md"),
            "---\ntype: Doc\n---\n\n[Root](/root.md) is absolute.\n\n[Sibling](../../index.md) resolves.\n",
        )
        .unwrap();
        let bundle = Bundle::load(&dir).unwrap();
        let id = ConceptId::parse("a/b/leaf").unwrap();
        let rendered = render(
            &bundle,
            &id,
            &bundle.get(&id).unwrap().document.body,
            "../../",
            &out_of,
        );
        assert!(
            rendered.html.contains(r#"href="../../root.html""#),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains(r#"href="../../index.html""#),
            "{}",
            rendered.html
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn intercepts_mermaid_blocks_with_escaped_pre() {
        let body = "Text before.\n\n```mermaid\nflowchart LR\n  A[\"x\"] --> B\n```\n\nText after.";
        let bundle = bundle_with(body);
        let id = ConceptId::parse("doc").unwrap();
        let rendered = render(
            &bundle,
            &id,
            &bundle.get(&id).unwrap().document.body,
            "",
            &out_of,
        );
        assert!(rendered.has_mermaid);
        assert!(
            rendered
                .html
                .contains(r#"<pre class="mermaid">flowchart LR"#),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("A[\"x\"] --&gt; B"),
            "diagram source survives with < > escaped: {}",
            rendered.html
        );
        // The block must NOT render as an ordinary code block.
        assert!(!rendered.html.contains("<code"), "{}", rendered.html);
    }

    #[test]
    fn drops_raw_html_but_keeps_code() {
        let body = "<script>alert(1)</script>\n\n```html\n<script>alert(2)</script>\n```\n";
        let bundle = bundle_with(body);
        let id = ConceptId::parse("doc").unwrap();
        let rendered = render(
            &bundle,
            &id,
            &bundle.get(&id).unwrap().document.body,
            "",
            &out_of,
        );
        assert!(
            !rendered.html.contains("<script>alert(1)"),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("&lt;script&gt;alert(2)"),
            "{}",
            rendered.html
        );
    }

    #[test]
    fn drops_raw_html_in_mermaid_source() {
        let body = "```mermaid\nflowchart LR\n  A --><script>evil()</script> B\n```";
        let bundle = bundle_with(body);
        let id = ConceptId::parse("doc").unwrap();
        let rendered = render(
            &bundle,
            &id,
            &bundle.get(&id).unwrap().document.body,
            "",
            &out_of,
        );
        assert!(
            rendered.html.contains("&lt;script&gt;"),
            "{}",
            rendered.html
        );
        assert!(!rendered.html.contains("<script>"), "{}", rendered.html);
    }

    #[test]
    fn tables_render() {
        let body = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let bundle = bundle_with(body);
        let id = ConceptId::parse("doc").unwrap();
        let rendered = render(
            &bundle,
            &id,
            &bundle.get(&id).unwrap().document.body,
            "",
            &out_of,
        );
        assert!(
            rendered.html.contains(r#"<table class="md-table">"#),
            "table carries its component class: {}",
            rendered.html
        );
    }

    #[test]
    fn links_in_code_fences_are_untouched() {
        let body = "```text\n[not a link](x.md)\n```\n\n[real](index.md)";
        let bundle = bundle_with(body);
        let id = ConceptId::parse("doc").unwrap();
        let rendered = render(
            &bundle,
            &id,
            &bundle.get(&id).unwrap().document.body,
            "",
            &out_of,
        );
        assert!(
            rendered.html.contains("[not a link](x.md)"),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains("index.html"), "{}", rendered.html);
    }
}
