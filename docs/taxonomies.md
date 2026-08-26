# Taxonomies

Taxonomies classify the explicitly selected members of a collection into
named terms. They are the generated-output counterpart to collections: a
collection decides which pages participate and in what order; a taxonomy
partitions that already ordered set into term listings.

```text
collection      = which rendered pages participate, in one explicit order
taxonomy        = named partitioning of that collection by explicit terms
term            = one ordered subset of the collection
pagination      = slices a term listing into generated output pages
```

A taxonomy is one generic, validated model for tags, categories, series,
topics, and similar classifications.

## Goals

- Keep taxonomy membership explicit in page metadata.
- Reuse collection membership and ordering rather than introducing a second
  query or sorting system.
- Generate an index and term listings as validated render tasks, never from a
  template-side loop.
- Preserve literal term identity and routes; never silently slugify, merge, or
  normalize terms.
- Use the configured locale for human-friendly term ordering without making
  locale collation part of term identity.
- Retain authored source locations for configuration, membership, route, and
  collision diagnostics.

## Authored configuration

Each taxonomy is a complete generated-output contract:

```toml
# heine.toml
[collections.posts]
order = "date-desc"

[taxonomies.tags]
collection = "posts"
path = "tags"
index_template = "taxonomy-index.tera"
term_template = "taxonomy-term.tera"
per_page = 20
```

All five fields are required. Taxonomy pages always use the HTML `.html`
suffix; a taxonomy is an HTML listing feature rather than a second general
output-format mechanism.

| Field | Meaning | Constraint |
|---|---|---|
| `collection` | Source collection | A configured collection name |
| `path` | Generated taxonomy root | A validated portable relative output path |
| `index_template` | Template for the term index | A validated template name |
| `term_template` | Template for term listings and later pagers | A validated template name |
| `per_page` | Members on each term-listing page | A positive integer |

The taxonomy name (`tags` above) is a simple identifier. It is available as a
template-map key and identifies the taxonomy in page metadata. `path` is
separate so a site can deliberately choose a different public route.

Taxonomy definitions are shared by all locales. Resolution and generated
output remain locale-local, just as they do for collections.

## Membership and terms

A page assigns terms through a table keyed by configured taxonomy names:

```toml
# content/en/blog/a-post.page
template = "post.tera"
collections = ["posts"]

[taxonomies]
tags = ["Rust", "static sites"]
```

Each taxonomy assignment has at least one term. Each term is literal text. It
is both its display name and its route component:
the example produces a term route below `tags/Rust.html` and one below
`tags/static sites.html`; URLs percent-encode the latter exactly once.

Terms must be nonempty portable filename components. In particular, they
cannot contain a path separator, end with a dot or space, or collide by case
or Unicode normalization. Heine rejects an invalid
or ambiguous term at its authored membership value instead of inventing a
slug.

A page may assign terms only for a taxonomy whose source collection contains
that page. This makes scope deliberate: a `posts` taxonomy cannot silently
collect an unrelated documentation page. A page may name a term only once for
one taxonomy; duplicate terms are an authored error that identifies both
values.

Membership is locale-local. Terms with the same spelling in two locale trees
remain independent, and no counterpart term is inferred for a language
switcher. A future explicit term-translation model may add that relationship
when a real use case needs it.

## Generated output

For the configuration above, English output includes:

```text
tags/index.html
tags/Rust.html
tags/static sites.html
tags/Rust/page/2.html       # only when the term has more than 20 members
```

The default locale is unprefixed. Another locale receives its existing locale
prefix, for example `de/tags/Rust.html`.

Heine always generates the configured taxonomy index, even when it has no
terms. A term listing exists only for a term that has at least one rendered
member. Later pager routes use the existing pagination route policy and the
term template. All generated routes participate in normal output ownership,
portable-path validation, collision detection, atomic publication, and
base-path-aware URL generation.

Both configured templates are validated on every build, including an empty
taxonomy whose term template has no current render task. A missing template is
therefore always reported at its `heine.toml` reference.

## Ordering

Each term preserves the source collection's already resolved ordering. The
taxonomy adds no competing order setting.

The index orders terms using ICU locale-aware collation for the locale being
rendered. Collation affects presentation order only: it never changes a term's
literal identity, display spelling, or route. Terms that compare equally under
the locale collator use their literal spelling as a deterministic tie-breaker.

This lets German, Polish, Arabic, and other locale trees receive natural
term-index ordering without silently merging terms that differ by case,
accents, or normalization.

## Template contract

An ordinary page exposes its assigned term links directly:

```tera
{% for term in page.taxonomies.tags %}
  <a href="{{ term.relative_url }}">{{ term.name }}</a>
{% endfor %}
```

Any template can resolve its current locale's taxonomy index through the
validated `taxonomy()` function. This is useful for a shared navigation
template without duplicating a generated route:

```tera
{% set tags = taxonomy(name="tags") %}
<a href="{{ tags.url }}">Tags</a>
```

The returned view has `name`, base-path-aware `url`, and
current-output-relative `relative_url`. A malformed or unknown taxonomy name
fails at the template call site and names the available configured taxonomies.

The taxonomy index template receives a `taxonomy` view with its name and
ordered terms:

```tera
{% for term in taxonomy.terms %}
  <a href="{{ term.relative_url }}">{{ term.name }} ({{ term.count }})</a>
{% endfor %}
```

A term template receives the current term and the existing pagination view:

```tera
<h1>{{ taxonomy.term.name }}</h1>

{% for post in pagination.pages %}
  <a href="{{ post.relative_url }}">{{ post.title }}</a>
{% endfor %}
```

Term templates are generated-resource templates. They receive `taxonomy` and
`pagination`, not a fabricated authored `page` identity. The render context
still carries the generated output directory, so `asset()`, `page()`, and all
relative links remain correct.

Term pagination uses the same compact navigation view as authored listings;
its fixed navigation window is `1`. Use `pagination_links()` when a template
intentionally needs every page link.

The [reference](reference.md) contains the complete field tables. Heine does
not add a general template query language: ordinary pages have their own terms
and the generated index already exposes every term.

## Validation and build behavior

Before rendering, Heine validates taxonomy definitions, page membership keys,
term values, required source-collection membership, template references,
generated routes, and all output collisions. Diagnostics retain the relevant
configuration and membership spans; route collisions identify every authored
claim that caused them.

Taxonomy relationships are full-build concerns. A quick build re-renders the
affected generated term listings when a present member page changes, but it
retains its existing limitations for configuration, template, relationship,
rename, and deletion changes.

## Deliberately deferred

- Per-term descriptions, images, aliases, display-label overrides, and manual
  term weights.
- Pagination for very large taxonomy indexes.
- Related-page ranking based on shared terms.
- Explicit cross-locale term relationships.
- Per-term Atom feeds, which depend on the later canonical-URL and feed work.

These remain possible because terms, their generated routes, and their
rendering context are explicit domain values. They are not part of the first
feature because they would add independent content, metadata, or ranking
models without a demonstrated need.
