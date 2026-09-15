# Absolute URLs

Most links in a site should use Heine's ordinary `url` or `relative_url`
fields. They work in local builds and beneath any configured `base_path`.
Use an absolute URL only when a document needs a canonical public address, such
as a canonical-link element or Open Graph metadata.

Configure the public origin once:

```toml
# heine.toml
[site]
origin = "https://example.org"
base_path = "/guide/"
```

The origin supplies the scheme and authority. `base_path` supplies the
deployment directory. Heine combines both with a known output path, so a page
at `blog/first.html` has this canonical address:

```tera
{{ page.absolute_url }} {# https://example.org/guide/blog/first.html #}
```

## Use checked resource URLs

The `site` value provides the canonical site root, including the trailing
slash. A page, copied asset, or generated-resource view provides
`absolute_url` when it has a known output path:

```tera
<link rel="canonical" href="{{ page.absolute_url }}">

{% set logo = asset(id="assets/logo.svg") %}
<meta property="og:image" content="{{ logo.absolute_url }}">
```

The same field accompanies `page()` results, page translations, pager links,
taxonomy and Series views, configured feeds, search indexes, attribution
assets, and license texts. See the [reference](reference.md#template-contract)
for the complete render-facing contract.

An authored page is not always the current output. A paginated listing, for
example, has an authored index page and several generated output pages. Use
the generated view that owns the current output when emitting its canonical
link:

```tera
{% if pagination %}
  <link rel="canonical" href="{{ pagination.current.absolute_url }}">
{% else %}
  <link rel="canonical" href="{{ page.absolute_url }}">
{% endif %}
```

Generated taxonomy and Series templates follow the same rule through
`taxonomy.absolute_url` and `series.absolute_url`.

## Keep the origin out of templates

Templates cannot read raw `site.origin`. It is not a site-root URL when a site
is published beneath a subdirectory, and concatenating strings would bypass
Heine's checked output relationships. There is no general `absolute_url()`
function for the same reason.

`site.origin` remains optional for sites that do not need canonical public
URLs. Directly rendering `site.url` or `absolute_url` requires it. If it is
absent, Heine stops with a source-located template error instead of emitting an
empty value, a development-server address, or an unchecked URL.

Do not use Tera conditionals or `default` to turn canonical markup into an
optional feature. Those constructs can intentionally suppress an absent value,
as can literal template text, but neither establishes a canonical URL.

## Local previews

Keep `site.origin` set to the intended public origin while developing locally.
`heine serve` does not infer or replace it with its local address. Use
`absolute_url` for canonical and metadata markup, not ordinary navigation: an
anchor using it deliberately points to the public host. Use `url` or
`relative_url` for links that a reader follows within the locally served site.

Atom feeds and sitemaps use the same configured origin and output-path
construction. See [sitemaps](sitemap.md) and [Atom feeds](atom-feeds.md) for
their additional configuration.
