# Multilingual sitemap alternates design

## Status

This is the accepted design for multilingual sitemap alternates. It extends
the existing sitemap and explicit page-translation contracts without adding
template-side markup.

## Decision

Sitemap alternates are an opt-in extension of sitemap generation:

```toml
[sitemap]
hreflang = true
```

`hreflang` accepts only `true`. Omitting it retains the existing sitemap
output, and `false` is rejected as a redundant restatement of that default.
As with every sitemap setting, `[sitemap]` requires `site.origin`.

When enabled, Heine adds `xmlns:xhtml="http://www.w3.org/1999/xhtml"` to each
generated sitemap URL-set. It adds `<xhtml:link rel="alternate" .../>`
children to an authored page's sitemap entry only when that page has at least
one sitemap-included counterpart in the same explicit `translationid` group.

For example, two counterparts produce identical alternate sets:

```xml
<url>
  <loc>https://example.org/guide/index.html</loc>
  <xhtml:link rel="alternate" hreflang="en"
              href="https://example.org/guide/index.html"/>
  <xhtml:link rel="alternate" hreflang="de"
              href="https://example.org/guide/de/index.html"/>
</url>
<url>
  <loc>https://example.org/guide/de/index.html</loc>
  <xhtml:link rel="alternate" hreflang="en"
              href="https://example.org/guide/index.html"/>
  <xhtml:link rel="alternate" hreflang="de"
              href="https://example.org/guide/de/index.html"/>
</url>
```

The `<loc>` remains the entry's own canonical URL. Every alternate set is
locale-sorted and includes that entry itself. Locale codes are the configured,
validated BCP-47 locale codes. URLs come from the same checked origin,
base-path, and literal output-path construction used by the existing sitemap.

## Relationship boundary

`translationid` is the sole source of alternate relationships. Heine never
infers a counterpart from matching page IDs, paths, titles, locale prefixes, or
directory structure.

The resolved sitemap relationship contains only rendered authored pages that
are included in the sitemap. A page excluded with `sitemap = false` is not
named as an alternate by another locale, because that would undermine its
explicit sitemap exclusion. The remaining sitemap members form a complete,
reciprocal subset. Partial groups are normal.

A `translationid` group with fewer than two sitemap-included pages has no
alternate relationship, so its entry has no `xhtml:link` children. A self-only
annotation would add bytes without identifying an alternate page.

An authored paginated listing may participate through its first, authored URL.
Its later pager URLs are generated views rather than translated authored pages,
so they never receive alternates. Taxonomy, Series, collection, search, feed,
and other generated resources likewise do not participate.

This feature does not add `x-default`. That value requires an authored policy
for a locale-neutral fallback target, which `translationid` does not express.
It also does not inject HTML-head annotations or HTTP `Link` headers. Sitemap
annotations are the one selected representation; templates remain responsible
for any later HTML-head contract.

## Resolution and output

After page relationships and pagination are resolved, sitemap generation builds
one locale-sorted alternate catalog from sitemap-included authored task URLs.
It retains only the first URL of an authored paginated listing. The sitemap
module receives resolved output identities, not raw paths or template values.

Each alternate URL is checked against the existing 2,048-character canonical
URL limit. XML text and attributes use the existing XML escaping operation.
Alternate elements count toward serialized sitemap bytes, but not toward the
Sitemap Protocol's URL-entry count. The XHTML namespace is emitted whenever
`hreflang = true`, which keeps the size calculation independent of the
contents of an individual split child sitemap. If one URL entry, including its
alternate set, cannot fit within the conservative byte limit, Heine reports
that canonical URL rather than emitting an oversized sitemap.

Quick builds do not update sitemaps and therefore do not update alternate
annotations. A full build is required after any change that can affect a page,
its translation membership, locale, output path, or sitemap inclusion.

## Diagnostics

The existing translation checks remain the only relationship validation:

- an empty `translationid` is invalid;
- a locale may declare a translation ID at most once; and
- `sitemap = false` retains its existing source-located checks.

No warning or error is emitted for a partial translation group, a group with
only one sitemap member, or an excluded counterpart. Those are ordinary,
authored publication choices. A malformed `sitemap.hreflang` value, including
redundant `false`, is a checked configuration error at its exact TOML span.

## Documentation and acceptance criteria

The sitemap guide introduces the opt-in before explaining its boundaries. The
internationalization guide links the page-counterpart model to sitemap output.
The reference records `sitemap.hreflang`, and the starter site enables it so
its English and German counterpart pages produce a real example.

- Omitted `hreflang` preserves existing sitemap bytes exactly.
- Enabled `hreflang` emits the XHTML namespace and canonical, locale-sorted,
  reciprocal sets containing self for each eligible multi-member group.
- A missing locale and a sitemap-excluded counterpart leave a valid partial
  set, without naming the excluded URL.
- A single surviving member has no alternate element.
- Generated resources and later pagination URLs never receive alternates, but
  the first URL of an authored translated listing can receive them.
- XML escaping, non-root `base_path`, URL-length checks, deterministic sitemap
  splitting, full-build behavior, and source-located configuration diagnostics
  remain covered by focused and fixture-site tests.
