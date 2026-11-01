# Citations

## Status and goal

Heine implements checked in-text citations. It connects citations
in one Markdown source to local BibTeX entries and a CSL style, then checks
that the page template places the resulting bibliography. The first release
covers numeric and author-date styles. It does not fetch files or run a
citation executable.

The [citation guide](../citations.md) shows the authored configuration and
syntax. The [reference](../reference.md) defines the field-level contract. The
[investigation report](citations-feasibility.md) records the evidence and its
limits.

## Input ownership and resolution

Bibliography and CSL files live under the non-published site-root `citations/`
directory. A page selects one or more `.bib` files, a style, and one declared
Markdown source. `_directory.toml` can supply a complete default; the nearest
setting wins, and `citations = false` suppresses an inherited setting. Paths
are relative to `citations/` and cannot escape it or traverse symbolic links.

Heine resolves multiple bibliography files in authored order. Duplicate entry
keys are errors, while abbreviation definitions follow `biblatex`'s normal
last-definition rule. Cross-file references are resolved before processing.
Heine checks cycles and limits recursive expansion before calling the library:
50 links per chain, 1,000 visits per field or entry, and 100,000 bytes of
expanded source text per abbreviation-bearing field.

A dependent CSL style requires a local independent parent. Heine checks the
parent's identity against the dependent style and does not fetch its URL.
An optional locale overrides the owning page locale for both citations and the
reference list. Inputs and their converted bibliography are shared across pages
that select the same bibliography files, including pages using different styles.

## Markdown and rendering

The selected source is an unsegmented Markdown document. Citation collection
shares Heine's existing analysis walk with other source facts; rendering uses
its existing Markdown walk. Citations inside a component belong to the enclosing
source. Ordinary and margin footnotes process citations at their first reference;
repeated margin notes reuse the result. An unreferenced ordinary footnote is
processed at its definition. Unselected sources do not join the sequence.

Bracketed citations name checked keys and may contain semicolon-separated
items and typed page or chapter locators. Code, raw HTML, links, images, and
renderer bodies retain literal citation-shaped text. A backslash before the
opening bracket writes a literal example in prose; an unclosed bracket remains
literal. Ambiguity with a resolved shortcut reference is diagnosed. BibTeX
aliases select the canonical work. Prefixes and suffixes are not supported.

The template renders the selected source once and places `bibliography()` once.
Heine verifies that the complete bibliography survives template rendering; a
call count alone cannot detect a discarded or duplicated result. Citations link
to the bibliography section. The reference list contains cited works and its
heading belongs to the template.

## Processor boundary

Hayagriva 0.10.1 processes CSL. Heine checks input and converts its structured
output into renderer events, escaping authored text and link attributes. It
rejects unsupported output instead of relying on Hayagriva's HTML writer to
sanitize it. The selected style controls locator presentation.

Note styles and reachable `ibid`, `ibid-with-locator`, and `near-note`
conditions and duplicate macro names are rejected. A numeric group with a
locator disables numeric collapse for that request; unsupported collapsed group
forms are diagnosed.
A locator containing a period is rejected when a reachable style rule can
strip it. These restrictions address observed processor behavior, not a claim
that all other CSL styles conform. See the [processor findings](citations-feasibility.md#note-styles-and-position-conditions).

Bibliography URLs must be absolute HTTP or HTTPS URLs without credentials.
Heine checks them before conversion because the processor can discard invalid
URLs. URL fields are literal: an escaped character such as `\&` receives a
source-located diagnostic rather than being silently rewritten. Consecutive
hyphens also produce an error because the parser would change them. See the
[URL findings](citations-feasibility.md#formatting-and-links).

## Remaining work

The fixture tests exercise representative numeric and author-date styles, and
the starter site's German reading cites the poem's newspaper and book
publications. Third-party notices have been regenerated for the locked
dependencies. The large-corpus investigation measured resolution work, not
build time or memory; its results and limitations are in the investigation
report. A runtime and memory comparison remains available for a later
performance review.
Quick builds do not fully track citation inputs or changed selections; a full
build is required for those changes.

## Deferred work

Note-generating styles need a shared numbering and punctuation plan with both
footnote modes and reliable position handling. Publication catalogs need a
separate contract for fields, custom metadata, filtering, ordering, and links.
Uncited works, multiple citation sources or segments, per-item affixes, and
numbered document cross-references also need separate checked relationships.

## Prior art

[Pandoc](https://pandoc.org/MANUAL.html#citations) and
[Quarto](https://quarto.org/docs/authoring/footnotes-and-citations) describe
citation workflows. [Jekyll Scholar](https://github.com/inukshuk/jekyll-scholar)
provides publication-list examples. Hayagriva's
[CSL compliance tracker](https://github.com/typst/hayagriva/issues/327) is
relevant when evaluating styles.
