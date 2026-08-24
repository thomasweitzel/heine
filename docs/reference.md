# Heine reference

This is the complete reference for Heine-authored files and command options. The [README](../README.md) is the guided introduction.

`heine.toml`, `.page`, and `_directory.toml` use strict schemas: unknown
fields are rejected. TOML loaded from `data/` or declared as page content is
arbitrary structured data.

## Site root and commands

A site root contains `content/` and `templates/`; `heine.toml` and `data/` are optional. Without `--root`, Heine checks the current directory and each parent directory. It never searches child directories.

| Command       | Option             | Default                    | Allowed values     | Effect                                            |
|---------------|--------------------|----------------------------|--------------------|---------------------------------------------------|
| `heine`       | `--root <path>`    | nearest ancestor site root | existing site root | Selects the site root                             |
| `heine`       | `--quick`          | off                        | flag               | Updates managed output for changed page, declared-content, inherited-descriptor, data, and present-asset inputs; it remains incomplete |
| `heine`       | `--clean-output`   | off                        | flag               | Adopts unmanaged output; conflicts with `--quick` |
| `heine serve` | `--root <path>`    | nearest ancestor site root | existing site root | Selects the site root                             |
| `heine serve` | `--clean-output`   | off                        | flag               | Adopts unmanaged output before its initial build  |
| `heine serve` | `--host <address>` | `127.0.0.1`                | IP address         | Interface on which to listen                      |
| `heine serve` | `--port <number>`  | `1111`                     | `1024`–`65535`     | First port to try; then the next free port        |

`--quick` does not apply to `serve`.

## `heine.toml`

`heine.toml` is optional. Omitted fields use the following defaults.

| Field                                    | Type             | Default                               | Allowed values and constraints                                                                                                     | Effect                                               |
|------------------------------------------|------------------|---------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------|
| `site.base_path`                         | string           | `"/"`                                 | ASCII absolute URL directory path; starts and ends with `/`; uppercase percent escapes only; no encoded separators or dot segments | Prefix for absolute URLs from `asset()` and `page()` |
| `build.output_dir`                       | string           | `"public"`                            | Non-empty relative filesystem path without `.` or `..`; cannot be an input directory or descendant                                 | Generated output directory relative to site root     |
| `assets.integrity`                       | string           | absent                                | `"sha256"`, `"sha384"`, `"sha512"`                                                                                                 | Enables Subresource Integrity values from `asset()`  |
| `markdown.extensions`                    | array of strings | `[]`                                  | Unique subset of `"tables"`, `"strikethrough"`, `"task_lists"`, `"footnotes"`                                                      | Enables Markdown extensions                          |
| `markdown.math`                          | string           | `"off"`                               | `"off"`, `"mathml"`                                                                                                                | Selects ordinary Markdown or MathML TeX rendering    |
| `markdown.highlighting.stylesheet`       | string           | absent                                | Safe site-output-relative path ending in `.css`; cannot use `__heine/`                                                             | Generated class-based syntax stylesheet              |
| `markdown.highlighting.missing_language` | string           | `"error"`                             | `"error"`, `"plain"`                                                                                                               | Unknown fenced-language policy                       |
| `markdown.highlighting.theme.light`      | string           | required when highlighting is enabled | Bundled Giallo theme name                                                                                                          | Light syntax palette                                 |
| `markdown.highlighting.theme.dark`       | string           | required when highlighting is enabled | Bundled Giallo theme name                                                                                                          | Dark syntax palette                                  |

## `_directory.toml`

Place `_directory.toml` in any `content/` directory. It is non-rendered, non-published, and provides directory metadata plus recursive policy.

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
| `pages.metadata.required` | array of strings                    | `[]`        | Same values as `metadata.required`                           | Adds requirements for descendant pages                |

Requirements accumulate from `content/_directory.toml` to a page or nested descriptor. A page template overrides its inherited default; `template = false` explicitly disables rendering.

## `*.page`

Each `.page` defines a page. Its ID is its path below `content/` without `.page`. Without a template, it remains available through `pages` but has no output path.

| Field            | Type                                | Default                 | Allowed values and constraints                                           | Template field / effect                                               |
|------------------|-------------------------------------|-------------------------|--------------------------------------------------------------------------|-----------------------------------------------------------------------|
| `template`       | string or boolean                   | inherited, or absent    | Template below `templates/`, or `false`                                  | Determines output; `false` disables inherited rendering               |
| `title`          | string                              | absent                  | TOML string                                                              | `page.title`                                                          |
| `description`    | string                              | absent                  | TOML string                                                              | `page.description`                                                    |
| `summary`        | string                              | absent                  | Authored TOML string; never derived from Markdown                        | `page.summary`                                                        |
| `date`           | TOML date or offset-aware date-time | absent                  | `2026-08-14` or `2026-08-14T09:30:00+02:00`; local date-times rejected   | `page.date` as ISO 8601/RFC 3339 text                                 |
| `tags`           | array of strings                    | `[]`                    | TOML strings                                                             | `page.tags`; site mapping is `tags`                                   |
| `weight`         | integer or string                   | `0`, except index pages | Integer, or exactly `"None"`                                             | `page.weight`; `"None"` excludes non-index page from sibling ordering |
| `suffix`         | string                              | `".html"`               | Non-empty extension without `/`, `\`, NUL, `.`, or `..`                  | Rendered filename extension                                           |
| `content.<name>` | string                              | absent                  | `<name>` is a simple template identifier; path is relative to page and remains under `content/` | `page.content.<name>`; input is not copied                            |
| `extra`          | TOML table                          | empty table             | Nested keys begin with ASCII letter or `_`, then letters, digits, or `_` | `page.extra`                                                          |

`index.page` has no default `weight` and is excluded from sibling ordering unless it has a numeric weight.

### Declared content files

Declared content is loaded but not copied as an asset. `.toml` files become
structured TOML, `.json` files become structured JSON, and every other file is
text. If its first line is `<!-- segment: name -->`, text becomes named content
segments; dotted names create nested values and repeated names become ordered
arrays. Markdown links and images may use `asset:<content-root-relative-id>`
for a validated copied asset; ordinary Markdown URLs are unchanged.

## `data/`

`data/` is optional and non-published. Files must be `.json` or `.toml`; their extensionless path below `data/` becomes a dot-addressable template key. `data/site.toml` becomes `data.site`, while `data/navigation/main.json` becomes `data.navigation.main`. Each path component and file stem is a simple identifier. Key collisions and symbolic links are rejected.

## Template contract

Templates use Tera2. Heine supplies `page`, `pages`, `tags`, `assets`, `data`, and `root`. A `page` has `id`, `path`, `filename`, `dir`, `subdirs`, `content`, `title`, `description`, `summary`, `date`, `extra`, `directory`, `tags`, `weight`, `siblings`, `lighter`, and `heavier`. `page.directory` has `id`, `title`, `description`, `summary`, and `extra`.

`asset(id="...")` accepts a copied asset ID relative to `content/` and returns `url`, `relative_url`, optional `integrity`, and image `width` and `height`. `page(id="...")` accepts a rendered page ID and returns `url` and `relative_url`. Both validate internal references at their template call site.

`highlight_css()` returns the base-path-aware URL for the configured generated
syntax stylesheet. It is available only when `[markdown.highlighting]` is
configured.

| Filter       | Input           | Result                                                                    |
|--------------|-----------------|---------------------------------------------------------------------------|
| `markdown`   | text            | Trusted HTML rendered under `[markdown]` settings                         |
| `latest`     | copied asset ID | URL for the highest versioned matching asset                              |
| `dimensions` | copied asset ID | `width` and `height` for an image, or no dimensions for a known non-image |

For `latest` and `dimensions`, a relative input is resolved from `page.dir`; a leading `/` makes it content-root-relative.
