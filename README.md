# Heine

Heine is a small, synchronous static-site generator written in Rust.

Hei·ne, [ˈhaɪ̯nə], _Heineken_ without _ken_

Named after Heinrich Heine, the German poet and prose writer known for his wit
and irony.

It turns a `content/` tree and [Tera2](https://github.com/Keats/tera)
templates into a `public/` tree.

## Status

Heine is an early-stage static-site generator. It prioritizes explicit content
composition and build-time validation of internal references over a
batteries-included workflow: for example, a missing asset or rendered page
reference fails at the template call site instead of in the browser. For now,
build and run Heine from a source checkout with `cargo run`.

Sitemaps, RSS/Atom feeds, and a plugin system are deliberately out of scope
for now.

## Contents

- [First site](#first-site)
- [Build a site](#build-a-site)
- [Serve a site locally](#serve-a-site-locally)
- [Site layout](#site-layout)
- [Deployment files](#deployment-files)
- [Asset licensing](#asset-licensing)
- [Internationalization and localization](#internationalization-and-localization)
- [Global data](#global-data)
- [Pages and content](#pages-and-content)
  - [Directory descriptors](#directory-descriptors)
- [Collections](#collections)
- [Taxonomies](#taxonomies)
- [Pagination](#pagination)
- [Templates](#templates)
- [Configuration](#configuration)
- [Build behavior](#build-behavior)
- [Starter site](#starter-site)
- [Development](#development)

For the complete, field-by-field contract for every authored site file and
command-line option, see the [Heine reference](docs/reference.md).

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

This builds `content/en/index.page` as `public/index.html`. The default locale
is unprefixed; add another configured locale and its source tree when the site
needs another language. The working [`starter-site/`](starter-site/) is the
best next example for pages with Markdown, data, assets, localization, and
collections, and taxonomies.

## Build a site

Install a current stable Rust toolchain, then run Heine from this repository:

```bash
cargo run -- --root /path/to/site
```

If `--root` is omitted, Heine starts in the current directory and walks toward
its parent directories until it finds one containing `heine.toml`, `content/`,
and `templates/`.

Heine treats a full-build output directory as generated and managed output. If
you are adopting an existing non-empty directory for the first time, confirm
that it may be replaced:

```bash
cargo run -- --root /path/to/site --clean-output
```

Use quick mode when only source inputs have changed:

```bash
cargo run -- --root /path/to/site --quick
```

Quick mode re-renders for changed page files, declared content, inherited
directory descriptors, shared or locale `data/`, Fluent resources, and present
copied assets and their present licensing sidecars. It also copies changed
present deployment files and license texts. It intentionally does not consider
template, configuration, or page-relationship changes. Use a normal build
after those changes.

## Serve a site locally

For local development, let Heine build, serve, and watch a site:

```bash
cargo run -- serve --root /path/to/site
```

The server performs a complete build before it starts, serves the generated
`build.output_dir` directory, and watches `content/`, `data/`, `deploy/`,
`LICENSES/`, `locales/`, `templates/`, and `heine.toml`. After a successful
rebuild, open pages reload automatically.
Its timestamp-seeded reload generation also refreshes a page that reconnects
after the development server restarts.
It never serves an in-memory representation or writes its reload client to the
generated site; the client is injected only into HTML responses in development.
The server publishes the output directory and base path from the same
successful build. If a rebuild fails, the previously successful output remains
available and the browser is not reloaded; the diagnostic is printed in the
terminal. If the operating system reports a watcher failure, Heine identifies
the watched project root and suggests saving again or running a full build.

All development responses use `Cache-Control: no-store`, so a browser reload
always fetches current generated files. If a successful rebuild changes
`site.base_path`, an open page moves to the equivalent route below the new base
path, preserving its query string and fragment.

As with ordinary builds, Heine refuses to replace a non-empty unmanaged output
directory. Explicitly adopt it for the initial served build only when it is
safe to replace:

```bash
cargo run -- serve --root /path/to/site --clean-output
```

By default it listens only on `127.0.0.1`, starting at port `1111`. If that
port is busy, Heine tries the next available unprivileged port and prints the
actual URL. Choose an interface or a different starting port explicitly:

```bash
cargo run -- serve --root /path/to/site --host 0.0.0.0 --port 3000
```

Ports below `1024` are rejected. `--host 0.0.0.0` exposes the development
server to the local network, so use it only on a network you trust.

The configured `site.base_path` is part of local serving. For example, with
`base_path = "/guide/"` and `output_dir = "public/guide"`, the generated
`public/guide/blog/post.html` is available at
`http://127.0.0.1:1111/guide/blog/post.html`; paths outside `/guide/` return
`404`. Requesting `/guide` redirects to `/guide/`.

The server handles `GET` and `HEAD`, guesses standard content types, and
supports a single `bytes` range for regular files. This permits local playback
and seeking for large audio or video files. Multiple or unsupported ranges are
served as a normal complete response. Full-build publication replaces the
output directory with two renames; when the directory is momentarily absent,
the server retries the lookup once. This deliberately keeps the hand-off small
and filesystem-based, but does not promise uninterrupted responses during that
tiny replacement interval. It is a development server, not a production HTTP
server: it does not implement production caching or cache revalidation, TLS,
compression, or conditional requests.

## Site layout

```text
my-site/
├── heine.toml          # required configuration, including locales
├── content/            # shared assets and one tree per configured locale
├── data/               # optional site-wide JSON and TOML
├── deploy/              # optional verbatim files for the output root
├── LICENSES/            # optional published license texts for copied assets
├── locales/            # optional Fluent resources per configured locale
├── templates/          # Tera2 templates
└── public/             # generated output
```

`content/` deliberately holds both build inputs and published assets. Its
top-level directories are `assets/` and the locale codes configured in
`heine.toml`; a source tree is required for every configured locale. A
`.page` file and every file it declares in `[content]` are build inputs, so
Heine reads them but never copies them to `public/`. Every other file below
`content/` is a published asset and is copied in the matching place below
`public/`. Because page IDs, asset IDs, and template names are text-based,
source-path components beneath `content/` and `templates/` must be valid
UTF-8 and must not contain backslashes; Heine reports an invalid path instead
of silently changing its generated URL or identifier.
Within each content tree, template tree, and locale-resource tree, filenames
must also be distinct after NFC normalization. Use one NFC-normalized spelling
for non-ASCII names; Heine preserves that literal spelling in output and URLs,
but reports both paths instead of silently choosing between equivalent names.

Every copied asset and every rendered page must additionally have portable
output filename components: they cannot be empty, `.`, `..`, Windows device
names such as `CON`, end in a dot or space, or contain `<`, `>`, `:`, `"`,
`/`, `\`, `|`, `?`, `*`, or control characters. Heine also rejects output
paths that differ only by case, rather than making a site depend on the
deployment filesystem's case-sensitivity policy. It also rejects paths that
differ only by Unicode normalization.
Content, data, deployment, license text, localization, and template files must be regular files:
symbolic links are rejected rather than silently being skipped.

`__heine/` is reserved in generated output for development-server endpoints.
Do not use that directory for pages or copied assets.

## Deployment files

`deploy/` is an optional source root for files that belong beside a deployed
site rather than inside its content model. Heine copies every regular file
below it unchanged to the matching path below `public/`: for example,
`deploy/.htaccess` becomes `public/.htaccess`, and
`deploy/google1234567890.html` becomes `public/google1234567890.html`.

Use it for host-specific configuration such as Apache headers or a Content
Security Policy, and for verification files requested by services such as
Google Search Console. Deployment files are deliberately opaque to Heine:
they are not assets, cannot be used with `asset()`, are not localized or
versioned, and are not rendered as templates. Like every output producer,
they are validated for safe portable paths and cannot collide with generated
pages, copied assets, or `__heine/`.

A normal build publishes an exact replacement output tree, including
`deploy/`; deleting a deployment file removes its output on the next normal
build. Quick builds copy changed present deployment files but deliberately do
not remove stale output. See the [deployment-files guide](docs/deployment-files.md)
for the complete contract and examples.

## Asset licensing

`LICENSES/` is an optional project-root directory for license texts associated
with copied content assets. A REUSE-style adjacent `asset.ext.license` sidecar
declares SPDX copyright and license information; it is validated, never copied
as an asset, and is available to templates through `asset().licensing` and
`attributions()`. License texts are copied unchanged to `public/LICENSES/`.
See the [asset-licensing guide](docs/asset-licensing.md) for the complete
layout, template views, and important attribution limitations.

## Global data

`data/` is an optional, non-published input root for site-wide structured
values. JSON and TOML files become fields of the read-only `data` template
value according to their path below `data/`, without the extension:

```text
data/
├── site.json
└── navigation/
    └── main.toml
```

```tera
{{ data.site.title }}
{% for link in data.navigation.main.links %}
  <a href="{{ link.path }}">{{ link.label }}</a>
{% endfor %}
```

Only `.json` and `.toml` files are allowed. Every path component must be a
simple template identifier: it starts with a letter or `_` and otherwise uses
only letters, digits, and `_`. This keeps the `data.*` interface consistent and
dot-addressable. Heine rejects unsupported files and file/directory key
collisions - for example, `data/navigation.json` together with
`data/navigation/main.toml` - rather than choosing one silently. Parse errors
identify the data file and source location. Symbolic links are rejected so the
data tree has the same source-root boundary as content and templates. Data
files are never copied to `public/`.

## Internationalization and localization

Every site explicitly declares its finite set of canonical BCP-47 locales.
Use a locale source tree even for a single-language site. The default locale
is unprefixed; every other locale receives its locale code as a URL prefix:

```toml
# heine.toml
[i18n]
default_locale = "en"

[i18n.locales.en]
direction = "ltr"

[i18n.locales.de]
direction = "ltr"

[i18n.locales.ar]
direction = "rtl"
```

```text
content/en/about.page       -> /about.html
content/de/ueber-uns.page   -> /de/ueber-uns.html
```

Every locale declares its base writing direction explicitly: `"ltr"` or
`"rtl"`. Heine does not infer direction from a BCP-47 code because one
language can use more than one script. Templates receive `locale.code` and
`locale.direction`; use both on the document element:

```tera
<html lang="{{ locale.code }}" dir="{{ locale.direction }}">
```

An LTR page may contain a separate RTL fragment, or the reverse. Mark the
authored fragment itself so the browser has an unambiguous boundary:

```html
<blockquote lang="ar" dir="rtl">
  <p>…</p>
</blockquote>
```

Heine owns document direction; site or theme CSS owns layout. Use logical CSS
properties such as `margin-inline`, `padding-inline`, and `text-align: end`
when a layout should follow its locale direction. Fonts must provide suitable
glyphs for every script a site uses.

Put reusable UI text in Fluent `.ftl` files below `locales/<locale>/` and use
`t()` in templates. Fluent select expressions provide the language-specific
plural categories needed by languages such as Polish:

```tera
{{ t(id="quiz-count", count=quizzes | length) }}
```

Read the [internationalization and localization guide](docs/internationalization.md)
before building a multilingual site. It explains source trees, shared and
locale-specific assets and data, translation groups, fallback, localized date
formatting, and validated page links. The [reference](docs/reference.md)
lists every configuration field and template function.

## Pages and content

A `.page` file is TOML. It can declare a template, explicit metadata, and
named content files:

```toml
template = "post.tera"
title = "A post"
date = 2026-08-14
collections = ["posts"]
[taxonomies]
tags = ["news"]

[extra]
featured = true

[content]
main = "post.md"
```

The page above renders to `post.html`. The core page fields are `template`,
`title`, `description`, `summary`, `date`, `[taxonomies]`, `weight`, `suffix`,
`translationid`, `collections`, `[pagination]`, `[content]`, and `[extra]`. Their defaults,
exact value rules, and render-facing fields are in the
[page reference](docs/reference.md#page).

### Directory descriptors

`content/<locale>/**/_directory.toml` is a non-published TOML descriptor for a
locale-owned content directory. It gives that directory metadata and can set a
recursive default template and metadata requirements for its pages. The closest
descriptor with `pages.template` wins; a page can override it with a template
name or use `template = false` to keep its authored content without rendering
an output page. Such a page cannot be resolved with `page()` or belong to a
collection.

```toml
# content/en/blog/_directory.toml
title = "Blog"
description = "Notes from the project."

[metadata]
required = ["title"]

[pages]
template = "post.tera"

[pages.metadata]
required = ["title", "date", "extra.author"]
```

Requirements accumulate from `content/<locale>/_directory.toml` to the page or
nested descriptor. `[metadata].required` validates encountered directory descriptors;
`[pages.metadata].required` validates descendant pages. Each required name is
one of `title`, `description`, `summary`, `date`, or a dot-addressable
`extra.<name>` path. Errors identify both the incomplete page or descriptor and
the policy that required the field.

Pages can use their immediate directory through `page.directory`, including
its `id`, `title`, `description`, `summary`, and `extra` values. This keeps
directory context explicit without turning `data/` into a filesystem API.

`extra` is intentionally small and template-facing: it must be a TOML table,
and every nested key must be a simple identifier (a letter or `_` followed by
letters, digits, or `_`). That makes values consistently available through
dot notation such as `page.extra.author.name`.

Page files and directory descriptors use strict schemas. Heine rejects unknown
fields, including misspelled policy names, rather than silently ignoring
authored configuration.

## Collections

Collections are explicit, named, locale-local ordered sets of rendered pages.
Define an ordering policy in `heine.toml`, then put membership where it belongs:
on the pages themselves or as a recursive directory default.

```toml
# heine.toml
[collections.posts]
order = "date-desc"
```

```toml
# content/en/blog/_directory.toml
[pages]
template = "post.tera"
collections = ["posts"]

# content/en/blog/index.page
template = "blog-index.tera"
collections = [] # do not list the index in its own collection
```

The available orders are `date-asc`, `date-desc`, `weight-asc`,
`weight-desc`, `id-asc`, and `id-desc`. Page ID breaks ties deterministically.
Date ordering requires a date on every member and does not allow date-only and
offset-aware date-times to be mixed; Heine will not invent a time zone or time
of day. Weight ordering requires a numeric weight on every member.

Use `collection()` to read a named collection in the current locale. Every
configured collection is available, even if it has no members:

```tera
{% set posts = collection(name="posts") %}
<p>{{ posts.count }} posts</p>
{% for id in posts.page_ids %}
  {% set post = page(id=id) %}
  <a href="{{ post.url }}">{{ post.title }}</a>
{% endfor %}
```

Collections only resolve relationships. Paginated listings and taxonomies are
their explicit generated-output consumers. See the [collection design](docs/collections.md),
[taxonomy guide](docs/taxonomies.md), and [pagination guide](docs/pagination.md)
for rationale and extension boundaries.

## Taxonomies

Taxonomies classify the explicitly selected members of one collection. A
taxonomy owns its generated index and term listings, while the collection
remains the sole source of membership order:

```toml
[taxonomies.tags]
collection = "posts"
path = "tags"
index_template = "taxonomy-index.tera"
term_template = "taxonomy-term.tera"
per_page = 20
```

Pages assign literal terms through a `[taxonomies]` table. A term is both its
display name and one route component; Heine does not silently slugify or merge
it. An ordinary page exposes its own validated term links as
`page.taxonomies.tags`; taxonomy templates receive a `taxonomy` value and the
existing `pagination` value. Read the [taxonomy guide](docs/taxonomies.md) for
the complete route, ordering, and template contract.

## Pagination

An ordinary rendered page can list a collection in slices. Its own output is
page one; Heine generates later pages and validates every route before writing:

```toml
# content/en/blog/index.page
template = "section-index.tera"
# Keep this listing page itself out of the posts collection.
collections = []

[pagination]
collection = "posts"
per_page = 10

[pagination.navigation]
window = 1
```

Templates receive `pagination.pages`, page-number and adjacent-link fields,
and a compact `pagination.navigation` sequence with gap items. Call
`pagination_links()` only when a template intentionally needs every page
number. The [pagination guide](docs/pagination.md) documents routes, the
template contract, navigation windows, validation, and quick-build behavior.

Content files are addressed relative to their `.page` file and must remain
inside that page's `content/<locale>/` tree. Symbolic links are not permitted
anywhere below `content/`; this keeps page inputs and copied assets inside the
declared source tree:

- `.toml` is parsed as structured TOML;
- `.json` is parsed as structured JSON;
- other files are text. When the first line is a marker such as
  `<!-- segment: quiz.question -->`, Heine parses the file into named content
  segments. Dotted names create nested fields, and repeated names become arrays
  in their original order.

Declaring a content file also makes its role explicit: it is source input, not
an asset. This permits asset-rich pages without special directories. For
example:

```text
content/
└── en/
    └── blog/
        ├── 2025-08-14.page
        └── 2025-08-14/
            ├── post.md
            └── images/
                └── sunset.jpg
```

```toml
# content/en/blog/2025-08-14.page
template = "post.tera"

[content]
main = "2025-08-14/post.md"
```

Here `post.md` is available as `page.content.main` but is not copied;
`images/sunset.jpg` is copied as an asset. This sidecar layout is a convention,
not a separate feature: flat sites work in exactly the same way.

Ordinary Markdown URLs are relative to the rendered page, not to the Markdown
file. Since this page renders to `blog/2025-08-14.html`, it can use
`![Sunset](2025-08-14/images/sunset.jpg)`. For an internal copied asset that
should be validated and version-resolved, use an explicit locale-root-relative
asset ID instead:

```markdown
![Sunset](asset:blog/2025-08-14/images/sunset.jpg)
```

`asset:` works in Markdown links and images. Its ID follows the same rules as
Tera's `asset()` function: it has no leading `/`, stays under the current
locale's content tree, and may omit the version from a versioned filename.
Other Markdown URLs retain their normal, unmodified Markdown behavior.

## Templates

Templates use Tera2 syntax and normally have a `.tera` extension. The main
template values are:

| Value         | Meaning                                                             |
|---------------|---------------------------------------------------------------------|
| `page`        | The current page's render-facing fields and content                 |
| `data`        | Site-wide values loaded from the optional `data/` tree              |
| `locale_data` | Values loaded from optional `data/<locale>/` for the current locale |
| `locale`      | Current locale identity; `locale.code` is its BCP-47 code           |
| `root`        | A relative prefix from the rendered page to the site root           |

`page` deliberately exposes only these fields:

| Field                                        | Meaning                                                                    |
|----------------------------------------------|----------------------------------------------------------------------------|
| `id`                                         | Logical page ID, relative to its locale tree without `.page`.              |
| `path` / `filename`                          | Rendered output path / its filename; absent for a page without a template. |
| `dir` / `subdirs`                            | The page's source directory and its immediate child directories.           |
| `content`                                    | Values declared by `[content]` in the page file.                           |
| `title` / `description` / `summary` / `date` | Explicit page metadata.                                                    |
| `extra` / `directory`                        | User metadata and the immediate directory’s metadata view.                 |
| `taxonomies` / `weight`                      | The page's assigned taxonomy-term links and ordering weight.               |
| `lighter` / `heavier`                        | Previous / next sibling IDs, when present.                                 |
| `translations`                               | Explicitly known counterparts, keyed by locale code.                       |

The global page, collection, and asset inventories are intentionally not
serialized into every render. Use the validating `page()`, `page_exists()`,
`collection()`, `taxonomy()`, `siblings()`,
`pagination_links()`, and `asset()` functions for the item a template needs.
This keeps large sites predictable without making relationships implicit.

`siblings()` returns only rendered pages that participate in sibling ordering:
pages with a numeric `weight`. An index page and a page using `weight =
"None"` can use `siblings()` but are not themselves included in that ordered
set.

Taxonomy index and term templates receive their generated taxonomy views;
ordinary pages expose only their own assigned terms through
`page.taxonomies.<name>`. The [taxonomy guide](docs/taxonomies.md) explains
the generated routes, explicit cross-locale term counterparts, and template
contract.

Heine provides these filters:

- `markdown` renders Markdown as HTML according to `[markdown]` configuration.
  Its result is marked safe for Tera output; raw HTML in trusted Markdown source
  is preserved rather than sanitized.
- `latest` resolves a versioned asset such as `styles.css` to
  `styles.v2.css` when that is the highest available version. Versioned asset
  names use `<name>.v<positive-integer><extension>`. Relative inputs are
  resolved from the current page directory and produce page-relative URLs. A
  leading `/` is locale-root-relative and produces an absolute site URL below
  `site.base_path`.
  Its input must identify a copied asset; an invalid or missing asset fails at
  the filter call site.
- `dimensions` returns `width` and `height` for discovered images. Its input
  is relative to the current page directory; a leading `/` is
  locale-root-relative. Its input must identify a copied asset; a known
  non-image returns no dimensions, while an invalid or missing asset fails at
  the filter call site.

`highlight_css()` returns the configured URL of Heine's generated syntax
highlighting stylesheet. It is available only when highlighting is enabled;
otherwise the call fails at its template call site.

Use the `asset` function for an internal copied asset, identified by its
canonical path relative to the current locale's content tree. It validates that
ID, resolves a versioned file, and returns a URL, optional integrity metadata,
and detected image dimensions when available:

```tera
{% set script = asset(id="quiz/script.js") %}
<script src="{{ script.url }}"{% if script.integrity %} integrity="{{ script.integrity }}" crossorigin="anonymous"{% endif %}></script>

{% set image = asset(id="blog/2025-08-14/images/sunset.jpg") %}
<img src="{{ image.url }}"{% if image.width %} width="{{ image.width }}" height="{{ image.height }}"{% endif %} alt="Sunset">
```

`asset` and `page` each return two link forms:

| Field          | Meaning                                               |
|----------------|-------------------------------------------------------|
| `url`          | An absolute path below the configured site base path. |
| `relative_url` | A path relative to the rendered page.                 |

Use `asset(id="css/styles.css")` for copied assets and
`page(id="blog/index")` for a rendered page. Asset IDs never begin with `/`:
they are always relative to the current locale's content tree. Page IDs are
the `.page` filename relative to the current locale's tree, without the `.page`
extension. Keeping asset and page lookups separate makes templates explicit and
lets Heine validate each kind of internal link. Leave external URLs as ordinary
template strings. See the [reference](docs/reference.md#template-contract) for
the complete list of Heine functions and filters.

IDs name literal source filenames; write a space, `#`, `%`, or non-ASCII
character exactly as it appears on disk, rather than pre-encoding it. Heine
percent-encodes those names once when it produces `url` or `relative_url`.
For non-ASCII names, use NFC-normalized spelling consistently; Heine preserves
that spelling rather than normalizing lookup IDs.

When a navigation item or other optional link may be absent in a locale, guard
the strict lookup with `page_exists(id="quiz/index")`. It returns `false` only
for a missing rendered page; malformed IDs and unknown locales still produce a
template diagnostic at the call site.

Template inheritance, page-template references, and Tera syntax/rendering
errors are reported with source file and line context. Invalid asset IDs and
unknown rendered page IDs are reported at their template call site as well.
Misspelled keyword names in Heine's fixed template functions are rejected at
that same location instead of being silently ignored.
For a missing field in ordinary Tera dotted access such as
`data.site.description`, Tera can identify the template expression but does
not expose the data lookup in a form that Heine can reliably trace back to its
source file. Heine therefore reports the template call site and Tera's
available-field list rather than guessing at a data-file location.
Tera2 is Heine's template contract. See the
[Tera2 documentation](https://github.com/Keats/tera) for the template language
itself.

## Configuration

### Syntax highlighting

Enable class-based fenced-code highlighting with one generated, public
stylesheet and a light/dark Giallo theme pair:

```toml
[markdown.highlighting]
stylesheet = "css/highlighting.css"
missing_language = "error" # default; or "plain"

[markdown.highlighting.theme]
light = "github-light"
dark = "github-dark"
```

Add the stylesheet explicitly to the base template, before an authored site
stylesheet that supplies any desired overrides:

```tera
<link rel="stylesheet" href="{{ highlight_css() }}">
{% set styles = asset(id="css/styles.css") %}
<link rel="stylesheet" href="{{ styles.url }}">
```

Heine generates the configured output file on every full build. It contains
Giallo's code-token rules and the small structural rule that lets the themed
code-block background show through the nested `<code>` element. The light theme
is the default and the dark theme is selected with `prefers-color-scheme`. Do not edit that generated file;
choose another bundled theme pair or override its classes from your authored
stylesheet. The [reference](docs/reference.md#heinetoml) links the exact
theme-name list for the bundled Giallo registry. `stylesheet` is a safe output-relative `.css` path and cannot
collide with a page or copied asset. Fenced blocks without a language remain
ordinary Markdown code blocks. `missing_language = "error"` rejects unknown
fence languages; `"plain"` renders them without syntax-specific tokens.

`heine.toml` is required. Its `[i18n]` section declares the default locale and
the complete finite locale set; unknown configuration keys are rejected so
mistakes are diagnosed early. Configure the locale model, deployment path, and
optional asset integrity hashes together:

```toml
[i18n]
default_locale = "en"

[i18n.locales.en]
direction = "ltr"

[i18n.locales.de]
direction = "ltr"

[site]
base_path = "/my-project/"

[build]
output_dir = "public/my-project"

[assets]
integrity = "sha384"

[markdown]
extensions = ["tables", "strikethrough", "task_lists", "footnotes"]
math = "mathml"
```

`site.base_path` defaults to `/`. It must start and end with `/`; use it when a
site is served from a subdirectory such as `/my-project/`. It is an ASCII URL
path, not a full origin URL. Use uppercase percent escapes for a space or
non-ASCII character (for example, `/guide%20notes/`), and do not use encoded
path separators or dot segments. A future full-site URL setting can therefore
be added without changing how internal links work.

`build.output_dir` defaults to `public`. It is a path relative to the site
root, and controls where generated files are written. It is independent of
`site.base_path`: use `public/my-project` when a simple file server should
serve the site below `/my-project/`, or keep `public` when a hosting platform
or reverse proxy maps that URL path to the generated directory. Output cannot
be placed inside `content/`, `data/`, `locales/`, or `templates/`. Its existing
ancestor directories must not be symbolic links. Like generated output names,
its components must use portable filename syntax.

`assets.integrity` defaults to no integrity hashes. Allowed values are
`sha256`, `sha384`, and `sha512`. When set, the `asset` function calculates and
returns Subresource Integrity values for referenced copied assets.

`markdown.extensions` defaults to an empty list, preserving CommonMark
behavior. Its allowed values are `tables`, `strikethrough`, `task_lists`, and
`footnotes`; duplicate and unknown values are rejected.

`markdown.math` defaults to `off`, which leaves text between `$...$` and
`$$...$$` to ordinary Markdown parsing. Set it to `mathml` to render inline
TeX between `$...$` and display TeX between `$$...$$` as MathML during the build.
Invalid TeX in a `.md` content file is reported at that file and expression;
invalid TeX written directly in a template is reported at its `markdown`
filter call. MathML rendering is self-contained and does not require
browser-side JavaScript or a TeX installation.

Set `markdown.math` to `tex` when a browser-side renderer such as KaTeX or
MathJax should handle the TeX instead. Heine recognizes the delimiters before
ordinary Markdown can interpret TeX characters such as `_` or `*`, then emits
the original delimited TeX as escaped text. It does not validate TeX or bundle
a client-side renderer in this mode; add the renderer and its styles through
your own templates, assets, or a future theme.

## Build behavior

Each normal build creates a complete output tree in a staging directory and
publishes it only after rendering and copying succeed. It removes files left
behind by renamed or deleted source files. If a build fails before publication,
the previous complete output remains in place.

Heine marks output directories that it creates. To avoid deleting unrelated
files, it refuses to replace a non-empty directory without that marker. Use
`--clean-output` once to explicitly adopt and replace an existing output
directory that is safe to discard. Do not place manually maintained files in a
managed output directory. The `.heine-output` marker is an internal ownership
record, not a published resource; `heine serve` never serves it.

For safety, Heine rejects symbolic links in an existing output path. This
includes a nested asset or page target during a quick build, so quick mode
cannot write through a link outside the managed output tree.

Heine assumes that the site source and managed output are not modified while a
build is running. It detects existing symbolic links, but it is not designed
as a security boundary against a concurrent hostile process racing filesystem
operations.

Heine rejects output collisions before writing. In particular, a rendered page
cannot overwrite an unrelated copied asset with the same output path.

`--quick` updates the managed output directory directly. It skips rendering a
page whose page file, declared content files, and inherited directory
descriptors are older than its existing output. Changed data or Fluent
resources also re-render affected pages; a default-locale Fluent change
re-renders every locale because it may be a fallback. A changed present asset
also re-renders pages: asset URLs can select a new version and integrity values
depend on asset contents. It copies an asset, deployment file, or license text
only when its source is newer than the existing output file or that output file
is missing. It does not notice template, configuration, or page-relationship
changes, or renamed or deleted files, and it never removes stale output. Run a
normal build after those changes. `--quick` cannot be combined with
`--clean-output`.

## Starter site

[`starter-site/`](starter-site/) is a Tera2 port of the reference starter and
shows pages, Markdown, segmented quiz data, template inheritance, copied assets,
versioned CSS, browser-side quiz JavaScript, and shared site data. Its
`data/site.toml` supplies the site identity and primary navigation while pages
retain their own titles and content.

It also has English and German source trees. Its collection-backed blog index
uses pagination, producing a second English listing page while the shorter
German collection remains a single page. The home page, blog indexes, and two
blog posts use explicit `translationid` values, so the shared header offers
links only to known counterparts. Fluent localizes navigation, the footer,
post controls, and the singular/plural blog-post count; shared site assets
live in `content/assets/`. Its data-defined language flags keep the compact
switcher accessible through localized link labels. Its small `components.tera`
component file keeps the shared pager UI consistent between blog and taxonomy
listings.

The first blog post also demonstrates the starter's `featured_image` template
convention. It resolves the declared asset and emits its detected intrinsic
dimensions while CSS keeps it responsive. It also includes a spoken MP3 in an
HTML audio element, so `heine serve` can demonstrate local byte-range seeking.

Its `public/` directory is generated output and can be regenerated with:

```bash
cargo run -- --root starter-site
```

If that directory is non-empty and unmanaged, regenerate it with
`cargo run -- --root starter-site --clean-output`.

To preview a site configured with `base_path = "/guide/"` and
`output_dir = "public/guide"`, serve its parent output directory and open the
base path:

```bash
static-web-server --root /path/to/site/public --port 8787
# Open http://localhost:8787/guide/index.html
```

## Development

Run the full local quality check before contributing changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```
