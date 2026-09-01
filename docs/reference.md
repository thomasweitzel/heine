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

| Command       | Option             | Default                    | Allowed values     | Effect                                                                                                                                                                                                  |
|---------------|--------------------|----------------------------|--------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `heine`       | `--root <path>`    | nearest ancestor site root | existing site root | Selects the site root                                                                                                                                                                                   |
| `heine`       | `--quick`          | off                        | flag               | Updates managed output for changed page, declared content, inherited descriptor, data, Fluent resources, present copied assets and sidecars, deployment files, and license texts; it remains incomplete |
| `heine`       | `--clean-output`   | off                        | flag               | Adopts unmanaged output; conflicts with `--quick`                                                                                                                                                       |
| `heine serve` | `--root <path>`    | nearest ancestor site root | existing site root | Selects the site root                                                                                                                                                                                   |
| `heine serve` | `--clean-output`   | off                        | flag               | Adopts unmanaged output before its initial build                                                                                                                                                        |
| `heine serve` | `--host <address>` | `127.0.0.1`                | IP address         | Interface on which to listen                                                                                                                                                                            |
| `heine serve` | `--port <number>`  | `1111`                     | `1024`–`65535`     | First port to try; then the next free port                                                                                                                                                              |

`--quick` does not apply to `serve`.

## `heine.toml`

`heine.toml` is required. Its `[i18n]` section is required; other sections and
fields use the following defaults.

| Field                                             | Type                 | Default                               | Allowed values and constraints                                                                                                                                        | Effect                                                                                      |
|---------------------------------------------------|----------------------|---------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `site.base_path`                                  | string               | `"/"`                                 | ASCII absolute URL directory path; starts and ends with `/`; uppercase percent escapes only; no encoded separators or dot segments                                    | Prefix for Heine-generated absolute URLs, including `asset()`, `page()`, and taxonomy links |
| `site.origin`                                     | string               | absent                                | Absolute `http` or `https` origin only: host and optional port, without credentials, query, fragment, or deployment path                                              | Required by sitemaps and Atom feeds for canonical URLs                                      |
| `site.time_zone`                                  | string               | absent                                | IANA time-zone name such as `"Europe/Berlin"`; resolved from Heine's bundled database                                                                                 | Resolves authored `published` and `updated` local times deterministically                   |
| `build.output_dir`                                | string               | `"public"`                            | Non-empty relative path with portable filename components; cannot be `content/`, `data/`, `deploy/`, `LICENSES/`, `templates/`, `locales/`, or a descendant           | Generated output directory relative to site root                                            |
| `assets.integrity`                                | string               | absent                                | `"sha256"`, `"sha384"`, `"sha512"`                                                                                                                                    | Enables Subresource Integrity values from `asset()`                                         |
| `i18n.default_locale`                             | string               | required                              | Canonical BCP-47 locale declared by an `i18n.locales.<code>` table                                                                                                    | Locale with unprefixed generated URLs                                                       |
| `i18n.locales`                                    | table                | required                              | Non-empty table keyed by canonical BCP-47 locale codes                                                                                                                | Finite locale source trees to render                                                        |
| `i18n.locales.<code>.direction`                   | string               | required                              | `"ltr"` or `"rtl"`                                                                                                                                                    | Base HTML writing direction for that locale                                                 |
| `collections.<name>.order`                        | string               | required                              | `"published-asc"`, `"published-desc"`, `"title-asc"`, `"title-desc"`, `"weight-asc"`, `"weight-desc"`, `"id-asc"`, or `"id-desc"`; `<name>` is a simple identifier    | Defines a named, locale-local ordered set of rendered pages                                 |
| `search.<name>.collections`                       | array of strings     | required                              | Non-empty array of distinct configured collection names; `<name>` is a simple identifier                                                                                  | Selects and unions locale-local members for one JSON search index                           |
| `search.<name>.path`                              | string               | required                              | Portable relative output path ending in lowercase `.json`, outside [`__heine/`](glossary.md#heine)                                                                         | Locale-relative logical JSON output path                                                    |
| `search.<name>.versioned`                         | boolean              | `true`                                | `true` or `false`                                                                                                                                                         | Inserts a full JSON SHA-256 digest before `.json`, or retains the stable configured path    |
| `feeds.<name>.collection`                         | string               | required                              | Configured collection name; `<name>` is a simple identifier                                                                                                           | Selects the locale-local members that supply Atom entries                                   |
| `feeds.<name>.path`                               | string               | required                              | Portable relative output path outside [`__heine/`](glossary.md#heine)                                                                                                 | Locale-relative Atom output path                                                            |
| `feeds.<name>.title_key`                          | string               | required                              | Non-empty Fluent message identifier                                                                                                                                   | Locale-local Atom feed title                                                                |
| `feeds.<name>.subtitle_key`                       | string               | absent                                | Non-empty Fluent message identifier                                                                                                                                   | Optional locale-local Atom feed subtitle                                                    |
| `feeds.<name>.limit`                              | integer              | `20`                                  | Positive integer                                                                                                                                                      | Maximum chronological entries in each locale feed                                           |
| `feeds.<name>.empty_updated`                      | TOML local date-time | conditionally required                | Same form as `published`; required when a locale feed is empty                                                                                                        | Atom update time for an empty locale feed                                                   |
| `feeds.<name>.empty_authors`                      | array of strings     | conditionally required                | Non-empty author names; required when a locale feed is empty                                                                                                          | Atom authors for an empty locale feed                                                       |
| `taxonomies.<name>.collection`                    | string               | required                              | Configured collection name; `<name>` is a simple identifier                                                                                                           | Selects and orders the taxonomy's rendered members                                          |
| `taxonomies.<name>.path`                          | string               | required                              | Portable relative output path outside [`__heine/`](glossary.md#heine)                                                                                                 | Generated taxonomy root                                                                     |
| `taxonomies.<name>.index_template`                | string               | required                              | Logical template name below `templates/`                                                                                                                              | Renders the taxonomy's term index                                                           |
| `taxonomies.<name>.term_template`                 | string               | required                              | Logical template name below `templates/`                                                                                                                              | Renders each term listing and its pagers                                                    |
| `taxonomies.<name>.per_page`                      | integer              | required                              | Positive integer                                                                                                                                                      | Members on each generated term-listing page                                                 |
| `taxonomies.<name>.counterparts.<group>.<locale>` | string               | absent                                | An existing literal term for a configured locale; each non-empty opaque `<group>` has at least two locale members, and a locale-term pair occurs in at most one group | Declares a deliberate cross-locale term counterpart                                         |
| `sitemap`                                         | table                | absent                                | Empty table; requires `site.origin`                                                                                                                                   | Generates canonical sitemap output                                                          |
| `markdown.extensions`                             | array of strings     | `[]`                                  | Unique subset of `"tables"`, `"strikethrough"`, `"task_lists"`, `"footnotes"`                                                                                         | Enables Markdown extensions                                                                 |
| `markdown.math`                                   | string               | `"off"`                               | `"off"`, `"mathml"`, `"tex"`                                                                                                                                          | Selects ordinary Markdown, build-time MathML, or escaped TeX passthrough                    |
| `markdown.highlighting.stylesheet`                | string               | absent                                | Safe site-output-relative path ending in `.css`; cannot use [`__heine/`](glossary.md#heine)                                                                           | Generated class-based syntax stylesheet                                                     |
| `markdown.highlighting.missing_language`          | string               | `"error"`                             | `"error"`, `"plain"`                                                                                                                                                  | Unknown fenced-language policy                                                              |
| `markdown.highlighting.theme.light`               | string               | required when highlighting is enabled | Bundled Giallo theme name; see the linked list below                                                                                                                  | Light syntax palette                                                                        |
| `markdown.highlighting.theme.dark`                | string               | required when highlighting is enabled | Bundled Giallo theme name; see the linked list below                                                                                                                  | Dark syntax palette                                                                         |

Collection definitions establish relationships only; they do not write output.
Every configured collection exists in every locale, including when it is
empty. Page IDs are the final deterministic tie-breaker. Published orders
require every member to declare `published`; title orders use the collection locale's ICU
collation and require titles, and weight orders require numeric weights on
every member.

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
  "format": 1,
  "locale": "en",
  "documents": [
    {
      "id": "blog/example/index",
      "url": "/guide/blog/example/index.html",
      "title": "Example",
      "description": null,
      "summary": "A short example.",
      "published": "2026-08-14T07:30:00Z",
      "updated": null,
      "taxonomies": { "tags": ["Rust"] },
      "text": "Source-derived searchable text."
    }
  ]
}
```

`format` is the positive schema version, currently `1`. `id` is the
locale-relative page ID and `url` names the literal generated page file with
`site.base_path` applied. `title`, `description`, `summary`, `published`, and
`updated` are their authored page facts or `null`. `taxonomies` has configured
taxonomy names in deterministic lexical order, with literal locale-local terms
in authored order. Documents are ordered by page ID.

`text` combines every declared `.md` input in content-name order, separated by
two line feeds. It includes parsed text, headings, link labels, image
alternatives, code, and recognized math source. Source-authored raw HTML adds
text nodes and `img` `alt` values, while markup, comments, URLs, attributes,
and `script`, `style`, or `template` contents do not contribute. Soft and hard
Markdown breaks remain line feeds; block boundaries become two line feeds.
Heine does not lower-case, tokenize, stem, truncate, or derive text from
rendered HTML.

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

| Field                     | Type              | Default     | Allowed values and constraints                                                          | Effect                                                                                             |
|---------------------------|-------------------|-------------|-----------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|
| `title`                   | string            | absent      | TOML string                                                                             | `page.directory.title`                                                                             |
| `description`             | string            | absent      | TOML string                                                                             | `page.directory.description`                                                                       |
| `summary`                 | string            | absent      | TOML string                                                                             | `page.directory.summary`                                                                           |
| `extra`                   | TOML table        | empty table | Dot-addressable keys; rules below                                                       | `page.directory.extra`                                                                             |
| `metadata.required`       | array of strings  | `[]`        | `title`, `description`, `summary`, or `extra.<name>`                                    | Adds requirements for this and nested descriptors                                                  |
| `pages.template`          | string or boolean | absent      | Template below `templates/`, or `false`                                                 | Recursive default; closest descriptor wins                                                         |
| `pages.collections`       | array of strings  | inherited   | Unique configured collection names; an empty array opts out                             | Recursive default membership for descendant pages                                                  |
| `pages.authors`           | array of strings  | inherited   | Non-empty author names                                                                  | Recursive author default; a page list replaces the nearest one                                     |
| `pages.sitemap`           | boolean           | absent      | Only `false`, and only when global `[sitemap]` is enabled                               | Recursively excludes descendant rendered pages and their pagers; no descendant may re-include them |
| `pages.toc.content`       | string            | absent      | Simple declared-content name for one unsegmented `.md` file; each effective descendant must render | Recursive default table-of-contents source                                                         |
| `pages.toc.min_level`     | integer           | `1`        | Integer from `1` through `6`                                                           | Lowest heading level exposed through the table of contents                                         |
| `pages.toc.max_level`     | integer           | `6`        | Integer from `1` through `6`; not lower than `min_level`                               | Highest heading level exposed through the table of contents                                        |
| `pages.toc`               | boolean           | absent      | Only `false`; requires an otherwise effective inherited table of contents               | Suppresses that inherited default for this descriptor subtree                                      |
| `pages.metadata.required` | array of strings  | `[]`        | `title`, `description`, `summary`, `authors`, `published`, `updated`, or `extra.<name>` | Adds requirements for descendant pages                                                             |

Requirements accumulate from `content/<locale>/_directory.toml` to a page or nested descriptor. A page template overrides its inherited default; `template = false` explicitly disables rendering.

## `*.page`

Each `.page` defines a page. Its ID is its path below its `content/<locale>/` tree without `.page`. Without a template, it has no output path and cannot be resolved with `page()`.

| Field                          | Type                       | Default                  | Allowed values and constraints                                                                                                 | Template field / effect                                               |
|--------------------------------|----------------------------|--------------------------|--------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------|
| `template`                     | string or boolean          | inherited, or absent     | Template below `templates/`, or `false`                                                                                        | Determines output; `false` disables inherited rendering               |
| `title`                        | string                     | absent                   | TOML string                                                                                                                    | `page.title`                                                          |
| `description`                  | string                     | absent                   | TOML string                                                                                                                    | `page.description`                                                    |
| `summary`                      | string                     | absent                   | Authored TOML string; never derived from Markdown                                                                              | `page.summary`                                                        |
| `authors`                      | array of strings           | inherited, or absent     | Non-empty author names                                                                                                         | `page.authors`; required for Atom feed entries                        |
| `published`                    | TOML local date-time       | absent                   | `2026-08-14T09:30:00`; requires `site.time_zone`                                                                               | `page.published` as UTC RFC 3339 text                                 |
| `updated`                      | TOML local date-time       | absent                   | Same form as `published`; must not be earlier than it                                                                          | `page.updated` as UTC RFC 3339 text                                   |
| `sitemap`                      | boolean                    | absent                   | Only `false`, and only when `[sitemap]` is enabled                                                                             | Excludes this rendered page and its derived pagers from the sitemap   |
| `toc.content`                  | string                     | absent                   | Rendered page; simple declared-content name for one unsegmented `.md` file                                                    | Selects the page's table-of-contents source                           |
| `toc.min_level`                | integer                    | `1`                      | Integer from `1` through `6`                                                                                                   | Lowest heading level exposed through `page.toc`                        |
| `toc.max_level`                | integer                    | `6`                      | Integer from `1` through `6`; not lower than `min_level`                                                                       | Highest heading level exposed through `page.toc`                       |
| `toc`                          | boolean                    | absent                   | Only `false`; requires an otherwise effective inherited table of contents                                                      | Suppresses that inherited directory default for this page             |
| `taxonomies.<name>`            | non-empty array of strings | absent                   | `<name>` must be configured, the page must belong to its source collection, and terms are literal portable filename components | `page.taxonomies.<name>` term links                                   |
| `weight`                       | integer or string          | `0`, except index pages  | Integer, or exactly `"None"`                                                                                                   | `page.weight`; `"None"` excludes non-index page from sibling ordering |
| `suffix`                       | string                     | `".html"`                | Non-empty extension without `/`, `\`, NUL, `.`, or `..`                                                                        | Rendered filename extension                                           |
| `translationid`                | string                     | absent                   | Non-empty opaque identifier; at most once per locale                                                                           | Explicitly joins pages across locales as `page.translations`          |
| `collections`                  | array of strings           | inherited, or `[]`       | Unique configured collection names; only rendered pages may belong                                                             | Replaces the inherited membership default; `[]` opts out              |
| `pagination.collection`        | string                     | absent                   | Configured collection name; page must render                                                                                   | Makes this authored page a collection listing                         |
| `pagination.per_page`          | integer                    | required when paginating | Positive integer                                                                                                               | Collection members on each pager                                      |
| `pagination.segment`           | string                     | `"page"`                 | Literal ASCII letters/digits or `-._~!$&'()*+,;=@`; no percent escapes, `.`/`..`, or trailing `.`                              | Generated-pager route segment                                         |
| `pagination.navigation.window` | integer                    | `1`                      | Integer from `1` through `5`                                                                                                   | Compact navigation link radius around boundaries and current pager    |
| `content.<name>`               | string                     | absent                   | `<name>` is a simple template identifier; path is relative to page and remains under its locale tree                           | `page.content.<name>`; input is not copied                            |
| `extra`                        | TOML table                 | empty table              | Nested keys begin with ASCII letter or `_`, then letters, digits, or `_`                                                       | `page.extra`                                                          |

`index.page` has no default `weight` and is excluded from sibling ordering unless it has a numeric weight.

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
including a page with `weight = "None"`, does not participate. `index.page`
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
navigation deliberately has one fixed directory-and-weight model.

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

For example, this declared text file begins with a segment marker, so it
becomes structured content rather than one text value:

```html
<!-- segment: intro -->
An introduction.
<!-- segment: questions.title -->
First question
<!-- segment: questions.item -->
One
<!-- segment: questions.item -->
Two
```

For a page declaring it as `main = "quiz.txt"`, templates receive
`page.content.main.intro`, `page.content.main.questions.title`, and the array
`page.content.main.questions.item`. Segmentation applies only when the first
line is a marker. Otherwise the complete file remains one text value.

## Markdown

Heine renders Markdown with `pulldown-cmark`. The `markdown` filter renders
arbitrary trusted text. The `markdown(content="…")` function reads one named,
unsegmented Markdown declaration from the current authored page. Use the
function for a page's selected [table of contents](table-of-contents.md)
source so its heading IDs and `page.toc` fragments agree.
The following boundaries are part of the current contract:

| Behavior                              | Current rule                                                                                                                                                                                                                                     |
|---------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Metadata                              | Heine does not parse Markdown front matter. YAML or TOML front matter is ordinary Markdown source; page metadata belongs in the adjacent `.page` file.                                                                                           |
| Extensions                            | Only the configured `tables`, `strikethrough`, `task_lists`, and `footnotes` extensions are enabled.                                                                                                                                             |
| Raw HTML                              | Trusted Markdown HTML is passed through unsanitized. Do not use untrusted Markdown as site input. Search indexes read semantic text and `img` alternatives from authored raw HTML, but never scrape rendered template HTML. |
| Heading anchors and table of contents | An opted-in page table of contents creates IDs only on headings in its selected source when rendered with `markdown(content="…")`; other Markdown remains unanchored. See [table of contents](table-of-contents.md).                       |
| Ordinary links and images             | Ordinary URLs, including relative URLs such as `./other.md`, are emitted unchanged. Heine does not resolve, convert, or check them.                                                                                                              |
| Checked internal links                | In Markdown links, use `page:<id>` or `page:<locale>:<id>` for a rendered page. In links or image sources, use `asset:<id>` for a copied asset. These forms resolve during the build and report invalid targets at the authored source location. |

`markdown.math` and `markdown.highlighting` add the optional math and
syntax-highlighting behavior described in the [`heine.toml`](#heinetoml)
table. They do not add a general client-side JavaScript or CSS pipeline.

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
exception identifier, or `LicenseRef-…`, are copied to `public/LICENSES/`.
An adjacent copied-asset sidecar named `asset.ext.license` supplies
`SPDX-FileCopyrightText`, `SPDX-License-Identifier`, and optional
`SPDX-FileComment` declarations. Sidecars are not copied. Each identifier in
an expression must have matching published text; malformed declarations and
orphan sidecars are errors. See the [asset-licensing guide](asset-licensing.md).
The `.license` suffix is reserved for these sidecars and is never copied as an
asset.

## Template contract

Templates use Tera2. Heine provides only the following render-facing values;
they are scoped to the locale currently being rendered unless noted otherwise.

| Value               | Fields and meaning                                                                                                                                                                                                                                                                                             |
|---------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `locale`            | `code` is the configured BCP-47 locale; `direction` is its configured `"ltr"` or `"rtl"` value. Use both on the document element.                                                                                                                                                                              |
| `page`              | An authored page's `id`, `path`, `filename`, `dir`, `subdirs`, `content`, `title`, `description`, `summary`, `authors`, `published`, `updated`, `extra`, `directory`, `taxonomies`, `toc`, `weight`, and `translations`; `null` for taxonomy-generated templates. `path` and `filename` are absent without rendering. |
| `page.toc`          | `null` unless the authored page has an effective table-of-contents setting. Otherwise `content` names the selected declaration and `headings` is the source-order array of `level`, `depth`, `text`, and `fragment`. See [table of contents](table-of-contents.md). |
| `page.directory`    | `id`, `title`, `description`, `summary`, and `extra` from the immediate directory descriptor.                                                                                                                                                                                                                  |
| `page.translations` | Known counterparts keyed by locale code; each has `id` and output-relative `path`.                                                                                                                                                                                                                             |
| `page.taxonomies`   | Assigned terms keyed by taxonomy name. Each term has literal `name` plus base-path-aware `url` and current-output-relative `relative_url`.                                                                                                                                                                     |
| `taxonomy`          | `null` for authored pages. A taxonomy index receives `name` and ordered `terms`; a term listing also receives `term`. Each term has `name`, `count`, `url`, `relative_url`, and explicitly configured `counterparts`, keyed by locale code.                                                                    |
| `data`              | Shared values from `data/`.                                                                                                                                                                                                                                                                                    |
| `locale_data`       | Values from `data/<locale>/`; it never falls back to shared data.                                                                                                                                                                                                                                              |
| `root`              | Relative prefix from the rendered page to the site root.                                                                                                                                                                                                                                                       |
| `pagination`        | `null` for ordinary pages; the current generated-listing view otherwise. See [pagination](pagination.md).                                                                                                                                                                                                      |

`page.dir` is the locale-relative containing directory, or the empty string
for a page at the locale root. `page.subdirs` is its sorted immediate list of
source subdirectory names. They describe the source-tree location; they do not
discover pages or create output.

Combine `root` with a counterpart's `path` for a relative language-switcher
link, or pass an exact locale and ID to `page()` for its base-path-aware URL.
All Heine functions check their authored inputs and fixed keyword names at
the template call site. The additional keyword names accepted by `t()` are
Fluent message arguments. Heine checks their value types while Fluent
formats the selected message; a message may deliberately ignore an argument
it does not use.

Template inheritance and components use Tera2's own syntax; the
[tutorial](tutorial.md#reuse-the-pager-as-a-tera2-component) shows one
component. Heine does not provide a configuration or plugin hook for
registering site-defined Rust filters or functions. The function and filter
tables below are the complete Heine additions to a template environment.

| Function               | Use                                                             | Result                                                                                                                                                                                                                                                                                                                                       |
|------------------------|-----------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `asset`                | `asset(id="assets/logo.svg")`                                   | A copied asset in the current locale: `url`, `relative_url`, optional `integrity`, `width`, `height`, and optional `licensing` (`copyrights`, `expressions`, `comments`, `license_texts`). Shared assets use the `assets/` prefix.                                                                                                           |
| `attributions`         | `attributions()`                                                | Deterministic inventory of assets with licensing sidecars. Each item has `asset` (`output_path`, `url`, `relative_url`) and `licensing` (`copyrights`, `expressions`, `comments`, `license_texts`). Each license text has `id`, `url`, and `relative_url`; `output_path` is the locale-qualified published path, not an `asset()` lookup ID. |
| `page`                 | `page(id="guide/install")`                                      | A rendered page in the current locale, including its render-facing metadata, content, and `url` / `relative_url`.                                                                                                                                                                                                                            |
| `page`                 | `page(locale="de", id="hilfe/installieren")`                    | An exact rendered page in the named configured locale; this never guesses a translation counterpart.                                                                                                                                                                                                                                         |
| `markdown`             | `markdown(content="main")`                                       | Trusted HTML from the named declared, unsegmented Markdown content of the current authored page. It applies matching heading IDs when that content is the page's selected table-of-contents source.                                                                                             |
| `page_exists`          | `page_exists(id="quiz/index")`                                  | Whether a rendered page exists in the current locale. It returns `false` for a missing page, but invalid IDs and locales still fail at the call site.                                                                                                                                                                                        |
| `collection`           | `collection(name="posts")`                                      | A configured current-locale collection with `name`, `count`, and ordered `page_ids`.                                                                                                                                                                                                                                                         |
| `collection_neighbors` | `collection_neighbors(name="posts")`                            | Immediate rendered-page views around the current authored page in the named current-locale collection order. `before` is the preceding member and `after` the following member; either is `null` at an end. It fails if the collection is unknown or the current page is not a member.                                                       |
| `search`               | `search(name="site")` or `search(name="site", locale="de")` | A configured JSON search index for the current or exact configured locale: `name`, selected-document `count`, base-path-aware `url`, and current-output-relative `relative_url`. Invalid or unknown names and locales fail at the call site.                                                                                                  |
| `feed`                 | `feed(name="posts")` or `feed(name="posts", locale="de")`       | A configured Atom feed for the current or exact configured locale: `name`, Fluent-resolved `title`, base-path-aware `url`, and current-output-relative `relative_url`. Invalid or unknown names and locales fail at the call site.                                                                                                           |
| `taxonomy`             | `taxonomy(name="tags")` or `taxonomy(name="tags", locale="de")` | A configured taxonomy index in the current or an exact configured locale, with `name`, base-path-aware `url`, and current-output-relative `relative_url`. Invalid or unknown names and locales fail at the call site.                                                                                                                        |
| `siblings`             | `siblings()` or `siblings(id="blog/index")`                     | Ordered IDs for the other eligible pages in the selected page’s directory. See [sibling navigation](#sibling-navigation). `locale=` is allowed only with `id=`.                                                                                                                                                                              |
| `sibling_neighbors`    | `sibling_neighbors()`                                           | Immediate rendered-page views around the current authored page in directory sibling order. `before` is the preceding member and `after` the following member; either is `null` at an end. It fails if the current page has no numeric sibling weight. See [sibling navigation](#sibling-navigation).                                         |
| `t`                    | `t(id="quiz-count", count=quizzes \| length)`                   | Escaped Fluent text for the current locale. It falls back to the default locale only when the message or requested attribute is absent. Use `attribute="label"` for a Fluent attribute; other named arguments must be strings, numbers, or booleans.                                                                                         |
| `format_date`          | `format_date(value=page.published, style="long")`               | A locale-formatted calendar date. Values are RFC 3339 timestamps, such as `page.published`, or bare `YYYY-MM-DD` values supplied by template data; styles are `short`, `medium` (default), and `long`.                                                                                                                                       |
| `highlight_css`        | `highlight_css()`                                               | The base-path-aware URL for the configured generated highlighting stylesheet. It fails if highlighting is disabled.                                                                                                                                                                                                                          |
| `pagination_links`     | `pagination_links()`                                            | Every lightweight pager link for the current paginated listing. It fails at the call site outside such a render.                                                                                                                                                                                                                             |

`asset()` IDs and `page()` IDs are relative to the current locale tree and never begin with `/`. Assign a function result with `{% set guide = page(id="guide") %}` before reading a field such as `guide.url`; Tera does not support a field access directly on a function call. `asset()` can resolve a shared asset such as `assets/logo.svg`; a non-default locale can override it below its own `assets/` directory. `page()` resolves only rendered pages.

IDs are literal source filenames, not URL fragments: do not pre-encode spaces,
`#`, `%`, or non-ASCII characters. Heine percent-encodes a resolved output
path exactly once when it exposes `url` or `relative_url`. For non-ASCII names,
use the same NFC-normalized spelling as the filename on disk; Heine preserves
that literal spelling rather than normalizing lookup IDs.

`highlight_css()` returns the base-path-aware URL for a stylesheet that Heine
generates at `markdown.highlighting.stylesheet`. Its content comes from the
configured light and dark Giallo themes and includes the corresponding
`prefers-color-scheme: dark` rule; it is not a copied or user-authored asset.
It is available only when `[markdown.highlighting]` is configured.

| Filter       | Input           | Result                                                                    |
|--------------|-----------------|---------------------------------------------------------------------------|
| `markdown`   | text            | Trusted HTML rendered under `[markdown]` settings. It cannot render an exact selected table-of-contents source; use `markdown(content="…")` for that page content. |
| `latest`     | copied asset ID | URL for the highest versioned matching asset                              |
| `dimensions` | copied asset ID | `width` and `height` for an image, or no dimensions for a known non-image |

For `latest` and `dimensions`, a relative input is resolved from `page.dir`; a leading `/` makes it locale-root-relative.
