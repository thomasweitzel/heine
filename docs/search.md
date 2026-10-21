# Search indexes

Use a search index when your site provides its own browser search interface.
Heine generates one locale-local JSON document from one or more
[collections](collections.md), with checked membership, stable page facts,
source-derived text, and URLs. Your site supplies the browser search engine and
user interface.

This keeps search independent of one JavaScript library. A site can use
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

The current JSON envelope schema is format `3`. Each document has stable
metadata, including `draft`. It is `false` in an ordinary index and `true`
only for a [draft](drafts.md) included through `heine serve --drafts`, so a
site-owned preview interface can label it. Its `sections` array contains
source-derived text from every declared Markdown input of its page. A section
has `text` and a `fragment`: the fragment is `null` unless that text begins at
a heading with an existing generated table-of-contents identifier. A browser
can therefore link a matching section directly, while a page without such a
heading remains linkable at its ordinary page URL.

Markdown text, headings, link labels, image alternatives, code, and recognized
math source contribute. Authored raw HTML contributes semantic text nodes and
`img` alternatives, but not markup, URLs, comments, attributes, or the
contents of `script`, `style`, and `template`. This means an Arabic quotation
inside trusted raw HTML remains searchable as authored Unicode text, even on
an English page. Heine does not inspect rendered HTML, templates, copied
assets, or an inferred `content.main` body. See the
[reference](reference.md#search-indexes) for the full JSON shape and extraction
rules.

## Browser behavior belongs to the site

The starter site includes a dependency-free example that fetches the index,
uses basic Unicode-aware local substring matching, safely creates DOM nodes,
and stores its query and result page in the URL. It is a usable starting point,
not a required ranking policy or a bundled Heine component.

Search indexes are rebuilt by full builds and `heine serve`. `--quick` leaves
the previously published index unchanged, because it cannot safely establish
complete collection membership after every possible source change.
