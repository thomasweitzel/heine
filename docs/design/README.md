# Heine design records

These records explain why a feature has its current boundary. They are useful
when you need to understand a trade-off, contribute to Heine, or evaluate a
future proposal. For configuration and template rules, start with the focused
guides and the [reference](../reference.md) instead.

## Implemented features

- [Absolute URLs](absolute-urls.md)
- [Copied-asset licensing coverage](asset-licensing-coverage.md)
- [Draft publication](drafts.md)
- [Kits](kits.md)
- [Localized value formatting](localized-value-formatting.md)
- [Markdown block components](markdown-block-components.md)
- [Markdown facts](markdown-facts.md)
- [Multilingual sitemap alternates](multilingual-sitemap-alternates.md)
- [Margin footnotes](margin-footnotes.md)
- [Search](search.md)
- [Series](series.md)
- [Table of contents](table-of-contents.md)
- [Plugins and renderer adapters](plugins.md)
- [Generated color-scheme pictures](renderer-pictures.md)

## Deferred proposals

- [Percentage and currency formatting](percentage-and-currency-formatting.md):
  extensions that await stable ICU4X formatter APIs.
- [Renderer output dependencies](renderer-output-dependencies.md): a future
  typed resource-manifest boundary for embedded renderer results.

## Exploratory records

- [External asset manifests](external-asset-manifests.md): why a per-URL
  cache-busting helper is insufficient, and the boundary a future external
  asset-manifest integration would need.
- [Published-artifact licensing](published-artifact-licensing.md): a candidate
  extension that would attach declared licensing facts to renderer resources
  as well as copied assets.

## Rejected proposals

- [Markdown template fragments](markdown-template-fragments.md): an
  investigated Tera2-in-Markdown proposal that conflicts with Heine's existing
  content-and-template composition boundary.
