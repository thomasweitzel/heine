# Heine

Heine is a synchronous Rust static-site generator. It compiles a
configured content tree and [Tera2](https://github.com/Keats/tera) templates
into a checked, managed output tree.

Pronounced Hei·ne, [ˈhaɪ̯nə], “Heineken” without “ken.” Named after Heinrich
Heine, the German poet and prose writer known for his wit and irony. Heine is
not affiliated with Heineken.

Heine owns the relationships it can check from a site's resolved model:
rendered pages, copied assets, locale-aware content, generated routes, and
generated resources. A missing internal page or copied asset therefore fails
where it is authored instead of becoming a browser 404.

## Status

Heine is pre-release and is not published to crates.io yet. A Linux x86-64
musl executable and a complete starter site are available from the
[Download page](https://heine.rs/download/).

It currently provides Markdown rendering with checked color-scheme pictures,
page-owned tables of contents and margin footnotes, Tera2 templates,
locale-aware content and Fluent localization, checked page and asset links,
collections, pagination, taxonomies, Series, Atom feeds, sitemaps,
search-document indexes, checked canonical absolute URLs, and a development
server with live reload, media range requests, and draft previews.
Copied assets may carry checked REUSE-compatible declarations, including
optional complete coverage and exact shared declarations for asset
sets.

Heine does not bundle a frontend toolchain or deployment integration. Use
ordinary tools for general CSS and JavaScript processing, general media
transformation, deployment, audits, and external services. Kits, plugins, and
RSS are not implemented yet.

## First site

A minimal site has a configuration file, one locale tree, and one template:

```text
my-site/
├── heine.toml
├── content/
│   └── en/
│       └── index.page
└── templates/
    └── page.tera
```

```toml
# heine.toml
[i18n]
default_locale = "en"

[i18n.locales.en]
direction = "ltr"
```

```toml
# content/en/index.page
template = "page.tera"
title = "Hello, Heine"
```

```tera
{# templates/page.tera #}
<!doctype html>
<html lang="{{ locale.code }}" dir="{{ locale.direction }}">
  <head><title>{{ page.title }}</title></head>
  <body><h1>{{ page.title }}</h1></body>
</html>
```

Build it from this repository:

```bash
cargo run -- --root /path/to/my-site
```

This generates `public/index.html`. The configured default locale is
unprefixed; later locale trees receive their locale code in generated URLs.

## Preview locally

Run the development server while writing:

```bash
cargo run -- serve --root /path/to/my-site
```

It first performs a complete build, serves the generated output, watches the
site inputs, and reloads open HTML pages after a successful rebuild. It starts
with port `1111`, then tries the next available unprivileged port.

The [development-server guide](docs/development-server.md) covers base paths,
live reload, range requests for media, failed rebuilds, and local-network use.

For a site being served below a subdirectory, set the
[base path](docs/glossary.md#base-path) and choose an output directory
appropriate for the host. The
[reference](docs/reference.md#heinetoml) gives the exact configuration rules.

## Where to go next

The [documentation map](docs/README.md) explains the available reading paths.

- Follow the [tutorial](docs/tutorial.md) to grow this minimal site into a
  multilingual blog with content, assets, data, collections, pagination, and
  tags.
- Use the [glossary](docs/glossary.md) when returning to a Heine term such as
  “page ID,” “copied asset,” or “managed output.”
- Read a focused guide when adding
  [localization](docs/internationalization.md),
  [collections](docs/collections.md), [pagination](docs/pagination.md),
  [taxonomies](docs/taxonomies.md), [deployment files](docs/deployment-files.md),
  [asset licensing](docs/asset-licensing.md), [sitemaps](docs/sitemap.md),
  [Atom feeds](docs/atom-feeds.md), [table of contents](docs/table-of-contents.md),
  [margin footnotes](docs/margin-footnotes.md),
  [color-scheme Markdown pictures](docs/markdown-pictures.md),
  [word counts and reading time](docs/markdown-facts.md),
  [absolute URLs](docs/absolute-urls.md),
  [search indexes](docs/search.md),
  [Series](docs/series.md), [drafts](docs/drafts.md),
  or the [development server](docs/development-server.md).
- Use the [reference](docs/reference.md) for exact fields, defaults, template
  functions, and constraints.
- Consult the [roadmap](docs/roadmap.md) for planned work and undecided
  boundaries.

## Starter site

[`starter-site/`](starter-site/) is a complete, working multilingual example.
Use it after the tutorial when you want to see several features composed in one
site rather than introduced one at a time:

```bash
cargo run -- --root starter-site
```

Its `public/` directory is generated output. If it already contains unrelated
files, explicitly adopt and replace it with:

```bash
cargo run -- --root starter-site --clean-output
```

## External asset tools

Heine copies final assets; it does not compile Sass, process images, bundle
JavaScript, or generate content-hashed filenames. Run such tools separately
and have them write their final files below `content/assets/` (or a locale's
asset tree). Heine then copies those files and `asset()` can link to them.

Run an asset transformation before Heine when a template references the asset
through `asset()`. Modifying managed output afterward can invalidate a
Heine-generated versioned URL or integrity value.

`heine serve` watches copied assets but does not start or supervise another
tool's watcher. Filename-based [versioned assets](docs/glossary.md#versioned-asset)
and optional integrity values are available when a site needs them; neither is
automatic content-hash generation.

## Build behavior

A normal build checks a complete site and publishes it only after rendering
and copying succeed. It replaces stale generated files left by renamed or
deleted inputs. If an existing output directory was not created by Heine, use
`--clean-output` only after confirming that it is safe to replace.

`--quick` updates changed present inputs in place. It does not
notice template, configuration, relationship, rename, or deletion changes, so
use a normal build after those changes. The [glossary](docs/glossary.md#quick-build)
and [reference](docs/reference.md#site-root-and-commands) describe the exact
contract.

## Development

Run the full local quality check before contributing changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```
