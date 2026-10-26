# Series

A Series is a checked, locale-local reading sequence. Unlike a collection, it
does not select pages through one shared ordering rule. Each page declares its
own position, so one page can occupy different positions in several Series.

Start by configuring the generated index and listing pages:

```toml
[series]
path = "series"
index_template = "series-index.tera"
listing_template = "series.tera"
index_title_key = "series-index-title"
per_page = 10

[series.definitions.getting-started]
title_key = "series-getting-started-title"
```

`path` is the locale-relative generated-output root. The index is generated in
every configured locale, including an empty one. A non-empty local Series gets
its own listing below that root. Both templates are checked even when a Series
has no members. The index title key must resolve in every locale. A definition
title or description key is needed only in a locale where that Series has
members and therefore receives a listing.

Add the corresponding Fluent messages:

```ftl
series-index-title = Series
series-getting-started-title = Getting started
```

Then place memberships beside their pages:

```toml
title = "Install Heine"

[[series]]
id = "getting-started"
position = 10
```

Membership is not inherited from `_directory.toml`. Every membership needs a
configured ID and a positive position. Only rendered pages with titles may be
members. Within one locale and Series, positions are unique. Use gaps such as
`10`, `20`, and `30` so inserting a later member does not immediately require
renumbering.

## Generated routes

For the example above, Heine writes these literal output-file routes:

```text
series/index.html
series/getting-started/index.html
series/getting-started/page/2.html
```

The first listing page is `index.html`. Later pages always use the fixed
`page` segment and compact navigation window `1`; an authored page's
`[pagination]` declaration does not affect Series. Series output participates
in ordinary output ownership and sitemaps. It does not create collection,
taxonomy, feed, or search membership.

Set `sitemap = false` in `[series]` to exclude the generated Series index,
every listing, and every derived pager from a configured sitemap. The setting
accepts only `false`; omit it to keep ordinary sitemap membership. It changes
neither generated routes nor page-level Series navigation.

## Build behavior

Series relationships are full-build concerns. A quick build may re-render an
already resolved Series index or listing when a present member page changes,
but it does not resolve configuration, membership, position, template, rename,
deletion, or other relationship changes. Run a full build, or use the
development server's normal full rebuild, after those changes.

## Page navigation

Every authored page receives `page.series` in the same order as its
`[[series]]` declarations. A membership has `id`, `title`, optional
`description`, `position`, `number`, `total`, `url`, `relative_url`,
`previous`, and `next`. `position` remains the authored ordering key, while
`number` is the resolved one-based place in the local Series. The neighbors
are lightweight page links with `id`, `title`, `url`, and `relative_url`.

```tera
{% for membership in page.series %}
  <nav aria-label="{{ membership.title }}">
    <p>{{ membership.title }}, part {{ membership.number }} of {{ membership.total }}</p>
    {% if membership.previous %}
      <a href="{{ membership.previous.relative_url }}">{{ membership.previous.title }}</a>
    {% endif %}
    {% if membership.next %}
      <a href="{{ membership.next.relative_url }}">{{ membership.next.title }}</a>
    {% endif %}
  </nav>
{% endfor %}
```

A page with several memberships should render several labeled navigation
blocks. Heine does not invent a primary Series or infer one from visitor
history.

## Generated templates and lookups

`series-index.tera` receives `series`, whose `id`, `title`, optional
`description`, `count`, URLs, and `series` entries describe the non-empty
local Series. Each entry has `id`, `title`, optional `description`, `count`,
`url`, and `relative_url`.

`series.tera` receives one `series` value with its `id`, title, description,
count, URLs, and ordered `page_ids`, plus the normal `pagination` view. Render
the current slice through `pagination.pages`.

```tera
{% for page in pagination.pages %}
  <a href="{{ page.relative_url }}">{{ page.title }}</a>
{% endfor %}
```

Use `series()` for the current locale's generated index, or
`series(id="getting-started")` for one local Series. `locale="de"` selects an
exact configured locale for `series()` and `series_has_members()`. A configured
but locally empty Series is not a listing, so use
`series_has_members(id="getting-started")` before an optional cross-page link.
It returns `false` only for that case.

All three functions require `[series]` in `heine.toml`. Invalid IDs and
unknown locales are checked template errors. `series(id="…")` also fails for
an empty local Series, while `series_has_members(id="…")` makes that one
condition available as `false` rather than forcing error-handling-shaped
template code.

## Checks

Heine reports unknown Series IDs, duplicate memberships, duplicate positions,
unrendered members, missing titles, missing Fluent keys for rendered indexes
or listings, missing templates, unsafe routes, colliding definition routes,
and output collisions at the authored setting. Definition-route checks also
apply to unused Series. Locales are independent: a Series may have members in
English and none in German without a warning.
