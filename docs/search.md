# Search indexes

Heine can generate a locale-local JSON document index from one or more
[collections](collections.md). It supplies checked membership, stable page
facts, source-derived text, URLs, and output ownership. Your site supplies the
browser search engine and user interface.

This keeps search independent of one JavaScript library. A small site can use
a local script, while a larger one can replace that script with a suitable
browser library without changing the generated document contract.

## Create an index

Start with a configured collection, then add a named search index:

```toml
[collections.posts]
order = "published-desc"

[search.site]
collections = ["posts"]
path = "search/site.json"
```

`collections` must name one or more distinct configured collections. `path`
must use the literal lowercase `.json` suffix. Heine unions members for each
locale, removes duplicates, and orders the documents by page ID. The default
locale receives `search/site.<hash>.json`; each other locale receives the same
path below its locale prefix.

The filename includes the complete SHA-256 digest of the serialized JSON by
default. A new index therefore has a new immutable URL when its contents
change. Set `versioned = false` only when a stable endpoint is necessary and
your deployment rechecks its cache.

## Use the checked template view

Expose an index URL through `search()`, rather than constructing a path in a
template:

```tera
{% set index = search(name="site") %}
<form data-search-url="{{ index.url }}">
  <input type="search" name="q">
</form>
```

The view contains `name`, `count`, base-path-aware `url`, and
current-output-relative `relative_url`. `search(name="site", locale="de")`
selects one exact configured locale. Unknown names and invalid locale values
fail at the template call site.

## Document contents

Each JSON document has stable metadata plus `text`, plain text obtained from
every declared Markdown input of its page. Heine does not inspect rendered
HTML, templates, copied assets, or an inferred `content.main` body.

Markdown text, headings, link labels, image alternatives, code, and recognized
math source contribute. Authored raw HTML contributes semantic text nodes and
`img` alternatives, but not markup, URLs, comments, attributes, or the
contents of `script`, `style`, and `template`. This means an Arabic quotation
inside trusted raw HTML remains searchable as authored Unicode text, even on
an English page. See the [reference](reference.md#search-indexes) for the full
JSON shape and extraction rules.

## Browser behavior belongs to the site

The starter site includes a dependency-free example that fetches the index,
uses basic Unicode-aware local substring matching, safely creates DOM nodes,
and stores its query and result page in the URL. It is a usable starting point,
not a required ranking policy or a bundled Heine component.

Search indexes are rebuilt by full builds and `heine serve`. `--quick` leaves
the previously published index unchanged, because it cannot safely establish
complete collection membership after every possible source change.
