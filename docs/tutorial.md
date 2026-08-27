# Tutorial: build a small multilingual blog

This tutorial grows one working site in small steps. Each step builds on the
previous one and leaves the site usable. It introduces Heine's main public
concepts; focused guides and the [reference](reference.md) provide the complete
rules when you need them.

The final site has Markdown posts, copied assets, shared data, English and
German pages, localized UI text, a post collection, pagination, and tags.
For a larger working example, see [`starter-site/`](../starter-site/).

## 1. Start with one page

Create this site root:

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

Run a full build from a Heine checkout:

```bash
cargo run -- --root /path/to/my-site
```

`content/en/index.page` is a [page file](glossary.md#page-file): its filename
becomes the page's [page ID](glossary.md#page-id), `index`, and it renders to
`public/index.html`. `en` is the [default locale](glossary.md#default-locale),
so its URL has no language prefix.

Heine uses strict schemas for `heine.toml`, `.page`, and `_directory.toml`:
a misspelled field is reported instead of being ignored. The
[reference](reference.md) lists every accepted field.

## 2. Add a shared template and Markdown content

Pages can declare named [content files](glossary.md#declared-content-file).
They are inputs, not published files. Create a base template and a post:

```tera
{# templates/base.tera #}
<!doctype html>
<html lang="{{ locale.code }}" dir="{{ locale.direction }}">
  <head><title>{% block title %}{{ page.title }}{% endblock %}</title></head>
  <body>
    {% set home = page(id='index') %}
    <header><a href="{{ home.url }}">Home</a></header>
    <main>{% block content %}{% endblock %}</main>
  </body>
</html>
```

```toml
# content/en/blog/first.page
template = "post.tera"
title = "First post"
summary = "A short first post."
date = 2026-08-14

[content]
main = "first.md"
```

```markdown
<!-- content/en/blog/first.md -->
Hello from **Heine**.
```

```tera
{# templates/post.tera #}
{% extends "base.tera" %}
{% block content %}
  <article>
    <h1>{{ page.title }}</h1>
    {{ page.content.main | markdown }}
  </article>
{% endblock %}
```

`page()` is a [template function](glossary.md#template-function): a mistyped or
missing internal page fails at the template expression during the build instead
of becoming a browser 404. For example, if the header used
`page(id='home')` before that page existed, the diagnostic identifies the
template expression:

```text
  × render error in template "base.tera": rendered page "home" was not found for locale "en"
    ╭─[templates/base.tera:6]
  6 │     {% set home = page(id='home') %}
    ·                        ────┬────
    ·                            ╰── error here
```

`summary` is optional authored [summary](glossary.md#summary) metadata. It is
useful in listings and is never extracted from Markdown.

## 3. Add a copied asset

Place a stylesheet below `content/assets/`:

```text
content/assets/css/site.css
```

It is a [copied asset](glossary.md#copied-asset): unlike a declared content
file, Heine publishes it. Link it through its [asset ID](glossary.md#asset-id)
from the base template:

```tera
{% set site_css = asset(id="assets/css/site.css") %}
<link rel="stylesheet" href="{{ site_css.url }}">
```

The asset becomes `public/assets/css/site.css`. `asset()` validates the ID and keeps
the URL correct when `site.base_path` changes. Use `page()` for rendered pages
and `asset()` for copied files; their separate contracts let Heine diagnose a
wrong kind of internal link.

An asset can use a [versioned asset](glossary.md#versioned-asset) name such as
`site.v2.css` when a site wants version selection. Its lookup ID remains
`assets/css/site.css`. That is optional; ordinary
filenames work without any versioning convention.

## 4. Add shared structured data

Create `data/site.toml`:

```toml
name = "My site"

[[navigation]]
label = "Blog"
page = "blog/index"
```

Data is not published. Templates receive it through `data`:

```tera
<nav>
  {% for item in data.site.navigation %}
    {% set target = page(id=item.page) %}
    <a href="{{ target.url }}">{{ item.label }}</a>
  {% endfor %}
</nav>
```

Use JSON or TOML files below `data/` for information shared by templates.
See the [reference](reference.md#data) for naming and input rules.

## 5. Add another language and localized UI text

Declare German alongside English:

```toml
[i18n.locales.de]
direction = "ltr"
```

Then create `content/de/` and `locales/`:

```text
content/
├── en/
└── de/
    └── index.page
locales/
├── en/site.ftl
└── de/site.ftl
```

```ftl
# locales/en/site.ftl
nav-home = Home
post-count = { $count ->
    [one] One post
   *[other] { $count } posts
}
```

```ftl
# locales/de/site.ftl
nav-home = Startseite
post-count = { $count ->
    [one] Ein Beitrag
   *[other] { $count } Beiträge
}
```

Use `t()` for reusable interface text:

```tera
{% set home = page(id='index') %}
<a href="{{ home.url }}">{{ t(id="nav-home") }}</a>
```

The default English page remains `/index.html`; the German page is generated
under `/de/`. A German page can use a different page ID and route. If two pages
are intentional counterparts, give them the same `translationid`; Heine then
exposes only those known links through `page.translations`.

Locale-specific structured values go below `data/<locale>/` and are available
as [`locale_data`](glossary.md#locale-data), separate from shared `data`:

```toml
# data/de/navigation.toml
label = "Blog"
```

```tera
{{ locale_data.navigation.label }}
```

Read [internationalization and localization](internationalization.md) before
adding a language switcher, locale-specific assets or data, RTL content, or
locale-formatted dates.

## 6. Create an ordered post collection

A [collection](glossary.md#collection) is an ordered set of rendered
pages. Define one once:

```toml
[collections.posts]
order = "date-desc"
```

Add your posts to it:

```toml
# content/en/blog/first.page
collections = ["posts"]
```

For a blog directory, put the membership default in
`content/en/blog/_directory.toml` instead:

```toml
[pages]
template = "post.tera"
collections = ["posts"]
```

An index page opts out of listing itself and reads the collection:

```toml
# content/en/blog/index.page
template = "blog-index.tera"
collections = []
```

```tera
{# templates/blog-index.tera #}
{% extends "base.tera" %}
{% block content %}
  {% set posts = collection(name="posts") %}
  {% for id in posts.page_ids %}
    {% set post = page(id=id) %}
    <article><h2><a href="{{ post.url }}">{{ post.title }}</a></h2></article>
  {% endfor %}
{% endblock %}
```

Collections can order by dates, titles, weights, or page IDs. A title order
uses locale-aware collation and requires every member to have a title. The
[collection guide](collections.md) explains the complete ordering contract.

For manual ordering, give every member a numeric
[weight](glossary.md#weight) and select `weight-asc` or `weight-desc`:

```toml
[collections.docs]
order = "weight-asc"

# content/en/docs/install.page
weight = 10
```

`weight = "None"` is only for sibling navigation; it cannot participate in a
weight-ordered collection. A directory descriptor can also set metadata and
requirements. See the complete `_directory.toml` table in the
[reference](reference.md#_directorytoml).

Numeric weights also create [sibling navigation](glossary.md#sibling-navigation)
among rendered pages in the same directory. It is separate from collection
ordering; the [reference](reference.md#sibling-navigation) shows how to render
the adjacent pages.

## 7. Paginate the blog index

Add this to the index page:

```toml
[pagination]
collection = "posts"
per_page = 10

[pagination.navigation]
window = 1
```

The authored index output becomes the first listing page. Heine generates
later [pagers](glossary.md#pager) and validates every route before writing. The
template receives only the current slice in `pagination.pages`:

```tera
{% for id in pagination.pages %}
  {% set post = page(id=id) %}
  <a href="{{ post.url }}">{{ post.title }}</a>
{% endfor %}

<nav aria-label="Pagination">
  {% for item in pagination.navigation %}
    {% if item.kind == "page" %}
      <a href="{{ item.url }}">{{ item.number }}</a>
    {% else %}
      <span aria-hidden="true">…</span>
    {% endif %}
  {% endfor %}
</nav>
```

The [pagination guide](pagination.md) documents routes, current-page values,
adjacent links, and the optional full `pagination_links()` view.

### Reuse the pager as a Tera2 component

Tera2 components keep repeated template markup in one place. Put this in
`templates/components.tera`:

```tera
{% component ui.pagination(pager, label: string) %}
  {% if pager.page_count > 1 %}
    <nav aria-label="{{ label }}">
      {% for item in pager.navigation %}
        {% if item.kind == "gap" %}
          <span aria-hidden="true">…</span>
        {% elif item.current %}
          <span aria-current="page">{{ item.page_number }}</span>
        {% else %}
          <a href="{{ item.relative_url }}">{{ item.page_number }}</a>
        {% endif %}
      {% endfor %}
    </nav>
  {% endif %}
{% endcomponent ui.pagination %}
```

Then call it from the listing template:

```tera
{{ <ui.pagination pager={pagination} label={t(id="pagination-label")} /> }}
```

Add `pagination-label = Pagination` to the English Fluent file and a matching
translation to each other locale. Components are Tera2's reusable template
mechanism; Heine supplies the values passed to them but does not add a second
component system.

## 8. Add tags

A [taxonomy](glossary.md#taxonomy) classifies the pages of one collection.
Define tags using the post collection:

```toml
[taxonomies.tags]
collection = "posts"
path = "tags"
index_template = "taxonomy-index.tera"
term_template = "taxonomy-term.tera"
per_page = 20
```

Assign literal [terms](glossary.md#term) on posts:

```toml
[taxonomies]
tags = ["Rust", "Static sites"]
```

Heine generates a tags index plus term listings. The taxonomy templates receive
their `taxonomy` view and, for term listings, the normal `pagination` view.
Use the [taxonomy guide](taxonomies.md) for templates, routes, checks, and
declared cross-locale term counterparts.

## 9. Enable optional Markdown features

Markdown extensions, mathematical rendering, and class-based syntax
highlighting are opt-in:

```toml
[markdown]
extensions = ["tables", "strikethrough", "footnotes"]
math = "mathml"

[markdown.highlighting]
stylesheet = "css/highlighting.css"

[markdown.highlighting.theme]
light = "github-light"
dark = "github-dark"
```

Add the generated highlighting stylesheet to the base template:

```tera
<link rel="stylesheet" href="{{ highlight_css() }}">
```

`math = "mathml"` renders supported TeX delimiters during the build.
`math = "tex"` instead preserves escaped delimiters for a renderer such as
KaTeX or MathJax that you add yourself. The [reference](reference.md#heinetoml)
lists all allowed modes and highlighting settings.

## 10. Prepare the generated site for deployment

Use `deploy/` only for files that belong at a fixed output-root path, such as
an Apache `.htaccess` file or a search-service verification file:

```text
deploy/.htaccess                 -> public/.htaccess
deploy/google1234567890.html     -> public/google1234567890.html
```

Use a copied asset when a page needs to link or embed the file. Deployment
files deliberately have no `asset()` identity, locale behavior, or template
rendering.

When copied assets require rights information, add adjacent `.license`
[asset sidecars](glossary.md#asset-sidecar) and matching texts under
`LICENSES/`. An authored attribution page can render `attributions()`.
Read the [deployment-files guide](deployment-files.md) and
[asset-licensing guide](asset-licensing.md) before publishing.

## 11. Preview while writing

Run the development server:

```bash
cargo run -- serve --root /path/to/my-site
```

It performs a full build before serving the generated output and reloads open
HTML pages after a successful rebuild. It starts with port `1111`, then tries
the next available unprivileged port. See the
[development-server guide](development-server.md) for base paths, live reload,
and media range requests, the [reference](reference.md#site-root-and-commands)
for command options, and the [README](../README.md#build-behavior) for full and
quick-build behavior.

## Common first problems

- A collection is empty when no rendered page in the current locale belongs to
  it. Check its `collections` membership and the locale tree you are viewing.
- `--quick` intentionally leaves some changes stale. Use a full build after
  changing templates, configuration, page relationships, or after renaming or
  deleting inputs.
- A failed `page()`, `asset()`, or `t()` call names the template expression
  that made the request. Start with that file and location; the message also
  names the unresolved value or accepted alternatives where available.
- A language switcher contains only pages sharing a `translationid`. This
  avoids a link to a page that has not been authored in that language.
