# Collections

> **Status:** explicit membership, deterministic ordering, paginated
> collection listings, and taxonomies are implemented. Other generated-resource
> consumers remain future features.

Collections are named, deterministic, locale-local ordered sets of rendered
pages. They provide one shared foundation for article listings, pagination,
taxonomies, archives, feeds, sitemaps, and search indexes. A template can read
a resolved collection, but can never create pages or alter its membership.

## Quick start

A collection has three explicit parts: define its ordering once, add pages to
it, then read it from a template. This example lists English blog posts newest
first.

First, define the collection in `heine.toml`:

```toml
[collections.posts]
order = "date-desc"
```

Then add a rendered page to it:

```toml
# content/en/blog/first.page
template = "post.tera"
title = "First post"
date = 2026-08-14
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
looks up each member's render-facing data and creates its validated URL. This
keeps one authoritative page representation without copying the whole site
into every template render.

For a directory of posts, put `collections = ["posts"]` in its
`_directory.toml` under `[pages]` instead of repeating it on every page. Give
the directory's own `index.page` `collections = []` when it should not list
itself. The [Membership](#membership) section describes that inheritance rule
in full.

Collections establish an ordered relationship only. The listing above is an
ordinary authored page; collections do not independently generate output.

## Goals

The design makes a common task—listing a deliberate group of pages—small
and unsurprising, while preserving Heine's existing guarantees:

- membership is explicit in authored page metadata;
- membership and ordering errors are found before rendering;
- each locale is independent, with deterministic output and diagnostics;
- templates receive a compact, read-only render-facing view; and
- later generated pages are explicit render tasks owned by the build, not
  template-side page generation.

Collections intentionally are not a generic query language. Heine does not
infer membership from a directory name, template name, filename convention, or
tag. Such inference is convenient until a site grows exceptions; explicit
membership stays easy to inspect and validate.

Nor does a collection itself write output. It is a resolved relationship. A
listing, feed, or other generated resource is a separate consumer that makes
its own validated output claim.

## Terms

| Term | Meaning |
|---|---|
| collection definition | A site-wide named ordering policy. |
| membership | A page's explicit inclusion in one or more named collections. |
| resolved collection | The ordered page IDs for one collection in one locale. |

A definition is global because its meaning is shared by every locale. A
resolved collection is locale-local because pages, their metadata, and their
routes belong to one locale tree.

## Authored configuration

`heine.toml` defines the available collections:

```toml
[collections.posts]
order = "date-desc"

[collections.docs]
order = "weight-asc"
```

Collection names are simple identifiers, consistent with other authored names
that become template keys. Every definition has exactly one `order` value:

| Value         | Primary ordering            |
|---------------|-----------------------------|
| `date-asc`    | page date, oldest first     |
| `date-desc`   | page date, newest first     |
| `weight-asc`  | page weight, lowest first   |
| `weight-desc` | page weight, highest first  |
| `id-asc`      | logical page ID, ascending  |
| `id-desc`     | logical page ID, descending |

Logical page ID is always the deterministic tie-breaker. A collection ordered
by date requires every member to have a date. A weight ordering uses the
page's already-resolved numeric weight; a member with `weight = "None"` is an
error. Unknown collection fields, duplicate names, and unknown ordering values
are errors at their authored locations.

Date-only values and offset-aware date-times are intentionally different
precisions. A date-ordered collection must use one precision consistently:
either every member has a TOML date or every member has an offset-aware
date-time. Heine must not invent a time zone or a time of day to make those
values comparable. A mixed collection is an error that labels the relevant
members; an author can make the intended precision explicit.

Title ordering is deliberately absent initially. Correct human-language title
ordering requires a defined locale-aware collation contract; byte ordering
would look like locale-aware behavior while silently being wrong. It can be
added later as an explicit extension.

## Membership

A page joins a collection through its `.page` metadata:

```toml
# content/en/blog/2026-08-14.page
template = "post.tera"
title = "First post"
date = 2026-08-14
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
show the relevant membership, date or weight value when one exists, and the
configured ordering declaration.

This is purposely a concise, explicit extension of page metadata—not a second
content-discovery mechanism.

## Resolution and ownership

Collection resolution belongs in the site domain. It runs after Heine has
loaded pages, applied inherited metadata and template policy, and validated
the page output model, but before any template is rendered or output is
written.

The resolved site stores this relationship conceptually as:

```text
locale → collection name → ordered page IDs
```

It stores IDs rather than cloned page values. The existing resolved page map
remains the single source of page facts; rendering follows an ID to obtain its
render-facing page value. `BTreeMap` provides stable iteration where it is
observable in output, diagnostics, and tests.

Resolution is straightforward: collect validated memberships, verify the
ordering data, sort once with the documented tie-breaker, then freeze the
result into the resolved `Site`. It is linear apart from the required sort of
each collection. This avoids repeated template-time filtering and makes later
generated render tasks use the same stable input.

Membership and ordering are relationships. Therefore they remain a full-build
concern: `--quick` retains its documented limitation for configuration and
relationship changes.

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

Each collection view initially exposes:

| Field      | Meaning                                  |
|------------|------------------------------------------|
| `name`     | The configured collection name.          |
| `count`    | Number of member pages in this locale.   |
| `page_ids` | The ordered logical IDs of member pages. |

`page_ids`, together with `page()`, is intentionally more
explicit than copying every full page into every collection. It avoids a
second, stale-prone representation of page data while keeping Tera usage
simple. `page()` remains the validated operation for producing a page URL.

Every configured collection is present for every locale, including when it has
no members. After `{% set posts = collection(name="posts") %}`, a template can
therefore rely on `posts.count` being zero rather than needing to distinguish
an empty collection from a missing definition.

Exposing a page's own collection names is not necessary for listings and will
be deferred until there is a demonstrated template use case.

## Locale behavior

Definitions are shared, but membership and resolution happen independently
for every configured locale. After resolving `posts`, its `count` may therefore
differ between English and German, and an empty collection is valid. Translation IDs
only connect known counterpart pages; they do not put pages into collections
or require matching membership across locales.

This lets a locale publish a deliberately smaller or differently organized
body of content without hidden fallback or invented translations.

## Design rationale

Collections are deliberately resolved before templates render. The resolved
site keeps ordered page IDs rather than cloned page data, so pages remain the
single source of metadata and rendered templates cannot change membership.
This also lets later output-generating features consume the same validated,
deterministic relationship.

Internally, authored names and order values become validated collection types;
raw configuration and membership declarations retain their source locations.
That is why configuration and relationship errors can identify both the
offending page value and the relevant collection definition. The template and
output domains receive only the information each needs.

## Pagination

Pagination is the first generated-output consumer of a resolved collection,
not an alternative collection mechanism. It attaches to an ordinary authored
listing page, which supplies page-one metadata, template, and canonical route.
Later pagers are explicit render tasks without authored page identities. The
[pagination guide](pagination.md) documents its syntax, routes, template
contract, navigation policy, diagnostics, and quick-build behavior.

## Taxonomies and other consumers

[Taxonomies](taxonomies.md) build term collections on this same core. A
configured taxonomy turns validated page terms into deterministic term
collections and generates index and term-listing pages from explicit render
tasks. Their literal term routes, locale-aware ordering, and output-safe
mapping are documented in the taxonomy guide.

Feeds, sitemaps, archives, and search indexes likewise consume resolved
collections. They describe generated output to the host; the output domain
continues to own safe paths, collision checks, and filesystem writes.

## Page metrics

Word count and reading time are properties of a page's rendered content, not
of a collection. If introduced, they belong in the content/site pipeline as
validated per-page metrics. Collections may then order, display, or aggregate
those already-computed values without coupling page analysis to a listing
feature.

## Diagnostics, tests, and documentation

Diagnostics for authored collection errors retain source text and spans for an
invalid collection name or ordering value, duplicate membership declarations,
and the relevant membership, date, weight, and ordering declaration for
relationship errors. Pagination route collisions are validated now and label
every claim that caused them; taxonomies use the same output rule.

The implementation includes focused unit tests and fixture-site tests for
deterministic ordering and tie-breaking, directory-default overrides,
per-locale independence, empty collections, Tera exposure, invalid
configuration, source-aware diagnostics, pagination route generation, and
quick-mode updates for present collection members. The starter site uses the
post collection for its localized paginated blog listing, while retaining its
sibling-list example for the quiz.
