# Tera fragments in Markdown

## Status

Rejected for now. This record preserves the investigated boundary and the
reason Heine does not add it. It adds no Markdown syntax, template value, or
rendering behavior.

## Decision

Heine will not add Tera fragments in Markdown now.

An investigated design could have preserved the resolved Site model by
treating its output as opaque HTML. That property is necessary, but it does
not justify a new content language. Heine already gives pages deliberate
composition through declared content, segments, structured page data,
page-specific templates, and Tera2 components. Those tools keep prose in
Markdown while templates own presentation and dynamic structure.

The candidate's unique benefit is an arbitrary Tera program at one authored
position between Markdown blocks. That convenience does not outweigh its
costs:

- Its HTML cannot take part in Markdown link checks, a table of contents,
  search, word counts, reading time, or margin footnotes.
- Its literal HTML URLs and IDs remain unchecked, like authored raw HTML.
- It makes presentation logic available in content, where Heine otherwise
  treats Markdown as prose.
- A page template can decide which components add CSS or JavaScript to the
  page head before it renders the body. A fragment runs later, so a component
  called from it cannot make that declaration. Solving that asymmetry would
  need a separate checked component-resource contract, not a render-time side
  effect.

The proposal therefore does not meet Heine's feature bar. A feature must
express a concrete checked relationship that existing typed primitives and
external tools cannot express cleanly. This candidate overlaps substantially
with existing composition while introducing another authoring surface with
weaker checks.

## Comparison with Zola 0.23

[Zola 0.23](https://github.com/getzola/zola/blob/master/CHANGELOG.md) removed
shortcodes and made page and section bodies available to Tera before it parses
them as Markdown. Its Tera components can therefore render at an author-chosen
location and accept a Markdown body. This is a useful capability.

That order has a cost. Zola documents that content containing literal Tera
syntax needs a `raw` block, unless a configuration pattern disables content
templating for that file. It also documents that page and section fields which
depend on other pages are empty while it templates content. See Zola's
[content overview](https://www.getzola.org/documentation/content/overview/).

Heine instead analyzes authored Markdown and resolves the Site model before
templates render. Running Tera first would reverse that order: a page's body
could not read its own table of contents, word count, or reading time, and
cross-page facts would need an additional ordering or cycle contract. A second
Markdown analysis could restore those facts, but then Tera output would need
rules for Markdown escaping, whitespace, and raw-HTML block behavior.

Zola's approach therefore gives generated headings, links, footnotes, and
Markdown block components a place in Markdown analysis. Heine keeps authored
Markdown literal and its resolved facts stable instead. The rejected
HTML-fragment candidate preserved that stability, but its output could not
join the Markdown document model. Neither trade-off is universally better;
Heine chooses the one that fits its checked model.

## What to use instead

An ordinary page body uses one unsegmented Markdown file. When a page needs
prose at several template locations, [declared-content segments](../declared-content.md)
keep that prose in one file while the template places each named part.
Independently authored documents can instead use several declared content
files. A page-specific template can render a data-driven list or component
between those parts, using page-owned structured data where needed.

This covers the cases that motivated the proposal:

- Use trusted raw HTML for one static local element, such as a native `audio`
  control.
- Use a template component between named prose segments for an audio-player
  panel that needs data or reusable presentation.
- Use a template loop between named prose segments for a data-driven list or
  table.

Markdown block components do not initially combine with segments. See their
[source-identity limit](markdown-block-components.md#limits-and-interactions)
before choosing segments as a workaround for a component-bearing page.

Heine offers a [Markdown block component](markdown-block-components.md) for a
note, disclosure, or figure whose body contains Markdown links, lists, or
footnotes. It is deliberately narrower than a Tera fragment: the body remains
authored Markdown, and the component only presents its rendered HTML.

This approach makes ordering visible in the template. Authors who only edit
Markdown cannot reposition a dynamic block themselves, and a one-off layout
may need a page-specific template. Heine accepts that cost because the
template then continues to own document structure and presentation.

## Why not named slots

A Markdown slot such as `:::slot[player]` appears smaller than a Tera
fragment, but segments already provide the template-directed placement model:
one Markdown source names prose parts and the template decides where each part
renders.

Slots would add a competing page-composition system. Heine would need to
define who declares slots, how templates fill them, which values they receive,
whether a slot repeats, how missing or unused slots fail, and how root and kit
component boundaries apply. Those are responsibilities that declared content
and templates already hold.

## When to reconsider

This decision is evidence-based, not permanent. A new proposal needs concrete
evidence that existing composition is not enough, for example:

- several independent sites need page-specific templates only to place one
  dynamic final-HTML island among otherwise ordinary prose;
- a block component establishes a concrete checked relationship that existing
  composition cannot express cleanly; or
- generated material must join the table of contents, search index, or other
  Markdown-derived facts and establishes a narrow checked structural-document
  contract.

The second and third cases do not automatically revive Tera fragments. A block
component and a generated structural document each need their own design
because they have different analysis and checking requirements. In particular,
a component with an authored body must keep that body as authored Markdown in
Heine's single analysis pass. It must not template the whole body before
Markdown analysis.

The [Markdown block component](markdown-block-components.md) feature is the
separate, narrow answer to this second case. Its static component name,
literal arguments, and source-aware body are materially narrower than an
arbitrary Tera program in Markdown. It does not revive Tera fragments or
weaken this decision.

## Appendix: investigated HTML-fragment design

The candidate used a block directive named `tera` with a variable-length
colon fence. Its body was Tera2 source and would have produced final HTML:

````markdown
::::tera
<nav aria-label="{{ t(id="recent-posts") }}">
  <!-- Tera control flow and checked functions would go here. -->
</nav>
::::
````

Heine would have analyzed authored Markdown first, recorded the directive in a
private structural plan, and inserted a block placeholder. During page
rendering, it would have replaced that placeholder with the fragment's
HTML-autoescaped output. The placeholder, directive body, and output would
not have appeared in page views, summaries, feeds, or search data.

This ordering would have kept the Site model stable. Fragment output would
have remained opaque to Markdown: it could not create headings, checked links,
margin footnotes, search text, or another `tera` or `render` directive. Each
fragment would have had its own variable scope. The `markdown()` function,
filesystem access, network access, clocks, randomness, shells, and processes
would have remained unavailable.

The design is technically safe enough to preserve as a reference. It is not a
suitable fit for Heine now because existing composition covers its useful
cases with stronger ownership and checks.

## Prior art

[Markdoc](https://markdoc.dev/docs/overview) treats extensions as parsed
document nodes with declared attributes, allowed children, and validation.
[MyST](https://mystmd.org/spec/overview) likewise separates block directives
from inline roles. These systems support broader document-extension models
than Heine needs today.
