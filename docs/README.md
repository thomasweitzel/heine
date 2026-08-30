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
- [Asset licensing](asset-licensing.md): REUSE-style declarations, published
  license texts, and template attribution data.
- [Development server](development-server.md): local serving, live reload,
  and byte-range responses for media.
- [Sitemaps](sitemap.md): canonical discovery documents generated from rendered output.
- [Atom feeds](atom-feeds.md): locale-local collection syndication feeds.

## Reference material

The [reference](reference.md) is the complete current contract. It lists every
authored configuration field and page field, along with template values,
functions, filters, defaults, and constraints. It intentionally does not
repeat the tutorials and examples from the guides.

The [glossary](glossary.md) defines Heine's public vocabulary. Guides explain
terms in context on first use; use the glossary when returning to a term later.
