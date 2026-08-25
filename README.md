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

Pagination, RSS feeds, and a plugin system are deliberately out of scope for
now.

## Contents

- [Build a site](#build-a-site)
- [Serve a site locally](#serve-a-site-locally)
- [Site layout](#site-layout)
- [Global data](#global-data)
- [Pages and content](#pages-and-content)
  - [Directory descriptors](#directory-descriptors)
- [Templates](#templates)
- [Configuration](#configuration)
- [Build behavior](#build-behavior)
- [Starter site](#starter-site)
- [Development](#development)

For the complete, field-by-field contract for every authored site file and
command-line option, see the [Heine reference](docs/reference.md).

## Build a site

Install a current stable Rust toolchain, then run Heine from this repository:

```bash
cargo run -- --root /path/to/site
```

If `--root` is omitted, Heine starts in the current directory and walks toward
its parent directories until it finds one containing both `content/` and
`templates/`.

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
directory descriptors, global `data/`, and present copied assets. It
intentionally does not consider template, configuration, or page-relationship
changes. Use a normal build after those changes.

## Serve a site locally

For local development, let Heine build, serve, and watch a site:

```bash
cargo run -- serve --root /path/to/site
```

The server performs a complete build before it starts, serves the generated
`build.output_dir` directory, and watches `content/`, `data/`, `templates/`,
and `heine.toml`. After a successful rebuild, open pages reload automatically.
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
├── heine.toml          # optional configuration
├── content/            # pages, content files, and copied assets
├── data/               # optional site-wide JSON and TOML
├── templates/          # Tera2 templates
└── public/             # generated output
```

`content/` deliberately holds both build inputs and published assets. A
`.page` file and every file it declares in `[content]` are build inputs, so
Heine reads them but never copies them to `public/`. Every other file below
`content/` is a published asset and is copied in the matching place below
`public/`. Because page IDs, asset IDs, and template names are text-based,
source-path components beneath `content/` and `templates/` must be valid
UTF-8 and must not contain backslashes; Heine reports an invalid path instead
of silently changing its generated URL or identifier.
Template files, like content files, must be regular files: symbolic links are
rejected rather than silently being skipped.

`__heine/` is reserved in generated output for development-server endpoints.
Do not use that directory for pages or copied assets.

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

## Pages and content

A `.page` file is TOML. It can declare a template, explicit metadata, and
named content files:

```toml
template = "post.tera"
title = "A post"
date = 2026-08-14
tags = ["news"]

[extra]
featured = true

[content]
main = "post.md"
```

The page above renders to `post.html`. The page fields are:

| Field         | Default           | Meaning                                                                                                                          |
|---------------|-------------------|----------------------------------------------------------------------------------------------------------------------------------|
| `template`    | inherited or none | Template below `templates/`; `false` explicitly disables an inherited template.                                                  |
| `title`       | none              | Page title, exposed as `page.title`.                                                                                             |
| `description` | none              | Short page description, exposed as `page.description`.                                                                           |
| `summary`     | none              | Authored summary text, exposed as `page.summary`; Heine does not derive one from Markdown.                                       |
| `date`        | none              | A TOML date or offset-aware date-time, exposed as `page.date`. Local date-times are rejected because their time zone is unclear. |
| `content`     | empty             | Named content files, relative to the `.page` file.                                                                               |
| `extra`       | empty table       | User-defined TOML values exposed as `page.extra`.                                                                                |
| `tags`        | empty list        | Tags for the page.                                                                                                               |
| `weight`      | `0`               | Sibling ordering weight. `index.page` has no weight by default and is excluded; use `weight = "None"` to exclude another page.   |
| `suffix`      | `.html`           | Output extension, for example `.xml`. It cannot contain path separators.                                                         |

### Directory descriptors

`content/**/_directory.toml` is a non-published TOML descriptor for a content
directory. It gives that directory metadata and can set a recursive default
template and metadata requirements for its pages. The closest descriptor with
`pages.template` wins; a page can override it with a template name or use
`template = false` to remain available through `pages` without rendering.

```toml
# content/blog/_directory.toml
title = "Blog"
description = "Notes from the project."

[metadata]
required = ["title"]

[pages]
template = "post.tera"

[pages.metadata]
required = ["title", "date", "extra.author"]
```

Requirements accumulate from `content/_directory.toml` to the page or nested
descriptor. `[metadata].required` validates encountered directory descriptors;
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

Content files are addressed relative to their `.page` file and must remain
inside `content/`. Symbolic links are not permitted anywhere below `content/`;
this keeps page inputs and copied assets inside the declared source tree:

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
└── blog/
    ├── 2025-08-14.page
    └── 2025-08-14/
        ├── post.md
        └── images/
            └── sunset.jpg
```

```toml
# blog/2025-08-14.page
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
should be validated and version-resolved, use an explicit content-root-relative
asset ID instead:

```markdown
![Sunset](asset:blog/2025-08-14/images/sunset.jpg)
```

`asset:` works in Markdown links and images. Its ID follows the same rules as
Tera's `asset()` function: it has no leading `/`, stays under `content/`, and
may omit the version from a versioned filename. Other Markdown URLs retain
their normal, unmodified Markdown behavior.

## Templates

Templates use Tera2 syntax and normally have a `.tera` extension. The main
template values are:

| Value    | Meaning                                                   |
|----------|-----------------------------------------------------------|
| `page`   | The current page's render-facing fields and content       |
| `pages`  | All pages, keyed by logical page ID                       |
| `tags`   | Tags mapped to ordered page IDs                           |
| `assets` | Copied asset paths, relative to `content/`                |
| `data`   | Site-wide values loaded from the optional `data/` tree    |
| `root`   | A relative prefix from the rendered page to the site root |

`page` deliberately exposes only these fields:

| Field                                        | Meaning                                                                    |
|----------------------------------------------|----------------------------------------------------------------------------|
| `id`                                         | Logical page ID, relative to `content/` without `.page`.                   |
| `path` / `filename`                          | Rendered output path / its filename; absent for a page without a template. |
| `dir` / `subdirs`                            | The page's source directory and its immediate child directories.           |
| `content`                                    | Values declared by `[content]` in the page file.                           |
| `title` / `description` / `summary` / `date` | Explicit page metadata.                                                    |
| `extra` / `directory`                        | User metadata and the immediate directory’s metadata view.                 |
| `tags` / `weight`                            | The page's tags and ordering weight.                                       |
| `siblings`                                   | Other rendered, ordered pages in the same directory.                       |
| `lighter` / `heavier`                        | Previous / next sibling IDs, when present.                                 |

Heine provides these filters:

- `markdown` renders Markdown as HTML according to `[markdown]` configuration.
  Its result is marked safe for Tera output; raw HTML in trusted Markdown source
  is preserved rather than sanitized.
- `latest` resolves a versioned asset such as `styles.css` to
  `styles.v2.css` when that is the highest available version. Versioned asset
  names use `<name>.v<positive-integer><extension>`. Relative inputs are
  resolved from the current page directory and produce page-relative URLs. A
  leading `/` is content-root-relative and produces an absolute site URL below
  `site.base_path`.
  Its input must identify a copied asset; an invalid or missing asset fails at
  the filter call site.
- `dimensions` returns `width` and `height` for discovered images. Its input
  is relative to the current page directory; a leading `/` is
  content-root-relative. Its input must identify a copied asset; a known
  non-image returns no dimensions, while an invalid or missing asset fails at
  the filter call site.

`highlight_css()` returns the configured URL of Heine's generated syntax
highlighting stylesheet. It is available only when highlighting is enabled;
otherwise the call fails at its template call site.

Use the `asset` function for an internal copied asset, identified by its
canonical path relative to `content/`. It validates that ID, resolves a
versioned file, and returns a URL, optional integrity metadata, and detected
image dimensions when available:

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
they are always relative to `content/`. Page IDs are the `.page` filename
relative to `content/`, without the `.page` extension. Keeping asset and page
lookups separate makes templates explicit and lets Heine validate each kind of
internal link. Leave external URLs as ordinary template strings.

Template inheritance, page-template references, and Tera syntax/rendering
errors are reported with source file and line context. Invalid asset IDs and
unknown rendered page IDs are reported at their template call site as well.
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
<link rel="stylesheet" href="{{ asset(id=\"css/styles.css\").url }}">
```

Heine generates the configured output file on every full build. It contains
Giallo's code-token rules and the small structural rule that lets the themed
code-block background show through the nested `<code>` element. The light theme
is the default and the dark theme is selected with `prefers-color-scheme`. Do not edit that generated file;
choose another bundled theme pair or override its classes from your authored
stylesheet. `stylesheet` is a safe output-relative `.css` path and cannot
collide with a page or copied asset. Fenced blocks without a language remain
ordinary Markdown code blocks. `missing_language = "error"` rejects unknown
fence languages; `"plain"` renders them without syntax-specific tokens.

`heine.toml` is optional. Unknown configuration keys are rejected so mistakes
are diagnosed early. Configure the deployment path and optional asset integrity
hashes like this:

```toml
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
be placed inside `content/`, `data/`, or `templates/`. Its existing ancestor
directories must not be symbolic links.

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
descriptors are older than its existing output. A changed present asset also
re-renders pages: asset URLs can select a new version and integrity values
depend on asset contents. It copies an asset only when its source is newer than
the existing output asset or that output asset is missing. It does not notice
template, configuration, or page-relationship changes, or renamed or deleted
files, and it never removes stale output. Run a normal build after those
changes. `--quick` cannot be combined with `--clean-output`.

## Starter site

[`starter-site/`](starter-site/) is a Tera2 port of the reference starter and
shows pages, Markdown, segmented quiz data, template inheritance, copied assets,
versioned CSS, browser-side quiz JavaScript, and shared site data. Its
`data/site.toml` supplies the site identity and primary navigation while pages
retain their own titles and content.

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
