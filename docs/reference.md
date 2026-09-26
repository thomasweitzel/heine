# Heine reference

This is the complete current contract for Heine-authored files, command
options, and template values. Start with the [README](../README.md) or
[tutorial](tutorial.md) when building a site for the first time; use this page
to look up exact fields, defaults, constraints, functions, and filters. The
[glossary](glossary.md) defines Heine-specific vocabulary used below.

`heine.toml`, `.page`, and `_directory.toml` use strict schemas: unknown
fields are rejected. TOML loaded from `data/` or declared as page content is
arbitrary structured data.

## Site root and commands

A site root contains `heine.toml`, `content/`, and `templates/`; `data/`, `deploy/`, `LICENSES/`, and `locales/` are optional. Without `--root`, Heine checks the current directory and each parent directory. It never searches child directories.

| Command       | Option                       | Default                    | Allowed values     | Effect                                                                                                                                                                                                  |
|---------------|------------------------------|----------------------------|--------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `heine`       | `--root <path>`              | nearest ancestor site root | existing site root | Selects the site root                                                                                                                                                                                   |
| `heine`       | `--quick`                    | off                        | flag               | Updates managed output for changed page, declared content, inherited descriptor, data, Fluent resources, present copied assets and sidecars, deployment files, and license texts; it remains incomplete |
| `heine`       | `--clean-output`             | off                        | flag               | Adopts unmanaged output; conflicts with `--quick`                                                                                                                                                       |
| `heine`       | `--allow-external-renderers` | off                        | flag               | Permits selected checked renderer requests to start their configured program                                                                                                                            |
| `heine`       | `--refresh-renderers`        | off                        | flag               | Re-runs permitted renderer requests instead of reusing their cached results                                                                                                                             |
| `heine serve` | `--root <path>`              | nearest ancestor site root | existing site root | Selects the site root                                                                                                                                                                                   |
| `heine serve` | `--clean-output`             | off                        | flag               | Adopts unmanaged output before its initial build                                                                                                                                                        |
| `heine serve` | `--drafts`                   | off                        | flag               | Includes otherwise unpublished `draft = true` pages in this local preview                                                                                                                               |
| `heine serve` | `--allow-external-renderers` | off                        | flag               | Permits selected checked renderer requests to start their configured program                                                                                                                            |
| `heine serve` | `--refresh-renderers`        | off                        | flag               | Re-runs permitted renderer requests instead of reusing their cached results                                                                                                                             |
| `heine serve` | `--host <address>`           | `127.0.0.1`                | IP address         | Interface on which to listen                                                                                                                                                                            |
| `heine serve` | `--port <number>`            | `1111`                     | `1024`-`65535`     | First port to try; then the next free port                                                                                                                                                              |

`--quick` does not apply to `serve`.

## `heine.toml`

`heine.toml` is required. Its `[i18n]` section is required; other sections and
fields use the following defaults.

| Field                                             | Type                 | Default                               | Allowed values and constraints                                                                                                                                        | Effect                                                                                            |
|---------------------------------------------------|----------------------|---------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------|
| `kit.name`                                        | string               | absent                                | Required when `[kit]` exists; simple identifier naming an installed `kits/<name>/` directory whose manifest has the same name                                         | Selects one kit                                                                                   |
| `kit.version`                                     | string               | absent                                | Optional semantic-version requirement satisfied by the selected manifest's `version`                                                                                  | Prevents an unintended installed kit release                                                      |
| `kit.extra`                                       | TOML table           | empty table                           | Keys, value types, and nested shape must be declared by the selected kit's `[extra]` defaults; tables merge, scalar values and arrays replace                         | Root-owned presentation settings exposed as `kit.extra`                                           |
| `site.base_path`                                  | string               | `"/"`                                 | ASCII absolute URL directory path; starts and ends with `/`; uppercase percent escapes only; no encoded separators or dot segments                                    | Prefix for base-path-aware `url` values, including `asset()`, `page()`, and taxonomy links        |
| `site.origin`                                     | string               | absent                                | Absolute `http` or `https` origin only: host and optional port, without credentials, query, fragment, or deployment path                                              | Required by sitemaps, Atom feeds, and templates that directly render `site.url` or `absolute_url` |
| `site.time_zone`                                  | string               | absent                                | IANA time-zone name such as `"Europe/Berlin"`; resolved from Heine's bundled database                                                                                 | Resolves authored `published` and `updated` local times deterministically                         |
| `build.output_dir`                                | string               | `"public"`                            | Non-empty relative path with portable filename components; cannot be `content/`, `data/`, `deploy/`, `LICENSES/`, `templates/`, `locales/`, or a descendant           | Generated output directory relative to site root                                                  |
| `assets.integrity`                                | string               | absent                                | `"sha256"`, `"sha384"`, `"sha512"`                                                                                                                                    | Enables Subresource Integrity values from `asset()`                                               |
| `licensing.required`                              | boolean              | absent                                | `true` only; `false` is a checked error                                                                                                                               | Requires one licensing declaration for every copied asset                                         |
| `licensing.groups`                                | array of tables      | `[]`                                  | Exact non-empty copied-asset `output_path` list and sidecar-style SPDX fields; no overlap, sidecar conflict, glob, or unknown fields                                  | Applies one shared declaration to an asset set                                                    |
| `i18n.default_locale`                             | string               | required                              | Canonical BCP-47 locale declared by an `i18n.locales.<code>` table                                                                                                    | Locale with unprefixed generated URLs                                                             |
| `i18n.locales`                                    | table                | required                              | Non-empty table keyed by canonical BCP-47 locale codes                                                                                                                | Finite locale source trees to render                                                              |
| `i18n.locales.<code>.direction`                   | string               | required                              | `"ltr"` or `"rtl"`                                                                                                                                                    | Base HTML writing direction for that locale                                                       |
| `collections.<name>.order`                        | string               | required                              | `"published-asc"`, `"published-desc"`, `"title-asc"`, `"title-desc"`, `"weight-asc"`, `"weight-desc"`, `"id-asc"`, or `"id-desc"`; `<name>` is a simple identifier    | Defines a named, locale-local ordered set of rendered pages                                       |
| `series.path`                                     | string               | required when enabled                 | Portable relative output path outside [`__heine/`](glossary.md#heine)                                                                                                 | Locale-relative generated Series root                                                             |
| `series.index_template`                           | string               | required when enabled                 | Logical template name below `templates/`                                                                                                                              | Renders every locale's Series index                                                               |
| `series.listing_template`                         | string               | required when enabled                 | Logical template name below `templates/`                                                                                                                              | Renders non-empty Series listings and their pagers                                                |
| `series.index_title_key`                          | string               | required when enabled                 | Non-empty Fluent message identifier                                                                                                                                   | Localized generated-index title                                                                   |
| `series.index_description_key`                    | string               | absent                                | Non-empty Fluent message identifier                                                                                                                                   | Optional localized generated-index description                                                    |
| `series.per_page`                                 | integer              | required when enabled                 | Positive integer                                                                                                                                                      | Members on every generated Series listing page                                                    |
| `series.definitions`                              | table                | required when enabled; may be empty   | Keys are literal portable URL components without percent escapes                                                                                                      | Declares the Series catalogue                                                                     |
| `series.definitions.<id>.title_key`               | string               | required                              | Non-empty Fluent message identifier; `<id>` is a literal portable URL component without percent escapes                                                               | Localized Series title and listing route component                                                |
| `series.definitions.<id>.description_key`         | string               | absent                                | Non-empty Fluent message identifier                                                                                                                                   | Optional localized Series description                                                             |
| `search.<name>.collections`                       | array of strings     | required                              | Non-empty array of distinct configured collection names; `<name>` is a simple identifier                                                                              | Selects and unions locale-local members for one JSON search index                                 |
| `search.<name>.path`                              | string               | required                              | Portable relative output path ending in lowercase `.json`, outside [`__heine/`](glossary.md#heine)                                                                    | Locale-relative logical JSON output path                                                          |
| `search.<name>.versioned`                         | boolean              | `true`                                | `true` or `false`                                                                                                                                                     | Inserts a full JSON SHA-256 digest before `.json`, or retains the stable configured path          |
| `feeds.<name>.collection`                         | string               | required                              | Configured collection name; `<name>` is a simple identifier                                                                                                           | Selects the locale-local members that supply Atom entries                                         |
| `feeds.<name>.path`                               | string               | required                              | Portable relative output path outside [`__heine/`](glossary.md#heine)                                                                                                 | Locale-relative Atom output path                                                                  |
| `feeds.<name>.title_key`                          | string               | required                              | Non-empty Fluent message identifier                                                                                                                                   | Locale-local Atom feed title                                                                      |
| `feeds.<name>.subtitle_key`                       | string               | absent                                | Non-empty Fluent message identifier                                                                                                                                   | Optional locale-local Atom feed subtitle                                                          |
| `feeds.<name>.limit`                              | integer              | `20`                                  | Positive integer                                                                                                                                                      | Maximum chronological entries in each locale feed                                                 |
| `feeds.<name>.empty_updated`                      | TOML local date-time | conditionally required                | Same form as `published`; required when a locale feed is empty                                                                                                        | Atom update time for an empty locale feed                                                         |
| `feeds.<name>.empty_authors`                      | array of strings     | conditionally required                | Non-empty author names; required when a locale feed is empty                                                                                                          | Atom authors for an empty locale feed                                                             |
| `taxonomies.<name>.collection`                    | string               | required                              | Configured collection name; `<name>` is a simple identifier                                                                                                           | Selects and orders the taxonomy's rendered members                                                |
| `taxonomies.<name>.path`                          | string               | required                              | Portable relative output path outside [`__heine/`](glossary.md#heine)                                                                                                 | Generated taxonomy root                                                                           |
| `taxonomies.<name>.index_template`                | string               | required                              | Logical template name below `templates/`                                                                                                                              | Renders the taxonomy's term index                                                                 |
| `taxonomies.<name>.term_template`                 | string               | required                              | Logical template name below `templates/`                                                                                                                              | Renders each term listing and its pagers                                                          |
| `taxonomies.<name>.per_page`                      | integer              | required                              | Positive integer                                                                                                                                                      | Members on each generated term-listing page                                                       |
| `taxonomies.<name>.counterparts.<group>.<locale>` | string               | absent                                | An existing literal term for a configured locale; each non-empty opaque `<group>` has at least two locale members, and a locale-term pair occurs in at most one group | Declares a cross-locale term counterpart                                                          |
| `sitemap`                                         | table                | absent                                | Empty table; requires `site.origin`                                                                                                                                   | Generates canonical sitemap output                                                                |
| `sitemap.hreflang`                                | boolean              | `false`                               | Only `true`; requires `[sitemap]`; applies only to explicit sitemap-included `translationid` counterparts                                                             | Adds locale-sorted XHTML sitemap alternates for authored page counterparts                        |
| `markdown.extensions`                             | array of strings     | `[]`                                  | Unique subset of `"tables"`, `"strikethrough"`, `"task_lists"`, `"footnotes"`                                                                                         | Enables Markdown extensions                                                                       |
| `markdown.tables.scroll_message`                  | string               | absent                                | Non-empty Fluent message identifier; enables responsive Markdown-table regions and requires `markdown.extensions` to include `"tables"`                               | Localized accessible name for scrollable Markdown-table regions                                   |
| `markdown.math`                                   | string               | `"off"`                               | `"off"`, `"mathml"`, `"tex"`                                                                                                                                          | Selects ordinary Markdown, build-time MathML, or escaped TeX passthrough                          |
| `markdown.margin_footnotes.marker_message`        | string               | absent                                | Non-empty Fluent message identifier; Heine supplies `$number` when formatting it; requires at least one effective page or directory footnote setting                  | Localized accessible name for margin-footnote controls                                            |
| `markdown.highlighting.stylesheet`                | string               | absent                                | Safe site-output-relative path ending in `.css`; cannot use [`__heine/`](glossary.md#heine)                                                                           | Generated class-based syntax stylesheet                                                           |
| `markdown.highlighting.missing_language`          | string               | `"error"`                             | `"error"`, `"plain"`                                                                                                                                                  | Unknown fenced-language policy                                                                    |
| `markdown.highlighting.theme.light`               | string               | required when highlighting is enabled | Bundled Giallo theme name; see the linked list below                                                                                                                  | Light syntax palette                                                                              |
| `markdown.highlighting.theme.dark`                | string               | required when highlighting is enabled | Bundled Giallo theme name; see the linked list below                                                                                                                  | Dark syntax palette                                                                               |
| `renderers.<name>.protocol`                       | string               | required                              | `"stream"` or `"workspace"`; `<name>` is a simple identifier                                                                                                          | Selects the external-renderer protocol                                                            |
| `renderers.<name>.program`                        | string               | required                              | Non-empty program path or operating-system-resolved program name                                                                                                      | Program started only for a permitted selected request                                             |
| `renderers.<name>.timeout_seconds`                | integer              | required                              | Positive                                                                                                                                                              | Direct-child timeout                                                                              |
| `renderers.<name>.cache_identity`                 | string               | required                              | Non-empty stable site-owner version or environment identity                                                                                                           | Deliberate renderer-cache invalidation input                                                      |
| `renderers.<name>.inputs`                         | array of strings     | required                              | Stream: unique subset of `"body"`, `"file"`; workspace: those plus `"bundle"`                                                                                         | Permitted request input forms                                                                     |
| `renderers.<name>.input_extension`                | string               | workspace body required               | Unprefixed ASCII alphanumeric extension; otherwise absent                                                                                                             | Filename extension for a staged body input                                                        |
| `renderers.<name>.result`                         | string               | required                              | `"html"`, `"mathml"`, `"svg"`, `"image"`, or `"link"`; SVG declares supported materializations, image and link declare `media_type` and `extension`                   | Checked renderer result shape                                                                     |
| `renderers.<name>.media_type`                     | string               | required for image or link            | Valid media type, starting with `image/` for image results; otherwise absent                                                                                          | Declared generated-resource media type                                                            |
| `renderers.<name>.extension`                      | string               | required for image or link            | Unprefixed ASCII alphanumeric extension; otherwise absent                                                                                                             | Generated-resource filename extension                                                             |
| `renderers.<name>.materializations`               | array of strings     | required for SVG                      | Non-empty subset of `"embedded"`, `"resource"`; otherwise absent                                                                                                      | Supported SVG result forms                                                                        |
| `renderers.<name>.default_output`                 | string               | sole SVG materialization              | Required for SVG with both materializations; one of the declared forms; otherwise absent                                                                              | Default SVG result form                                                                           |
| `renderers.<name>.inline.arguments`               | array of strings     | absent                                | Fixed arguments, no workspace placeholders                                                                                                                            | Enables inline requests                                                                           |
| `renderers.<name>.block.arguments`                | array of strings     | absent                                | Fixed arguments, no workspace placeholders                                                                                                                            | Enables block requests                                                                            |

Collection definitions establish relationships only; they do not write output.
Every configured collection exists in every locale, including when it is
empty. Page IDs are the final deterministic tie-breaker. Published orders
require every member to declare `published`; title orders use the collection locale's ICU
collation and require titles, and weight orders require numeric weights on
every member.

`[kit]` selects at most one installed kit. Heine checks the selected
directory, manifest name, optional site version requirement, the manifest's
compatible Heine-version requirement, and its required self-selecting
`site/heine.toml` before loading kit runtime resources. A kit's `[extra]`
defaults define the complete allowed root overlay. See [Kits](kits.md) for the
installation, adapter, and ownership model.

Taxonomies turn one configured collection into locale-local generated index
and term-listing pages. Their terms are literal display and route components;
they are neither slugified nor inferred across locales. See
[taxonomies](taxonomies.md) for the route and rendering model.

Atom feeds turn one configured collection into one XML document per locale.
They use collection membership but order entries by `published` descending,
then page ID. See [Atom feeds](atom-feeds.md) for entry requirements, empty
locale feeds, and template discovery links.

Search indexes turn one or more configured collections into one JSON document
per locale. Their union is ordered by page ID, independently of every
collection's display order. The complete document is content-hashed by default;
`--quick` does not update it. See [search indexes](search.md) for the JSON and
browser-boundary contract.

The complete set of accepted theme names is in
[Giallo's source README](https://docs.rs/crate/giallo/latest/source/README.md)
under “Themes”; the linked source matches Heine's bundled registry.
Use those exact names; Heine reports an unknown name at its configuration
location.

## Search indexes

Each configured index writes one UTF-8 JSON document per configured locale:

```json
{
  "format": 3,
  "locale": "en",
  "documents": [
    {
      "id": "blog/example/index",
      "url": "/guide/blog/example/index.html",
      "draft": false,
      "title": "Example",
      "description": null,
      "summary": "A short example.",
      "published": "2026-08-14T07:30:00Z",
      "updated": null,
      "taxonomies": { "tags": ["Rust"] },
      "sections": [
        { "fragment": null, "text": "Source-derived opening text." },
        { "fragment": "installation", "text": "Installation\n\nSource-derived section text." }
      ]
    }
  ]
}
```

`format` is the positive schema version, currently `3`. `id` is the
locale-relative page ID and `url` names the literal generated page file with
`site.base_path` applied. `draft` is `true` only in a `heine serve --drafts`
index. `title`, `description`, `summary`, `published`, and
`updated` are their authored page facts or `null`. `taxonomies` has configured
taxonomy names in deterministic lexical order, with literal locale-local terms
in authored order. Documents are ordered by page ID.

`sections` preserves every declared `.md` input in content-name order as
non-empty source-derived text sections. Each section has `text` and a
`fragment`. `fragment` is `null` unless the section begins at a Markdown
heading with a generated identifier from the page's effective table of
contents. A browser can search section text and append a non-null fragment to
the document URL, or search all sections as one document by joining their
texts. The source text includes headings, link labels, image alternatives,
code, and recognized math source. Source-authored raw HTML adds text nodes and
`img` `alt` values, while markup, comments, URLs, attributes, and `script`,
`style`, or `template` contents do not contribute. Soft and hard Markdown
breaks remain line feeds; block boundaries become two line feeds. Heine does
not lower-case, tokenize, stem, truncate, or derive text from rendered HTML.

## Markdown facts

Every declared Markdown input exposes `page.markdown.<name>.word_count`.
`<name>` is the same simple name used by `content.<name>`; non-Markdown inputs
do not appear in `page.markdown`. The count uses Unicode default word
segmentation over readable source-level Markdown text. See [word counts and
reading time](markdown-facts.md) for its inclusion rules and limitations.

| Field                                    | Type             | Default | Constraints                                                                  | Effect                                      |
|------------------------------------------|------------------|---------|------------------------------------------------------------------------------|---------------------------------------------|
| `reading_time.words_per_minute.<locale>` | positive integer | absent  | One exact canonical configured BCP-47 locale key for every configured locale | Enables `page.markdown.<name>.reading_time` |

`[reading_time]` is optional. When absent, every `reading_time` template field
is `null`; when present, every configured locale must have exactly one positive
rate and no unconfigured locale may appear. Heine rounds nonzero estimates up
to whole minutes and reports zero for zero counted words.

## `_directory.toml`

Place `_directory.toml` in a configured `content/<locale>/` tree. It is non-rendered, non-published, and provides directory metadata plus recursive policy.

```toml
title = "Blog"

[metadata]
required = ["title"]

[pages]
template = "post.tera"

[pages.metadata]
required = ["title", "published", "extra.author"]
```

| Field                     | Type              | Default     | Allowed values and constraints                                                                                                              | Effect                                                                                             |
|---------------------------|-------------------|-------------|---------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|
| `title`                   | string            | absent      | TOML string                                                                                                                                 | `page.directory.title`                                                                             |
| `description`             | string            | absent      | TOML string                                                                                                                                 | `page.directory.description`                                                                       |
| `summary`                 | string            | absent      | TOML string                                                                                                                                 | `page.directory.summary`                                                                           |
| `extra`                   | TOML table        | empty table | Dot-addressable keys; rules below                                                                                                           | `page.directory.extra`                                                                             |
| `metadata.required`       | array of strings  | `[]`        | `title`, `description`, `summary`, or `extra.<name>`                                                                                        | Adds requirements for this and nested descriptors                                                  |
| `pages.template`          | string or boolean | absent      | Template below `templates/`, or `false`                                                                                                     | Recursive default; closest descriptor wins                                                         |
| `pages.collections`       | array of strings  | inherited   | Unique configured collection names; an empty array opts out                                                                                 | Recursive default membership for descendant pages                                                  |
| `pages.authors`           | array of strings  | inherited   | Non-empty author names                                                                                                                      | Recursive author default; a page list replaces the nearest one                                     |
| `pages.sitemap`           | boolean           | absent      | Only `false`, and only when global `[sitemap]` is enabled                                                                                   | Recursively excludes descendant rendered pages and their pagers; no descendant may re-include them |
| `pages.toc.content`       | string            | absent      | Simple declared-content name for one unsegmented `.md` file; each effective descendant renders it exactly once with `markdown(content="…")` | Recursive default table-of-contents source                                                         |
| `pages.toc.min_level`     | integer           | `1`         | Integer from `1` through `6`                                                                                                                | Lowest heading level exposed through the table of contents                                         |
| `pages.toc.max_level`     | integer           | `6`         | Integer from `1` through `6`; not lower than `min_level`                                                                                    | Highest heading level exposed through the table of contents                                        |
| `pages.toc`               | boolean           | absent      | Only `false`; requires an otherwise effective inherited table of contents                                                                   | Suppresses that inherited default for this descriptor subtree                                      |
| `pages.footnotes.content` | string            | absent      | Simple declared-content name for one unsegmented `.md` file; each effective descendant must render                                          | Recursive default margin-footnote source                                                           |
| `pages.footnotes`         | boolean           | absent      | Only `false`; requires an otherwise effective inherited margin-footnote setting                                                             | Suppresses that inherited default for this descriptor subtree                                      |
| `pages.metadata.required` | array of strings  | `[]`        | `title`, `description`, `summary`, `authors`, `published`, `updated`, or `extra.<name>`                                                     | Adds requirements for descendant pages                                                             |

Requirements accumulate from `content/<locale>/_directory.toml` to a page or nested descriptor. A page template overrides its inherited default; `template = false` explicitly disables rendering.

## `*.page`

Each `.page` defines a page. Its ID is its path below its `content/<locale>/` tree without `.page`. Without a template, it has no output path and cannot be resolved with `page()`.

| Field                          | Type                       | Default                  | Allowed values and constraints                                                                                                           | Template field / effect                                                      |
|--------------------------------|----------------------------|--------------------------|------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------|
| `template`                     | string or boolean          | inherited, or absent     | Template below `templates/`, or `false`                                                                                                  | Determines output; `false` disables inherited rendering                      |
| `draft`                        | boolean                    | absent                   | Only `true`; omit for ordinary publication; never inherited                                                                              | Excludes this page from ordinary builds and exposes `page.draft` in previews |
| `title`                        | string                     | absent                   | TOML string                                                                                                                              | `page.title`                                                                 |
| `description`                  | string                     | absent                   | TOML string                                                                                                                              | `page.description`                                                           |
| `summary`                      | string                     | absent                   | Authored TOML string; never derived from Markdown                                                                                        | `page.summary`                                                               |
| `authors`                      | array of strings           | inherited, or absent     | Non-empty author names                                                                                                                   | `page.authors`; required for Atom feed entries                               |
| `published`                    | TOML local date-time       | absent                   | `2026-08-14T09:30:00`; requires `site.time_zone`                                                                                         | `page.published` as UTC RFC 3339 text                                        |
| `updated`                      | TOML local date-time       | absent                   | Same form as `published`; must not be earlier than it                                                                                    | `page.updated` as UTC RFC 3339 text                                          |
| `sitemap`                      | boolean                    | absent                   | Only `false`, and only when `[sitemap]` is enabled                                                                                       | Excludes this rendered page and its derived pagers from the sitemap          |
| `toc.content`                  | string                     | absent                   | Rendered page; simple declared-content name for one unsegmented `.md` file, rendered exactly once with `markdown(content="…")`           | Selects the page's table-of-contents source                                  |
| `toc.min_level`                | integer                    | `1`                      | Integer from `1` through `6`                                                                                                             | Lowest heading level exposed through `page.toc`                              |
| `toc.max_level`                | integer                    | `6`                      | Integer from `1` through `6`; not lower than `min_level`                                                                                 | Highest heading level exposed through `page.toc`                             |
| `toc`                          | boolean                    | absent                   | Only `false`; requires an otherwise effective inherited table of contents                                                                | Suppresses that inherited directory default for this page                    |
| `footnotes.content`            | string                     | absent                   | Rendered page; simple declared-content name for one unsegmented `.md` file; requires the `footnotes` extension and global marker message | Selects the page's margin-footnote source                                    |
| `footnotes`                    | boolean                    | absent                   | Only `false`; requires an otherwise effective inherited margin-footnote setting                                                          | Suppresses that inherited directory default for this page                    |
| `taxonomies.<name>`            | non-empty array of strings | absent                   | `<name>` must be configured, the page must belong to its source collection, and terms are literal portable filename components           | `page.taxonomies.<name>` term links                                          |
| `weight`                       | integer or boolean         | `0`, except index pages  | Integer, or exactly `false` on a non-index page                                                                                          | `page.weight`; `false` excludes a non-index page from sibling ordering       |
| `suffix`                       | string                     | `".html"`                | Non-empty extension without `/`, `\`, NUL, `.`, or `..`                                                                                  | Rendered filename extension                                                  |
| `translationid`                | string                     | absent                   | Non-empty opaque identifier; at most once per locale                                                                                     | Explicitly joins pages across locales as `page.translations`                 |
| `collections`                  | array of strings           | inherited, or `[]`       | Unique configured collection names; only rendered pages may belong                                                                       | Replaces the inherited membership default; `[]` opts out                     |
| `series`                       | array of tables            | `[]`                     | Each table has configured `id` and positive `position`; membership is not inherited; only rendered titled pages may belong               | Adds this page to one or more ordered locale-local Series                    |
| `pagination.collection`        | string                     | absent                   | Configured collection name; page must render                                                                                             | Makes this authored page a collection listing                                |
| `pagination.per_page`          | integer                    | required when paginating | Positive integer                                                                                                                         | Collection members on each pager                                             |
| `pagination.segment`           | string                     | `"page"`                 | Literal ASCII letters/digits or `-._~!$&'()*+,;=@`; no percent escapes, `.`/`..`, or trailing `.`                                        | Generated-pager route segment                                                |
| `pagination.navigation.window` | integer                    | `1`                      | Integer from `1` through `3`                                                                                                             | Adjacent links on each side of the current pager; first and last always show |
| `content.<name>`               | string                     | absent                   | `<name>` is a simple template identifier; path is relative to page and remains under its locale tree                                     | `page.content.<name>`; input is not copied                                   |
| `extra`                        | TOML table                 | empty table              | Nested keys begin with ASCII letter or `_`, then letters, digits, or `_`                                                                 | `page.extra`                                                                 |

`index.page` has no default `weight` and is excluded from sibling ordering unless it has a numeric weight.

`draft = true` keeps an unfinished page out of an ordinary build and every
relationship derived from it, including links, collections, feeds, sitemaps,
and search. Use `heine serve --drafts` to preview it in its real site context.
When a published Markdown or template reference names an excluded draft, its
checked diagnostic names the draft source path and state. A local
`heine serve --drafts` preview can include the target when it has an effective
template.
`draft = false` is a checked error because absence already means published, and
`_directory.toml` cannot declare `draft` because publication state is local to
one page. A draft's declared content paths remain reserved from copied assets,
but their files, metadata, template, and Markdown are checked only in that
preview.

`published` and `updated` must include both a calendar date and a clock time,
without a UTC offset. A bare TOML date and an offset-bearing timestamp are
rejected; Heine resolves the local time through `site.time_zone` before
exposing UTC RFC 3339 text to templates.

## Sibling navigation

The preceding page fields define one page's metadata and output. The following
directory-local relationship is exposed through the template values listed
later in this reference.

Sibling navigation is a directory-local sequence, independent of collections.
For each locale and immediate page directory, Heine includes every rendered
page with a numeric `weight`. It sorts them by ascending weight and then by
page ID, so equal weights have a stable order. A page without a numeric weight,
including a non-index page with `weight = false`, does not participate. `index.page`
also needs an explicit numeric weight to participate.

`siblings()` returns all other participating page IDs in the same order. The
optional `id` argument selects an exact rendered page; `locale=` is allowed
only with `id=`. `sibling_neighbors()` returns the immediate values around the
current authored page as `before` and `after`: the preceding and following
members of the sibling sequence. Either is `null` at an end. It fails when the
current page does not participate in sibling ordering.

```tera
{% set neighbors = sibling_neighbors() %}
{% if neighbors.before %}
  <a href="{{ neighbors.before.url }}">Previous</a>
{% endif %}
{% if neighbors.after %}
  <a href="{{ neighbors.after.url }}">Next</a>
{% endif %}
```

Use a collection when a listing needs its own configured ordering; sibling
navigation has one fixed directory-and-weight model.

## Output filenames

Rendered pages, like copied assets, must derive portable output filename
components. They cannot use empty components, `.`, `..`, Windows device names,
trailing dots or spaces, or `<`, `>`, `:`, `"`, `/`, `\`, `|`, `?`, `*`, or
control characters. Heine rejects output paths that differ only by case, so a
published tree does not depend on the deployment filesystem's case-sensitivity
policy. It also rejects NFC-equivalent Unicode spellings, such as composed and
decomposed forms of `café`. Use one NFC-normalized spelling; Heine preserves
that literal spelling rather than silently rewriting it. A page with `template
= false` has no output path and is not subject to this output rule.

Within each content tree, template tree, and locale-resource tree, filenames
must likewise be distinct after NFC normalization. Heine preserves their
literal spelling, but names both source paths when it finds an ambiguity.

## Declared content files

Declared content is loaded but not copied as an asset. `.toml` files become
structured TOML, `.json` files become structured JSON, and every other file is
text. If its first line is `<!-- segment: name -->`, text becomes named content
segments; dotted names create nested values and repeated names become ordered
arrays. Markdown links and images may use `asset:<locale-root-relative-id>`
for a checked copied asset; ordinary Markdown URLs are unchanged.

Segment markers are recognized before Markdown is parsed. In a segmented
Markdown file, a line that looks like `<!-- segment: name -->` is therefore a
marker even when it appears inside a fenced code block. Keep literal marker
examples in an unsegmented input, or phrase them so they are not marker-shaped.

For example, this declared text file begins with a segment marker, so it
becomes structured content rather than one text value:

```html
<!-- segment: introduction -->
An introduction.
<!-- segment: questions.title -->
First question.
<!-- segment: questions.item -->
One.
<!-- segment: questions.item -->
Two.
```

For a page declaring it as `main = "questions.md"`, templates receive
`page.content.main.introduction`, `page.content.main.questions.title`, and the
ordered array `page.content.main.questions.item`. Segmentation applies only
when the first line is a marker. Otherwise the complete file remains one text
value. The [declared-content guide](declared-content.md) explains when normal
Markdown, segments, structured data, or several inputs fit a page.

## Markdown

Heine renders Markdown with `pulldown-cmark`. The `markdown` filter renders
arbitrary trusted text. The `markdown(content="…")` function reads one named,
unsegmented Markdown declaration from the current authored page. Use the
function for a page's selected [table of contents](table-of-contents.md) or
[margin-footnote](margin-footnotes.md) source so its heading IDs, local note
instances, and page facts agree.
The following boundaries are part of the current contract:

| Behavior                              | Current rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
|---------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Metadata                              | Heine does not parse Markdown front matter. YAML or TOML front matter is ordinary Markdown source; page metadata belongs in the adjacent `.page` file.                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Extensions                            | Only the configured `tables`, `strikethrough`, `task_lists`, and `footnotes` extensions are enabled. The `tables` extension alone renders ordinary native tables. Its optional `[markdown.tables]` setting wraps parsed Markdown tables in a focusable, localized `role="region"` `.heine-table-scroll` container, so site CSS can make that wrapper scroll on narrow screens without changing native table semantics. Trusted raw HTML tables remain author-owned. Task-list checkboxes receive an accessible name from their direct authored task text; nested list text is excluded. Empty tasks have no authored name. |
| Raw HTML                              | Trusted Markdown HTML is passed through unsanitized. Do not use untrusted Markdown as site input. Search indexes read semantic text and `img` alternatives from authored raw HTML, but never scrape rendered template HTML.                                                                                                                                                                                                                                                                                                                                                                                                |
| Heading anchors and table of contents | An opted-in page table of contents creates IDs only on headings in its selected source when rendered with `markdown(content="…")`; other Markdown remains unanchored. See [table of contents](table-of-contents.md).                                                                                                                                                                                                                                                                                                                                                                                                       |
| Margin footnotes                      | An opted-in page margin-footnote source replaces ordinary endnotes with checked, numbered local note instances when rendered with `markdown(content="…")`. References must occur in an ordinary paragraph or heading. See [margin footnotes](margin-footnotes.md).                                                                                                                                                                                                                                                                                                                                                         |
| Ordinary footnotes                    | With the `footnotes` extension, every `[^label]` reference needs a definition. Otherwise, outside a selected margin-footnote source, Heine preserves `pulldown-cmark`'s ordinary endnote behavior: unreferenced definitions are permitted, and definitions remain where the author placed them and in that source order. Generated ordinary and margin-footnote fragments share the reserved `__heine-footnote-…` namespace, separate from generated heading fragments. Trusted raw HTML and templates remain responsible for avoiding manually authored `id` collisions.                                                  |
| Ordinary links and images             | Ordinary URLs, including relative URLs such as `./other.md`, are emitted unchanged. Heine does not resolve, convert, or check them.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Checked internal links                | In Markdown links, use `page:<id>` or `page:<locale>:<id>` for a rendered page. In links or image sources, use `asset:<id>` for a copied asset. These forms resolve during the build and report invalid targets at the authored source location.                                                                                                                                                                                                                                                                                                                                                                           |
| Color-scheme pictures                 | A standalone `:::picture` block names one `alt` string and checked `light` and `dark` image `asset:<id>` sources. An optional plain-text `caption` emits a native `figure` and `figcaption`. Optional positive integer `width` and `height` values set emitted HTML dimensions. Heine emits detected dimensions only for a known matched pair, derives one missing authored dimension from the light source when possible, and rejects known ratios that would change layout or distort the image. See [color-scheme Markdown pictures](markdown-pictures.md).                                                             |

`markdown.math` and `markdown.highlighting` add the optional math and
syntax-highlighting behavior described in the [`heine.toml`](#heinetoml)
table. They do not add a general client-side JavaScript or CSS pipeline.

## Renderer adapters

A [renderer profile](glossary.md#renderer-profile) lives in the root
`heine.toml` under `[renderers.<name>]`. It describes one trusted external
program, its fixed arguments, a positive timeout, accepted input forms, and
the expected result shape. A valid profile remains inert until authored
Markdown selects it and the build includes `--allow-external-renderers`.
Heine never discovers, installs, or version-checks that program.

The stream protocol accepts a non-empty input subset of `"body"` and `"file"`.
The workspace protocol also accepts `"bundle"`. Both protocols support HTML,
MathML, SVG, image, and link result profiles when they declare a fixed-argument
inline or block variant. HTML must be UTF-8. MathML and embedded or published
SVG must be well-formed XML with `math` and `svg` document elements
respectively; Heine strips XML declarations and doctypes before embedding.
When a renderer fails, Heine labels its complete Markdown request and presents
the program's diagnostic output separately without rewriting it. For a
workspace body request, paths and line numbers in that output refer to the
isolated staged input.

A workspace profile may render the same embedded result shapes from a body,
declared file, or declared bundle. Heine creates one temporary `input/`,
`output/`, and `work/` layout per request, stages only the checked input,
runs the program in `work/`, and accepts only its host-selected regular file
at `{output_file}`. It expands `{input_file}`, `{input_dir}`, `{output_file}`
and `{work_dir}` only when an argument is exactly that placeholder. Extra
output files, missing or empty primary output, failed processes, and timeouts
fail at the request. Heine removes the temporary workspace after execution
when the operating system permits it. If cleanup fails, Heine retains that
workspace slot, warns about its path, and retries cleanup before another
request uses the slot.
SVG, image, and link resource requests publish one non-empty primary output as
`heine-renderers/sha256-<digest>.<extension>`. The digest names the returned
bytes, so identical results share one generated file. Heine replaces image
requests with an `img` element using the required `alt` text and link requests
with an `a` element using the required `label` text. Resource bytes remain
outside copied-asset licensing coverage.

```toml
[renderers.katex]
protocol = "stream"
program = "katex"
timeout_seconds = 10
cache_identity = "katex-0.16.27"
inputs = ["body"]
result = "html"

[renderers.katex.inline]
arguments = ["--format", "htmlAndMathml"]
```

Use an inline request:

```markdown
:render[katex]`body`
```

Or use a block request:

```markdown
:::render[katex]
x^2
:::
```

A request may instead select a page-owned `[render.<name>] file = "…"`
declaration when the profile accepts `"file"`; declared files are read as
renderer input and are never published as copied assets. Stream programs read
their input on standard input and write one replacement to standard output.
Heine captures bounded standard-error text for a failed program and terminates
the direct child when it exceeds `timeout_seconds`. It does not manage a
renderer's descendant process tree.

Heine caches successful renderer results below `.heine/cache/renderers/v1/`,
outside published output. The key includes the profile identity and effective
fixed configuration, placement, requested output form, and the complete
checked input bytes. A source bundle contributes every checked relative path
and file. A profile's `cache_identity` is therefore the site owner's deliberate
promise to update when the installed renderer or its meaningful environment
changes. `--refresh-renderers` bypasses reads for one build and atomically
replaces matching entries. Heine removes incomplete cache companions from an
interrupted write, while complete entries remain disposable working state.

## `data/`

`data/` is optional and non-published. Files must be `.json` or `.toml`; their extensionless path below `data/` becomes a dot-addressable template key. `data/site.toml` becomes `data.site`, while `data/navigation/main.json` becomes `data.navigation.main`. Each path component and file stem is a simple identifier. Key collisions and symbolic links are rejected. TOML calendar dates and date-times become template strings, so `format_date()` can format a bare `YYYY-MM-DD` data value. A configured locale directory such as `data/de/` is reserved for `locale_data` while rendering German pages and never falls back to shared data.

## `deploy/`

`deploy/` is optional. Each regular file below it is copied byte-for-byte to
the matching output-root path; it is neither an asset nor template input. For
example, `deploy/.htaccess` is published as `public/.htaccess`, and
`deploy/google123.html` becomes `public/google123.html`. Use it for
host-specific configuration or external verification files, not for files a
template needs to link with `asset()`.

Deployment paths use the same UTF-8, symbolic-link, portable-filename,
case-collision, Unicode-normalization, reserved-output, and output-collision
rules as copied assets. A full build removes output for deleted deployment
files; `--quick` only updates present files and can leave deleted paths stale.
See the [deployment-files guide](deployment-files.md) for the rationale and
examples.

## `LICENSES/` and asset sidecars

`LICENSES/` is optional. Direct `.txt` files named for an SPDX license or
exception identifier, or `LicenseRef-…`, are copied to
`<build.output_dir>/LICENSES/`.
An adjacent copied-asset sidecar named `asset.ext.license`, or one exact
`[[licensing.groups]]` declaration, supplies `SPDX-FileCopyrightText`,
`SPDX-License-Identifier`, and optional `SPDX-FileComment` facts. Sidecars are
not copied. Each identifier in an expression must have matching published text;
malformed declarations, orphan sidecars, overlapping group members, and a
group-sidecar conflict are errors. `licensing.required = true` requires one
such declaration for every copied asset. Groups use literal published
`output_path` values, without a leading slash or `site.base_path`. See the
[asset-licensing guide](asset-licensing.md). The `.license` suffix is reserved
for these sidecars and is never copied as an asset.

Each `[[licensing.groups]]` table has the following strict fields:

| Field                     | Type             | Default  | Constraints                                                                                        | Effect                                        |
|---------------------------|------------------|----------|----------------------------------------------------------------------------------------------------|-----------------------------------------------|
| `assets`                  | array of strings | required | Non-empty distinct literal copied-asset `output_path` values; no leading slash or `site.base_path` | Exact published assets that share this record |
| `SPDX-FileCopyrightText`  | string or array  | required | One or more non-empty values                                                                       | Ordered copyright notices                     |
| `SPDX-License-Identifier` | string or array  | required | One or more non-empty SPDX expressions; an array does not imply `AND` or `OR`                      | Ordered authored SPDX tag values              |
| `SPDX-FileComment`        | string or array  | absent   | Non-empty values when present                                                                      | Optional ordered comments                     |

## Template contract

Templates use Tera2. Heine provides only the following render-facing values;
they are scoped to the locale currently being rendered unless noted otherwise.

| Value               | Fields and meaning                                                                                                                                                                                                                                                                                                                                                                                                          |
|---------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `kit`               | `null` without a selected kit. Otherwise `extra` holds the kit manifest defaults overlaid by checked root `[kit.extra]` settings. Kit templates and root adapters can read this value; it does not expose the manifest or filesystem paths.                                                                                                                                                                                 |
| `site`              | `url` is present only when `site.origin` is configured. It is the canonical site root formed from the origin and base path, including the trailing slash. Templates never receive the raw origin. See [absolute URLs](absolute-urls.md).                                                                                                                                                                                    |
| `locale`            | `code` is the configured BCP-47 locale; `direction` is its configured `"ltr"` or `"rtl"` value. Use both on the document element.                                                                                                                                                                                                                                                                                           |
| `page`              | An authored page's `id`, `path`, `filename`, `dir`, `subdirs`, `content`, `markdown`, `title`, `description`, `summary`, `authors`, `published`, `updated`, `extra`, `directory`, `taxonomies`, `series`, `toc`, `footnotes`, `draft`, `weight`, `translations`, and `absolute_url`; `null` for generated templates. `path` and `filename` are absent without rendering. `absolute_url` is present only with `site.origin`. |
| `page.markdown`     | Facts keyed by declared Markdown content name. Each record has deterministic `word_count` and nullable `reading_time`; non-Markdown inputs are absent. See [word counts and reading time](markdown-facts.md).                                                                                                                                                                                                               |
| `page.draft`        | `true` for an authored draft included through `heine serve --drafts`, otherwise `false`. It never appears for a generated template. See [drafts](drafts.md).                                                                                                                                                                                                                                                                |
| `page.toc`          | `null` unless the authored page has an effective table-of-contents setting. Otherwise `content` names the selected declaration and `headings` is the source-order array of `level`, `depth`, `text`, and `fragment`. See [table of contents](table-of-contents.md).                                                                                                                                                         |
| `page.footnotes`    | `null` unless the authored page has an effective margin-footnote setting. Otherwise `content` names the selected declaration. Render that source exactly once through `markdown(content="…")`. See [margin footnotes](margin-footnotes.md).                                                                                                                                                                                 |
| `page.directory`    | `id`, `title`, `description`, `summary`, and `extra` from the immediate directory descriptor.                                                                                                                                                                                                                                                                                                                               |
| `page.translations` | Known counterparts keyed by locale code; each has `id`, output-relative `path`, and `absolute_url` when `site.origin` is configured.                                                                                                                                                                                                                                                                                        |
| `page.taxonomies`   | Assigned terms keyed by taxonomy name. Each term has literal `name`, base-path-aware `url`, current-output-relative `relative_url`, and `absolute_url` when `site.origin` is configured.                                                                                                                                                                                                                                    |
| `page.series`       | Authored-order Series memberships. Each has `id`, Fluent-resolved `title`, optional `description`, authored `position`, resolved one-based `number`, `total`, base-path-aware `url`, current-output-relative `relative_url`, `absolute_url` when configured, and optional membership-local `previous` and `next` page links. See [Series](series.md).                                                                       |
| `taxonomy`          | `null` for authored pages. A taxonomy index receives `name`, `url`, `relative_url`, `absolute_url` when configured, and ordered `terms`; a term listing also receives `term`. Each term has `name`, `count`, the same URL fields, and explicitly configured `counterparts`, keyed by locale code.                                                                                                                           |
| `series`            | `null` for authored pages. A generated Series index has `id`, `title`, optional `description`, `count`, URL fields, and local non-empty Series entries. A generated listing has `id`, title, optional description, count, URL fields, and ordered `page_ids`.                                                                                                                                                               |
| `data`              | Shared values from `data/`.                                                                                                                                                                                                                                                                                                                                                                                                 |
| `locale_data`       | Values from `data/<locale>/`; it never falls back to shared data.                                                                                                                                                                                                                                                                                                                                                           |
| `root`              | Relative prefix from the rendered page to the site root.                                                                                                                                                                                                                                                                                                                                                                    |
| `pagination`        | `null` for ordinary pages; the current generated-listing view otherwise, including `first_item_number` for continuing ordered-list markers across pager pages. Every pager link has `absolute_url` when `site.origin` is configured. See [pagination](pagination.md).                                                                                                                                                       |

`page.dir` is the locale-relative containing directory, or the empty string
for a page at the locale root. `page.subdirs` is its sorted immediate list of
source subdirectory names. They describe the source-tree location; they do not
discover pages or create output.

Combine `root` with a counterpart's `path` for a relative language-switcher
link, or pass an exact locale and ID to `page()` for its base-path-aware URL.
All Heine functions check their authored inputs and fixed keyword names at
the template call site. The additional keyword names accepted by `t()` are
Fluent message arguments. Heine checks their value types while Fluent
formats the selected message; a message may ignore an argument
it does not use.

Template inheritance and components use Tera2's own syntax; the
[tutorial](tutorial.md#reuse-the-pager-as-a-tera2-component) shows one
component. Heine does not provide a configuration or plugin hook for
registering site-defined Rust filters or functions. The function and filter
tables below are the complete Heine additions to a template environment.

With a selected kit, ordinary template names still resolve only below root
`templates/`. A kit original uses `kit/<name>/…`; a root template can name it
only when the kit declares that original public. Kit templates cannot name root
templates. Root templates may invoke only manifest-declared public kit
components, and root components cannot occupy the selected kit's component
namespace. See [Kits](kits.md).

| Function               | Use                                                                                                   | Result                                                                                                                                                                                                                                                                                                                                                                                                                    |
|------------------------|-------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `asset`                | `asset(id="assets/logo.svg")`                                                                         | A copied asset in the current locale: `url`, `relative_url`, `absolute_url` when configured, optional `integrity`, `width`, `height`, and optional `licensing` (`copyrights`, `expressions`, `comments`, `license_texts`). Shared assets use the `assets/` prefix.                                                                                                                                                        |
| `attributions`         | `attributions()`                                                                                      | Deterministic inventory of assets with resolved licensing declarations. Each item has `asset` (`output_path`, `url`, `relative_url`, `absolute_url` when configured) and `licensing` (`copyrights`, `expressions`, `comments`, `license_texts`). Each license text has `id`, `url`, `relative_url`, and `absolute_url` when configured; `output_path` is the locale-qualified published path, not an `asset()` lookup ID. |
| `page`                 | `page(id="guide/install")`                                                                            | A rendered page in the current locale, including its render-facing metadata, content, `url`, `relative_url`, and `absolute_url` when configured.                                                                                                                                                                                                                                                                          |
| `page`                 | `page(locale="de", id="hilfe/installieren")`                                                          | An exact rendered page in the named configured locale; this never guesses a translation counterpart.                                                                                                                                                                                                                                                                                                                      |
| `markdown`             | `markdown(content="main")`                                                                            | Trusted HTML from the named declared, unsegmented Markdown content of the current authored page. It applies matching heading IDs and checked local margin-footnote structure when that content is selected by either feature.                                                                                                                                                                                             |
| `page_exists`          | `page_exists(id="guide/index")`                                                                       | Whether a rendered page exists in the current locale. It returns `false` for a missing page, but invalid IDs and locales still fail at the call site.                                                                                                                                                                                                                                                                     |
| `collection`           | `collection(name="posts")`                                                                            | A configured current-locale collection with `name`, `count`, and ordered `page_ids`.                                                                                                                                                                                                                                                                                                                                      |
| `collection_neighbors` | `collection_neighbors(name="posts")`                                                                  | Immediate rendered-page views around the current authored page in the named current-locale collection order. `before` is the preceding member and `after` the following member; either is `null` at an end. It fails if the collection is unknown or the current page is not a member.                                                                                                                                    |
| `search`               | `search(name="site")` or `search(name="site", locale="de")`                                           | A configured JSON search index for the current or exact configured locale: `name`, selected-document `count`, base-path-aware `url`, current-output-relative `relative_url`, and `absolute_url` when configured. Invalid or unknown names and locales fail at the call site.                                                                                                                                              |
| `feed`                 | `feed(name="posts")` or `feed(name="posts", locale="de")`                                             | A configured Atom feed for the current or exact configured locale: `name`, Fluent-resolved `title`, base-path-aware `url`, current-output-relative `relative_url`, and `absolute_url` when configured. Invalid or unknown names and locales fail at the call site.                                                                                                                                                        |
| `taxonomy`             | `taxonomy(name="tags")` or `taxonomy(name="tags", locale="de")`                                       | A configured taxonomy index in the current or an exact configured locale, with `name`, base-path-aware `url`, current-output-relative `relative_url`, and `absolute_url` when configured. Invalid or unknown names and locales fail at the call site.                                                                                                                                                                     |
| `series`               | `series()` or `series(id="getting-started")`                                                          | The current locale's generated Series index, or one non-empty local Series. `locale=` selects an exact configured locale. An invalid ID, disabled Series, invalid locale, or direct lookup of an empty local Series fails at the call site.                                                                                                                                                                               |
| `series_has_members`   | `series_has_members(id="getting-started")` or `series_has_members(id="getting-started", locale="de")` | Whether a configured Series has local members in the current or selected exact locale. It returns `false` only for an empty local Series; invalid IDs, locales, and disabled Series fail at the call site.                                                                                                                                                                                                                |
| `siblings`             | `siblings()` or `siblings(id="blog/index")`                                                           | Ordered IDs for the other eligible pages in the selected page’s directory. See [sibling navigation](#sibling-navigation). `locale=` is allowed only with `id=`.                                                                                                                                                                                                                                                           |
| `sibling_neighbors`    | `sibling_neighbors()`                                                                                 | Immediate rendered-page views around the current authored page in directory sibling order. `before` is the preceding member and `after` the following member; either is `null` at an end. It fails if the current page has no numeric sibling weight. See [sibling navigation](#sibling-navigation).                                                                                                                      |
| `t`                    | `t(id="entry-count", count=entries \| length)`                                                        | Escaped Fluent text for the current locale. It falls back to the default locale only when the message or requested attribute is absent. Use `attribute="label"` for a Fluent attribute; other named arguments must be strings, numbers, or booleans.                                                                                                                                                                      |
| `format_date`          | `format_date(value=page.published, style="long")`                                                     | A locale-formatted calendar date. Values are RFC 3339 timestamps, such as `page.published`, or bare `YYYY-MM-DD` values supplied by template data; styles are `short`, `medium` (default), and `long`.                                                                                                                                                                                                                    |
| `highlight_css`        | `highlight_css()`                                                                                     | The base-path-aware URL for the configured generated highlighting stylesheet. It fails if highlighting is disabled.                                                                                                                                                                                                                                                                                                       |
| `pagination_links`     | `pagination_links()`                                                                                  | Every lightweight pager link for the current paginated listing. It fails at the call site outside such a render.                                                                                                                                                                                                                                                                                                          |

`asset()` IDs and `page()` IDs are relative to the current locale tree and never begin with `/`. Assign a function result with `{% set guide = page(id="guide") %}` before reading a field such as `guide.url`; Tera does not support a field access directly on a function call. `asset()` can resolve a shared asset such as `assets/logo.svg`; a non-default locale can override it below its own `assets/` directory. `page()` resolves only rendered pages.

IDs are literal source filenames, not URL fragments: do not pre-encode spaces,
`#`, `%`, or non-ASCII characters. Heine percent-encodes a resolved output
path exactly once when it exposes `url`, `relative_url`, or `absolute_url`. For non-ASCII names,
use the same NFC-normalized spelling as the filename on disk; Heine preserves
that literal spelling rather than normalizing lookup IDs.

`highlight_css()` returns the base-path-aware URL for a stylesheet that Heine
generates at `markdown.highlighting.stylesheet`. Its content comes from the
configured light and dark Giallo themes and includes the corresponding
`prefers-color-scheme: dark` rule; it is not a copied or user-authored asset.
It is available only when `[markdown.highlighting]` is configured.

| Filter       | Input           | Result                                                                                                                                                                                |
|--------------|-----------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `markdown`   | text            | Trusted HTML rendered under `[markdown]` settings. It cannot render an exact selected table-of-contents or margin-footnote source; use `markdown(content="…")` for that page content. |
| `latest`     | copied asset ID | URL for the highest versioned matching asset                                                                                                                                          |
| `dimensions` | copied asset ID | `width` and `height` for an image, or no dimensions for a known non-image                                                                                                             |

For `latest` and `dimensions`, a relative input is resolved from `page.dir`; a leading `/` makes it locale-root-relative.
