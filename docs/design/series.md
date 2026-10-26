# Series design

## Status

This is the accepted design for Series. The implemented user-facing contract
is documented in the [Series guide](../series.md) and
[reference](../reference.md).

## Decision

Heine provides **Series** as checked, locale-local ordered reading
sequences. A rendered page may belong to zero or more named Series. Each
membership declares its own positive position, so the same page can occupy a
different place in every Series it joins.

Heine resolves every membership into template-facing facts, including the
Series title, its authored position, resolved one-based number and size, and
the immediately preceding and following members of that particular Series. A
template renders zero, one, or several labeled navigation blocks.
Heine does not invent a primary
Series, infer visitor state from a referrer, or make one page URL vary by the
path used to reach it.

Series generate a locale-local index and paginated listing pages, but they
neither select a source collection nor change page rendering. Collections,
taxonomies, and Series are separate
relationships with distinct purposes.

## Motivation

A Series describes an authored reading path. It is not merely a classification
such as a tag, and it is not a broad listing selected by a shared sort order.
For example, one article may be the second member of a Rust-performance
sequence and the fifth member of a backend-systems sequence. Its ordinary page
weight or publication time cannot represent both positions.

Existing SSG mechanisms generally leave this problem incomplete. Taxonomies
provide convenient groups and generated term listings, but their membership
ordering is normally shared by every term. Directory or collection navigation
has one ordering policy for all members. Documentation sidebars can represent
several outlines, but a static document URL has no inherent current-sidebar
state. Picking one silently, adding query-string state, or duplicating a page
under several routes makes the model less honest.

Heine instead retains every true membership and makes each navigation path
visible to the template. A page with several Series memberships can render
several blocks, each labeled with its Series title and position. The site
chooses the presentation, while Heine checks the relationship and supplies the
navigation facts.

## Terminology

- **Series definition**: the site-wide configuration record for one named
  sequence, including its Fluent title and optional description keys.
- **Series membership**: one page's declaration that it occupies one position
  in one Series.
- **Series listing**: a generated, paginated page showing the ordered members
  of one Series in one locale.
- **Series index**: the generated locale-local page listing the Series that
  have members in that locale.
- **Series neighbor**: the preceding or following member in one resolved
  Series. It is never an unqualified global page neighbor.

## Configuration

Enabling Series configures one generated-resource domain:

```toml
[series]
path = "series"
index_template = "series-index.tera"
listing_template = "series.tera"
index_title_key = "series-index-title"
index_description_key = "series-index-description"
per_page = 10

[series.definitions.heinrich-heine]
title_key = "series-heinrich-heine-title"
description_key = "series-heinrich-heine-description"
```

`path` is a portable relative output path outside `__heine/`. It is the
locale-relative root for generated Series output. `index_template` and
`listing_template` are logical template names below `templates/` and are
checked even when a locale currently has no Series members. `per_page` is a
required positive integer. Like generated taxonomy listings, Series pagers use
the fixed `page` route segment and a fixed compact navigation window of `1`;
authored-page `[pagination]` settings do not apply. A template may use
`pagination_links()` when it needs every pager link.

`index_title_key` is a required Fluent message identifier. Its optional
description counterpart supplies descriptive text for the generated index.
Every definition ID is a literal portable URL component without percent
escapes. `title_key` is required; `description_key` is optional. These keys
are references to localized resources, not interface text stored in generator
configuration. Index keys are resolved in every locale because every locale
receives an index. Definition keys are resolved only for locales where that
Series has members and therefore receives a listing.

A definition may be unused. This permits a site to prepare a planned Series
without creating placeholder content. Unknown definition fields, malformed
IDs, empty keys, invalid paths, non-positive `per_page`, and missing templates
are checked at their authored settings. Fluent lookup follows Heine's normal
locale fallback and error rules.

Series definitions are a checked catalog, not names discovered implicitly
from pages. A misspelled membership ID must therefore fail at the page that
declared it instead of silently creating a new sequence.

## Membership

A `.page` file declares memberships with an array of tables:

```toml
[[series]]
id = "rust-performance"
position = 2

[[series]]
id = "backend-systems"
position = 5
```

The field may be omitted when a page belongs to no Series. Each membership
uses one configured definition and one positive integer position. Positions
need not be consecutive, so `10`, `20`, and `30` leave room for a later
insertion.

Membership is page-local and is not inherited from `_directory.toml`. A
directory cannot know which distinct positions its descendant pages should
occupy. Only rendered pages may be members, and every member must have a
title so that navigation can display a label.

For each locale and Series ID, Heine checks that:

- every member refers to a configured definition;
- a page declares that Series at most once;
- every position is positive and used by at most one page; and
- every member is rendered and has a title.

The same Series definition may have different memberships, positions, and
lengths in different locales. Heine does not require translated page trees or
Series structures to match. A page's membership array preserves authored order
for template presentation; this ordering does not make its first membership a
primary Series. A locale that has no members for a Series does not produce a
warning: partial locale structures are valid by design.

## Position maintenance

Positions are authored integers, not an inferred order. Use gaps
appropriate to the expected editing frequency. For example, `100`, `200`, and
`300` leave substantially more room for insertion than `1`, `2`, and `3`.
Eventually a densely edited Series may need renumbering; Heine does not
automatically rewrite authored page files in the first version. A later
explicit maintenance command such as `heine series renumber <id>` may be
justified by real use, but it must remain a visible author-controlled change.

## Resolution and generated output

Series resolve after pages and their rendered output facts are known. Heine
groups memberships by locale and Series ID, then orders every group by
ascending membership position. Duplicate positions are errors rather than an
implicit page-ID tie-breaker, because an authored sequence has one intended
member at each position.

The generated index orders its non-empty local Series by localized title using
the locale's ICU collation. Literal Series ID is the final deterministic
tie-breaker. Collation affects presentation only: it neither changes Series
identity nor rewrites the route component.

The configured Series index is generated in every configured locale, including
when it is empty. It lists only the Series with local members. This mirrors the
existing generated taxonomy-index behavior and makes the index's checked URL
available in every locale. A definition with no members creates no individual
listing page.

For an English `heinrich-heine` Series, Heine claims these literal output
files:

```text
series/index.html
series/heinrich-heine/index.html
series/heinrich-heine/page/2.html
```

For a non-default locale, the normal locale prefix precedes the same paths.
The listing's first page uses `index.html`; later pages use the existing
pagination segment and page number. Heine emits literal output-file URLs, not
directory URLs. Standard output ownership, portable-path checks, collision
detection, atomic full publication, quick-build constraints, and sitemap
inventory rules apply unchanged.

Series output is independent of collections, taxonomies, feeds, and search
indexes. It does not add members to a collection, make feed entries, or add
terms to a taxonomy. Generated Series index and listing pages participate in a
sitemap unless `[series] sitemap = false` excludes the complete generated
family. `sitemap.include_pagers = false` instead retains the index and first
listing page while excluding only derived pager URLs. Page-level `sitemap =
false` does not suppress a generated Series output: it has no one authored
page owner, just as a generated taxonomy output has none.

Series relationships are full-build concerns. A quick build may re-render
already resolved Series outputs when a present member page changes, but it
retains the normal quick-build limits for configuration, membership, position,
template, rename, deletion, and other relationship changes. A full build, or
the development server's normal full rebuild, is required to resolve those
changes.

## Template contract

Every authored rendered page receives `page.series`, an array in its authored
membership order. Each membership exposes:

```text
id
title
description
position
number
total
url
relative_url
previous
next
```

`url` and `relative_url` point to that Series listing. `previous` and `next`
are either `null` or lightweight checked page-link views with `id`, `title`,
`url`, and `relative_url`. They identify neighbors only within this particular
membership's Series. They do not represent directory siblings, collection
neighbors, or a global predecessor and successor.

There are three Series views. `page.series` describes
the current authored page's memberships and their neighbors. `series()`
describes the generated locale-local index of all non-empty Series.
`series(id="…")` describes one complete locale-local Series. While rendering a
generated `series.tera` listing, its `series` value describes that Series and
`pagination.pages` supplies the current rendered slice of member page views.
Keeping these scopes separate avoids copying a complete member list into every
ordinary page render or treating a generated listing as an authored page.

A normal page template can render every navigation path directly:

```tera
{% for membership in page.series %}
  <nav aria-label="{{ membership.title }}">
    <p>{{ membership.title }}, {{ membership.number }} of {{ membership.total }}</p>
    {% if membership.previous %}
      <a href="{{ membership.previous.url }}">‹ {{ membership.previous.title }}</a>
    {% endif %}
    {% if membership.next %}
      <a href="{{ membership.next.url }}">{{ membership.next.title }} ›</a>
    {% endif %}
  </nav>
{% endfor %}
```

Human-facing labels such as "of", "previous", and "next" remain site-owned
Fluent messages. The example's strings are structural illustration only.

Heine also provides a checked Tera function:

```tera
{% set index = series() %}
{% set heine = series(id="heinrich-heine") %}
```

`series()` returns the current locale's generated Series index view. It has
its localized title and description, base-path-aware URLs, and the ordered
non-empty local Series entries. `series(id="…")` returns one locale-local
Series listing view with its title, optional description, count, URLs, and
ordered member page IDs. It accepts an exact configured `locale=` in the same
way as other checked locale-aware template lookups. This follows the existing
`collection()` contract: a template that needs a full page view for an ID uses
the checked `page(id=...)` lookup.

For optional cross-page links, Heine provides a checked predicate:

```tera
{% if series_has_members(id="heinrich-heine") %}
  {% set heine = series(id="heinrich-heine") %}
  <a href="{{ heine.url }}">{{ heine.title }}</a>
{% endif %}
```

`series_has_members(id="…")` returns whether the configured Series has one
or more members in the current locale. It accepts the same exact `locale=`
argument as `series()`. It returns `false` for a configured Series with no
members in that locale, while an unknown Series ID, invalid locale, or a site
without Series configured remains a checked template error at the predicate
call site. `locale=` selects only that configured locale: it never infers a
translation counterpart or falls back to another locale for membership.

An unknown Series ID, an unknown locale, or a Series with no members in the
requested locale is a checked template error at the call site. Calling
`series()` when Series are not configured is also a checked template error at
the call site. The index is available even when empty because it always has a
generated output claim.

Generated templates receive views for generated output rather than an authored
`page`:

- `series-index.tera` receives the index view and no page identity;
- `series.tera` receives the current Series listing view plus the usual
  generated-pagination view. Render its current member slice through the
  existing `pagination.pages` page-link views, exactly as a taxonomy listing
  does.

Templates cannot generate, reorder, or add Series members.

## Diagnostics

Configuration errors retain the `heine.toml` source and span. Membership
errors retain the declaring `.page` source and span. A duplicate position
diagnostic identifies the later membership and includes the earlier member's
source location as related context. A duplicate membership likewise shows both
declarations. Errors involving a missing title, non-rendered page, unknown
definition, configured template, or Fluent key identify every relevant
authored location available to the resolver.

All Series route claims use the existing output domain. A collision therefore
reports both the Series configuration or definition responsible for the
generated claim and the existing claimant. Template errors identify the exact
`series()` call. Definition IDs whose generated routes collide on portable
filesystems report both definition locations, even when neither definition has
members yet.

## Rejected alternatives

### Model every Series as a collection

Collections can approximate a chronological Series, but their order comes
from shared page facts such as publication time or global page weight. They
cannot express a page at two independently authored positions in two
collections, and they do not generate a catalog or listing output by
themselves.

### Model every Series as a taxonomy

Taxonomies classify pages by terms and partition an already ordered collection.
Adding per-term membership positions would complicate ordinary tags and
categories for a distinct reading-sequence use case. An ordered-taxonomy mode
would effectively be Series under misleading terminology.

### Permit only one Series per page

This would make one unqualified previous/next pair convenient, but it would
artificially forbid a page that legitimately belongs in several reading paths.
Several labeled navigation blocks communicate the actual relationships more
faithfully than an invented primary-Series rule.

### Infer a current Series from navigation history

A static page URL does not retain the path by which a reader reached it.
Referrers are unreliable; query-string state makes navigation dynamic; and
duplicating one page under every Series route creates redundant output and
canonical-URL problems. Heine instead exposes all memberships without hidden
state.

### Store a central ordered member list in configuration

A central list can represent several orders, but it separates a page from its
own editorial membership, becomes a merge-conflict hotspot, and turns
generator configuration into a second content-maintenance location. Page-local
memberships keep the relationship near the page while positions retain the
necessary ordering fact.

## Starter-site example

The starter site uses the existing English Heinrich Heine documentation
pages, `life`, `writing`, and `afterlives`, as members of a
`heinrich-heine` Series. Its section index remains an ordinary authored
overview. The shared page template iterates over `page.series`, even though
each initial example page has one membership. This demonstrates the general
multi-Series template contract without adding artificial content.

The example supplies all labels through English Fluent resources and presents
one responsive, labeled navigation block per membership. It does not
add a German placeholder sequence solely to demonstrate the feature.

## Boundaries

The first version does not provide:

- a primary-Series field or unqualified global previous/next navigation;
- inherited Series memberships or inferred positions;
- a central list of member page IDs;
- cross-locale requirements that translated Series have matching members or
  positions;
- per-Series pagination overrides;
- nested Series, Series-as-taxonomy aliases, or collection-specific Series;
- automatic feed or search membership; or
- dynamic URL, referrer, cookie, or query-string state to select a Series.

A later feature may add a per-definition pagination override if a concrete
site needs different listing sizes. It must not change membership semantics or
introduce an implicit primary relationship.

## Acceptance criteria

Implementation must cover at least:

- empty and non-empty configured Series across default and non-default locales;
- page memberships in zero, one, and several Series;
- authored membership order in `page.series` views;
- independently ordered positions for one page in several Series;
- valid partial locale membership without a non-fatal diagnostic;
- generous non-consecutive positions and the same checked duplicate-position
  behavior after an author renumbers a Series;
- unknown IDs, duplicate page memberships, duplicate positions, zero and
  negative positions, non-rendered members, and members without titles, with
  source spans and related locations;
- checked definition paths, template names, index keys and definition keys for
  emitted listings, `per_page`, and all generated output collisions;
- generated locale-local index, listing, and pager paths, including their
  fixed `page` segment, fixed navigation window, literal output-file URLs, and
  sitemap inventory;
- locale-aware Series-index title ordering with literal-ID tie-breaking;
- `series()` lookups for current and exact locales, empty indexes, unavailable
  local Series, a disabled Series feature, and precise template-call spans;
- `series_has_members()` for populated and empty configured local Series,
  exact locales, invalid IDs, and disabled Series;
- membership-local previous and next views at both sequence ends and in the
  middle, without confusing them with sibling or collection neighbors;
- interaction with base paths, localization, page translations, taxonomies,
  collections, pagination, feeds, search indexes, sitemap ownership, quick
  builds, and output publication; and
- complete updates to the tutorial, glossary, reference, a focused Series
  guide, the starter site, and fixture-site coverage.
