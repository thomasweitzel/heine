# Internationalization and localization

This guide explains how one Heine site can render configured locale trees. Start
with the [tutorial](tutorial.md) for a first multilingual site, and use the
[reference](reference.md) for the complete configuration and template contract.

## Terms

Internationalization (i18n) is Heine's locale-aware site model: configured
[locales](glossary.md#locale), source trees, page identity, routes, links,
assets, data, and output checks. Localization (l10n) is the locale-specific
result: messages, plural forms, formatted values, and authored page content.

Heine renders a finite configured set of locales at build time. URLs select a
concrete locale; it does not negotiate a language at request time.

## Locale configuration and routes

`heine.toml` declares a default locale and the finite set of supported
BCP-47 locales. A source directory is required for every configured locale.

```toml
[i18n]
default_locale = "en"

[i18n.locales.en]
direction = "ltr"

[i18n.locales.de]
direction = "ltr"

[i18n.locales.pl]
direction = "ltr"
```

The default locale is unprefixed. Every other locale has its locale code as a
URL prefix.

```text
content/en/about.page        -> /about.html
content/de/ueber-uns.page    -> /de/ueber-uns.html
```

Rendered routes continue to derive from checked source-relative paths. There
is no i18n-specific route override.

## Text direction

Each locale table requires `direction = "ltr"` or `direction = "rtl"`. The
choice is explicit because a BCP-47 locale alone does not reliably establish a
writing direction: a language can be written in more than one script. Heine
exposes the configured identity as `locale.code` and `locale.direction`:

```tera
<html lang="{{ locale.code }}" dir="{{ locale.direction }}">
```

An RTL locale can therefore render alongside LTR locales in one site without
changing routes, content lookup, assets, translated-page links, or Fluent
message lookup. Heine sets no CSS direction rules and does not rewrite authored
content. Themes and sites should use logical CSS properties such as
`margin-inline`, `padding-inline`, `border-inline-start`, and `text-align: end`
for direction-neutral layout. Their font stack must cover every script they
publish.

For an embedded fragment whose direction differs from its page, mark the
fragment in authored HTML or Markdown:

```html
<blockquote lang="ar" dir="rtl">
  <p>…</p>
</blockquote>
```

Use a nested `dir` only for a genuine directional boundary. Source-code blocks
normally remain LTR regardless of their containing locale.

## Source trees

The source tree gives each role a distinct location.

```text
content/
├── assets/                  # shared copied assets; default-locale variants
│   └── images/logo.svg
├── en/                      # default-locale pages, content, and local assets
│   └── index.page
└── de/                      # German pages, content, and local assets
    ├── start.page
    └── assets/
        └── images/logo.svg  # German overlay of the shared logo

data/
├── site.toml                # shared structured data
├── en/
│   └── navigation.toml
└── de/
    └── navigation.toml

locales/
├── en/
│   └── site.ftl
└── de/
    └── site.ftl
```

`content/assets/` is copied to the shared asset URL space. It is also the
default-locale form of an overridable shared asset. A non-default locale may
place the same logical asset below `content/<locale>/assets/`; it is selected
only while rendering that locale and receives that locale's URL prefix. This
is a narrow, deterministic overlay: Heine checks the current non-default
locale and then the shared asset. It never searches other locales.

The default locale does not overlay `content/assets/`: its desired variant is
the shared file itself. Defining the same asset below
`content/<default-locale>/assets/` is a build error, rather than an implicit
precedence rule. Locale-specific assets without a shared counterpart belong
below their locale's source tree. Article assets may remain beside their
locale-specific article content.

Shared data remains below `data/`. Locale-specific data belongs below
`data/<locale>/`; it has no implicit fallback. Templates receive these as
separate `data` and [`locale_data`](glossary.md#locale-data) values. They are
not merged:

```toml
# data/site.toml
name = "Example site"

# data/de/navigation.toml
label = "Blog"
```

```tera
<p>{{ data.site.name }}</p>
{% set blog = page(id='blog/index') %}
<a href="{{ blog.url }}">{{ locale_data.navigation.label }}</a>
```

The second value exists only while rendering German pages. A missing
`locale_data.navigation` value is a template error; it does not read
`data.navigation` instead.

## Page counterparts

Heine never infers that matching filenames or paths are translations. A page
may instead declare a `translationid`:

```toml
translationid = "welcome-post"
```

The same ID on pages from different locales creates a translation group. A
group may be partial: it may have English and German pages without
Polish. A page belongs to at most one group, and a group may contain no more
than one page from each locale. Duplicate pages in one locale are a build
error that identifies both declarations.

The render-facing page view exposes only known counterparts through
`page.translations`. Its keys are locale codes and its values contain a
counterpart `id` plus output-relative `path`; use `root` with `path`, or the
checked `page()` helper, to create a link. A language switcher therefore
offers only existing pages; Heine never guesses a URL, redirects to an
unrelated language, or requires matching source-tree structures.

For example, this shared-header switcher renders links only for the current
page's known counterparts. The `language-<locale>` message IDs are a site
convention for localized labels; Heine does not prescribe label text.

```tera
{% if page %}
  {% if page.translations %}
    <nav aria-label="{{ t(id="language") }}">
      {% for locale, translation in page.translations %}
        {% set target = page(locale=locale, id=translation.id) %}
        {% set label = "language-" ~ locale %}
        <a href="{{ target.url }}" hreflang="{{ locale }}">{{ t(id=label) }}</a>
      {% endfor %}
    </nav>
  {% endif %}
{% endif %}
```

When a site enables sitemap `hreflang` annotations, these same explicit groups
also identify localized sitemap counterparts. Sitemap membership remains
separate: a page excluded with `sitemap = false` is not named as an alternate.
See [sitemaps](sitemap.md#localized-counterparts) for that output contract.

`page(id="...")` addresses an exact page ID in the locale currently being
rendered. Its optional `locale` argument addresses an exact page ID in that
named locale; it never resolves a translation group implicitly. Markdown has
the equivalent checked page-link syntax: `page:<id>` addresses the current
locale, and `page:<locale>:<id>` addresses an explicit locale. Both support
query strings and fragments. It applies to Markdown
links, including reference-style links, but not to image sources. Template and
Markdown references share one page-resolution path and report a missing locale
or page at the authored call site.

## Messages and formatting

Fluent resources under `locales/<locale>/` provide reusable localized text.
The `t()` template function uses the locale currently being rendered and
returns escaped text.

```ftl
# locales/en/site.ftl
welcome = Welcome, { $name }.
post-count = { $count ->
    [one] One post
   *[other] { $count } posts
}
```

```tera
{{ t(id="welcome", name=data.site.name) }}
{{ t(id="post-count", count=posts.count) }}
```

Fluent messages may use named values, terms, attributes, message references,
and select expressions. Select expressions provide correct language-specific
plural categories and can also express gender or other controlled choices.
The initial value contract accepts strings, numbers, and booleans/select
values. Dates use the separate `format_date()` template function so the stable
UTC RFC 3339 timestamp can remain suitable for HTML `datetime` attributes. It
accepts the rendered `page.published` value and formats its calendar date for the current locale
with `short`, `medium`, or `long` styles. It formats calendar dates, not times
or time zones. The template boundary does not expose arbitrary objects, arrays,
raw HTML, Fluent internals, or filesystem access.

Keep the machine-readable value in `datetime` and format only the visible
text:

```tera
<time datetime="{{ page.published }}">
  {{ format_date(value=page.published, style="long") }}
</time>
```

Long-form prose belongs in locale-specific content files, rather than being
assembled from message fragments. A template may safely place `t()` output
inside its HTML structure; messages themselves are not trusted HTML.

Message lookup first uses the current locale, then the configured default
locale, but only when the requested message or attribute is absent. A malformed
current-locale message or invalid formatting argument is not hidden by a
fallback value. A message absent after fallback is a build error with the
message call site and attempted locale chain. Invalid Fluent resources,
duplicate IDs, invalid arguments, and invalid locale configuration are build
errors. Heine does not use warnings for translation-resolution failures.

## Render boundary and checks

Templates receive a render-facing model, including `locale`, shared
`data`, `locale_data`, `page`, `page.translations`, `t()`, `asset()`, and
`page()`. They do not receive raw resource bundles, source paths, mutable site
state, or arbitrary access to every locale's content.

Heine owns locale resolution, fallback, translation-group checks, route
and output collision detection, and all writes. Every generated page and
asset still claims its output through the output domain before rendering.
