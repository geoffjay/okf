---
type: Reference
title: Markdown style reference
description: Every GitHub-flavoured markdown element rendered by the okf site generator, used to tune the site's typographic styles.
tags: [reference, okf-web, styles]
---

This page renders one example of every GitHub-flavoured markdown element
the site generator supports, so typography changes can be judged in place.
Headings below deliberately use every level.

# Heading level 1

Body text follows a heading. The main complaint this page exists to fix:
paragraph text was visually too close to headings. Paragraphs are
`16px/1.6 system-ui` with margins separating them.

## Heading level 2

### Heading level 3

#### Heading level 4

##### Heading level 5

###### Heading level 6

## Emphasis

Plain, **bold**, *italic*, ***bold italic***, ~~strikethrough~~, and
`inline code`. Also a [link](https://example.com), and an auto-link:
<https://example.com>.

## Lists

Unordered:

- First bullet with a second line of text to check wrapping.
- Second bullet
  - Nested bullet
  - Another nested bullet
- Third bullet

Ordered:

1. First item
2. Second item
   1. Nested ordered item
3. Third item

Task list:

- [ ] Unchecked task
- [x] Checked task

## Blockquote

> A single-level quote.
>
> > A nested quote inside it.

## Code

Fenced code with language:

```rust
fn main() {
    println!("Hello, world!");
}
```

Indented code:

    indented code block
    second line

## Tables

| Feature | Supported | Notes |
| ------- | --------- | ----- |
| Tables | yes | with alignment |
| Alignment | yes | below |
| Strikethrough | yes | GFM extension |

## Horizontal rule

Above the rule.

---

Below the rule.

## Footnotes

A sentence with a footnote reference.[^note]

[^note]: The footnote body.

## Images

Inline image:

![tiny red dot](data:image/gif;base64,R0lGODlhAQABAPAAAP8AAP///yH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==)

## HTML

Raw `<b>HTML</b>` is dropped by the renderer (permissive-input, safe-output),
so `<b>bold</b>` shows as plain text above.