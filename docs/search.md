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

Each structural Markdown block and image alternative remains separate in the
extracted text, including a nested list below a labeled list item. Inline
Markdown and inline HTML keep the adjacency that the author wrote.

## Browser behavior belongs to the site

The starter site includes a dependency-free browser example. It searches the
generated index locally, creates its result DOM safely, and keeps the query
and result page in the URL. It is a usable starting point, not a required
ranking policy or a bundled Heine component.

Its search form may set `data-context-before` and `data-context-after` to the
preferred Unicode code-point context before and after a complete match. The
client uses 80 and 160 when those attributes are absent. The maintained example
sites use those same values.

`data-context-before-max` and `data-context-after-max` bound an extension to a
nearby word boundary. They default to 100 and 200, or to the corresponding
preferred value when it is larger. When a preferred edge falls inside a word in
a script with ordinary word boundaries, the client includes the whole word if
its boundary lies within that maximum. Otherwise it cuts at the maximum. Scripts
without ordinary word boundaries use the preferred edge directly. These limits
bound context rather than matching text itself, so an unusually long query can
make an excerpt exceed its context values.

The example matches each query term at the start of a word. It gives title
matches priority, adds context from matching Markdown when it can, and falls
back to a page description for a title-only match. Its exact ranking and
excerpt choices are site policy, not part of Heine's search-index contract.

Search indexes are rebuilt by full builds and `heine serve`. `--quick` leaves
the previously published index unchanged, because it cannot safely establish
complete collection membership after every possible source change.
