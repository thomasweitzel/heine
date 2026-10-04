# Taxonomies

Taxonomies classify the explicitly selected members of a
[collection](glossary.md#collection) into
named terms. They are the generated-output counterpart to collections: a
collection decides which pages participate and in what order; a taxonomy
partitions that already ordered set into term listings.

```text
collection      = which rendered pages participate, in one explicit order
taxonomy        = named partitioning of that collection by explicit terms
term            = one ordered subset of the collection
pagination      = slices a term listing into generated output pages
```

A taxonomy is one generic model for tags, categories, topics, and similar
classifications. A checked ordered reading sequence is a separate
[Series](series.md) relationship because each page may need a different
position in more than one sequence. Heine checks its authored membership and
output relationships during the build. Start with the
[tutorial's tag step](tutorial.md#8-add-tags) when adding a taxonomy for the
first time.

## Authored configuration

Each taxonomy is a complete generated-output contract:

```toml
# heine.toml
[collections.posts]
order = "published-desc"

[taxonomies.tags]
collection = "posts"
path = "tags"
index_template = "taxonomy-index.tera"
term_template = "taxonomy-term.tera"
per_page = 20
```

The five fields are required; optional `counterparts` groups declare explicit
cross-locale term counterparts. Optional `sitemap = false` excludes this
taxonomy's complete generated listing family from a configured sitemap.
Taxonomy pages always use the HTML `.html`
suffix; a taxonomy is an HTML listing feature rather than a second general
output-format mechanism.

A counterpart group is taxonomy structure, not a Fluent resource or a display
translation. Its name is an opaque local identifier; its locale values name
the literal terms that correspond across those locales.

| Field            | Meaning                                     | Constraint                              |
|------------------|---------------------------------------------|-----------------------------------------|
| `collection`     | Source collection                           | A configured collection name            |
| `path`           | Generated taxonomy root                     | A checked portable relative output path |
| `index_template` | Template for the term index                 | A checked template name                 |
| `term_template`  | Template for term listings and later pagers | A checked template name                 |
| `per_page`       | Members on each term-listing page           | A positive integer                      |
| `sitemap`        | Sitemap membership for generated listings   | Only `false`; requires `[sitemap]`      |

The taxonomy name (`tags` above) is a simple identifier. It is available as a
template-map key and identifies the taxonomy in page metadata. `path` is
separate so a site can choose a different public route.

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
that page. A `posts` taxonomy cannot silently
collect an unrelated documentation page. A page may name a term only once for
one taxonomy; duplicate terms are an authored error that identifies both
values.

Membership is locale-local. Terms with the same spelling in two locale trees
remain independent, and no counterpart term is inferred for a language
switcher. Declare counterparts in the taxonomy's optional
`counterparts` groups, described in the [template contract](#template-contract).

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
member. Later pagers always use the fixed `page` route segment shown above and
the term template; authored `pagination.segment` settings do not apply to
taxonomy listings. All generated routes participate in normal output ownership,
portable-path checks, collision detection, atomic publication, and
base-path-aware URL generation.

Both configured templates are checked on every build, including an empty
taxonomy whose term template has no current render task. A missing template is
therefore always reported at its `heine.toml` reference.

## Ordering

Each term preserves the source collection's already resolved ordering. The
taxonomy adds no competing order setting.

The index orders terms using ICU locale-aware collation for the locale being
rendered. Collation affects presentation order only: it never changes a term's
literal identity, display spelling, or route. Terms that compare equally under
the locale collator use their literal spelling as a deterministic tie-breaker.

This gives German, Polish, Arabic, and other locale trees locale-aware
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
checked `taxonomy()` function. A shared navigation template can use it without
duplicating a generated route:

```tera
{% set tags = taxonomy(name="tags") %}
<a href="{{ tags.url }}">Tags</a>
```

The returned view has `name`, base-path-aware `url`, and
current-output-relative `relative_url`. A malformed or unknown taxonomy name
fails at the template call site and names the available configured taxonomies.

Pass an exact configured locale when a template needs another locale's index:

```tera
{% set german_tags = taxonomy(name="tags", locale="de") %}
<a href="{{ german_tags.url }}">Schlagwörter</a>
```

This selects only the taxonomy index. It never infers that a literal term in
one locale corresponds to a term in another. Declare known counterparts in
the taxonomy configuration instead:

```toml
[taxonomies.tags.counterparts.rust]
en = "Rust"
de = "Rust"

[taxonomies.tags.counterparts.static-sites]
en = "Static sites"
de = "Statische Seiten"
```

The final component (`rust`, `static-sites`) is an opaque, non-empty group
name. Each group has at least two configured locales. Every listed term must
exist literally in that locale's taxonomy, and a locale-term pair may belong
to only one group. Groups may be partial: a term such as `MathML` that exists
only in English simply has no group and therefore no language counterpart.

The generated term views expose the declared relationships:

```tera
{% for target_locale, target in taxonomy.term.counterparts %}
  <a href="{{ target.url }}" hreflang="{{ target_locale }}">{{ target.name }}</a>
{% endfor %}
```

Each target has its literal `name`, base-path-aware `url`, and
current-output-relative `relative_url`. This is available both for a term in
the taxonomy index's `taxonomy.terms` and for the current term listing's
`taxonomy.term`. Empty `counterparts` means that no counterpart was declared.
Each counterpart always points to that term's first listing page; Heine does
not infer a correspondence between later pagers in different locales.

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
needs every page link.

The [reference](reference.md) contains the complete field tables. Heine does
not add a general template query language: ordinary pages have their own terms
and the generated index already exposes every term.

## Checks and build behavior

Before rendering, Heine checks taxonomy definitions, page membership keys,
term values, required source-collection membership, template references,
generated routes, and all output collisions. Diagnostics retain the relevant
configuration and membership spans; route collisions identify every authored
claim that caused them.

Taxonomy relationships are full-build concerns. A quick build re-renders the
affected generated term listings when a present member page changes, but it
retains its existing limitations for configuration, template, relationship,
rename, and deletion changes.
