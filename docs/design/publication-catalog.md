# Publication catalogs

## Status and decision

The initial catalog is implemented. This record retains the design and the
evidence behind its boundaries. Heine can present works from local BibTeX
files on a publications page, including works no page cites. The catalog does
not add a query language, a particular layout, or a workaround for every
limitation of its bibliography libraries.

The goal is to let an author maintain bibliographic information once and use
it in a publication list and in citations. Heine checks the inputs and makes
publication facts available before templates render. The site owns the page,
its selection and presentation; the catalog does not generate pages.

The catalog exposes all resolved entries in source order and lets templates
select individual works by key. Sites own their presentation and any links
made from additional fields. The sections after the contract preserve the
evidence and alternatives that led to this boundary.

## Author and template contract

This example uses one site-wide catalog backed by local BibTeX files. A catalog
does not require citations on the page or a CSL style.

```toml
# heine.toml
[publications]
bibliography = ["works.bib"]
extra_fields = ["pdf", "code"]
```

Paths are relative to the existing non-published `citations/` directory.
The files are resolved together in the declared order using the existing
bibliography rules. Each entry, including an uncited work or a proceedings
entry used as a parent, is available; Heine does not guess which entries
are publications to exclude. Input errors fail a full build, even if no
template calls a catalog function. If citations and the catalog select the
same files in the same order, they share the resolved bibliography.
The selected files must parse and resolve in full even when a template displays
only some works. Filtering entries later cannot bypass an unsupported preamble
or malformed file. This follows the existing bibliography boundary, which
resolves abbreviations and cross-references across files. Authors can prepare
a compatible export or select a separate bibliography; lazy per-entry parsing
would require a different resolution design.

A bibliography may contain works by other authors as well as the site's own
publications. A site can select a dedicated publications file, or keep a
master bibliography and use a curated key list for the works it presents.
The full list includes all entries, including parent proceedings and books.
Heine does not infer authorship or decide which entries belong on the page.

```bibtex
@article{paper,
  author = {Smith, Alice and García, Bob},
  title = {A Study},
  journal = {Example Journal},
  year = {2024},
  doi = {10.1000/example},
  pdf = {assets/papers/study.pdf},
  code = {https://example.org/study}
}

@book{another-work,
  editor = {Lee, Robin},
  title = {Another Work}
}
```

A template can call `publications()` for a list in bibliography source order
or use `publication(key=...)` to resolve one work. A missing key fails at
the template call. For example, `data/featured.toml` could contain an
author-ordered list:

```toml
keys = ["paper", "another-work"]
```

```tera
{% for key in data.featured.keys %}
  {% set work = publication(key=key) %}
  <article>
    <h2>{% if work.title %}{{ work.title }}{% else %}{{ work.key }}{% endif %}</h2>
    {% if work.authors %}
      <p>{{ work.authors | join(sep=", ") }}</p>
    {% elif work.editors %}
      {% for group in work.editors %}<p>{{ group.names | join(sep=", ") }}</p>{% endfor %}
    {% endif %}
    {% if work.date %}<p>{{ work.date.text }}</p>{% endif %}
    {% if work.journal %}<p>{{ work.journal }}</p>{% elif work.book_title %}<p>{{ work.book_title }}</p>{% endif %}
    {% if work.extra.pdf is defined %}
      {% set pdf = asset(id=work.extra.pdf) %}
      <a href="{{ pdf.url }}">{{ t(id="publication-pdf") }}</a>
    {% endif %}
    {% if work.extra.code is defined %}<p>{{ work.extra.code }}</p>{% endif %}
  </article>
{% endfor %}
```

The full list can loop over `publications()` and use the same presentation.

The site supplies the Fluent message and decides that its `pdf` field
contains a Heine asset ID. `asset()` checks that ID when the call executes;
Heine does not infer that an arbitrary BibTeX PDF path is an asset. The code
field is displayed as text, not checked as a URL or fetched. HTML escaping
does not check a URL's scheme: a custom field containing `javascript:` can
remain dangerous in an `href`. A site that turns such a field into a link
must trust its source or check it with an external tool. Only
works the loop reaches are checked by its `publication()` and `asset()` calls;
the catalog does not claim that the whole `data.featured.keys` list was
checked before rendering.

### Work view

| Field                   | Meaning                                                                                                                                                                                                                                                |
|-------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `key`                   | The canonical BibTeX entry key.                                                                                                                                                                                                                        |
| `type`                  | The `biblatex` entry type in lowercase. Known types use the library's canonical name; for an unknown type, use the text retained in `EntryType::Unknown`, since its display formatter returns `unknown`.                                               |
| `title`                 | Plain text derived from the resolved title, or null when absent.                                                                                                                                                                                       |
| `authors`               | An ordered list of display-name strings; an absent author is an empty list. Heine uses `biblatex`'s person display behavior.                                                                                                                           |
| `editors`               | An ordered list of objects with `role` and `names`, where `names` is an ordered list of display-name strings. Roles use the library's lowercase text, including unknown roles; a missing role defaults to `editor`. An absent editor is an empty list. |
| `date`                  | Null when absent; otherwise an object with `text` from the resolved date field, or the year field when no date is present.                                                                                                                             |
| `year`                  | The numeric year of a single parsed date, or null for a missing, unparsed, or range date.                                                                                                                                                              |
| `journal`, `book_title` | Separate plain-text venue facts, each null when absent. The journal uses the library's journal-field aliases.                                                                                                                                          |
| `publisher`             | An ordered list of plain-text publishers, empty when absent, using the library's publisher parsing.                                                                                                                                                    |
| `doi`, `url`            | Authored identifier and resolved URL values when present. Heine does not fetch either; turning a DOI into a link requires an explicit policy.                                                                                                          |
| `extra`                 | A map containing only fields explicitly named by `extra_fields` and present on the entry. Keys are lowercase field names and values are resolved plain strings, not trusted HTML.                                                                      |

The library displays a person as given name, prefix, family name, then suffix,
omitting absent parts. The tested examples are `Johannes Diderik van der Waals`,
`Jane Doe Jr`, and the braced organization `Research Group`. BibTeX
`and others` is exposed as the name `others`.

These fields are a starting point for a small useful view, not a promise that
all BibTeX constructs can be flattened faithfully. In particular, the probe
does not establish how mathematical title content should be represented.
Templates receive ordinary values subject to autoescaping. Authors should not
use raw filters for untrusted input. The view should avoid an additional
Hayagriva conversion when only structured facts are requested.

### Selection, checking, and limits

The full list preserves bibliography order; the featured list preserves the
order authored in site data. The template can choose which entries to show
and can apply its own presentation, without a built-in filter or grouping
language. Repeated keys in site data render repeatedly, as ordinary template
input does. `publication(key=...)` accepts canonical keys and BibTeX `ids`
aliases, returning the same canonical `key` for either. An unknown key fails
at the template call. Both functions require `[publications]`; calling either
without it fails at the template call with guidance to configure the catalog.
An enabled catalog with no entries returns an empty list. Function arguments
follow Heine's existing checked template-function rules.

Invalid bibliography inputs fail during full-build loading, even when no
template calls either function. A field that the library cannot convert to a
promised view field fails when that work is converted, with the work key, field,
and template call in the diagnostic. Use the authored field location when it
is available; inherited fields with synthetic spans must not be presented as
authored source positions. This keeps a single-work lookup from constructing
every work view. A full-list call converts every entry and reports a mapping
error in bibliography order.

For a newest-first list, a template can sort the result of `publications()` by
`year` and render works without a single year separately. A fixture tests the
same value shape with site data. Tera
places null values first after a reverse sort, so the template must separate
them if it wants undated works last. Date ranges retain their display text and
have no single year under this proposal; the site may choose their placement.
The equivalent template structure tested by the fixture is:

```tera
{% set works = publications() %}
{% for work in works | sort(attribute="year") | reverse %}
  {% if work.year is none %}{% else %}{{ work.key }}{% endif %}
{% endfor %}
{% for work in works %}
  {% if work.year is none %}{{ work.key }}{% endif %}
{% endfor %}
```

Reversing the sorted list also reverses the source order of equal years. A
site that needs a specific order within a year can use a curated key list.

The `publication_names_and_date_ranges_follow_biblatex` test confirms that
the library retains a name particle and suffix, displays the sampled
organization name, and recognizes a date range. The existing partial-metadata
test covers an unparsed date. Display names follow the library; no separate
name parser or guessed date is proposed.

The `extra_fields` list makes sites opt in to additional lowercase field
names; an absent field is omitted from `extra`. It does not interpret a value
as a URL or asset. The list can include standard fields such as `abstract`,
`pages`, `eprint`, or `langid`, as well as custom fields. This avoids copying
every field, including local attachment paths, into each template view. A test
shows that the library's verbatim text
formatter retains case-protected text and dollar-delimited math in a sample,
but the result is ordinary text, not rendered mathematics or a lossless
BibTeX representation. Configuration rejects invalid or duplicate extra field
names. Inherited fields with synthetic source spans follow the
diagnostic rule above.

Quick builds do not promise to track changed bibliography files or catalog
configuration until dependency tracking supports those relationships. A full
build is required after such changes. There is no generated publication page,
CSL-formatted catalog entry, remote lookup, or checked relationship for an
unrendered supporting link.

### Scope and cost

One site-wide catalog keeps selection and names simple for the common case of
one researcher's or group's bibliography. Several files can contribute to it,
and site-owned key lists can produce separate views for people or projects.
The files still share one key namespace and existing cross-file resolution.
Independent bibliographies with conflicting keys cannot be combined unchanged;
such a site can use a separate `data/` list or maintain distinct sites until a
named-catalog contract is justified. This trade-off should be reconsidered
if actual examples show that independent namespaces are common.

Heine offers a full list through `publications()` and converts one
selected work at a time through `publication()`. Bibliography loading is a
shared build-scoped operation. Both the
catalog and citations use its existing cache, keyed by root and the ordered
list of files. Identical selections share the resolved bibliography and its
source text, regardless of style. A catalog alone must not load a CSL style or
construct a Hayagriva library. Sites without a catalog must not construct a
catalog view. The current cache does not share parsed individual files between
different file lists. Converting the full list costs time and output space proportional to
the exposed fields of all N works per call. Converting one work costs time and
output space proportional to that work's exposed fields. Keyed lookup avoids
constructing a full view merely to select a few works. The template
engine serializes returned values, so repeated calls repeat full-catalog
conversion. These are complexity estimates, not measured time or memory
guarantees.

The `publication_keys_and_selected_fields_use_the_resolved_bibliography` test
confirms source-order keys, explicit selected-key order, repeated keys, and a
missing lookup at the library boundary. The
`publication_view_example_renders_partial_metadata` fixture renders the
proposed value shapes from ordinary site data for an article, an editor-only
chapter, and a minimal entry. That earlier fixture exercises site data rather
than the catalog functions. Catalog fixture tests now exercise the implemented
functions and field mappings.

## What people publish and present

Publication lists serve more than one discipline. The Oxford Global History of
Capitalism project separates [journal articles and book chapters](https://globalcapitalism.history.ox.ac.uk/publications).
Princeton mathematician Ramon van Handel lists [publications and preprints](https://web.math.princeton.edu/~rvan/)
with links to PDFs and arXiv. Haining Wang's [research page](https://hainingwang.org/research)
includes selected papers, preprints, code, data, and slides. These examples
establish useful patterns, not their frequency across all academic sites.

The [al-folio documentation](https://github.com/alshedivat/al-folio/blob/main/docs/CUSTOMIZE.md)
describes BibTeX-backed publication pages, newest-first sorting, and fields for
abstracts, papers, code, slides, and other resources. Academic Pages documents
[grouping publications by type and ordering them by date](https://github.com/academicpages/academicpages.github.io/wiki/Customizing-the-content-on-your-site).
Jekyll Scholar supports [formatted bibliographies, sorting, grouping, and selection](https://github.com/inukshuk/jekyll-scholar).
These are evidence of distinct needs, not a list of features Heine must copy.

The first design should account for:

- Articles, conference papers, books, chapters, preprints, reports, datasets,
  and other BibTeX entry types. A catalog should not silently discard an entry
  simply because its type is unfamiliar.
- Titles, author or editor names, dates, venues, and identifiers. Missing or
  partial fields need a clear representation: works can be forthcoming,
  undated, institutional, or authored by several people.
- A full list and a curated subset on another page. Authors may group by year
  or type, and may use an order that is not publication-date order.
- Links to the work and supporting materials such as PDFs, datasets, code,
  slides, and videos. The links and their labels can differ by site and field.
- Different presentation needs: some sites want structured facts, while others
  need a formatted bibliographic reference in a particular style.

## Existing tools and library boundary

[Hayagriva](https://github.com/typst/hayagriva/tree/v0.10.1) provides a
structured bibliography model, BibTeX conversion, CSL formatting, and a
selector language. Heine can use its library API without running a command or
accessing a network. The selector language is not a reason to add a query
language to Heine. Hayagriva's formatted output also retains the
[documented CSL limitations](../citations.md), including author-name
disambiguation; a catalog must not imply that formatting is always correct.

The local `publication_fields_survive_resolution_and_expose_conversion_boundaries`
test uses Heine's existing resolver and `biblatex` 0.12.0 with a proceedings
parent, conference paper, journal article, book, chapter, preprint, and
dataset-like record. Its inputs are synthetic, informed by the examples above,
not a claim to represent every exporter or discipline. In this sample:

- The resolved `biblatex` entry retains the authored `pdf`, `code`, and
  `selected` fields. It also resolves a cross-reference and an abbreviation
  into the child's date and book title.
- Hayagriva conversion exposes the sampled title, authors, DOI, and article
  URL, but its entry does not expose those three custom fields. It produces
  entries for all the sampled types; the test does not establish that their
  types or all fields are mapped correctly.
- The inherited date in the resolved `biblatex` entry has a synthetic span,
  not an authored source position. A catalog cannot assume every resolved field
  can supply its own precise diagnostic location.

Hayagriva's [entry format](https://github.com/typst/hayagriva/blob/v0.10.1/docs/file-format.md)
represents recognized identifiers and relationships. Its model and CSL
conversion do not make arbitrary website-specific fields first-class. An
upstream [issue about `shorthand`](https://github.com/typst/hayagriva/issues/190)
illustrates that individual BibTeX fields can be omitted by conversion; it is
not evidence that the catalog's custom fields will be added upstream.

## Candidate boundary

Investigate a render-facing view built from the existing resolved bibliography,
with a deliberate mapping for the fields the catalog promises. Reuse the
build-scoped input cache and the existing local-path, parsing, inheritance,
alias, cycle, and URL checks where they apply. A catalog must work without a
page citation setting or CSL style. If citations and a catalog select the same
inputs, they should share resolved inputs without making one depend on the
other's rendered output.

The `biblatex` and Hayagriva representations have different strengths. Do not
silently switch between their versions of a field or promise lossless raw
BibTeX through a normalized view. If the catalog exposes a custom field, define
its source and meaning and test inherited values. Keep unknown fields and
library limitations visible during design; a field belongs in the public view
only when its meaning and presentation are useful and supportable. Document
unsupported constructs rather than building another BibTeX parser.

## Comparison with site data

The `publication_list_from_site_data_is_independent_of_bibtex` fixture builds
a publication page using Heine's current `data/` and template APIs. Its TOML
contains an ordered list with titles, authors, years, types, selection flags,
and local PDF asset IDs. The template loops over that list twice: once for
selected works and once for the full list. It uses `asset(id=work.pdf)` for
each PDF. A missing PDF fails the build when the template resolves it.

The same fixture cites one of the works from a BibTeX file. Changing its title
in BibTeX changes the citation, while the publication list keeps the old
title and the full build succeeds. Both inputs are valid on their own; Heine
has no declared relationship saying that the data record and BibTeX entry
describe the same work. An author can also maintain a JSON or TOML list without
any BibTeX input, in which case this duplication does not arise.

| Need                              | Site data today                                   | A BibTeX-backed catalog would need to provide                                            |
|-----------------------------------|---------------------------------------------------|------------------------------------------------------------------------------------------|
| Full and selected lists           | A list, authored order, and template conditions   | A list and a checked way to identify the selected subset                                 |
| Titles, authors, dates, and types | Authored directly in data                         | A documented mapping from resolved BibTeX, including missing values                      |
| Local PDFs                        | Asset IDs resolved by `asset()` in the template   | A defined relationship to checked assets, without assuming arbitrary paths are asset IDs |
| Citations and publication entries | Two independent inputs can duplicate and disagree | One selected bibliography as the source of shared facts                                  |
| Layout and labels                 | Site-owned template                               | The same site-owned template control                                                     |

This comparison supported a narrower goal than a publication-page framework:
expose bibliographic facts and their identity to the template so that an
author need not copy those facts into `data/`. It does not show that Heine
needs built-in grouping, client-side filters, generated pages, or a general
query language. The comparison alone did not establish a catalog syntax,
curation mechanism, or how an optional PDF field becomes an asset ID; the
contract above records the resulting decisions.

## Field investigation

The test-only `candidate_publication_view_uses_resolved_fields_without_csl`
probes a view assembled directly from resolved `biblatex` entries. It is not
a template API. In the sampled records, the library provides a key and entry
type; a title with preserved capitalization; structured author names; a typed
date or a retained unparsed date; an inherited book title; a journal alias;
and the authored DOI, PDF, and code strings. It also exposes editor roles.
The test includes a corporate author, an entry without authors, and an entry
with neither a date nor an identifier. It does not establish presentation or
diagnostic behavior for all names, dates, fields, and entry types.

The work view above uses these findings. Its display names follow
`biblatex`; they are not a promise of CSL formatting. Dates preserve the
resolved date or year text, including ranges and unparsed values. Legacy
separate month and day fields do not become part of that display text in the
first version. Local asset relationships remain explicit template calls.
Mathematical chunks remain plain text, not rendered mathematics.

Direct access to the resolved `biblatex` view can avoid constructing a second
Hayagriva library when CSL output is not requested. Whether and how to cache
the render-facing view is a separate implementation decision; this test does
not measure its time or memory. The work view specifies unknown entry types and
absent fields above.

## Selection and ordering

The locally available CryptoBib `crypto.bib` contains approximately 90,000
records with keys, titles, authors, years, and many DOI or URL fields. A scan
of field names in that copy found no `selected`, `pdf`, or `code` fields. It
is useful evidence for realistic keys, bibliography data, and scale, but it
does not say which works a particular site owner wants to feature or which
local files they want to publish. Those choices need site-specific input. The
scan is not a statistical study of other bibliographies.

As an exploratory selection, the keys `ACISP:LiSun25`, `ACISP:SKSI25`, and
`ACISP:BanYan25` all exist in that copy of CryptoBib. Their chosen order differs
from the source order. This shows a concrete reason to preserve a curated list's
authored order. It is an illustrative selection, not a claim about the
researchers' own preferences.

Two fixture tests examine current template behavior with site data:

- `curated_publications_from_site_data_follow_selected_key_order` loops over
  selected IDs and searches an array of records. The result follows the ID
  order, but a missing ID silently produces no item. The nested iteration also
  performs more comparisons as both lists grow.
- `curated_site_data_can_resolve_keys_with_template_lookup` uses a map keyed
  by CryptoBib-style IDs, including colons. Direct template lookup follows
  the ID order and a missing key causes a render error. This checks a lookup
  in site data; it does not establish a relationship with a BibTeX entry or
  guarantee a diagnostic at the selected ID's authored location.

A catalog exposing a source-ordered list and keyed access lets a template order a
curated ID list. A separate checked selection
could provide earlier, source-located missing-key diagnostics, but should be
added only if that extra relationship justifies its configuration and cost.
The catalog treats repeated selected IDs as ordinary repeated template
input. The initial catalog has one bibliography namespace; independent
namespaces remain a separate future question.

Templates should receive plain, escaped-as-usual values rather than trusted
HTML from authored metadata. A DOI is an identifier; turning it into a link
requires an explicit contract. A remote URL and a local copied asset are
different relationships. If a field selects a local asset, use Heine's checked
asset boundary rather than silently treating an arbitrary filesystem path as a
published URL. External links require no network access.

## Implementation checks

Exercise the actual functions with and without citations, including shared
inputs, an empty catalog, missing configuration, aliases, unknown keys, and
source-ordered mapping errors. Cover the specified view fields, unusual names
and dates, inheritance, local and localized assets, and template sorting.
Check full- and quick-build behavior and avoid materializing a catalog view
when neither function is called. CSL-formatted entries and checked per-work
link declarations remain separate possible extensions.

Additional date range kinds remain a possible extension. Test actual exporter
fixtures before claiming broad Zotero or Better BibTeX compatibility.

## Supporting links and assets

The site-data fixture resolves PDF IDs through `asset()` and fails when an
asset is missing. The `publication_links_in_site_data_are_not_validated_as_urls`
test shows the other side of the boundary: a string placed directly into a
template link is rendered even when it is not a valid URL. Template escaping
protects HTML syntax; it does not turn a string into a checked URL.

The `custom_link_fields_have_no_checked_url_or_asset_meaning` test shows that
resolved BibTeX fields `pdf` and `code` remain strings, even when they contain
a missing local file or an invalid URL. Their presence alone does not establish
a checked asset or external link. Conversely, Heine's existing bibliography
`url` check expects one HTTP or HTTPS URL; the
`multiple_urls_in_one_field_are_not_a_single_checked_url` test confirms that
a semicolon-separated list is rejected. A catalog that reuses this input
boundary inherits that limitation.

The catalog exposes selected supporting-link fields as ordinary strings and
lets templates choose how to use them. Sites that mean a field to
name an asset can call `asset()` with its value, just as they do with site
data; this checks the asset when rendered. A site can instead keep a separate
keyed list of asset IDs for works whose BibTeX `pdf` field uses a different
convention. This does not require Heine to infer whether a string is a URL, a
path relative to the bibliography, or an asset ID. It also means unrendered
links are not checked as relationships, and arbitrary external link strings
need a separate contract if Heine is to validate them.

Another option is an explicit per-work link declaration that distinguishes an
asset ID from an external URL and supplies a site-owned label. Heine could
resolve declared assets and validate URL syntax while constructing the site
model, including for works no template happens to render. This checks a
stronger relationship, but introduces additional configuration and must define
how the declaration identifies a work, handles multiple links and locales,
and interacts with fields already in BibTeX. Do not add it solely to avoid a
template call when the current approach meets the use case.

A third option is to declare that a named field contains asset IDs and check
that field for every work. This avoids per-work link configuration but needs a
rule for shared assets, locale overlays, and fallback before the check can be
defined. The catalog leaves this stronger relationship to a later
design rather than guessing which locales must have an asset.

### Worked comparison

The `publication_link_shapes_resolve_only_assets_rendered_by_the_template`
fixture renders a selected work with a local PDF and an external code link.
The same work can be described with two direct fields:

```toml
pdf = "assets/paper.pdf"
code = "https://example.org/code"
```

Alternatively, ordinary site data can describe each link's kind, target, and
Fluent label key:

```toml
links = [
  { kind = "asset", target = "assets/paper.pdf", label = "pdf" },
  { kind = "external", target = "https://example.org/code", label = "code" },
]
```

These examples are test data, not a proposed catalog schema. Both templates
produce the two links, call `asset()` for the PDF, and obtain labels from
Fluent. In both cases the build succeeds when an unselected work names a
missing PDF, then fails when the template selects that work. Both templates
also render an invalid external URL string. The generic link-list template
silently omits an unrecognized kind because its conditional has no matching
branch. The fixture confirms that adding a `kind` field to ordinary data does
not make it a checked declaration.

| Question              | Template-resolved fields                                            | Possible model-checked declaration                                                          |
|-----------------------|---------------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| Local asset           | `asset()` checks it when the template renders the work              | Heine could check every declared asset while resolving the catalog                          |
| External link         | A template string is escaped, but its URL is not checked            | Heine could check a declared URL's syntax and permitted scheme                              |
| Multiple links        | The site chooses fields or its own list shape                       | The public contract must define repeated links, labels, and kinds                           |
| Input and maintenance | BibTeX fields can be used where the site gives them a known meaning | A declaration needs an explicit work key and link data, potentially repeating BibTeX fields |
| Diagnostics           | A rendered missing asset fails at the template call                 | A model check could point at a link declaration, including for unrendered works             |

The model-checked column describes a possible feature; it has not been
implemented or tested. Both approaches still leave external link availability
to the site owner, since Heine does not fetch remote pages.

The smaller approach fits existing template behavior and avoids a second link
schema. The stronger approach would add a useful guarantee for links attached
to unrendered works, but the example alone does not establish that this
guarantee is worth the additional authored contract. For the initial catalog
design, prefer exposing supporting values without inventing a link registry;
retain the checked declaration as a separate option if a concrete use case
needs its stronger guarantee. This preference does not authorize treating a
relative BibTeX `pdf` path as an asset ID automatically.

## Additional bibliography evidence

The `export_shaped_fields_and_latex_text_follow_biblatex` test uses a
synthetic entry shaped around common export fields, including an attachment
path, abstract, keywords, URL date, and language identifier. It is not an
actual Zotero export or a compatibility survey. Better BibTeX documents
[ASCII BibTeX export and attachment-field options](https://github.com/retorquere/zotero-better-bibtex/wiki/Configuration/a59496f3cca44d8900c9bbb35981bbc0868667cc),
which motivated testing both LaTeX text and a local attachment path.

For the sampled text, `biblatex` converts the accent macros to `ü` and `é`,
`\&` to `&`, and `--` to an en dash. It preserves `~` and the unsupported
`\textit{example}` command in the plain text. It presents `and others` as a
second name, `others`. The work view follows these results; it does not
turn retained commands into HTML or infer an author-list abbreviation. Authors
who need different presentation can adjust their bibliography or template.
These observations cover the sample, not every LaTeX command or export.

On November 3, 2026, three [TUG archive](https://ftp.math.utah.edu/pub/tex/bib/index.html)
bibliographies were downloaded to a temporary directory: [British Journal for
the History of Science](https://ftp.math.utah.edu/pub/tex/bib/bjhs2010.bib),
[Historia Mathematica](https://ftp.math.utah.edu/pub/tex/bib/histmath.bib),
and [Journal of Statistical Software](https://ftp.math.utah.edu/pub/tex/bib/jstatsoft.bib).
Their headers identify them as public domain. Line-based scans counted 1,583,
3,410, and 1,400 work entries respectively, with books and articles in the
first two and predominantly articles in the third. These are file-specific
counts, not estimates of research disciplines as a whole.

Each file failed Heine's existing bibliography resolver at a concatenated
`@Preamble`, before the entries were resolved. The
`concatenated_bibtex_preamble_is_not_supported` test records a minimal example.
Do not remove or rewrite the preamble automatically: this is an existing
library-input limitation, not evidence that the publication entries themselves
are unusable. The experiment did not establish how many entries would pass
after an author prepares a compatible export.

A line-oriented scan found semicolons in 40 of 430 quoted URL fields in the
history-of-science file, 40 of 3,312 in Historia Mathematica, and 609 of 1,396
in the statistics file. The statistics bibliography contains fields listing
article, PDF, and code addresses together. These counts are a rough indication
of input shape, not a parse or a link-validity check. In particular, silently
splitting every semicolon would be an unproven interpretation of a URL field.
The catalog uses the existing single-URL check rather than inventing a new
URL parser to accommodate this corpus.

For reproducibility, the downloaded files had these SHA-256 hashes:

```text
bjhs2010.bib  d53ef1dc581c139ead9b90d55fc2c877cd7c59b2676c40a6f6c4fc2db0c324c5
histmath.bib  b3f28d2bba3d3be0e2a90f13ff90acc780a1edb23babcf7743f9e052572a4316
jstatsoft.bib a13ceb1b1feceecd5d5a82af007afdfaebd5427530a817311286dee675ae3ee4
```

## Alternatives and limits

An authored JSON or TOML file in `data/` already lets a template present a
publication list. It can be the better choice for a site without BibTeX or
with a small, independently curated list. It requires authors who also cite
those works from BibTeX to maintain overlapping facts in two places. A
catalog is justified only if reusing checked bibliography data materially
improves that workflow.

Formatting every publication through CSL would reuse a known standard but
would not by itself expose custom links or give a template control over each
field. Exposing only raw fields would retain flexibility but could push
normalization, inheritance, escaping, and inconsistent interpretation into
every template. The implemented view follows the evidence from the site
examples while leaving CSL-formatted catalog entries for a later decision.

Do not add network lookup, DOI resolution by fetching a service, automatic
output pages, JavaScript search, citation metrics, a general query language,
or compatibility with every external theme as part of this exploration.
Those are independent choices. No rule here requires a fixed size for the
evidence set: broaden it when a future contract leaves an important user case
or library behavior unclear.
