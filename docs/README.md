# Heine documentation

Heine's documentation has complementary layers. Choose the one that
matches what you need now.

| Need                                                      | Start here                                    |
|-----------------------------------------------------------|-----------------------------------------------|
| Build a first working site                                | [README: First site](../README.md#first-site) |
| Learn how the pieces compose                              | [Tutorial](tutorial.md)                       |
| Understand a named Heine concept                          | [Glossary](glossary.md)                       |
| Add one capability to an existing site                    | The focused guides below                      |
| Check an exact option, field, template value, or function | [Reference](reference.md)                     |

## Recommended reading path

The [README](../README.md) builds the smallest possible site. Continue with
the [tutorial](tutorial.md) to grow it into a multilingual blog with assets,
data, listings, pagination, and tags. The tutorial links to a focused guide
whenever a feature needs more explanation.

For a complete working site rather than a teaching-sized example, inspect and
build [`starter-site/`](../starter-site/). It is a cohesive example, not a
required prerequisite for the guides.

## Focused guides

- [Internationalization and localization](internationalization.md): locale
  trees, Fluent messages, writing direction, and declared counterparts.
- [Collections](collections.md): page membership and ordering.
- [Pagination](pagination.md): generated slices of a collection.
- [Taxonomies](taxonomies.md): tags and other literal term classifications.
- [Deployment files](deployment-files.md): output-root files for hosting and
  verification services.
- [Asset licensing](asset-licensing.md): REUSE-compatible declarations,
  published license texts, optional complete copied-asset coverage, and
  template attribution data.
- [Development server](development-server.md): local serving, live reload,
  and byte-range responses for media.
- [Drafts](drafts.md): unpublished pages and complete local previews.
- [Sitemaps](sitemap.md): canonical discovery documents generated from rendered output.
- [Atom feeds](atom-feeds.md): locale-local collection syndication feeds.
- [Search indexes](search.md): locale-local JSON documents for a site-owned
  browser search experience.
- [Table of contents](table-of-contents.md): page-owned Markdown heading
  navigation.
- [Margin footnotes](margin-footnotes.md): checked local presentation of
  ordinary Markdown footnotes.
- [Color-scheme Markdown pictures](markdown-pictures.md): checked light and
  dark variants of one authored image.
- [Word counts and reading time](markdown-facts.md): deterministic Markdown
  facts and an opt-in, locale-specific reading-time estimate.
- [Series](series.md): checked page-local ordered reading sequences.

## Reference material

The [reference](reference.md) is the complete current contract. It lists every
authored configuration field and page field, along with template values,
functions, filters, defaults, and constraints. It does not
repeat the tutorials and examples from the guides.

The [glossary](glossary.md) defines Heine's public vocabulary. Guides explain
terms in context on first use; use the glossary when returning to a term later.

The [roadmap](roadmap.md) records planned capabilities and the open questions
that need a concrete design before implementation.
