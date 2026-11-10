# Heine documentation

Start with the smallest working site, then add the pieces your site needs.
The [First site](../README.md#first-site) page shows how to create one page
and one template. The [tutorial](tutorial.md) continues from that point and
builds a multilingual blog with content, assets, data, listings, pagination,
and tags.

For a ready-to-edit single-language site, run `heine new my-site` and follow
the README in the generated directory.

When you return with a specific task, use the guides below. The
[reference](reference.md) gives the exact contract for every setting and
template value, while the [glossary](glossary.md) explains Heine terms.

For a complete working site rather than a teaching-sized example, inspect and
build [`starter-site/`](../starter-site). It shows several features working
together, but you do not need it before using the guides. Its [source
tour](../starter-site/README.md#what-this-site-demonstrates) maps a feature to
the rendered example, its authored files, and the relevant guide.

## Shape a site

- [Internationalization and localization](internationalization.md): add
  languages, translated text, and declared counterparts.
- [Collections](collections.md): group and order pages that belong together.
- [Pagination](pagination.md): divide a collection into generated listings.
- [Taxonomies](taxonomies.md): classify pages with tags or other literal
  terms.
- [Series](series.md): create checked reading sequences between pages.
- [Drafts](drafts.md): keep work out of publication while previewing the full
  site locally.

## Write and present pages

- [Declared content](declared-content.md): choose ordinary Markdown, named
  prose segments, structured data, or several page inputs.
- [Table of contents](table-of-contents.md): add navigation from a page's
  Markdown headings.
- [Citations](citations.md): cite local bibliography entries and place a reference list.
- [Publication catalogs](publications.md): present works from local BibTeX files.
- [Margin footnotes](margin-footnotes.md): place ordinary Markdown footnotes
  beside their references when the layout allows it.
- [Markdown block components](markdown-components.md): wrap authored Markdown
  in a site or kit component without templating the whole document.
- [Color-scheme Markdown pictures](markdown-pictures.md): use light and dark
  variants of one authored image.
- [Word counts and reading time](markdown-facts.md): show deterministic word
  counts and opt-in, locale-specific reading-time estimates.
- [Localized values](localized-values.md): format numbers, dates, and times
  in Tera and Fluent messages.
- [Kits](kits.md): install a checked presentation package and adapt its public
  templates from the site root.

## Publish and help readers find it

- [Deployment files](deployment-files.md): publish output-root files for
  hosting and verification services.
- [Absolute URLs](absolute-urls.md): give pages and resources canonical
  addresses from a configured origin.
- [Sitemaps](sitemap.md): generate discovery documents from rendered output.
- [Atom feeds](atom-feeds.md): publish locale-local feeds from a collection.
- [Search indexes](search.md): build JSON documents for a site-owned browser
  search experience.

## Operate a site

- [Development server](development-server.md): local serving, live reload,
  and byte-range responses for media.
- [Published-artifact licensing](asset-licensing.md): REUSE-compatible
  declarations, published license texts, optional complete coverage for copied
  assets and renderer resources, and template attribution data.
- [Renderer adapters](renderer-adapters.md): configure checked replacements
  produced by external tools.
- [Renderer examples](renderer-examples.md): install Typst or KaTeX and build
  diagrams or accessible mathematics locally.

## Architecture and project decisions

The [architecture overview](architecture.md) explains how a full build moves
from authored files to published output. If you want to understand a trade-off
or contribute, the [design records](design/README.md) explain why features have
their current boundaries and preserve investigations for later proposals. The
[roadmap](roadmap.md) records planned work and open questions.

The [changelog](../CHANGELOG.md) records user-visible changes and migrations,
including work that has not reached a release yet.
