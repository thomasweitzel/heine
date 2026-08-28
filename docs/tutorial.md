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

Then start the development server in a second terminal and leave it running as
you follow the tutorial:

```bash
cargo run -- serve --root /path/to/my-site
```

Open the local address it prints—normally `http://127.0.0.1:1111/`. Each later
successful rebuild reloads that browser page automatically.

`content/en/index.page` is a [page file](glossary.md#page-file): its filename
becomes the page's [page ID](glossary.md#page-id), `index`, and it renders to
`public/index.html`. `en` is the [default locale](glossary.md#default-locale),
so its URL has no language prefix.

**See it now:** the browser shows the `My site` heading at `/`.

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

The post below declares `published`. Add this `[site]` table to `heine.toml` so
Heine can resolve that local publication time to one unambiguous instant:

```toml
# heine.toml
[site]
time_zone = "Europe/Berlin"
```

```toml
# content/en/blog/first.page
template = "post.tera"
title = "First post"
summary = "A short first post."
published = 2026-08-14T09:30:00

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

Update the home page to use this Markdown-capable layout too:

```toml
# content/en/index.page
template = "post.tera"
title = "My site"

[content]
main = "index.md"
```

```markdown
<!-- content/en/index.md -->
Welcome to my site. [Read the first post](page:blog/first).
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

**See it now:** visit `/` and `/blog/first.html`. The home page contains the
Markdown link and the post renders its Markdown body.

## 3. Add a copied asset

Create `content/assets/css/site.css`:

```css
/* content/assets/css/site.css */
body {
  color: #222;
  background: #f3f7fb;
}
```

It is a [copied asset](glossary.md#copied-asset): unlike a declared content
file, Heine publishes it. Inside the `<head>` in `templates/base.tera`, before
the closing `</head>`, add this link through its [asset ID](glossary.md#asset-id):

```tera
{% set site_css = asset(id="assets/css/site.css") %}
<link rel="stylesheet" href="{{ site_css.url }}">
```

The asset becomes `public/assets/css/site.css`. `asset()` checks the ID and keeps
the URL correct when `site.base_path` changes. Use `page()` for rendered pages
and `asset()` for copied files; their separate contracts let Heine diagnose a
wrong kind of internal link.

An asset can use a [versioned asset](glossary.md#versioned-asset) name such as
`site.v2.css` when a site wants version selection. Its lookup ID remains
`assets/css/site.css`. That is optional; ordinary
filenames work without any versioning convention.

**See it now:** the page at `/` has a pale blue background from the copied
stylesheet.

## 4. Add shared structured data

Create `data/site.toml`:

```toml
name = "My site"

[[navigation]]
label = "Start"
page = "index"
```

Data is not published. Templates receive it through `data`. In `base.tera`,
replace the one Home link inside `<header>` with this navigation:

```tera
<header>
  <nav aria-label="Main navigation">
    {% for item in data.site.navigation %}
      {% set target = page(id=item.page) %}
      <a href="{{ target.url }}">{{ item.label }}</a>
    {% endfor %}
  </nav>
</header>
```

Use JSON or TOML files below `data/` for information shared by templates.
See the [reference](reference.md#data) for naming and input rules.

**See it now:** the header at `/` now displays the `Start` link rendered from
`data/site.toml`.

## 5. Add another language and localized UI text

Add this table to `heine.toml`, alongside the existing English locale table:

```toml
# heine.toml
[i18n.locales.de]
direction = "ltr"
```

Then create the German counterpart and the locale resources:

```text
content/
├── en/
└── de/
    └── index.page
locales/
├── en/site.ftl
└── de/site.ftl
```

Give the two home pages the same explicit counterpart identity:

```toml
# content/en/index.page
template = "post.tera"
title = "My site"
translationid = "home"

[content]
main = "index.md"
```

```toml
# content/de/index.page
template = "post.tera"
title = "Meine Seite"
translationid = "home"

[content]
main = "index.md"
```

Create its declared content file:

```markdown
<!-- content/de/index.md -->
Willkommen auf meiner Seite.
```

Create or replace the two locale files with these messages:

```ftl
# locales/en/site.ftl
nav-home = Home
nav-main = Main navigation
```

```ftl
# locales/de/site.ftl
nav-home = Startseite
nav-main = Hauptnavigation
```

To localize the header navigation, replace the `<header>…</header>` block
added in step 4 in `templates/base.tera` with:

```tera
<header>
  {% set home = page(id="index") %}
  <nav aria-label="{{ t(id="nav-main") }}">
    <a href="{{ home.url }}">{{ t(id="nav-home") }}</a>
  </nav>
</header>
```

The default English page remains `/index.html`; the German page is generated
under `/de/`. A German page can use a different page ID and route. If two pages
are intentional counterparts, give them the same `translationid`; Heine then
exposes only those known links through `page.translations`.

Read [internationalization and localization](internationalization.md) before
adding a language switcher, locale-specific assets or data, RTL content, or
locale-formatted dates.

**See it now:** visit `/de/`. It has German page content and a German
navigation label.

## 6. Create an ordered post collection

A [collection](glossary.md#collection) is an ordered set of rendered
pages. Add this definition to `heine.toml`:

```toml
[collections.posts]
order = "published-desc"
```

Add this field to `content/en/blog/first.page`:

```toml
# content/en/blog/first.page
collections = ["posts"]
```

For a blog directory, replace that per-page membership with this recursive
default in `content/en/blog/_directory.toml` instead:

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

Create its template:

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

Collections can order by publication times, titles, weights, or page IDs. A title order
uses locale-aware collation and requires every member to have a title. The
[collection guide](collections.md) explains the complete ordering contract.

The following is an optional, separate manual-ordering example; it is not part
of the blog built by this tutorial. Give every member a numeric
[weight](glossary.md#weight) and select `weight-asc` or `weight-desc`:

```toml
# heine.toml
[collections.docs]
order = "weight-asc"
```

```toml
# content/en/docs/install.page
weight = 10
```

`weight = "None"` lets a non-index page opt out of sibling navigation. It is
not a numeric weight, so that page cannot participate in a weight-ordered
collection. An `index.page` likewise needs an explicit numeric weight to join
sibling navigation. A directory descriptor can also set metadata and
requirements. See the complete `_directory.toml` table in the
[reference](reference.md#_directorytoml).

Numeric weights also create [sibling navigation](glossary.md#sibling-navigation)
among rendered pages in the same directory. It is separate from collection
ordering; the [reference](reference.md#sibling-navigation) shows how to render
the adjacent pages.

**See it now:** visit `/blog/`. It lists `First post` from the `posts`
collection.

## 7. Paginate the blog index

To make a second pager visible, create another post. The directory policy from
step 6 supplies its template and collection membership:

```toml
# content/en/blog/second.page
title = "Second post"
summary = "A short second post."
published = 2026-08-15T09:30:00

[content]
main = "second.md"
```

```markdown
<!-- content/en/blog/second.md -->
Another post for the paginated listing.
```

Add this table to `content/en/blog/index.page`:

```toml
[pagination]
collection = "posts"
per_page = 1

[pagination.navigation]
window = 1
```

The authored index output becomes the first listing page. Heine generates
later [pagers](glossary.md#pager) and checks every route before writing.
Replace the complete `templates/blog-index.tera` file with this paginated
listing. `pagination.pages` contains only the current slice:

```tera
{# templates/blog-index.tera #}
{% extends "base.tera" %}
{% block content %}
{% for post in pagination.pages %}
  <a href="{{ post.url }}">{{ post.title }}</a>
{% endfor %}

<nav aria-label="Pagination">
  {% for item in pagination.navigation %}
    {% if item.kind == "gap" %}
      <span aria-hidden="true">…</span>
    {% elif item.current %}
      <span aria-current="page">{{ item.page_number }}</span>
    {% else %}
      <a href="{{ item.url }}">{{ item.page_number }}</a>
    {% endif %}
  {% endfor %}
</nav>
{% endblock %}
```

**See it now:** visit `/blog/` and `/blog/page/2.html` to see the two listing
pages.

The [pagination guide](pagination.md) documents routes, current-page values,
adjacent links, and the optional full `pagination_links()` view.

### Optional: reuse the pager as a Tera2 component

This optional refactoring replaces only the `<nav>…</nav>` portion of the
preceding `templates/blog-index.tera` file. Tera2 components keep repeated
template markup in one place. Heine loads every template below `templates/`, and Tera2 makes component definitions available
by name across that set; no import or include is needed. `components.tera` is
therefore a convention, not a special filename. Put this in
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

Replace that `<nav>…</nav>` portion with this component call:

```tera
{{ <ui.pagination pager={pagination} label={t(id="pagination-label")} /> }}
```

Append these messages to the existing locale files:

```ftl
# locales/en/site.ftl
pagination-label = Pagination
```

```ftl
# locales/de/site.ftl
pagination-label = Seitennavigation
```

Components are Tera2's reusable template
mechanism; Heine supplies the values passed to them but does not add a second
component system.

## 8. Add tags

A [taxonomy](glossary.md#taxonomy) classifies the pages of one collection.
Add this taxonomy definition to `heine.toml`:

```toml
[taxonomies.tags]
collection = "posts"
path = "tags"
index_template = "taxonomy-index.tera"
term_template = "taxonomy-term.tera"
per_page = 20
```

Add these literal [terms](glossary.md#term) to `content/en/blog/first.page`:

```toml
[taxonomies]
tags = ["Rust", "Static sites"]
```

Create the two generated-resource templates named in the taxonomy definition:

```tera
{# templates/taxonomy-index.tera #}
{% extends "base.tera" %}
{% block title %}Tags{% endblock %}
{% block content %}
  <h1>Tags</h1>
  <ul>
    {% for term in taxonomy.terms %}
      <li><a href="{{ term.relative_url }}">{{ term.name }}</a> ({{ term.count }})</li>
    {% endfor %}
  </ul>
{% endblock %}
```

```tera
{# templates/taxonomy-term.tera #}
{% extends "base.tera" %}
{% block title %}{{ taxonomy.term.name }}{% endblock %}
{% block content %}
  <h1>{{ taxonomy.term.name }}</h1>
  {% for post in pagination.pages %}
    <article><h2><a href="{{ post.relative_url }}">{{ post.title }}</a></h2></article>
  {% endfor %}
{% endblock %}
```

Heine generates a tags index plus term listings. The taxonomy templates receive
their `taxonomy` view and, for term listings, the normal `pagination` view.
Use the [taxonomy guide](taxonomies.md) for templates, routes, checks, and
declared cross-locale term counterparts.

**See it now:** visit `/tags/`.

## 9. Enable optional Markdown features

Markdown extensions, mathematical rendering, and class-based syntax
highlighting are opt-in. Add these tables to `heine.toml`:

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

Inside the `<head>` in `templates/base.tera`, before the closing `</head>`,
add the generated highlighting stylesheet:

```tera
<link rel="stylesheet" href="{{ highlight_css() }}">
```

`math = "mathml"` renders supported TeX delimiters during the build.
`math = "tex"` instead preserves escaped delimiters for a renderer such as
KaTeX or MathJax that you add yourself. The [reference](reference.md#heinetoml)
lists all allowed modes and highlighting settings.

To see highlighting, replace `content/en/blog/second.md` with:

````markdown
<!-- content/en/blog/second.md -->
```rust
fn main() {
    println!("Hello, Heine!");
}
```
````

**See it now:** visit `/blog/second.html`. The Rust code block uses the
generated highlighting stylesheet.

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

Read the [deployment-files guide](deployment-files.md) before publishing.

Once the site has a stable deployed origin, the [sitemap guide](sitemap.md)
shows how to generate canonical URLs for search-engine discovery.

## 11. Add asset licensing and attributions

When copied assets require rights information, add adjacent `.license`
[asset sidecars](glossary.md#asset-sidecar) and matching texts under
`LICENSES/`. An authored attribution page can render `attributions()`. Read
the [asset-licensing guide](asset-licensing.md) before publishing assets that
carry those requirements.

## 12. Development server details

The server was started in step 1. If it is not running, start it with:

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
