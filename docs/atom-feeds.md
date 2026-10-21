# Atom feeds

Use an Atom feed when feed readers should discover recent pages from one
[collection](collections.md). Heine takes the collection's existing members,
orders them by publication time, and generates the feed as a checked output
file.

Feeds are opt-in. They are locale-local, checked generated resources: one
configured feed produces one document for every configured locale. An English
and German site therefore gets separate English and German feeds, never an
implicit mixed-language feed.

## Smallest configuration

Feeds need canonical absolute URLs, so configure an origin. Publication times
also need an editorial timezone:

```toml
# heine.toml
[site]
origin = "https://example.org"
time_zone = "Europe/Berlin"

[collections.posts]
order = "published-desc"

[feeds.posts]
collection = "posts"
path = "atom.xml"
title_key = "feed-posts-title"
empty_updated = 2026-08-01T09:00:00
empty_authors = ["Example Author"]
```

Templates use the same origin and base-path construction through [checked
absolute URL views](absolute-urls.md).

The title is locale-facing text, so `title_key` names a Fluent message rather
than containing literal Atom text:

```ftl
# locales/en/site.ftl
feed-posts-title = Example posts
```

Each feed entry comes from a page in the configured collection. A feed member
must have a title, publication time, summary, and at least one author:

```toml
# content/en/blog/first.page
template = "post.tera"
title = "First post"
published = 2026-08-14T09:30:00
summary = "An introduction to the site."
authors = ["Example Author"]
collections = ["posts"]
```

With a default English locale and German `de` locale, this configuration writes
these files:

```text
public/atom.xml
public/de/atom.xml
```

`site.base_path` changes each feed URL, not this output layout. As with pages,
`build.output_dir` alone chooses where managed output is written.

## Configuration

Each `[feeds.<name>]` table defines one feed family. `<name>` is a configured
feed name used by templates; it is not a URL segment.

The table is shared policy for every locale-local document in that family.
For example, `limit` is the same for English and German feeds, while their
members and Fluent-resolved title text may differ. Heine does not add
per-locale feed overrides without a concrete use case and a complete contract.

| Field           |      Required | Meaning                                                                 |
|-----------------|--------------:|-------------------------------------------------------------------------|
| `collection`    |           yes | Existing collection whose members supply entries.                       |
| `path`          |           yes | Portable relative output path below each locale's output root.          |
| `title_key`     |           yes | Fluent message identifier for the feed title.                           |
| `subtitle_key`  |            no | Fluent message identifier for an optional feed subtitle.                |
| `limit`         |            no | Maximum entries after feed ordering; defaults to `20`.                  |
| `empty_updated` | conditionally | Local date-time used only when a locale feed has no entries.            |
| `empty_authors` | conditionally | Non-empty author-name list used only when a locale feed has no entries. |

All feed configuration is strict. Unknown fields, unknown collections,
duplicate feed names, invalid output paths, and a non-positive `limit` are
errors at their authored settings. Feed paths participate in the ordinary
output-collision check.

`site.origin` is required whenever any feed is configured. It supplies the
scheme and authority for Atom identifiers and links; `site.base_path` supplies
the deployment path. Heine does not guess the eventual host.

## Locale behavior

Heine generates every configured feed once per locale, whether that locale's
source collection is empty or not. The default locale remains unprefixed;
non-default locale feed paths receive their normal locale prefix.

`title_key` and `subtitle_key` resolve through the same Fluent fallback chain
as `t()`. A message absent from both the requested locale and the default
locale is a build error that identifies the feed setting and resolution chain.

Every feed declares its locale with `xml:lang`. A template can refer to a feed
without reconstructing paths:

```tera
{% set posts_feed = feed(name="posts") %}
<link rel="alternate"
      type="application/atom+xml"
      href="{{ posts_feed.url }}"
      title="{{ posts_feed.title }}">
```

`feed()` uses the current render locale by default. Its optional `locale`
argument selects another configured locale. Unknown feed names, invalid
arguments, and unavailable locales are errors at the template call site.
Heine does not inject feed-discovery links automatically; templates decide
where those links belong.

## Entries and chronology

A collection defines feed membership only. Heine then orders feed members by
resolved `published` instant, newest first, using logical page ID as a stable
tie-breaker. It applies `limit` after ordering. This keeps a feed chronological
even when the source collection is ordered for a different presentation.

Every selected member must provide `title`, `published`, `summary`, and a
non-empty `authors` list. `updated` is optional and falls back to `published`.
The resulting Atom entry contains:

- a canonical absolute ID and alternate link for the rendered page;
- plain-text title and summary;
- `published` and effective `updated` timestamps;
- one Atom author element for every page author.

Heine emits summaries as text, not rendered Markdown or HTML. Full entry
content, enclosures, categories, and Atom extension elements are outside this
first feature.

`authors` is an ordinary page field containing names:

```toml
authors = ["Example Author", "Guest Author"]
```

Email addresses, URIs, author profiles, and an author registry are not part of
this feature. A future extension can add those only when a concrete use case
justifies the additional model.

A directory descriptor may provide a convenient default:

```toml
# content/en/blog/_directory.toml
[pages]
authors = ["Example Author"]
```

A page's own `authors` list replaces the nearest inherited list. This is an
authoring policy, not a feed guarantee: collections may span unrelated
directories, so feed generation always checks each selected member itself.

## Empty feeds

Atom requires a feed-level `updated` value. It also requires a feed-level
author unless every entry has an author. An empty feed has no entry from which
either fact can be derived.

For a non-empty locale feed, Heine derives the feed update time from the
included entries and emits their page authors. It does not emit a feed-level
author. For an empty locale feed, it uses `empty_updated` and emits
`empty_authors` as feed-level Atom authors.

This is why the settings are named `empty_updated` and `empty_authors`: they
are not fallbacks for ordinary pages and never replace entry authors. If a
locale feed is empty and either setting is absent, the build fails with a
diagnostic naming the feed and locale.

`empty_updated` follows the same local date-time and `site.time_zone`
resolution rules as page `published` and `updated` values. It must name one
unambiguous real instant.

## Atom identity and output

The feed's Atom ID and `rel="self"` link are its canonical absolute feed URL.
Each entry's Atom ID and `rel="alternate"` link are its canonical absolute page
URL. These URLs name literal generated files: if a page writes
`blog/first.html`, its entry uses `/blog/first.html`; Heine does not assume a
production server's directory-index behavior.

The alternate link does not declare an HTML media type. A rendered page may use
a different suffix, so Heine exposes its checked canonical URL without claiming
a representation type it cannot establish.

Feed output escapes XML markup and rejects XML-forbidden control characters
before publication. It is included in the same full-build staging and
atomic publication step as other managed output. Quick builds do not regenerate
feeds, because they do not reconstruct complete collection and
output relationships.

Atom feeds are not rendered pages and are not added to the sitemap. The
development server serves the generated file from managed output; it does not
rewrite its origin or content for localhost.

Heine writes the document but does not control production HTTP headers. Configure
the production host to serve the chosen feed path as `application/atom+xml`
when that host does not already associate the extension with Atom.

## Limits

This feature generates Atom 1.0 only. It does not generate RSS or JSON Feed,
create feeds directly from taxonomies or directories, render entry bodies,
offer feed templates, or support arbitrary XML extensions. Those capabilities
need their own use cases and contracts rather than becoming incidental options
on a collection feed.
