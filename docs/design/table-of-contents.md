# Table of contents design

## Status

This is the accepted design implemented by Heine. The
[table-of-contents guide](../table-of-contents.md) introduces it for site
authors, and the [reference](../reference.md) defines the exact public
contract. This document records the design rationale and boundaries.

## Decision

Heine provides an opt-in, page-owned table of contents derived from one
selected declared Markdown content input. It exposes a structural,
render-facing view and assigns deterministic HTML fragment identifiers while
rendering that same selected source. Templates decide whether and how to
display the structure.

Heine does not infer a page body, scrape rendered HTML, generate a fixed
navigation element, or treat collections and taxonomies as documents.

## Motivation

Many documentation and article pages need navigation over their headings. A
page can, however, have several declared content inputs, structured or
segmented content, no Markdown at all, or Markdown that is not its primary
document. Treating `content.main` as a universal body would make one common
convention into an implicit framework rule.

The selected declared content name makes the relationship explicit. The
Markdown domain can then derive the headings and their anchors from one source
of truth, and the template receives data rather than a prescribed navigation
layout.

`main` remains a useful conventional name for a page's primary document, but
it has no privileged meaning to Heine.

## Eligibility and scope

The authored model is intentionally narrow: a complete page `[toc]` table or
recursive `[pages.toc]` directory default selects one declared, unsegmented
Markdown input of a rendered authored page. A page or descriptor can use
`toc = false` only to suppress an inherited setting. Nearest settings replace,
rather than merge with, earlier settings. The [guide](../table-of-contents.md)
and [reference](../reference.md#_directorytoml) define that public syntax and
its complete constraints.

Segmented Markdown has multiple independently rendered values, so it has no
single heading order to which one table of contents could reliably refer.

Collections do not have ToC configuration. A collection member remains an
ordinary page and may use a ToC independently. An authored collection listing
may use its own ToC in the same way. Its generated pager tasks retain that
authored page, so they render its selected Markdown source and ToC normally.

Generated taxonomy index and term pages have no authored page or selected
Markdown source. They do not receive an automatic ToC. Their templates may
render taxonomy navigation from taxonomy data, but that is a distinct feature.

## Template contract

`page.toc` is a flat, source-order view. Its numeric depth describes
indentation relative to the configured minimum heading level, not a
parent-child relationship. This leaves list markup, labels, styling, and
disclosure behavior with the template instead of making heading skips an
engine concern.

The `markdown(content="…")` function reads named declared Markdown from the
current authored page. An effective ToC renders its selected source exactly
once, applying the precomputed IDs to one real heading sequence. The existing
`markdown` filter remains for arbitrary trusted template text. The exact
render-facing fields and function behavior are in the
[reference](../reference.md#template-contract); the
[guide](../table-of-contents.md) shows the complete template pattern.

Tera passes the filter a string rather than its authored provenance. Heine can
therefore reject only an exact selected-source match passed through the filter.
A transformed selected source has changed the input from which the ToC was
derived and must not use that ToC.

### Margin-footnote composition

[Margin footnotes](margin-footnotes.md) may select the same source as a ToC.
The source must be rendered exactly once per output document through
`markdown(content="…")`. Zero or repeated calls are template errors: generated
heading and local-note identifiers need one rendered source occurrence. One
call satisfies both consumers when they select the same source.

A Markdown heading inside a margin-footnote definition is ancillary note
content. It does not enter `page.toc` or receive a generated ToC anchor. This
is the only exception to the usual selected-source heading rule.

## Fragment identifiers

The Markdown domain derives heading text and fragment identifiers from parsed
Markdown events. It never parses rendered HTML.

For each Markdown heading, it collects plain text from text and inline-code
events, treating soft and hard line breaks as one space. Markdown formatting,
links, and image alt text contribute their textual contents. Raw inline HTML
does not contribute synthetic text. Raw HTML block headings such as
`<h2>Manual heading</h2>` are not Markdown heading events, so they are ignored
by ToC extraction and receive no generated ID.

Heine derives the base identifier as follows:

1. normalize the text to Unicode NFC;
2. apply Rust's `str::to_lowercase()` Unicode lowercase mapping without
   locale-specific transliteration;
3. retain Unicode letters and numbers, plus combining marks only after a
   retained letter, number, or mark in the same identifier run;
4. replace each maximal run of other characters with one ASCII hyphen;
5. remove leading and trailing hyphens;
6. use `section` when no retained letter or number remains.

Heine assigns identifiers in source order. The first available base identifier
is used unchanged. Later collisions receive `-2`, `-3`, and so on, checking
the complete candidate against every identifier already assigned. Thus two
`Introduction` headings become `introduction` and `introduction-2`; an
earlier literal `Introduction 2` cannot cause a duplicate identifier.

Identifiers preserve meaningful Unicode rather than transliterating it. For
example, `Überblick` becomes `überblick`. `fragment` contains the identifier
itself, not a percent-encoded URL fragment; templates use it after `#` as in
the example above. Heine's own generated identifiers are unique within the
selected document. It does not inspect trusted raw HTML or template markup
for manually authored `id` attributes.

Only headings in the selected declared Markdown source receive these generated
IDs. Headings in other Markdown values and template markup remain untouched.

## Resolution and rendering model

Source discovery and page loading resolve the effective ToC setting after
directory policy and page metadata are known. The Markdown domain parses the
selected source into a small heading and anchor plan. The resolved page retains
only the selected content name and render-facing heading facts.

During `markdown(content=...)`, the Markdown renderer uses that anchor plan
when the requested name is selected. It associates anchors with headings in
parser source order. Both the render-facing view and emitted IDs therefore use
one fragment-assignment algorithm. Heine does not create a second slugging
path for templates or inspect rendered output after the fact.

Ordinary Markdown rendering and the existing Markdown link, math, and syntax
highlighting behavior remain unchanged. The ToC representation retains heading
metadata, not pre-rendered HTML; it adds memory proportional only to headings
on opted-in pages.

## Diagnostics

Configuration diagnostics name the authored `.page` or `_directory.toml`
field. Template-function and exact-filter-match diagnostics name the Tera call
site. Duplicate headings are not errors: source-order suffixes resolve them.
The [reference](../reference.md#markdown) is the canonical list of current
constraints and error behavior.

## Intentional boundaries

The first version does not provide:

- automatic ToC generation for every Markdown file or every page;
- a global ToC setting;
- a universal `content.main` or body convention;
- ToCs assembled from multiple content inputs;
- hierarchical ToC trees or injected navigation markup;
- automatic visible anchor-link controls beside headings;
- ToCs for generated taxonomy pages;
- raw-HTML or rendered-HTML scraping;
- automatic checks for IDs manually produced by trusted Markdown HTML or
  templates.

These limits keep a ToC tied to an explicit parsed Markdown document. A future
extension needs a concrete use case and must preserve that source-of-truth
boundary.

## Acceptance criteria

Implementation must cover at least:

- page-local ToCs, directory defaults, nested replacements, and opt-outs;
- null and non-null render-facing `page.toc` values, including an enabled ToC
  with no headings;
- all heading levels and configured ranges;
- headings with formatting, links, code, line breaks, Unicode, punctuation,
  standalone combining marks, raw HTML headings, empty usable text, duplicates,
  and collisions with generated suffixes;
- one-based `depth` values for every configured heading range;
- generated heading IDs and source-order fragment links in rendered output;
- ordinary filter rendering remaining unanchored and unchanged for arbitrary
  text, plus a checked error for an exact selected-source match;
- source-aware `markdown(content=...)` rendering with and without a selected
  ToC source;
- template diagnostics for invalid function use with useful spans;
- page and descriptor diagnostics for invalid settings and ineligible inputs;
- interaction with Markdown links, math modes, syntax highlighting,
  localization, paginated listings, and quick builds;
- complete updates to the tutorial, glossary, reference, a focused ToC guide,
  and a natural starter-site example.
