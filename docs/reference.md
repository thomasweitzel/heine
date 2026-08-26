# Heine reference

This is the complete reference for Heine-authored files and command options. The [README](../README.md) is the guided introduction.

`heine.toml`, `.page`, and `_directory.toml` use strict schemas: unknown
fields are rejected. TOML loaded from `data/` or declared as page content is
arbitrary structured data.

## Site root and commands

A site root contains `heine.toml`, `content/`, and `templates/`; `data/` and `locales/` are optional. Without `--root`, Heine checks the current directory and each parent directory. It never searches child directories.

| Command       | Option             | Default                    | Allowed values     | Effect                                                                                                                                 |
|---------------|--------------------|----------------------------|--------------------|----------------------------------------------------------------------------------------------------------------------------------------|
| `heine`       | `--root <path>`    | nearest ancestor site root | existing site root | Selects the site root                                                                                                                  |
| `heine`       | `--quick`          | off                        | flag               | Updates managed output for changed page, declared-content, inherited-descriptor, data, Fluent-resource, and present-asset inputs; it remains incomplete |
| `heine`       | `--clean-output`   | off                        | flag               | Adopts unmanaged output; conflicts with `--quick`                                                                                      |
| `heine serve` | `--root <path>`    | nearest ancestor site root | existing site root | Selects the site root                                                                                                                  |
| `heine serve` | `--clean-output`   | off                        | flag               | Adopts unmanaged output before its initial build                                                                                       |
| `heine serve` | `--host <address>` | `127.0.0.1`                | IP address         | Interface on which to listen                                                                                                           |
| `heine serve` | `--port <number>`  | `1111`                     | `1024`–`65535`     | First port to try; then the next free port                                                                                             |

`--quick` does not apply to `serve`.

## `heine.toml`

`heine.toml` is required. Its `[i18n]` section is required; other sections and
fields use the following defaults.

| Field                                    | Type             | Default                               | Allowed values and constraints                                                                                                     | Effect                                                                   |
|------------------------------------------|------------------|---------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| `site.base_path`                         | string           | `"/"`                                 | ASCII absolute URL directory path; starts and ends with `/`; uppercase percent escapes only; no encoded separators or dot segments | Prefix for absolute URLs from `asset()` and `page()`                     |
| `build.output_dir`                       | string           | `"public"`                            | Non-empty relative path with portable filename components; cannot be an input directory or descendant                                | Generated output directory relative to site root                         |
| `assets.integrity`                       | string           | absent                                | `"sha256"`, `"sha384"`, `"sha512"`                                                                                                 | Enables Subresource Integrity values from `asset()`                      |
| `i18n.default_locale`                    | string           | required                              | Canonical BCP-47 locale declared by an `i18n.locales.<code>` table                                                                  | Locale with unprefixed generated URLs                                    |
| `i18n.locales`                           | table            | required                              | Non-empty table keyed by canonical BCP-47 locale codes                                                                              | Finite locale source trees to render                                     |
| `i18n.locales.<code>.direction`          | string           | required                              | `"ltr"` or `"rtl"`                                                                                                               | Base HTML writing direction for that locale                              |
| `collections.<name>.order`               | string           | required                              | `"date-asc"`, `"date-desc"`, `"weight-asc"`, `"weight-desc"`, `"id-asc"`, or `"id-desc"`; `<name>` is a simple identifier | Defines a named, locale-local ordered set of rendered pages              |
| `markdown.extensions`                    | array of strings | `[]`                                  | Unique subset of `"tables"`, `"strikethrough"`, `"task_lists"`, `"footnotes"`                                                      | Enables Markdown extensions                                              |
| `markdown.math`                          | string           | `"off"`                               | `"off"`, `"mathml"`, `"tex"`                                                                                                       | Selects ordinary Markdown, build-time MathML, or escaped TeX passthrough |
| `markdown.highlighting.stylesheet`       | string           | absent                                | Safe site-output-relative path ending in `.css`; cannot use `__heine/`                                                             | Generated class-based syntax stylesheet                                  |
| `markdown.highlighting.missing_language` | string           | `"error"`                             | `"error"`, `"plain"`                                                                                                               | Unknown fenced-language policy                                           |
| `markdown.highlighting.theme.light`      | string           | required when highlighting is enabled | Bundled Giallo theme name; see the linked list below                                                                              | Light syntax palette                                                     |
| `markdown.highlighting.theme.dark`       | string           | required when highlighting is enabled | Bundled Giallo theme name; see the linked list below                                                                              | Dark syntax palette                                                      |

Collection definitions establish relationships only; they do not write output.
Every configured collection exists in every locale, including when it is
empty. Page IDs break ties in ascending order. Date orders require every member
to have the same explicit date precision (all TOML dates or all offset-aware
date-times), and weight orders require numeric weights on every member.

The complete set of accepted theme names is in
[Giallo's source README](https://docs.rs/crate/giallo/0.5.2/source/README.md)
under “Themes”; the linked source matches Heine's bundled registry.
Use those exact names; Heine reports an unknown name at its configuration
location.

## `_directory.toml`

Place `_directory.toml` in a configured `content/<locale>/` tree. It is non-rendered, non-published, and provides directory metadata plus recursive policy.

```toml
title = "Blog"

[metadata]
required = ["title"]

[pages]
template = "post.tera"

[pages.metadata]
required = ["title", "date", "extra.author"]
```

| Field                     | Type                                | Default     | Allowed values and constraints                               | Effect                                                |
|---------------------------|-------------------------------------|-------------|--------------------------------------------------------------|-------------------------------------------------------|
| `title`                   | string                              | absent      | TOML string                                                  | `page.directory.title`                                |
| `description`             | string                              | absent      | TOML string                                                  | `page.directory.description`                          |
| `summary`                 | string                              | absent      | TOML string                                                  | `page.directory.summary`                              |
| `date`                    | TOML date or offset-aware date-time | absent      | Local date-times are rejected                                | Validated metadata; not currently in `page.directory` |
| `extra`                   | TOML table                          | empty table | Dot-addressable keys; rules below                            | `page.directory.extra`                                |
| `metadata.required`       | array of strings                    | `[]`        | `title`, `description`, `summary`, `date`, or `extra.<name>` | Adds requirements for this and nested descriptors     |
| `pages.template`          | string or boolean                   | absent      | Template below `templates/`, or `false`                      | Recursive default; closest descriptor wins            |
| `pages.collections`       | array of strings                    | inherited   | Unique configured collection names; an empty array opts out  | Recursive default membership for descendant pages     |
| `pages.metadata.required` | array of strings                    | `[]`        | Same values as `metadata.required`                           | Adds requirements for descendant pages                |

Requirements accumulate from `content/<locale>/_directory.toml` to a page or nested descriptor. A page template overrides its inherited default; `template = false` explicitly disables rendering.

## `*.page`

Each `.page` defines a page. Its ID is its path below its `content/<locale>/` tree without `.page`. Without a template, it has no output path and cannot be resolved with `page()`.

| Field            | Type                                | Default                 | Allowed values and constraints                                                                       | Template field / effect                                               |
|------------------|-------------------------------------|-------------------------|------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------|
| `template`       | string or boolean                   | inherited, or absent    | Template below `templates/`, or `false`                                                              | Determines output; `false` disables inherited rendering               |
| `title`          | string                              | absent                  | TOML string                                                                                          | `page.title`                                                          |
| `description`    | string                              | absent                  | TOML string                                                                                          | `page.description`                                                    |
| `summary`        | string                              | absent                  | Authored TOML string; never derived from Markdown                                                    | `page.summary`                                                        |
| `date`           | TOML date or offset-aware date-time | absent                  | `2026-08-14` or `2026-08-14T09:30:00+02:00`; local date-times rejected                               | `page.date` as ISO 8601/RFC 3339 text                                 |
| `tags`           | array of strings                    | `[]`                    | TOML strings                                                                                         | `page.tags`; use `tag(name="…")` for the locale's ordered members    |
| `weight`         | integer or string                   | `0`, except index pages | Integer, or exactly `"None"`                                                                         | `page.weight`; `"None"` excludes non-index page from sibling ordering |
| `suffix`         | string                              | `".html"`               | Non-empty extension without `/`, `\`, NUL, `.`, or `..`                                              | Rendered filename extension                                           |
| `translationid`  | string                              | absent                  | Non-empty opaque identifier; at most once per locale                                                 | Explicitly joins pages across locales as `page.translations`          |
| `collections`    | array of strings                    | inherited, or `[]`       | Unique configured collection names; only rendered pages may belong                                    | Replaces the inherited membership default; `[]` opts out              |
| `pagination.collection` | string                       | absent                   | Configured collection name; page must render                                                         | Makes this authored page a collection listing                          |
| `pagination.per_page` | integer                          | required when paginating | Positive integer                                                                                      | Collection members on each pager                                      |
| `pagination.segment` | string                           | `"page"`                | Literal ASCII letters/digits or `-._~!$&'()*+,;=@`; no percent escapes, `.`/`..`, or trailing `.`   | Generated-pager route segment                                         |
| `pagination.navigation.window` | integer               | `1`                      | Integer from `1` through `5`                                                                         | Compact navigation link radius around boundaries and current pager    |
| `content.<name>` | string                              | absent                  | `<name>` is a simple template identifier; path is relative to page and remains under its locale tree | `page.content.<name>`; input is not copied                            |
| `extra`          | TOML table                          | empty table             | Nested keys begin with ASCII letter or `_`, then letters, digits, or `_`                             | `page.extra`                                                          |

`index.page` has no default `weight` and is excluded from sibling ordering unless it has a numeric weight.

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

### Declared content files

Declared content is loaded but not copied as an asset. `.toml` files become
structured TOML, `.json` files become structured JSON, and every other file is
text. If its first line is `<!-- segment: name -->`, text becomes named content
segments; dotted names create nested values and repeated names become ordered
arrays. Markdown links and images may use `asset:<locale-root-relative-id>`
for a validated copied asset; ordinary Markdown URLs are unchanged.

## `data/`

`data/` is optional and non-published. Files must be `.json` or `.toml`; their extensionless path below `data/` becomes a dot-addressable template key. `data/site.toml` becomes `data.site`, while `data/navigation/main.json` becomes `data.navigation.main`. Each path component and file stem is a simple identifier. Key collisions and symbolic links are rejected. A configured locale directory such as `data/de/` is reserved for `locale_data` while rendering German pages and never falls back to shared data.

## Template contract

Templates use Tera2. Heine provides only the following render-facing values;
they are scoped to the locale currently being rendered unless noted otherwise.

| Value | Fields and meaning |
|-------|--------------------|
| `locale` | `code` is the configured BCP-47 locale; `direction` is its configured `"ltr"` or `"rtl"` value. Use both on the document element. |
| `page` | `id`, `path`, `filename`, `dir`, `subdirs`, `content`, `title`, `description`, `summary`, `date`, `extra`, `directory`, `tags`, `weight`, `lighter`, `heavier`, and `translations`. `path` and `filename` are absent without rendering. |
| `page.directory` | `id`, `title`, `description`, `summary`, and `extra` from the immediate directory descriptor. |
| `page.translations` | Known counterparts keyed by locale code; each has `id` and output-relative `path`. |
| `data` | Shared values from `data/`. |
| `locale_data` | Values from `data/<locale>/`; it never falls back to shared data. |
| `root` | Relative prefix from the rendered page to the site root. |
| `pagination` | `null` for ordinary pages; the current generated-listing view otherwise. See [pagination](pagination.md). |

Combine `root` with a counterpart's `path` for a relative language-switcher
link, or pass an exact locale and ID to `page()` for its base-path-aware URL.
All Heine functions validate their authored inputs and fixed keyword names at
the template call site. The additional keyword names accepted by `t()` are
Fluent message arguments. Heine validates their value types while Fluent
formats the selected message; a message may deliberately ignore an argument
it does not use.

| Function        | Use                                           | Result                                                                                                                                                                                                                                               |
|-----------------|-----------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `asset`         | `asset(id="assets/logo.svg")`                 | A copied asset in the current locale: `url`, `relative_url`, optional `integrity`, `width`, and `height`. Shared assets use the `assets/` prefix.                                                                                                    |
| `page`          | `page(id="guide/install")`                    | A rendered page in the current locale, including its render-facing metadata, content, and `url` / `relative_url`.                                                                                                                                    |
| `page`          | `page(locale="de", id="hilfe/installieren")`  | An exact rendered page in the named configured locale; this never guesses a translation counterpart.                                                                                                                                                 |
| `page_exists`   | `page_exists(id="quiz/index")`                 | Whether a rendered page exists in the current locale. It returns `false` for a missing page, but invalid IDs and locales still fail at the call site.                                                                                                  |
| `collection`    | `collection(name="posts")`                     | A configured current-locale collection with `name`, `count`, and ordered `page_ids`.                                                                                                                                                                  |
| `tag`           | `tag(name="news")`                              | A current-locale tag with `name`, `count`, and rendered `page_ids` in ascending page-ID order. It fails for an unknown tag.                                                                                                                              |
| `siblings`      | `siblings()` or `siblings(id="blog/index")`    | Ordered IDs for the other rendered pages in the same directory that participate in sibling ordering. `locale=` is allowed only with `id=`.                                                                                                             |
| `t`             | `t(id="quiz-count", count=quizzes \| length)` | Escaped Fluent text for the current locale. It falls back to the default locale only when the message or requested attribute is absent. Use `attribute="label"` for a Fluent attribute; other named arguments must be strings, numbers, or booleans. |
| `format_date`   | `format_date(value=page.date, style="long")`  | A locale-formatted calendar date. Values are `YYYY-MM-DD` or RFC 3339 offset-aware date-times; styles are `short`, `medium` (default), and `long`.                                                                                                   |
| `highlight_css` | `highlight_css()`                             | The base-path-aware URL for the configured generated highlighting stylesheet. It fails if highlighting is disabled.                                                                                                                                  |
| `pagination_links` | `pagination_links()`                       | Every lightweight pager link for the current paginated listing. It fails at the call site outside such a render.                                                                                                                                     |

`asset()` IDs and `page()` IDs are relative to the current locale tree and never begin with `/`. Assign a function result with `{% set guide = page(id="guide") %}` before reading a field such as `guide.url`; Tera does not support a field access directly on a function call. `asset()` can resolve a shared asset such as `assets/logo.svg`; a non-default locale can override it below its own `assets/` directory. `page()` resolves only rendered pages.

IDs are literal source filenames, not URL fragments: do not pre-encode spaces,
`#`, `%`, or non-ASCII characters. Heine percent-encodes a resolved output
path exactly once when it exposes `url` or `relative_url`. For non-ASCII names,
use the same NFC-normalized spelling as the filename on disk; Heine preserves
that literal spelling rather than normalizing lookup IDs.

`highlight_css()` returns the base-path-aware URL for the configured generated
syntax stylesheet. It is available only when `[markdown.highlighting]` is
configured.

| Filter       | Input           | Result                                                                    |
|--------------|-----------------|---------------------------------------------------------------------------|
| `markdown`   | text            | Trusted HTML rendered under `[markdown]` settings                         |
| `latest`     | copied asset ID | URL for the highest versioned matching asset                              |
| `dimensions` | copied asset ID | `width` and `height` for an image, or no dimensions for a known non-image |

For `latest` and `dimensions`, a relative input is resolved from `page.dir`; a leading `/` makes it locale-root-relative.
