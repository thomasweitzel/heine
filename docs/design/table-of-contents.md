# Table of contents design

## Status

This is an accepted implementation design for a future Heine feature. Tables
of contents are not implemented yet, so this document is not part of the
current user-facing contract.

## Decision

Heine will provide an opt-in, page-owned table of contents derived from one
selected declared Markdown content input. It will expose a structural,
render-facing view and assign stable HTML fragment identifiers while rendering
that same selected source. Templates decide whether and how to display the
structure.

Heine will not infer a page body, scrape rendered HTML, generate a fixed
navigation element, or treat collections and taxonomies as documents.

## Motivation

Many documentation and article pages need navigation over their headings. A
page can, however, have several declared content inputs, structured or
segmented content, no Markdown at all, or Markdown that is not its primary
document. Treating `content.main` as a universal body would make one common
convention into an implicit framework rule.

The selected declared content name makes the relationship explicit. The
Markdown domain can then derive the headings and their anchors from one source
of truth, and the template receives data rather than a prescribed navigation
layout.

`main` remains a useful conventional name for a page's primary document, but
it has no privileged meaning to Heine.

## Authored configuration

A rendered page enables a table of contents with a complete `[toc]` table:

```toml
[content]
main = "article.md"

[toc]
content = "main"
min_level = 2
max_level = 3
```

`content` is required. It names a key in the page's `[content]` table, not a
filename. The selected declaration must name one unsegmented `.md` file.
`min_level` and `max_level` are optional integers from `1` through `6`; they
default to `1` and `6`. `min_level` must not exceed `max_level`.

The complete heading sequence receives anchors. The configured range selects
only the headings exposed through the table of contents, so changing the range
does not rename anchors.

### Directory defaults and opt-outs

`_directory.toml` may establish a recursive default for descendant rendered
pages:

```toml
[pages.toc]
content = "main"
min_level = 2
max_level = 3
```

The nearest ancestor descriptor setting is the effective directory default.
A page's own `[toc]` table replaces that default as a complete setting. Its
fields do not merge with descriptor fields. This keeps the inheritance model
small and visible: a page that changes a range repeats its selected content
name.

A page may suppress an inherited default:

```toml
toc = false
```

A nested descriptor may do the same for its subtree:

```toml
[pages]
toc = false
```

Directory settings are defaults, not irreversible policy. A descendant page
or nested descriptor may establish a new complete `[toc]` or `[pages.toc]`
table after an ancestor disables its inherited default. A `toc = false`
setting is valid only when it suppresses an otherwise effective inherited ToC
configuration. The same rule applies to `[pages] toc = false`. A disabling
setting must not become inert metadata.

## Eligibility and scope

Only a rendered authored page may have an effective ToC table. An unrendered
page may still use `toc = false` to suppress an inherited directory default.
The selected content declaration must:

- exist on that page after normal page loading;
- name a Markdown file;
- produce one plain Markdown string rather than segmented content.

An effective ToC table on an unrendered page, a missing declaration, a
non-Markdown declaration, or a segmented Markdown file is a checked
configuration error. Segmented Markdown has multiple independently rendered
values, so it has no single heading order to which one table of contents could
reliably refer.

Collections do not have ToC configuration. A collection member remains an
ordinary page and may use a ToC independently. An authored collection listing
may use its own ToC in the same way. Its generated pager tasks retain that
authored page, so they render its selected Markdown source and ToC normally.

Generated taxonomy index and term pages have no authored page or selected
Markdown source. They do not receive an automatic ToC. Their templates may
render taxonomy navigation from taxonomy data, but that is a distinct feature.

## Template contract

`page.toc` is `null` when the current authored page has no effective ToC
setting. Otherwise it is an object with these fields:

| Field      | Meaning                                                        |
|------------|----------------------------------------------------------------|
| `content`  | The selected declared content name                             |
| `headings` | A flat, source-order array of headings in the configured range |

Every entry in `page.toc.headings` has:

| Field      | Meaning                                                    |
|------------|------------------------------------------------------------|
| `level`    | Numeric Markdown heading level, from `1` through `6`       |
| `depth`    | One-based level relative to the effective minimum level    |
| `text`     | Plain heading text                                         |
| `fragment` | Stable generated HTML fragment identifier, without the `#` |

`depth` is `level - effective_min_level + 1`. It describes indentation, not a
parent-child relationship: a source may legitimately move directly from one
heading level to another. The structure is intentionally flat. Nesting,
omitted levels, list markup, labels, styling, and disclosure behavior are
presentation decisions for the template. A flat list with CSS classes such as
`toc-depth-2` is the recommended rendering approach.

Heine adds a source-aware `markdown()` template function alongside the
existing `markdown` filter:

```tera
{% if page.toc and page.toc.headings %}
  <nav aria-label="Table of contents">
    <ol>
      {% for heading in page.toc.headings %}
        <li class="toc-depth-{{ heading.depth }}">
          <a href="#{{ heading.fragment }}">{{ heading.text }}</a>
        </li>
      {% endfor %}
    </ol>
  </nav>
{% endif %}

{{ markdown(content="main") }}
```

The function reads the named declared Markdown input of the current authored
page. It checks the content name and its Markdown eligibility at the template
call site. When that name is the page's selected ToC content, it renders the
headings with the corresponding generated `id` attributes. Otherwise it
renders ordinary Markdown.

The existing filter keeps its distinct role for arbitrary template text:

```tera
{{ data.notice | markdown }}
```

Tera2 keeps function and filter registrations in separate namespaces, so both
operations use the familiar name without ambiguity in template syntax. The
function form is the supported way to render a selected ToC source. Heine does
not inspect rendered HTML to determine whether a template chose to display a
table of contents or its selected document. While rendering a ToC-enabled
page, the filter checks for an exact match with its selected raw Markdown
source. Such a match is a checked template error that directs the author to
`markdown(content="…")`; it prevents the ordinary habitual use of
`page.content.main | markdown` from silently omitting generated IDs.

Tera passes the filter a string rather than the value's authored provenance.
Heine therefore cannot reliably identify transformed copies of the selected
source. A template that transforms selected Markdown before rendering has
changed the source from which the ToC was derived and must not use that ToC.

The function is unavailable while rendering a generated resource without an
authored page, such as a taxonomy page. Calling it there is a checked template
error. A pager is not such a resource: it is a generated view of an authored
listing page and retains that page's declared content.

## Fragment identifiers

The Markdown domain derives heading text and fragment identifiers from parsed
Markdown events. It never parses rendered HTML.

For each Markdown heading, it collects plain text from text and inline-code
events, treating soft and hard line breaks as one space. Markdown formatting,
links, and image alt text contribute their textual contents. Raw inline HTML
does not contribute synthetic text. Raw HTML block headings such as
`<h2>Manual heading</h2>` are not Markdown heading events, so they are ignored
by ToC extraction and receive no generated ID.

Heine derives the base identifier as follows:

1. normalize the text to Unicode NFC;
2. apply Rust's `str::to_lowercase()` Unicode lowercase mapping without
   locale-specific transliteration;
3. retain Unicode letters and numbers, plus combining marks only after a
   retained letter, number, or mark in the same identifier run;
4. replace each maximal run of other characters with one ASCII hyphen;
5. remove leading and trailing hyphens;
6. use `section` when no retained letter or number remains.

Heine assigns identifiers in source order. The first available base identifier
is used unchanged. Later collisions receive `-2`, `-3`, and so on, checking
the complete candidate against every identifier already assigned. Thus two
`Introduction` headings become `introduction` and `introduction-2`; an
earlier literal `Introduction 2` cannot cause a duplicate identifier.

Identifiers preserve meaningful Unicode rather than transliterating it. For
example, `Überblick` becomes `überblick`. `fragment` contains the identifier
itself, not a percent-encoded URL fragment; templates use it after `#` as in
the example above. Heine's own generated identifiers are unique within the
selected document. It does not inspect trusted raw HTML or template markup
for manually authored `id` attributes.

Only headings in the selected declared Markdown source receive these generated
IDs. Headings in other Markdown values and template markup remain untouched.

## Resolution and rendering model

Source discovery and page loading resolve the effective ToC setting after
directory policy and page metadata are known. The Markdown domain parses the
selected source into a small heading and anchor plan. The resolved page retains
only the selected content name and render-facing heading facts.

During `markdown(content=...)`, the Markdown renderer uses that anchor plan
when the requested name is selected. It associates anchors with headings in
parser source order. Both the render-facing view and emitted IDs therefore use
one fragment-assignment algorithm. Heine does not create a second slugging
path for templates or inspect rendered output after the fact.

Ordinary Markdown rendering and the existing Markdown link, math, and syntax
highlighting behavior remain unchanged. The ToC representation retains heading
metadata, not pre-rendered HTML; it adds memory proportional only to headings
on opted-in pages.

## Errors and diagnostics

Authored configuration errors retain the `.page` or `_directory.toml` source
and span for the offending field. When a descriptor default fails for a
particular descendant, the diagnostic also identifies that page and relevant
content declaration.

Checked errors include:

- invalid ToC table fields, levels, or level range;
- `toc = false` with no inherited ToC to suppress;
- ToC configuration on an unrendered page;
- a selected content name that is absent, non-Markdown, or segmented;
- `markdown(content=...)` with an unknown, non-Markdown, or segmented declared
  content value;
- `markdown(content=...)` while rendering without an authored page.

The ordinary `markdown` filter reports an error at its call site when its input
is an exact match for the selected ToC source. It directs the author to the
source-aware function.

The function-specific errors point to the Tera call site. They explain whether
the requested name is absent, ineligible, or unavailable in the current
rendering context.

Duplicate headings are not errors. Their deterministic suffixes are the
defined resolution.

## Intentional boundaries

The first version does not provide:

- automatic ToC generation for every Markdown file or every page;
- a global ToC setting;
- a universal `content.main` or body convention;
- ToCs assembled from multiple content inputs;
- hierarchical ToC trees or injected navigation markup;
- automatic visible anchor-link controls beside headings;
- ToCs for generated taxonomy pages;
- raw-HTML or rendered-HTML scraping;
- automatic checks for IDs manually produced by trusted Markdown HTML or
  templates.

These limits keep a ToC tied to an explicit parsed Markdown document. A future
extension needs a concrete use case and must preserve that source-of-truth
boundary.

## Acceptance criteria

Implementation must cover at least:

- page-local ToCs, directory defaults, nested replacements, and opt-outs;
- null and non-null render-facing `page.toc` values, including an enabled ToC
  with no headings;
- all heading levels and configured ranges;
- headings with formatting, links, code, line breaks, Unicode, punctuation,
  standalone combining marks, raw HTML headings, empty usable text, duplicates,
  and collisions with generated suffixes;
- one-based `depth` values for every configured heading range;
- generated heading IDs and source-order fragment links in rendered output;
- ordinary filter rendering remaining unanchored and unchanged for arbitrary
  text, plus a checked error for an exact selected-source match;
- source-aware `markdown(content=...)` rendering with and without a selected
  ToC source;
- template diagnostics for invalid function use with useful spans;
- page and descriptor diagnostics for invalid settings and ineligible inputs;
- interaction with Markdown links, math modes, syntax highlighting,
  localization, paginated listings, and quick builds;
- complete updates to the tutorial, glossary, reference, a focused ToC guide,
  and a natural starter-site example.
