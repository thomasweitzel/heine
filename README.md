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

## Documentation and downloads

See the [Download page](https://heine.rs/download/) for release artifacts and
the complete starter site. The
[documentation overview](docs/README.md) maps the implemented features to
their guides and reference material.

Heine does not bundle a frontend toolchain or deployment integration. Use
ordinary tools for CSS and JavaScript processing, media transformation,
deployment, audits, and external services.

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
title = "My site"
```

```tera
{# templates/page.tera #}
<!doctype html>
<html lang="{{ locale.code }}" dir="{{ locale.direction }}">
  <head><title>{{ page.title }}</title></head>
  <body><h1>{{ page.title }}</h1></body>
</html>
```

Run the downloaded executable:

```bash
/path/to/heine --root /path/to/my-site
```

This generates `public/index.html`. The configured default locale is
unprefixed; later locale trees receive their locale code in generated URLs.

## Preview locally

Run the development server while writing:

```bash
/path/to/heine serve --root /path/to/my-site
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

Follow the [tutorial](docs/tutorial.md) to grow this minimal site into a
multilingual blog with content, assets, data, collections, pagination, and
tags. The [documentation overview](docs/README.md) groups the focused guides by
task. Use the [glossary](docs/glossary.md) for Heine terms, the
[reference](docs/reference.md) for exact fields and template values, and the
[roadmap](docs/roadmap.md) for planned work.

## Starter site

The [downloadable starter site](starter-site) is a complete, working
multilingual example. Use it after the tutorial when you want to see several
features composed in one site rather than introduced one at a time. Extract the
archive, change into its top-level directory, and run the executable downloaded
from the [Download page](https://heine.rs/download/):

```bash
/path/to/heine --root .
```

Its `public/` directory is generated output. If it already contains unrelated
files, explicitly adopt and replace it with:

```bash
/path/to/heine --root . --clean-output
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
contract. A changed present renderer input can update its selected page, but a
full build remains necessary after renderer-profile, request, or input-layout
changes.

## Development

Run the full local quality check before contributing changes:

These commands use Rustup to select Heine's supported compiler. Install it once
with `rustup toolchain install 1.88.0`.

```bash
cargo +1.88.0 fmt --check
cargo +1.88.0 clippy --all-targets --all-features -- -D warnings
cargo +1.88.0 test --all-targets --all-features
```

## License

Heine is available under EUPL-1.2-or-later. The copyable starter kit and its
reference-site material are available under MIT, except where a file declares
another licence. Building an independently authored site with Heine does not
place that site under the EUPL. See [LICENSING.md](LICENSING.md) for the
precise boundary, third-party exceptions, and contribution terms.
