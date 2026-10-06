# Heine documentation

Heine's documentation has complementary layers. Choose the one that
matches what you need now.

| Need                                                      | Start here                                    |
|-----------------------------------------------------------|-----------------------------------------------|
| Build a first working site                                | [First site](../README.md#first-site)         |
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

## Build site structure

- [Internationalization and localization](internationalization.md): locale
  trees, Fluent messages, writing direction, and declared counterparts.
- [Collections](collections.md): page membership and ordering.
- [Pagination](pagination.md): generated slices of a collection.
- [Taxonomies](taxonomies.md): tags and other literal term classifications.
- [Series](series.md): checked page-local ordered reading sequences.
- [Drafts](drafts.md): unpublished pages and complete local previews.

## Publish and discover content

- [Deployment files](deployment-files.md): output-root files for hosting and
  verification services.
- [Absolute URLs](absolute-urls.md): canonical site and resource addresses
  built from a configured origin.
- [Sitemaps](sitemap.md): canonical discovery documents generated from rendered output.
- [Atom feeds](atom-feeds.md): locale-local collection syndication feeds.
- [Search indexes](search.md): locale-local JSON documents for a site-owned
  browser search experience.

## Author and present pages

- [Kits](kits.md): install one checked presentation package and adapt its
  public templates from the site root.
- [Declared content](declared-content.md): choose ordinary Markdown, named
  prose segments, structured data, or several page inputs.
- [Table of contents](table-of-contents.md): page-owned Markdown heading
  navigation.
- [Margin footnotes](margin-footnotes.md): local presentation of ordinary
  Markdown footnotes.
- [Color-scheme Markdown pictures](markdown-pictures.md): light and dark
  variants of one authored image.
- [Word counts and reading time](markdown-facts.md): deterministic Markdown
  facts and opt-in, locale-specific reading-time estimates.
- [Localized numbers](localized-values.md): deterministic decimal formatting
  shared by Tera and Fluent.
- [Renderer adapters](renderer-adapters.md): configure and use checked
  external stream and workspace-renderer replacements.
- [Reproducible renderer examples](renderer-examples.md): install Typst or
  KaTeX and build light and dark SVG or accessible embedded mathematics
  locally.

## Operate a site and account for assets

- [Development server](development-server.md): local serving, live reload,
  and byte-range responses for media.
- [Asset licensing](asset-licensing.md): REUSE-compatible declarations,
  published license texts, optional complete copied-asset coverage, and
  template attribution data.

## Design records

The focused guides and reference define Heine's user-facing contract. These
records explain the accepted design, rationale, and boundaries behind current
features and deferred proposals.

### Implemented features

- [Absolute URLs](design/absolute-urls.md)
- [Copied-asset licensing coverage](design/asset-licensing-coverage.md)
- [Draft publication](design/drafts.md)
- [Kits](design/kits.md)
- [Localized value formatting](design/localized-value-formatting.md)
- [Markdown facts](design/markdown-facts.md)
- [Multilingual sitemap alternates](design/multilingual-sitemap-alternates.md)
- [Margin footnotes](design/margin-footnotes.md)
- [Search](design/search.md)
- [Series](design/series.md)
- [Table of contents](design/table-of-contents.md)
- [Plugins and renderer adapters](design/plugins.md)
- [Generated color-scheme pictures](design/renderer-pictures.md)

### Deferred proposals

- [Percentage and currency formatting](design/percentage-and-currency-formatting.md):
  deferred extensions that await stable ICU4X formatter APIs.
- [Renderer output dependencies](design/renderer-output-dependencies.md): a
  future typed resource-manifest boundary for embedded renderer results.

### Exploratory records

- [External asset manifests](design/external-asset-manifests.md): why a
  per-URL cache-busting helper is insufficient, and the boundary a future
  external asset-manifest integration would need.

## Reference material

The [reference](reference.md) is the complete current contract. It lists every
authored configuration field and page field, along with template values,
functions, filters, defaults, and constraints. It does not
repeat the tutorials and examples from the guides.

The [glossary](glossary.md) defines Heine's public vocabulary. Guides explain
terms in context on first use; use the glossary when returning to a term later.

The [changelog](../CHANGELOG.md) records user-visible changes and their
migrations, including work that has not reached a release yet.

The [roadmap](roadmap.md) records planned capabilities and the open questions
that need a concrete design before implementation.
