# Markdown block components

## Status

Accepted and implemented. This record defines a narrow alternative to
templating whole Markdown files.

The [rejected Tera-fragment design](markdown-template-fragments.md) explains
why a Tera program embedded in Markdown is not suitable for Heine. This record
asks whether Heine can provide one useful capability that Zola's
pre-Markdown-templating model makes convenient: a block component that wraps
authored Markdown.

## Question

Can an author place a named block component that wraps authored Markdown while
Heine still analyzes that body once as authored Markdown and keeps the resolved
Site model stable?

Zola 0.23 demonstrates that Markdown-wrapping components are useful. It
enables them by templating content before Markdown parsing. That established
use is enough reason to pursue this narrow Heine design. It does not establish
that broad content templating, arbitrary Tera programs in Markdown, or every
kind of Zola component fits Heine.

## Goal

A block component lets an author invoke a named component with checked
literal arguments and authored Markdown. Heine analyzes that body as ordinary
Markdown, then gives the component its rendered HTML only to present
around it.

When present, the body remains part of the page's Markdown document. Its
headings, links, ordinary footnotes, search text, word count, and reading time
come from the authored source, regardless of the component's
presentation. A component could add surrounding HTML and presentation, but
could not create Markdown facts or reparse the body. Page summaries remain
authored metadata; they are never derived from Markdown.

For example:

```markdown
:::component[site.note kind="warning" title="Check the migration"]
This remains ordinary **Markdown**. Its [internal link](page:guide/install)
and footnote[^migration] remain part of the page.
:::

[^migration]: Read the migration guide before publishing the new version.
```

The directive names one existing Tera2 component. Its arguments are literal
values, not Tera expressions. Its header uses
`:::component[name key = "value"]`. The component name comes first inside the
brackets. Arrays and maps remain out of scope. A string argument is
plain text, not inline Markdown, so a title argument cannot contain emphasis
or a checked link. Tera continues to validate the component's declared
parameter names and types. A component with a declared rest parameter may
intentionally accept additional named arguments under Tera's existing contract.

A directive argument remains literal, but the named component may declare the
deliberate implicit context it needs, such as `@page`, `@data`, or
`@locale_data`. The component may use ordinary Tera2 logic to select and
present that context. For example, a literal `product="starter"` can select a
corresponding entry in `data.products`, or a component can select data through
`page.extra.product`. The directive still cannot evaluate a Tera expression
itself.

Heine supplies declared implicit parameters itself. A directive must reject a
literal argument that has the name of an implicit parameter or the host-owned
`body` value. This prevents a Markdown author from shadowing `@page`, `@data`,
or another component dependency. A directive call has no template caller
context: its implicit values come only from the rendered page's deliberate
context.

## Boundaries

The feature preserves these boundaries:

- Markdown remains literal outside an explicit block directive. A code example
  containing `{{ ... }}` or `{% ... %}` remains ordinary text.
- The directive has no inline form. Its non-whitespace body is authored
  Markdown, not Tera source. An empty or whitespace-only body is a build
  error. The explicit opening and closing block distinguish the directive from
  ordinary Markdown.
- It normally uses the three-colon opening and closing fences shown above. An
  author can use `::::component[...]` with an otherwise empty `::::` closing
  fence when the body contains a literal `:::` line, following the existing
  `render` directive's fence rule.
- The directive cannot contain Tera expressions, loops, conditions, template
  includes, renderer requests, or another component directive in its first
  version.
- The named component is static. Heine can therefore check its existence,
  Tera signature, and root or selected-kit visibility before publication.
- A body may use ordinary supported Markdown. Heine assigns heading IDs and
  derives page facts in source order across the surrounding document and the
  body. Link reference definitions may occur inside or outside the body and
  resolve across that boundary. Ordinary and margin-footnote definitions must
  remain outside the body, where their current placement and backlink behavior
  remain intact. Footnote numbering remains page-wide.
- The component itself adds no text to Markdown-derived facts. Its argument
  values and surrounding HTML do not enter search text, summaries, word
  counts, reading time, or the table of contents.
- The component receives the rendered body as trusted HTML through Tera's
  component-body mechanism. The pinned Tera version constructs a host-supplied
  body as a safe value, which regression tests verify. A component can
  therefore use `{{ body }}` without asking an author to apply `safe`. Heine
  calls Tera's component renderer with autoescaping enabled, independent of
  the defining template's filename. This does not sanitize raw HTML authored
  in the body, because trusted Markdown already permits raw HTML.

The directive must be a structural Markdown construct rather than a string
replacement. Heine needs a source-aware document plan that records the
component call and keeps the body as a normal Markdown child. Rendering then
follows this order:

```text
authored inputs
    -> resolved Site relationships
    -> checked document plan and Markdown-derived facts
    -> render planned Markdown body slices to HTML from that shared analysis
    -> render the named Tera component around that HTML
    -> page-template rendering and checked output publication
```

Heine analyzes the complete source once. Rendering later emits each planned
body slice from that shared analysis, including its resolved links, heading
IDs, and footnote numbering. It does not parse Markdown a second time. A
component can read only the same deliberate render context that an ordinary
Tera component can read. A directive cannot inject values from a
template-local loop or assignment.

## Performance boundary

The document plan may retain spans and component-call facts, but it must not
copy or reparse the complete Markdown document for every component. Rendering
a component necessarily holds its rendered body and wrapper result while it
constructs the page output. Those bounded values are acceptable. Repeated
whole-document parsing or copying is not.

Before Heine accepts a public contract, benchmarks must compare a
no-component baseline with pages that contain one small component and several
large component bodies. The benchmark should use a representative multi-page
site and report both elapsed build time and peak memory. This establishes the
cost of the structural integration rather than assuming that the feature is
free.

## What prior art teaches

[Hugo shortcodes](https://gohugo.io/content-management/shortcodes/) expose two
notations with different timing. Its Markdown notation runs before Markdown
rendering, so its headings can join the table of contents. Its standard
notation runs separately after Markdown rendering, so they cannot. Hugo's
shortcode documentation also requires authors to choose how to render a
shortcode body and warns that raw HTML-block indentation and blank-line rules
matter.

That flexibility is useful, but it makes the same-looking authoring construct
mean different things at different phases. Heine should not expose two
component notations or ask authors to choose a Markdown-rendering phase. A
block component has one meaning: its body is always analyzed as authored
Markdown before its surrounding component renders.

[Markdoc](https://markdoc.dev/docs/overview) takes a closer approach. It
parses declared tags into a document structure with typed attributes and
validated children rather than mixing arbitrary code into content.
[MyST](https://mystmd.org/spec/overview) likewise parses directives as
structural nodes whose bodies can become parsed document children before later
transforms render them. These systems support much larger extension models,
but they establish the useful principle here: a block component is a document
node, not a template expansion.

## Component contract

[Tera 2](https://keats.github.io/tera/#components) already supports rendering
a component by a known name with typed arguments, explicit implicit context,
and an optional body. This makes a host-controlled integration practical
without generating Tera source from Markdown.

Components remain hygienic. A component receives only its declared arguments,
its body, and the implicit render context that it explicitly declares. Heine
does not add directive values to Tera's global context. That keeps a
component's data dependencies visible in its template contract.

Every directive invocation has one rendered body value. Tera does not declare
whether a component expects a body, so Heine must reject an empty or
whitespace-only directive before it calls Tera.

Heine needs one small, reserved component-metadata declaration for the
exceptional case. A component keeps its body visible when it omits metadata.
One that may hide its body declares the flat key `"heine.body_may_hide"` with
the literal Boolean value `true`. Any other value, including `false`, is an
error at the directive. Regression tests verify that Tera exposes literal
metadata on a component definition to its host. The declaration is a trusted
statement of the component's intended presentation, not proof of what later
site CSS does.

Heine keeps that declaration with the component rather than adding a
separate manifest or naming convention. The existing kit declaration answers
which components content may name; it does not describe their presentation.
Reusing it for a second responsibility would make both contracts less clear.
Tera metadata provides the required stable host-visible declaration.

A component must preserve the rendered body exactly once in its result. Heine
can check this at render time with a private marker pair around the
host-supplied body, then remove the pair before publication. This avoids a
static analysis of Tera control flow while still rejecting an omitted,
duplicated, or altered body. The result must contain exactly one intact marker
pair, and the bytes between that pair must exactly equal the rendered body that
Heine supplied. A filter that strips, truncates, or rewrites either the marker
or body is a build error. The marker must survive ordinary HTML autoescaping
unchanged.

The production document plan, not a textual search and replacement, identifies
where the component result belongs. The marker pair protects only the body
value while Tera renders the component. Heine chooses a deterministic reserved
pair after rendering the body and verifies that the pair is absent from those
bytes. If a trusted component template emits the same pair itself, the count
check fails. This is an integrity check for Heine's own integration, not a
security boundary against a malicious template.

That check proves one preserved body serialization, not that CSS makes the
body visible, accessible, or structurally useful. A trusted component template
still owns those presentation choices. The feature supplies a body rendered
as HTML, so it is suitable only where the surrounding template consumes that
HTML. A future component contract that supplies XML or another representation
would need its own rendering and preservation rules. Heine does not parse this
component output as HTML or judge where the body appears. The check only
prevents the component integration itself from mechanically omitting,
duplicating, or rewriting authored headings, links, footnotes, and their
generated IDs.

Each directive runs once for its owning page and source-aware Markdown render.
Its `@page` value names that rendered page, not a transient template that later
embeds its output. Rendering the same Markdown source for another page creates
another owning context and another component call.

## Resources and page head

The component name is known while Heine analyzes Markdown, unlike the dynamic
component calls in the rejected Tera-fragment design. A later feature could
use that fact to model checked component-owned CSS or JavaScript resources.

That resource contract is not part of this feature. A block
component must work with resources that the page template or site already
loads. It must not let a component mutate the page head during body rendering.
A future resource declaration needs its own design for ownership, ordering,
deduplication, kit visibility, and output relationships.

Tera component metadata already provides a host-visible literal map. This
feature reserves the flat `heine.body_may_hide` key for its own checked
contract. A future resource contract could use the same component-owned
channel rather than a second manifest, but it needs separate keys and
semantics.

## Limits and interactions

The public contract has these limits:

- **Placement and parsing:** A component directive is valid only at an ordinary top-level
  block boundary. It is inert in code fences and rejected inside paragraphs,
  headings, lists, block quotes, tables, and footnote definitions. The body
  itself may contain ordinary supported Markdown blocks. The public syntax
  must define its opening and closing fences, fence-length rules, indentation,
  and the diagnostic for directive-looking text in an unsupported context.
  Notes inside a numbered step or a quotation are common documentation cases,
  so the list and block-quote exclusions are deliberate first-version limits,
  not claims that those uses lack value.
- **Nested components:** The first version rejects a component directive inside
  another component body. Nested components need a recursive source-aware plan and
  must earn their added complexity through a later use case.
- **Source identity:** Component directives occur only in unsegmented declared
  Markdown rendered through `markdown(content="…")`. A page cannot initially
  combine segments and block components. Arbitrary template strings, data
  values, summaries, and segmented inputs do not recognize directives. They
  must diagnose a directive-looking block rather than silently render it as
  ordinary text. A directive whose opening and closing fences fall in different
  segments must identify both fences and explain that a component cannot cross
  a segment boundary. Supporting components in segments is an open source-order
  and source-aware-rendering question for a later proposal. The rejected
  Tera-fragment record recommends segments for other composition needs, so it
  must also link to this limit.
- **Table of contents and headings:** A heading in the body remains an authored
  heading. The component cannot allocate headings of its own. A component that
  declares `heine.body_may_hide = true` cannot wrap a body containing a
  heading, because a table-of-contents or search-result anchor cannot open a
  closed disclosure or similar container. This declaration cannot prove what
  site CSS does later, but it makes the component's own intended presentation
  a checked contract.
- **Ordinary and margin footnotes:** The design must prove one Markdown pass:
  link reference definitions outside a body resolve inside it and vice versa.
  Ordinary and margin-footnote definitions inside a body are a build error, so
  their current page-level placement and backlinks remain unchanged. Footnote
  numbering remains in source order across the boundary, and heading IDs remain
  unique before, inside, and after it. A source selected for margin-footnote
  presentation may contain a component body because the same source-aware
  rendering path preserves the local note structure.
- **Summaries, feeds, and search:** The body, but not the component markup or
  arguments, must remain the source of search text. Summaries and feeds keep
  their existing authored-summary contract. A component title and its literal
  arguments do not enter search text or the table of contents.
- **Kits:** Root content may name only root components or components that the
  selected kit declares public.
- **Diagnostics:** An unknown component, malformed literal argument, invalid
  component argument, or invalid placement must identify the directive in the
  Markdown source. A component rendering error must retain both the component
  location and that call site. When a component treats a generic string
  argument as a page or asset ID, a failed lookup identifies the lookup in the
  component as its primary location and the directive call as related context.
  Heine does not initially add provenance-carrying template values or semantic
  component argument types merely to point that failure at one literal
  argument. A repeated need for source-precise page or asset arguments would
  need a separate argument-contract design.
- **Pre-publication execution:** Heine must execute every recognized component
  in its owning page and locale context, even when no current page template
  renders that source. This is how Tera argument and component-rendering
  failures remain build errors rather than hidden content defects.
- **Incremental builds:** A full build accounts for every component template
  and implicit context dependency before it publishes output. `--quick` stays
  intentionally incomplete: it does not promise to discover component-template
  changes or indirect dependencies through `@data` and other implicit context.
  A later dependency-aware build design would need to make those relationships
  explicit.

## Non-goals

This feature does not introduce:

- Tera templating for Markdown files;
- expressions or control flow in Markdown directives;
- generated Markdown that changes the document model;
- inline components;
- bodyless components;
- a generic directive registry or plugin mechanism;
- head-resource injection during body rendering; or
- a replacement for declared-content segments and page templates when they
  remain the clearer composition mechanism.

Adding Tera expressions to directive arguments or body content would reopen
the rejected Tera-fragment decision.

## What it does not replace

A block component provides reusable presentation at an authored block
boundary. It does not make Markdown a second template language.

An author therefore cannot interpolate a page or data value into an ordinary
Markdown sentence, conditionally include arbitrary surrounding paragraphs, or
choose a component name from an expression. A component may use its declared
arguments and explicit implicit context to make those choices inside its own
presentation, but the directive remains static.

Likewise, a component may render data-selected text as final HTML, but that
text does not become searchable Markdown, a table-of-contents entry, a
footnote relationship, or a word-count contribution. Use a page template and
declared-content segments when generated material belongs to the document
model.

Bodyless components are not part of this feature. A site that needs one can
use trusted raw HTML or a template component between declared-content segments.
If real evidence later justifies author-positioned bodyless components, an
empty block can gain that meaning or Heine can add a self-closing form. Empty
blocks are errors in this feature, so either extension would remain
backwards-compatible.

## Use cases and implementation evidence

Zola establishes that Markdown-body components solve a recurring authoring
need. The cases below identify the part Heine should support: a reusable
presentation around authored Markdown that raw HTML or segments plus a
template component express poorly.

| Case                                                           | Current approach                                         | Why it may be insufficient                                                                                                                             |
|----------------------------------------------------------------|----------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------|
| Note or disclosure with links, a list, or a footnote reference | Ordinary Markdown, raw HTML, or a page-specific template | Raw HTML loses Markdown structure. A template moves the surrounding prose and its order out of the source.                                             |
| Reusable figure with a Markdown caption or explanation         | `:::picture`, raw HTML, or a page-specific template      | The existing picture directive remains better for an image-only case. The feature matters only when the authored body has ordinary Markdown structure. |

These cases guide the public examples and acceptance tests. Zola establishes
that this authoring need recurs, while the cases identify the narrow part that
fits Heine.

The implementation demonstrates that Heine can:

1. recognize directives without weakening ordinary Markdown or code fences;
2. preserve source order and source spans while it analyzes the component body
   with surrounding Markdown;
3. call the static Tera component through the existing typed component API,
   with HTML autoescaping enabled, passing rendered body HTML safely and
   preserving kit visibility;
4. keep component markup and argument values out of Markdown-derived facts;
5. read a component's optional `heine.body_may_hide = true` declaration and
   reject an empty or whitespace-only body;
6. reject a literal directive argument that would shadow a declared implicit
   component parameter or the host-owned `body` value;
7. require a component to preserve its marked rendered body exactly once and
   byte-for-byte unchanged;
8. keep directives out of segmented sources;
9. render one complete source across component boundaries, including link
   reference definitions in either direction, unique heading IDs, and ordinary
   and selected margin-footnote numbering; and
10. reject ordinary and margin-footnote definitions inside a body, and reject
    a heading-bearing body for a component that declares it may hide its body.

The body-preservation check must show that an altered marker or body, including
one passed through a destructive filter, fails. It does not determine whether
the component places the body in visible element content, an attribute, a
comment, or another presentation-specific location. That remains the
responsibility of the trusted component template.

## Implementation

Heine masks directive fences without changing source offsets, then analyzes
the complete remaining Markdown document. The page view retains the resulting
component plan. Before the outer page template renders, Heine renders every
component-bearing declared source with the page's deliberate Tera context and
stores that prepared result for `markdown(content="…")`.

The ordinary renderer replaces planned event ranges before it serializes the
document. The margin-footnote renderer captures the same ranges while it still
processes each footnote-reference event, so a local note inside a component
remains local to that component. Neither path reparses a component body or
rewrites finished page HTML.

Regression coverage includes literal code fences, empty bodies, longer fences,
heading IDs, checked links, component metadata, implicit-argument shadowing,
body-integrity failures, source diagnostics, and ordinary and margin-footnote
rendering.

## Future questions

The first version does not support nested components, segments, placement
inside lists or block quotes, or components
with several authored parts, such as tabs, step walkthroughs, or card grids.
The feature does not establish a component-resource contract, richer literal
argument types, or a bodyless form. Each needs a separate concrete use case
before Heine expands this contract.

## Alternatives

### Template every Markdown file

Zola's pre-Markdown templating gives the broadest flexibility, including
components with Markdown bodies. It also makes Tera syntax special throughout
content and prevents content templates from reading facts that require the
complete resolved site model. Heine rejects that order.

### Opaque Tera fragments

The [rejected fragment design](markdown-template-fragments.md) would have run
Tera after Markdown analysis and inserted final HTML. It preserved the Site
model, but the generated HTML could not participate in Markdown facts. A
block component reverses that relationship: authored body Markdown participates
in the facts, while the component only presents its final HTML.

### Built-in presentation directives

Heine could add built-in directives for notes, disclosures, or captions
instead of exposing components. That would make Heine own their presentation,
styling, arguments, and future variations. These concerns belong to a site or
kit, which already owns Tera components and their public visibility boundary.
Heine therefore reuses that boundary while keeping the Markdown
directive structural and checked. Built-in directives would give Heine tighter
control over body placement and presentation, so they would not need this
component metadata and integrity machinery. The generic design accepts those
weaker presentation guarantees because sites and kits need to own the result.

### Raw HTML, segments, and page templates

These remain the right choice for a static element, a fixed arrangement of
prose, or a data-driven section that belongs to the template. A component earns
its complexity only when its Markdown body is the relationship
that those tools cannot express cleanly.

## Prior art

[Zola 0.23](https://www.getzola.org/documentation/content/overview/) uses
Tera components in content by templating the entire body before Markdown
parsing. [Markdoc](https://markdoc.dev/docs/overview) and
[MyST](https://mystmd.org/spec/overview) treat directives as structural
document nodes. Heine borrows the structural-directive idea
while retaining Heine's model-first rendering order.
