# Collections

Collections are named, deterministic, locale-local ordered sets of rendered
pages. They provide the shared input for article listings, pagination,
taxonomies, and [Atom feeds](atom-feeds.md). A template can read a resolved
collection, but can never create pages or alter its membership. If collections are new to you, build the
[tutorial's collection step](tutorial.md#6-create-an-ordered-post-collection) first.

## Quick start

A collection has three parts: define its ordering once, add pages to
it, then read it from a template. This example lists English blog posts newest
first.

First, define the collection in `heine.toml`:

```toml
[collections.posts]
order = "published-desc"
```

Then add a rendered page to it:

```toml
# content/en/blog/first.page
template = "post.tera"
title = "First post"
published = 2026-08-14T09:30:00
collections = ["posts"]
```

Finally, let a listing template read the collection:

```tera
{% set posts = collection(name="posts") %}
<p>{{ posts.count }} posts</p>

{% for id in posts.page_ids %}
  {% set post = page(id=id) %}
  <article>
    <h2><a href="{{ post.url }}">{{ post.title }}</a></h2>
    <p>{{ post.summary }}</p>
  </article>
{% endfor %}
```

`page_ids` is the already ordered list of logical page IDs. `page(id=id)`
looks up each member's render-facing data and creates its checked URL. This
keeps one authoritative page representation without copying the whole site
into every template render.

For a directory of posts, put `collections = ["posts"]` in its
`_directory.toml` under `[pages]` instead of repeating it on every page. Give
the directory's own `index.page` `collections = []` when it should not list
itself. The [Membership](#membership) section describes that inheritance rule
in full.

Collections establish an ordered relationship only. The listing above is an
ordinary authored page; collections do not independently generate output.

## Terms

| Term                  | Meaning                                                |
|-----------------------|--------------------------------------------------------|
| collection definition | A site-wide named ordering policy.                     |
| membership            | A page's inclusion in one or more named collections.   |
| resolved collection   | The ordered page IDs for one collection in one locale. |

A definition is global because its meaning is shared by every locale. A
resolved collection is locale-local because pages, their metadata, and their
routes belong to one locale tree.

## Authored configuration

`heine.toml` defines the available collections:

```toml
[collections.posts]
order = "published-desc"

[collections.docs]
order = "weight-asc"
```

Collection names are simple identifiers, consistent with other authored names
that become template keys. Every definition has exactly one `order` value:

| Value            | Primary ordering               |
|------------------|--------------------------------|
| `published-asc`  | publication time, oldest first |
| `published-desc` | publication time, newest first |
| `title-asc`      | page title, locale ascending   |
| `title-desc`     | page title, locale descending  |
| `weight-asc`     | page weight, lowest first      |
| `weight-desc`    | page weight, highest first     |
| `id-asc`         | logical page ID, ascending     |
| `id-desc`        | logical page ID, descending    |

Logical page ID is the final deterministic tie-breaker. A collection ordered
by publication time or title requires every member to have that value. Title ordering uses
the resolved collection locale's ICU collation, with literal title spelling as
an additional deterministic tie-breaker. A weight ordering uses the page's
already-resolved numeric weight; a member with `weight = "None"` is an error.
Unknown collection fields, duplicate names, and unknown ordering values are
errors at their authored locations.

Publication ordering compares the resolved instants from required local
`published` timestamps. The site-wide editorial time zone makes this ordering
deterministic without a machine-local default.

## Membership

A page joins a collection through its `.page` metadata:

```toml
# content/en/blog/2026-08-14.page
template = "post.tera"
title = "First post"
published = 2026-08-14T09:30:00
collections = ["posts"]
```

A directory descriptor can provide the normal recursive default:

```toml
# content/en/blog/_directory.toml
[pages]
collections = ["posts"]
```

Page-level `collections` replaces the inherited default. An explicit empty
array opts out, which is useful for a directory index that should share its
directory's template policy but not appear in its article listing.

Each listed name must refer to a configured collection, and a page may name a
collection only once. Only rendered pages may be members: a page with no
template, or with `template = false`, cannot be listed. Ordering diagnostics
show the relevant membership, publication time, title, or weight value when one exists,
and the configured ordering declaration.

This is a concise extension of page metadata, not a second
content-discovery mechanism.

## Template contract

Use `collection()` to read a current-locale collection:

```tera
{% set posts = collection(name="posts") %}
<p>{{ posts.count }} posts</p>

{% for id in posts.page_ids %}
  {% set post = page(id=id) %}
  <article>
    <h2><a href="{{ post.url }}">{{ post.title }}</a></h2>
    <p>{{ post.summary }}</p>
  </article>
{% endfor %}
```

Each collection view exposes:

| Field      | Meaning                                  |
|------------|------------------------------------------|
| `name`     | The configured collection name.          |
| `count`    | Number of member pages in this locale.   |
| `page_ids` | The ordered logical IDs of member pages. |

Use each ID with `page()` to obtain the page's render-facing data and checked
URL. This keeps the collection compact while the page remains authoritative for
its own metadata and content.

Every configured collection is present for every locale, including when it has
no members. After `{% set posts = collection(name="posts") %}`, a template can
therefore rely on `posts.count` being zero rather than needing to distinguish
an empty collection from a missing definition.

## Locale behavior

Definitions are shared, but membership and resolution happen independently
for every configured locale. After resolving `posts`, its `count` may therefore
differ between English and German, and an empty collection is valid. Translation IDs
only connect known counterpart pages; they do not put pages into collections
or require matching membership across locales.

This lets a locale publish a deliberately smaller or differently organized
body of content without hidden fallback or invented translations.

## Pagination

Pagination is the first generated-output consumer of a resolved collection,
not an alternative collection mechanism. It attaches to an ordinary authored
listing page, which supplies page-one metadata, template, and canonical route.
Later pagers are generated render tasks without authored page identities. The
[pagination guide](pagination.md) documents its syntax, routes, template
contract, navigation policy, diagnostics, and quick-build behavior.

## Taxonomies

[Taxonomies](taxonomies.md) classify the members of a collection into literal
terms and generate taxonomy-index and term-listing pages. They preserve the
collection's member order; the taxonomy guide explains term routes and
template values.
