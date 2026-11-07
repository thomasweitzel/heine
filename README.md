# Heine

Heine is a fast static-site generator. It builds a complete model of your
site before it creates the files for your website. When a page, asset,
translation, listing, or generated file refers to another part of the site,
Heine can check that relationship during the build and point to its source when
it is wrong.

Pronounced Hei·ne, [ˈhaɪ̯nə], “Heineken” without “ken.” Named after Heinrich
Heine, the German poet and prose writer known for his wit and irony. Heine is
not affiliated with Heineken.

For example, a missing internal page or copied asset fails where it is
authored instead of becoming a browser 404. The same model lets multilingual
content, collections, taxonomies, and generated routes use the pages that
already exist instead of reconstructing them in templates.

## Documentation and downloads

See the [Download page](https://heine.rs/download/) for release artifacts and
the complete starter site. The [documentation overview](docs/README.md) starts
with a first site and tutorial, then groups the focused guides by task.

To build and install Heine from crates.io with Rust, run:

```bash
cargo install heine --locked
```

Use your usual tools for CSS and JavaScript processing, media transformation,
deployment, audits, and external services. Heine reads their final files and
builds the pages and relationships it can check from them.

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

The [citations guide](docs/citations.md) shows how to build a reference list
from local BibTeX and CSL files. The [publication catalogs guide](docs/publications.md)
shows how to present works from those files in templates.

## Starter site

The [downloadable starter site](starter-site) is a complete, working
multilingual example. Use it after the tutorial when you want to see several
features composed in one site rather than introduced one at a time. Extract the
archive, change into its top-level directory, and run the executable downloaded
from the [Download page](https://heine.rs/download/):

```bash
/path/to/heine --root .
```

Its [source tour](starter-site/README.md#what-this-site-demonstrates) maps each
demonstrated feature to its rendered page, source files, and focused guide.

Its `public/` directory is generated output. If it already contains unrelated
files, explicitly adopt and replace it with:

```bash
/path/to/heine --root . --clean-output
```

## Use external asset tools

Heine copies final assets. Run Sass compilers, image processors, JavaScript
bundlers, and similar tools separately, then have them write final files below
`content/assets/` or a locale's asset tree. Heine copies those files, and
`asset()` can link to them.

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

The checked-in `rust-toolchain.toml` selects Heine's supported compiler and
adds the required lint and formatting components. Rustup installs it
automatically when needed. To install it before running a check, use
`rustup toolchain install` from the repository root.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## License

Heine is available under EUPL-1.2-or-later. The copyable starter kit and its
reference-site material are available under MIT, except where a file declares
another license. Building an independently authored site with Heine does not
place that site under the EUPL. See [LICENSING.md](LICENSING.md) for the
precise boundary, third-party exceptions, and contribution terms.
