# Pagination

Use pagination when an ordered [collection](glossary.md#collection) needs more
than one listing page. An ordinary listing declares `[pagination]`; a taxonomy
term listing uses the `per_page` value from its taxonomy definition. Heine
keeps the collection's existing membership and order, then creates the needed
listing pages from that result.

Start with the [collection guide](collections.md) or the
[tutorial's pagination step](tutorial.md#7-paginate-the-blog-index) if the
collection model is new to you.

## A paginated listing

First define an ordered collection in `heine.toml`:

```toml
[collections.posts]
order = "published-desc"
```

Then make an ordinary page a listing:

```toml
# content/en/blog/index.page
template = "section-index.tera"
title = "Blog"
# Keep this listing page itself out of the posts collection.
collections = []

[pagination]
collection = "posts"
per_page = 10

[pagination.navigation]
window = 1
```

`collection` and `per_page` are required. `per_page` must be positive. The
listing page itself is page one; Heine renders later pages only when the
collection has more members. An empty collection still renders its authored
listing once.

Collection order remains the single source of ordering. A listing cannot
filter, reverse, or sort its collection.

Taxonomy term listings use the same `pagination` view and
`pagination_links()` function. Their generated page-one route is the literal
term route, and later pages use `path/<term>/page/<number>.html`; their compact
navigation window is `1`.

[Series](series.md) listings also use this view. Their route segment and
navigation window are fixed by the Series contract rather than an authored
`[pagination]` table.

## Routes

Later pagers use the `page` route segment by default. For an authored listing
at `blog/index.html`, they are `blog/page/2.html`, `blog/page/3.html`, and so
on. For `archive.html`, they are `archive/page/2.html`, `archive/page/3.html`,
and so on. There is no duplicate `page/1` output.

Change the segment only when a site has a concrete routing need:

```toml
[pagination]
collection = "posts"
per_page = 10
segment = "older"
```

`segment` must use ASCII letters or digits, or `-._~!$&'()*+,;=@`; it cannot
be `.` or `..`, end in `.`, or contain percent escapes. Generated paths are
both URLs and filesystem names, so use `older-posts` rather than
`older%20posts`.
Generated routes use
the listing's configured suffix, locale prefix, and site `base_path`, and they
participate in normal output-collision checks before anything is written.

## Template values

`pagination` is `null` while rendering an ordinary page. A paginated listing
receives this value:

| Field                      | Meaning                                                                                          |
|----------------------------|--------------------------------------------------------------------------------------------------|
| `collection`               | configured collection name                                                                       |
| `pages`                    | render-facing page views in the current slice, each with `url` and pager-relative `relative_url` |
| `item_count`               | total collection members                                                                         |
| `per_page`                 | configured slice size                                                                            |
| `first_item_number`        | one-based number of the first current-slice item, or `0` for an empty listing                    |
| `page_number`              | current one-based pager number                                                                   |
| `page_count`               | total pager count; at least one                                                                  |
| `current`, `first`, `last` | pager link views                                                                                 |
| `previous`, `next`         | adjacent pager link views, or `null`                                                             |
| `navigation`               | compact link and gap items for ordinary navigation                                               |

A pager link has `page_number`, `url`, `relative_url`, and `current` fields.
The URL fields have the same base-path and relative-link behavior as Heine's
other resource views.

```tera
{% if pagination %}
  {% for post in pagination.pages %}
    <article>
      <h2><a href="{{ post.relative_url }}">{{ post.title }}</a></h2>
      {% if post.summary %}<p>{{ post.summary }}</p>{% endif %}
    </article>
  {% endfor %}

  <nav aria-label="Pagination">
    {% for item in pagination.navigation %}
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
```

The authored `page` remains the listing page for every pager. Use
`pagination.current` where a template needs the generated page's URL.

## Navigation window

`[pagination.navigation].window` defaults to `1` and accepts values from `1`
through `3`. The first and last page always remain visible. The window preserves
that many adjacent links on each side of the current page. Overlapping ranges
merge before gaps are added.

For pager 101 of 1,202:

```text
window = 1:  1 … 100 101 102 … 1202
window = 2:  1 … 99 100 101 102 103 … 1202
window = 3:  1 … 98 99 100 101 102 103 104 … 1202
```

Listings with no omitted ranges show every pager once: two pagers always yield
`1 2`, never repeated links or ellipses.

For a complete sequence, call `pagination_links()` inside a
paginated listing template. It returns every pager link, without gap items:

```tera
{% for link in pagination_links() %}
  {% if link.current %}
    <span aria-current="page">{{ link.page_number }}</span>
  {% else %}
    <a href="{{ link.relative_url }}">{{ link.page_number }}</a>
  {% endif %}
{% endfor %}
```

This function is explicit because a complete sequence can be large. Calling
it outside a paginated listing is a template call-site error.

## Build behavior

Heine resolves paginated listings after collections and before template
rendering. Every generated pager is an output task, not an authored page: it
has no page ID and does not appear in `page()`, collections, siblings, or
translations.

Full builds check every generated route and render every pager before
publishing. Quick builds re-render all pagers of a listing when a present
member page changes. As elsewhere, quick builds do not detect
renames, deletions, template changes, configuration changes, or changed
relationships; use a full build for those changes.
