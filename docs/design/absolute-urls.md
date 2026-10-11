# Checked absolute URLs design

## Status

This is the accepted design for checked absolute URLs. It extends Heine's
existing canonical URL generation for Atom feeds and sitemaps. The
[reference](../reference.md) defines the implemented user-facing contract.

## Decision

Heine exposes canonical absolute URLs only for its site root and for resources
with a checked output identity. A site opts in by configuring an HTTP(S)
origin:

```toml
[site]
origin = "https://example.org"
base_path = "/guide/"
```

The configured origin contains scheme, host, and an optional port only. It
never contains a deployment path, credentials, a query, or a fragment.
`base_path` remains the sole deployment-path setting. Together they produce
the canonical site-root URL:

```tera
{{ site.url }} {# https://example.org/guide/ #}
```

Every render-facing view that already identifies a known output resource also
has `absolute_url` when an origin is configured:

```tera
<link rel="canonical" href="{{ page.absolute_url }}">
<meta property="og:url" content="{{ page.absolute_url }}">
```

An absolute URL always comes from the validated origin, canonical base path,
and the resource's literal output path. It does not use the development-server
address, a request host, a template string, or a guessed translation path.

`site.origin` is configuration-only. Templates receive `site.url`, not the raw
origin, because a raw origin omits the deployment path and invites incorrect
URL construction below a subpath.

## Missing origin

`site.origin` is optional because an ordinary static site need not publish
canonical URLs, a sitemap, or an Atom feed. It is not optional once a template
directly renders `site.url` or `absolute_url`: Heine reports a checked template
error that names the missing setting and the template location.

Neither value is nullable. Heine never substitutes an empty string, localhost,
the current request host, or a relative URL. A template that uses one of these
values therefore declares that its site has a canonical public origin.

This deliberately does not add a conditional `site.origin` template value.
Tera's own conditional and `default` forms can intentionally suppress an
absent field, just as a template can always author a literal URL. Heine does
not treat either as a canonical-URL mechanism. A reusable template that emits
canonical metadata must document its origin requirement, or leave that markup
to an extending template.

## Render-facing URL views

The existing fields keep their meanings:

- `url` is base-path-aware and begins with `site.base_path`;
- `relative_url` is relative to the output currently being rendered; and
- `absolute_url` is canonical and begins with the configured scheme and host.

`absolute_url` accompanies every current link view for a known output resource:

- the current authored `page`, and `page()` results;
- `asset()` results, attributions, and referenced license texts;
- page translations, ordinary-template `taxonomy()` and `taxonomies()` views,
  generated taxonomy indexes and terms, taxonomy counterparts, and page-local
  taxonomy memberships;
- collection and sibling neighbor links, paginated listings and navigation;
- Series indexes, listings, memberships, and their page links; and
- configured search-index and Atom-feed views.

Generated templates continue to receive `page = null`. Their own canonical
identity is available through the generated view that owns it, such as
`pagination.current.absolute_url`, `taxonomy.absolute_url`, or
`series.absolute_url`.

No generic `absolute_url()` function is added. Such a function would let a
template attach a canonical-looking URL to arbitrary text, bypassing Heine's
checked page, asset, and generated-output relationships.

## Shared construction

Atom feeds and sitemaps already require `site.origin`. This feature routes
their canonical URL construction through the same base-path-aware operation
used by template views. The operation accepts only a validated `SiteOrigin`,
a `BasePath`, and an `OutputPath`; callers cannot concatenate an arbitrary
path into a canonical URL.

The generated URL preserves Heine's literal output-file convention. For
example, `blog/index.html` under `/guide/` becomes
`https://example.org/guide/blog/index.html`. Local development may resolve a
directory request to that file, but does not alter the URL Heine emits.

## Consequences and boundaries

Existing `url` and `relative_url` values remain suitable for normal navigation
and local builds. A future site-level URL must be base-path-aware and tied to
an established site or output identity. This design does not expose raw
configuration values merely to make string concatenation possible.

## Acceptance criteria

- `site.url` is the origin joined to `site.base_path`, including its canonical
  trailing slash.
- Every supported resource view has an exact `absolute_url` when an origin is
  configured, including non-root `base_path` and non-default locale output.
- Direct rendering of `site.url` or `absolute_url` without an origin fails
  with a useful, source-located diagnostic naming `site.origin`.
- Templates never receive raw `site.origin`, a localhost replacement, an empty
  URL, or a general raw-string URL converter.
- Atom-feed and sitemap canonical URLs retain their existing bytes and use the
  shared construction path.
- Tests cover configured and absent origins, a subpath deployment, and every
  render-facing link family.
