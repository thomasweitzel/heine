# Search design

## Status

This design is implemented. The [search-index guide](../search.md) and
[reference](../reference.md#search-indexes) define the current user-facing
contract. This document records the design reasoning and the library-neutral
boundary: the browser search experience remains site-owned.

## Decision

Heine will generate checked, locale-local JSON **search document indexes**
from one or more configured collections. An index is a deterministic set of
source-derived documents, not a JavaScript search engine and not a generated
search page.

The browser fetches an index and chooses how to search it. A site may use a
local script, MiniSearch, Fuse, FlexSearch, a hosted service, or another
compatible client. Heine neither bundles JavaScript nor imposes tokenization,
stemming, ranking, fuzzy matching, query syntax, or a search-result user
interface.

Heine extracts searchable text from authored Markdown inputs, never from
rendered HTML. Templates therefore do not accidentally determine search
membership, and source text omitted by presentation can remain searchable under
the extraction rules below.

## Motivation

A static deployment has no query endpoint. A browser search feature therefore
needs either static data that it can query locally or a separately operated
service. Heine can reliably provide the former without claiming ownership of
the latter's ranking and interface decisions.

An engine-specific serialized index would bind Heine's output contract to one
JavaScript library's tokenizer, language support, serialization format, and
release cycle. Emitting documents instead gives a site a stable input for an
exact matcher today and a more capable engine later.

Parsing rendered HTML would be less trustworthy. It would index navigation,
footers, repeated interface text, and arbitrary template markup while missing
authored source removed or transformed by templates. Parsed declared Markdown
is the narrow source Heine can identify without guessing a page's visual body.

## Scope and terminology

- **Search document index**: one locale-local generated JSON document selected
  by a named `[search.<name>]` configuration table.
- **Search document**: one rendered collection member's stable facts and
  source-derived Markdown text.
- **Search text**: plain text extracted from every declared Markdown input of
  one page. It is data for a browser engine, not rendered HTML or an excerpt.
- **Search section**: one non-empty, source-ordered region of search text. It
  has a generated heading fragment only when that region begins at a heading
  the page already renders with an identifier.
- **Search UI**: site-owned HTML, CSS, JavaScript, and Fluent interface text
  that fetches an index and presents results.

Search indexes are generated resources. They participate in normal output
ownership and collision checks, but do not create rendered pages, template
tasks, feeds, taxonomies, or a server query endpoint.

## Configuration and output

Each index has a simple identifier, a non-empty array of configured
collections, and a locale-relative output path:

```toml
[search.site]
collections = ["posts", "documentation"]
path = "search/site.json"
# versioned = true # the default
```

`collections` may contain more than one collection because a visitor often
expects one search box to cover, for example, articles and documentation.
Every listed collection must exist. Duplicate collection names are an error at
the later duplicate entry. The array must not be empty.

`path` is a portable relative output path ending in literal lowercase `.json`
and outside `__heine/`, using the same path rules as other authored generated
resources. It is a logical base path, not necessarily the final published
filename.

Search indexes are versioned by default. Heine serializes the complete
locale-local JSON document, calculates its SHA-256 digest, and inserts the
lowercase hexadecimal digest before the `.json` suffix. For the configuration
above, Heine may write:

```text
search/site.83f0…c94a.json
de/search/site.10a2…e7b1.json
```

The examples abbreviate the digest only for readability: the published name
uses the complete digest. This makes an index URL change whenever its bytes
change, allowing a deployment to cache it immutably without serving a new page
against an old index. `search()` always returns the current generated URL, so
templates never construct this name.

`versioned = false` opts out when a site has a concrete reason to require a
stable endpoint. Heine then publishes the literal configured path, and the
site is responsible for suitable cache revalidation headers. The derived
versioned filename is checked for portable output-path limits before any
output is claimed.

Every final path is locale-relative: for the default locale above Heine writes
below `search/`; for German it writes below `de/search/`. Existing output
claims, including pages, copied assets, deployment files, feeds, taxonomies,
and other search indexes, can never be silently replaced.

Every configured index exists in every configured locale. A locale with no
selected members receives a valid index with an empty `documents` array. A
site does not get an implicit mixed-language index.

There is no page-level `search = false` setting in the first version.
Collections already express membership. A site that needs
a narrower searchable set creates a collection for that relationship instead
of accumulating hidden, global exceptions on unrelated pages.

## Document contract

Heine serializes UTF-8 JSON with this versioned envelope:

```json
{
  "format": 3,
  "locale": "en",
  "documents": [
    {
      "id": "blog/mathml-audio/index",
      "url": "/blog/mathml-audio/index.html",
      "draft": false,
      "title": "MathML, audio, and a site generator",
      "description": null,
      "summary": "A practical look at MathML in Heine.",
      "published": "2026-08-14T07:30:00Z",
      "updated": null,
      "taxonomies": {
        "tags": ["MathML", "Rust"]
      },
      "sections": [
        {
          "fragment": null,
          "text": "Source-derived opening text goes here."
        },
        {
          "fragment": "mathml",
          "text": "MathML\n\nSource-derived section text goes here."
        }
      ]
    }
  ]
}
```

The fields have these meanings:

| Field                             | Contract                                                                                                                                                                                 |
|-----------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `format`                          | Positive integer schema version. The current version is `3`; version `1` did not include each document's `draft` flag, while version `2` represented all page text as one `text` string. |
| `locale`                          | The configured canonical BCP-47 locale of this index.                                                                                                                                    |
| `documents`                       | Deterministically ordered search documents, possibly empty.                                                                                                                              |
| `id`                              | The locale-relative Heine page ID. It is a stable programmatic identity, not a display label.                                                                                            |
| `url`                             | The page's base-path-aware URL naming its literal generated output file.                                                                                                                 |
| `draft`                           | `true` only for a draft included in a `heine serve --drafts` index; otherwise `false`.                                                                                                   |
| `title`, `description`, `summary` | Authored page metadata, each a string or `null`. They remain separate so a client can rank and display them differently.                                                                 |
| `published`, `updated`            | Resolved RFC 3339 UTC publication facts, each a string or `null`.                                                                                                                        |
| `taxonomies`                      | Object keyed by configured taxonomy name. Keys serialize in deterministic lexical order; each value is that page's literal locale-local term array, in authored order.                   |
| `sections`                        | Ordered non-empty source-derived text regions. Each object has a `text` string and a `fragment` string or `null`; fragments name existing generated Markdown heading identifiers only.   |

Search documents are ordered by page ID, independently of the presentation
order of every selected collection. If a page belongs to more than one selected
collection, it occurs once. This prevents the first collection listed in the
configuration from becoming an accidental ranking or tie-breaking policy.
The taxonomy object uses the same deterministic key order, so identical site
facts always serialize to identical bytes and therefore to the same versioned
filename.

Heine preserves text rather than converting it into a lowercased word array.
There is no universal definition of a word, especially across writing systems,
and clients need original text for excerpts and highlighting. Case folding,
Unicode normalization, token boundaries, phrase matching, stemming, and fuzzy
matching belong to the selected browser engine or site script. A client that
does not need fragment-aware results can join `sections[].text` in order and
index the resulting text as one document.

## Markdown extraction

For every selected page, Heine examines every `.md` file declared in that
page's `[content]` table. It parses each original source with the site's
configured `pulldown-cmark` options and processes inputs in stable
content-name order. A selected page with no declared Markdown files remains a
search document with `sections: []`, allowing metadata and taxonomy searches.

Extraction contributes, in source order:

- ordinary text;
- heading text;
- link labels and image alternative text;
- inline code and fenced or indented code-block text; and
- inline and display math source when the Markdown parser recognizes it.

Raw HTML source events are handled as a separate source-level case. Heine
processes each authored event independently with an HTML5 tokenizer and
contributes its semantic text. This is not a pass over rendered output:
templates, generated navigation, CSS visibility, and presentation-time
transformations remain outside the index. Tokenization, rather than DOM
fragment construction, avoids context-dependent tree repair for orphan table
elements such as `<td>`.

For a raw HTML fragment, Heine contributes text nodes and an `img` element's
`alt` value. It does not contribute element names, comments, attribute values,
URLs, or the contents of `script`, `style`, and `template` elements. Text
inside the other elements is included even if a site's CSS later hides it,
because authored source, rather than presentation, defines this index. Parsed
HTML block containers add the same two-line-feed boundary used for Markdown
blocks. The fixed container set is `address`, `article`, `aside`, `blockquote`,
`div`, `dl`, `dt`, `dd`, `fieldset`, `figcaption`, `figure`, `footer`, `form`,
`h1` through `h6`, `header`, `hgroup`, `hr`, `li`, `main`, `nav`, `ol`, `p`,
`pre`, `section`, `table`, `thead`, `tbody`, `tfoot`, `tr`, `th`, `td`, and
`ul`. This prevents neighboring structural text from being fused without
depending on browser styles.

HTML tokenization follows HTML's normal error-recovery model. A malformed but
renderable raw event therefore contributes the text the tokenizer can recover,
just as trusted raw HTML remains renderable under Heine's Markdown contract.
Resetting the tokenizer for every raw event ensures an unclosed tag cannot
alter extraction from a later raw event or an ordinary Markdown text event.
The fragment's `lang` and `dir` attributes do not alter extraction: for
example, Arabic text in an English page's `dir="rtl"` quotation becomes part of
that English index as authored Unicode text.

It does not contribute Markdown punctuation, link destinations, image source
URLs, code-fence language labels, task-list controls, footnote identifiers, or
raw HTML markup itself. In particular, the index never receives an HTML
serialization that a browser would have produced.

Extraction preserves characters inside a contributing parsed text event. It
preserves a soft or hard Markdown break as one line-feed character. It adds a
two-line-feed boundary after a block-level Markdown container and trims each
section. A Markdown heading begins the next section. Thus formatting cannot
fuse words accidentally, while Heine does not impose an ASCII word boundary on
scripts that do not conventionally separate words with spaces. This is a
stable plain-text extraction rule, not a general-purpose Markdown-to-text
renderer. A browser engine may apply its own locale-aware whitespace and word
segmentation policy to the preserved text.

Heine assigns a section's `fragment` only when the source is the page's
effective table-of-contents input and the section begins at one of its parsed
Markdown headings. The fragment is the same checked identifier used in the
rendered heading, including headings outside the ToC's displayed level range.
Text before the first heading, another declared Markdown input, raw HTML
headings, and headings in footnote definitions have no generated fragment and
therefore serialize `fragment: null`. A client may link such a result to the
page URL, never inventing an anchor.

Segmented Markdown files are included by parsing their original source. Their
segment-marker HTML comments contribute nothing, while all segment bodies do.
This preserves the rule that search is about the page's declared Markdown
source, not about guessing which segment a template presents.

The initial feature does not inspect non-Markdown declared content, arbitrary
template data, rendered HTML, copied files, or template source. It also does
not extract text from future external renderer requests by treating their raw
syntax as prose. A renderer adapter may later return a declared source-text
contribution through its own checked contract; that extension must identify
which text belongs in search.

## Template contract

Heine adds a checked Tera function:

```tera
{% set index = search(name="site") %}
```

It returns the current locale's configured index view:

```text
name
count
url
relative_url
```

`url` includes `site.base_path`; `relative_url` is relative to the currently
rendered output file, following the existing generated-resource convention.
The function also accepts an exact configured `locale=` when a template has a
real reason to link another locale's index:

```tera
{% set german_index = search(name="site", locale="de") %}
```

Unknown names, malformed names, and unknown locales are errors at the Tera
call site. A configured index is returned even when that locale has no
documents. The function exposes index facts only. Templates cannot inspect or
generate documents, and a search index needs no special search-page template.

## Starter-site reference search UI

The starter site should demonstrate this boundary with one search index spanning
its article and documentation collections. It supplies a search page in each
of its existing locales, each resolving the current locale's
`search(name="site")` view.

The example uses a copied `search.js`, not a bundled third-party search engine.
It demonstrates the boundary that Heine supplies and leaves browser policy to
the site. A site needing fuzzy matching, stemming, or a large index can replace
only this file with another client engine.

The generated JSON schema and `search()` view are Heine's public search
contract. The starter script is not a second public Heine API or a compatibility
promise: it demonstrates one browser policy. A site
that copies and changes it owns that copy. Tests for the starter site protect
its documented behavior and accessibility, not a claim that its scoring policy
is universally correct or permanently frozen.

The search page has one ordinary form, a live status region, an ordered result
list, and result-navigation controls. Its query input uses HTML `dir="auto"`,
so browser editing follows the first strong directional character in a query
without changing the page locale or search-document selection. The script:

1. fetches the JSON once when the search interface is used, retaining a
   successful result for the life of that page and clearing a failed request so
   a later interaction retries it;
2. NFC-normalizes searchable fields once after a successful fetch and each
   query when it is entered. Before collapsing remaining Unicode whitespace to
   one ASCII space, it removes a single source line feed between adjacent
   characters from scripts that commonly do not use spaces between words: Han,
   Hiragana, Katakana, Hangul, Thai, Lao, Khmer, and Myanmar, plus East Asian
   punctuation. This is a narrow matching convenience, not universal Unicode
   word segmentation;
3. uses `toLocaleLowerCase(document.documentElement.lang)` for its local,
   case-insensitive matching policy;
4. splits a query into non-empty whitespace fragments and requires every
   fragment to occur as a substring somewhere in a document;
5. adds the highest matching field weight for every fragment: title `3`,
   summary or description `2`, and search-section text `1`; and
6. preserves JSON document order for score ties;
7. presents ten results per client-side result page; and
8. renders a one-link page-number window around the current page, retaining
   first and last page links and collapsing gaps with ellipses.
9. links a result to the section matching the most query fragments, retaining
   source order for a tie; it appends that section's non-null fragment to the
   page URL and otherwise links to the page itself.

Typing uses a short debounce and resets the result page to one. The form's
ordinary submit behavior performs the same search immediately, so keyboard
submission is never merely decorative. Result-page controls preserve the
current query. The example uses `history.replaceState()` for debounced typing
so every keystroke does not become a Back-button entry; a chosen result page
uses `history.pushState()`.

### Result-navigation algorithm

The starter script uses the same visible navigation rule as Heine's generated
collection pagers, but applies it to the transient result set in the browser.
With `window = 1`, it retains:

- the first page;
- every page from `current - window` through `current + window`, clamped to
  the available range; and
- the last page.

It puts retained numbers in a numeric set, then emits them in ascending order.
Whenever consecutive retained numbers have one or more omitted page numbers
between them, it emits exactly one non-link ellipsis. The set prevents repeated
numbers when the retained ranges overlap; the gap rule prevents ellipses for
result sets with no omitted ranges or adjacent ranges.

For example:

| Current page | Total pages | Navigation items         |
|-------------:|------------:|--------------------------|
|            1 |           1 | `1`                      |
|            2 |           4 | `1 2 3 4`                |
|          101 |       1,202 | `1 … 100 101 102 … 1202` |

These vectors are the shared behavioral examples for the Rust collection pager
and the starter script. They keep the browser control consistent without
pretending that the two implementations share code or public APIs.
When either rule changes, its focused tests and these vectors must be reviewed
together. The result-navigation behavior remains starter-site policy, not part
of the JSON schema or Heine's generated-page pagination contract.

The script uses the resulting result-page number to slice the already ranked
matches. It does not ask Heine to generate result pages and never writes a new
file for a query:

```js
const start = (currentPage - 1) * 10;
const visibleResults = matches.slice(start, start + 10);
```

The browser state is shareable and history-aware:

```text
/search/index.html?q=math&page=2
/de/search/index.html?q=Mathematik&page=2
```

The script reads `q` and `page` through `URLSearchParams`. It stores the
decoded query as entered, applies its local normalization only while matching,
and writes URL state through `URLSearchParams` and `history.pushState()`.
Unicode query text is therefore UTF-8 percent-encoded by the browser when
needed, without inventing a separate language-specific URL format. The current
rendered page selects the locale and index; the script does not trust a
visitor-provided locale parameter. It handles `popstate` so Back and Forward
restore the corresponding query and result page.

`page` accepts only a positive base-10 integer. Missing, invalid, or too-large
values resolve to the first or final available result page as appropriate. A
search query affects neither Heine routes nor static-host file resolution:
servers resolve the search page from the URL path and ignore its query string.

This is not a universal Heine search algorithm. It has no
stemming, typo tolerance, CJK segmentation, query language, server endpoint,
or Heine-generated result pages. A production site may replace only this
script with a local browser library or a service integration while preserving
the generated JSON contract.

The starter script does not mark matched text in result summaries. If a site
adds match highlighting later, it must construct marked text through DOM text
nodes and elements, never by interpolating an authored field or query into an
HTML string.

The Tera template remains structural. It uses `search()` only to expose the
index URL and places all visible and assistive text in Fluent resources,
including the input label, placeholder, loading state, result count,
zero-results text, network-failure text, pagination labels, and
JavaScript-required fallback. The result-navigation element has a
Fluent-supplied `aria-label`; its current-page span has `aria-current="page"`,
and ellipses are non-interactive and hidden from assistive technology. The
script creates result and navigation elements with DOM APIs and `textContent`,
never inserting authored titles, summaries, search text, or query text as HTML.

## Resolution and output model

Source loading retains each declared Markdown source with its owning page until
collection membership is resolved. The search domain then:

1. resolves every configured collection for every locale;
2. unions every configured index's selected rendered page IDs into one
   deterministic locale-local ID-ordered set;
3. derives one reusable page search-document catalog from retained page
   metadata, taxonomy facts, output targets, and declared Markdown sources,
   extracting each selected page's sections once even when several indexes
   select it;
4. selects documents from that catalog for each named index;
5. serializes each selected set, derives its final versioned or stable output
   path, and allocates locale-qualified output claims; and
6. writes JSON as generated files in the ordinary output plan.

The template runtime receives only resolved index views. The output domain owns
the JSON bytes, safe output paths, collision checks, staging, and atomic
publication. Search extraction does not enter Tera and does not observe page
rendering results.

Because a complete index depends on collection relationships and every chosen
member, quick builds do not update search indexes. A quick build retains its
existing incomplete-output contract. Full builds and `heine serve` rebuild the
complete index before publication. A full build stages a complete managed
output tree and atomically replaces the prior tree, so superseded
digest-named search indexes disappear with other stale managed output. Quick
builds never create a new search-index filename, so they cannot accumulate
superseded index files either.

## Diagnostics

Configuration diagnostics retain the relevant `heine.toml` source text and
span. They identify an empty `collections` array, duplicate collection entry,
unknown collection, unsafe output path, or output collision at the authored
location. An output collision includes every competing claim's authored
location where one exists.

Template diagnostics identify the exact `search()` call for unknown or invalid
arguments. Markdown extraction itself is a deterministic parser pass over
already loaded, checked source. Any source-file access or existing Markdown
configuration failure retains the originating content-file diagnostic.

## Boundaries

The first version does not provide:

- a bundled JavaScript engine, widget, CSS, or search-result template;
- a library-specific serialized index;
- forced case-insensitive search or a universal Unicode word tokenizer;
- stemming, typo tolerance, synonym dictionaries, highlighting, snippets,
  filters, facets, or query syntax;
- mixed-locale indexes;
- cross-locale result counterparts or translation relationships in a search
  document. `id` is locale-local; a later cross-locale search
  use case must design its relationship view from explicit `translationid`
  groups rather than infer it from page IDs or paths;
- rendered-HTML scraping or an inferred page-body convention;
- page-level search exclusion flags;
- non-Markdown full-text extraction;
- a production search server or hosted-search integration; or
- a Pagefind-like post-processing step over generated output.

The current document format keeps every selected document in one JSON file. Its
section-text payload grows linearly with declared Markdown source, and a
browser must fetch and parse that JSON document before a local engine can
search it. Collection selection is the first size control. Heine does not add
per-field omission or silent text truncation until
a concrete use case defines their semantics. A chunked full-text search feature
is a separate future design, not a reason to weaken this source-derived
contract.

Pagefind-style chunked full-text search may later be justified for very large
sites, but it needs a separate design for staging, publication, development
server behavior, output ownership, language selection, and its external-tool
boundary. It is not a reason to bind the initial Heine document contract to
one browser engine.

## Acceptance criteria

An implementation must include focused and fixture-site coverage for:

- empty, duplicate, unknown, and valid collection configuration with source
  spans and remedies;
- safe locale-relative versioned and stable output paths, `__heine/`
  rejection, digest-derived filename limits, and collisions with pages, copied
  assets, deployment files, feeds, taxonomies, and other indexes;
- one generated index for every configured locale, including empty locales;
- deterministic union and ID ordering, including a page that belongs to more
  than one selected collection, repeated index selection of the same page,
  and deterministic taxonomy-object key serialization;
- complete JSON escaping, schema version, base-path-aware literal-file URLs,
  nullable metadata, chronology, taxonomy facts, section ordering, and
  fragments that match existing ToC heading identifiers;
- extraction from multiple declared Markdown files, empty Markdown sets,
  segmented Markdown, formatting, preserved line boundaries, links, images,
  code, math, Unicode, raw HTML text and `img` alternatives, excluded raw HTML
  constructs, orphan table elements, unclosed raw tags that do not affect later
  Markdown events, and excluded Markdown constructs;
- exclusion of non-Markdown declared content and template-rendered output;
- checked `search()` calls for current and exact locales, with template spans;
- full-build publication, quick-build non-updating behavior, and development
  server rebuilds; and
- complete updates to the README, tutorial, glossary, reference, focused
  search guide, and an accessible starter-site example, including its
  Unicode URL state, matching, score ties, result-navigation algorithm, and
  history behavior.
