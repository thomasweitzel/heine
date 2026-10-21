# Sitemaps

Use a sitemap when search engines should discover a site's public pages. Heine
generates an XML list from the pages and generated listings it will publish;
the sitemap itself is separate from a site's reader-facing navigation.

## Smallest configuration

Sitemap generation is opt-in. A site enables it with an empty `[sitemap]`
table and supplies its deployed origin separately from its deployment path:

```toml
[site]
origin = "https://example.org"
base_path = "/guide/"
time_zone = "Europe/Berlin"

[sitemap]
```

`site.origin` supplies the scheme and authority: `https://example.org` in this
example. It is an absolute HTTP or HTTPS origin with no credentials, query,
fragment, or path other than `/`. `site.base_path` supplies the path below that
origin where the site is deployed. Together they produce canonical URLs such
as `https://example.org/guide/blog/first-post.html`.
Templates use the same construction through [checked absolute URL views](absolute-urls.md).

Sitemap locations use Heine's literal generated-file URLs. For example,
`public/blog/index.html` becomes
`https://example.org/guide/blog/index.html`; Heine does not remove
`index.html` or assume that a production host resolves directory requests.
`heine serve` may resolve `/blog/` locally as a browsing convenience, but that
does not alter generated links or sitemap locations.

Keeping these settings separate makes a deployment below a subdirectory
unambiguous. A site at the root uses the default `base_path = "/"`.
`site.time_zone` is an IANA time-zone identifier used to resolve authored
publication timestamps. It is required when any page declares one, but not for
sitemap generation itself. Without it, pages may not declare `published` or
`updated`, and their sitemap entries simply omit `<lastmod>`. Heine has no
implicit UTC or machine-local default: `Z` appears only when Heine serializes
an already resolved authored instant as UTC.

The timezone is site-wide. Heine does not infer one from a locale, languages are
not geographic regions, and pages do not override it in this version. A site
therefore chooses one editorial timezone for its publication chronology.

`[sitemap]` without `site.origin` is an error. A sitemap must contain absolute
URLs, and guessing an eventual deployment host would make generated output
unreliable.

The names have separate scopes: the site-level `[sitemap]` table enables
generation and may gain sitemap-wide settings later. The `sitemap` field in a
page or `[pages]` descriptor only removes authored output from that generated
document; it never enables the feature or adds a page to it.

## Localized counterparts

A multilingual site can add [hreflang sitemap alternates](glossary.md#hreflang-sitemap-alternate)
for pages that it has explicitly declared as counterparts:

```toml
[sitemap]
hreflang = true
```

This setting is optional. Omit it to produce the ordinary sitemap format;
`hreflang = false` is rejected because it restates that omission. It does not
invent language relationships. A page participates only when it shares a
[`translationid`](internationalization.md#page-counterparts) with a rendered,
sitemap-included page in another locale.

For every participating page, Heine writes one `<xhtml:link
rel="alternate">` for every sitemap-included member of that group, including
the page itself. The entries have fully qualified canonical URLs and are sorted
by configured locale code. The XHTML namespace is added to the sitemap URL-set
automatically.

The group may be partial. If an English and German page share an ID but no
Polish page exists, their entries name only English and German. If a member
uses `sitemap = false`, it is not named by another locale's entry. The
remaining included members still form a reciprocal set. A group with fewer
than two included pages emits no self-only annotation.

Only authored pages participate. The first URL of an authored paginated
listing can be a counterpart, but later pager URLs, taxonomies, Series, feeds,
search indexes, and other generated resources do not have an authored
translation identity. Heine does not emit `x-default`, HTML-head annotations,
or HTTP `Link` headers. Those need an additional, separately authored policy.

## Generated file

A full build writes `sitemap.xml` at the root of managed output:

```text
public/sitemap.xml
```

The file is a Heine-generated resource. It participates in output collision
checking and is published as part of the same complete, atomic output tree as
the rest of the site. A conflicting copied asset, deployment file, license
text, page, or other generated resource is a build error.

The sitemap is served unchanged by `heine serve`. In particular, development
does not rewrite it to `localhost` or to the request host. The generated file
therefore remains deployable and always describes the configured canonical
origin.

`base_path` changes URLs, not the output-directory layout. A site deployed at
`/guide/` normally configures an output directory that the host mounts there:

```toml
[site]
base_path = "/guide/"

[build]
output_dir = "public/guide"
```

That build writes `public/guide/sitemap.xml`, which the host serves as
`/guide/sitemap.xml`. As with every generated file, `build.output_dir` is the
only setting that determines its filesystem location.

Quick builds do not update the sitemap. They do not resolve the
complete set of output paths, so regenerating it could make the sitemap
disagree with stale generated output. Run a full build after changes that affect
site structure.

## Sitemap size

The Sitemap Protocol limits one sitemap to 50,000 URLs and 50 MB of
uncompressed XML. Heine must never publish a sitemap beyond either limit.

When the resolved inventory fits within Heine's conservative limit of 45,000
entries and 45,000,000 serialized UTF-8 bytes, `sitemap.xml` is a normal
URL-set document. When it does not, Heine generates `sitemap.xml` as a
sitemap-index document and writes deterministically numbered child URL-set
documents below `sitemaps/`. The index refers to those children with canonical
URLs. Child boundaries follow deterministic URL order and account for both
entry count and final UTF-8 XML size.

Localized alternate elements count toward XML size, but not toward the URL
entry count. If one page and its alternate set cannot fit within the byte
limit, Heine reports that canonical page URL instead of writing an oversized
sitemap.

Every child is a generated-output claim and participates in the same collision
checks and atomic publication as `sitemap.xml`. If an individual canonical URL
would reach the Sitemap Protocol's 2,048-character URL limit, Heine reports
the responsible rendered route instead of emitting an invalid entry.
The same limits apply to the sitemap index; an inventory too large for one
index is a build error rather than invalid XML. Recursive index trees are not
part of the initial feature.

## Publication chronology

Sitemap timestamps and Atom feeds need a statement about publication,
not a generic calendar date. Heine therefore replaces the existing `date` page
field with these optional fields:

```toml
# content/en/blog/first.page
published = 2026-08-14T09:30:00
updated = 2026-08-27T14:35:00 # optional
```

`published` is when the page first became publicly available. `updated` is the
most recent change that matters to readers and search
engines. It is not changed for a rebuild, a filesystem timestamp, or an
insignificant site-wide edit. `updated` must not precede `published`.

Both values are local TOML date-times without an offset. Heine resolves them
with the configured `site.time_zone`, then retains the resulting instant. The
generated sitemap uses `updated` as `<lastmod>` when it exists and otherwise
uses `published`; it emits canonical RFC 3339 UTC timestamps. A page with
neither field remains in the sitemap without `<lastmod>`.

Templates receive the same resolved values as `page.published` and
`page.updated`: UTC RFC 3339 text such as `2026-08-14T07:30:00Z`. This makes
them directly usable with `format_date()`, which converts them into the
editorial time zone for reader-facing calendar dates, and avoids exposing an
ambiguous local time at the template boundary.

Heine never uses the build machine's time zone. A named IANA zone makes the
intended interpretation of source dates unambiguous and accounts for
daylight-saving changes. Heine resolves it using a bundled, versioned IANA
timezone database, not the host's zoneinfo files, so the same Heine version
produces the same instant on every supported platform.

A local date-time that never occurred during a clock change is an error. A
local date-time that occurred twice is also an error. In both cases, the
diagnostic identifies the authored timestamp and configured zone and explains
the transition. The repeated-hour restriction exists because without an offset
or another disambiguator, the source cannot name which of the two real
instants it means. Heine does not accept offset-bearing timestamps as an
escape hatch in this version.

`published` and `updated` are page chronology, not general-purpose historical
dates. A page needing another date-only fact may place it in `extra`. The
renamed collection orders are `published-asc` and `published-desc`.

## When chronology is required

Chronology is optional for ordinary rendered pages. A contact page, legal
notice, or long-lived landing page may have no trustworthy publication instant;
requiring one would encourage invented metadata. A sitemap may therefore omit
`<lastmod>` for such a page.

Features that depend on publication time require it at their own boundary.
A publication-ordered collection requires `published` for every member, just
as its current ordering rules require their ordering metadata. A directory can
apply the same policy to its descendant pages:

```toml
# content/en/blog/_directory.toml
[pages.metadata]
required = ["published"]
```

An Atom feed likewise requires a real publication timestamp for every included
entry. `updated` remains optional everywhere: a page that has not changed
significantly since publication uses `published` as its initial update instant.
Requiring both fields would merely duplicate that same fact.

## Included URLs

Heine derives sitemap entries from its resolved rendered-output inventory, not
by scanning files below `public/`. This includes every rendered authored page
and every generated pagination, taxonomy-index, taxonomy-term, Series-index,
and Series-listing page across all configured locales.

It excludes copied assets, declared content files, deployment files, published
license texts, non-rendered pages, `sitemap.xml` itself, and the reserved
`__heine/` development-server namespace. Each route appears once, in
deterministic URL-path order.

Generated pagination, taxonomy, and Series pages have no authored chronology
of their own. Their initial sitemap entries therefore omit `<lastmod>`.

For a multilingual site, the default locale's routes remain unprefixed and
other locales retain their normal locale prefixes. The sitemap lists all of
those canonical URLs. When `[sitemap] hreflang = true` is configured, explicit
page counterpart groups add their localized alternate annotations as described
above.

## Sitemap membership

Rendered authored pages are included by default. A page may stay public while
opting out of discovery through the sitemap:

```toml
# content/en/private-preview.page
template = "page.tera"
title = "Private preview"
sitemap = false
```

This affects only sitemap membership. It does not suppress rendering, linking,
collections, taxonomies, or other page behavior. Use `template = false` when a
page should not produce output at all. It also does not add an HTML `noindex`
directive, restrict access, or change `robots.txt`; those are separate web
publishing concerns.

When the opted-out page is a paginated listing, the setting excludes both its
authored first page and every pager generated from it. They are one listing
family with one authored owner.

Page sitemap settings require sitemap generation to be enabled. If a page has
`sitemap = false` but `heine.toml` has no `[sitemap]` table, Heine reports
an error at the page setting. Remove the now-inapplicable field or enable the
feature. This prevents inactive settings from silently changing behavior if a
sitemap is enabled later.

The page field accepts only `false`; omit it to use ordinary membership.
`sitemap = true` is rejected rather than accepted as an inert restatement of
the default. Sitemap settings on a non-rendered page are rejected for the same
reason.

### Derived pagers

By default, a sitemap includes every pager because each has its own canonical
URL. When every primary page is already listed directly, a site can keep pager
navigation public while omitting its derived pages from the sitemap:

```toml
[sitemap]
include_pagers = false
```

This retains every first listing page, including authored listings, taxonomy
indexes and term listings, and Series indexes and listings. It excludes only
later `page/2.html`, `page/3.html`, and similar outputs. The setting applies
to all pager kinds so the sitemap has one consistent pager policy. It neither
changes pager routes or links nor asks search engines not to index them.

`include_pagers` defaults to `true`. The field accepts only `false`; omit it
when derived pagers should remain sitemap entries.

### Directory boundaries

A directory descriptor can exclude an entire subtree:

```toml
# content/en/drafts/_directory.toml
[pages]
sitemap = false
```

This boundary applies recursively to every descendant rendered page and to
every pager derived from those pages. It is not a default: a nested descriptor
or page cannot re-include a descendant. A page setting of `sitemap = true` is
always invalid; below this boundary its diagnostic also points to the
descriptor that established the exclusion. This makes a directory exclusion a
policy for drafts and previews rather than a preference a nested page
can silently undo.

As with page opt-outs, `[pages] sitemap = false` requires global sitemap
generation. A redundant page opt-out below a directory boundary is allowed: it
has the same effect and can state an individual page's intent without
weakening the boundary.

Generated pagers cannot have an independent opt-out. Taxonomy pages have no
authored owner, so their taxonomy definition owns one whole listing-family
boundary instead:

```toml
[taxonomies.tags]
collection = "posts"
path = "tags"
index_template = "taxonomy-index.tera"
term_template = "taxonomy-term.tera"
per_page = 20
sitemap = false
```

This excludes the taxonomy index, every term listing, and every derived
pager. The setting does not change the generated routes or any other taxonomy
behavior. Likewise, `sitemap = false` in `[series]` excludes the Series index,
every Series listing, and their pagers. Both settings require `[sitemap]` and
accept only `false`; omit them to include the complete generated family.

Generated pages have no independent sitemap setting. A whole-family boundary
keeps their sitemap membership aligned with the configuration that owns their
routes.

## First-version boundary

The initial sitemap contains required `<loc>` entries and `<lastmod>` only when
page chronology supplies it. It does not provide:

- arbitrary externally supplied URLs;
- `changefreq` or `priority` fields;
- individual generated-listing-page opt-outs;
- XML template overrides or template-side sitemap generation; or
- a development-only sitemap with local-host URLs.

These omissions keep the sitemap an exact, checked view of output that Heine
knows it rendered. Later extensions must retain that property and add authored
metadata only when its semantics are defined. Atom feeds use the same page
chronology while separately defining feed-level metadata and timestamps; see
the [Atom-feed guide](atom-feeds.md).
