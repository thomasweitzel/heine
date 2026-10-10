# Margin footnotes design

## Status

Implemented. The focused [margin-footnotes guide](../margin-footnotes.md) and
[reference](../reference.md) are the current user-facing contract. This record
retains the design rationale and accepted boundaries.

## Decision

Heine will provide an opt-in, page-owned **margin-footnote presentation** for
ordinary Markdown footnotes. The author continues to use the standard
reference and definition syntax enabled by the existing `footnotes` Markdown
extension. Heine checks the selected source's footnote graph and renders every
reference as a local note instance.

On a site with suitable CSS, activating a numbered marker reveals its note in a
margin lane beside the containing prose block when room is available, and
below that block otherwise. The interaction uses native HTML controls and CSS,
not JavaScript or URL fragments. Without CSS, the note content remains visible
in source order.

Heine owns the source relationship and semantic structure. A site or kit owns
the margin geometry, typography, colors, and responsive breakpoints.

## Motivation

Footnotes support citations, qualifications, translations, and dry
editorial comments. An end-of-document note section interrupts that local
reading relationship: a reader follows a marker away from the relevant prose,
then must find their way back.

Raw HTML, split declared-content inputs, and template components can imitate a
margin note, but none expresses the relationship in Markdown. They either make
ordinary prose awkward to author or leave the connection between text and note
unchecked. Standard Markdown footnotes already provide one familiar source
concept. Margin presentation should reuse it rather than create a competing
note syntax.

## Authored model and inheritance

Margin presentation selects one declared, unsegmented Markdown content input
on a rendered authored page:

```toml
# content/en/guide/chronology.page
template = "guide.tera"

[content]
main = "main.md"

[footnotes]
content = "main"
```

The existing parser extension remains an explicit site-wide choice:

```toml
[markdown]
extensions = ["footnotes"]

[markdown.margin_footnotes]
marker_message = "footnote-marker"
```

An effective `[footnotes]` table requires the `footnotes` extension. It
selects a declared unsegmented Markdown document, not a filename or an
implicit page body. When any page has an effective setting,
`markdown.margin_footnotes.marker_message` is required. It names a Fluent
message that receives the numeric `$number` argument and becomes the localized
accessible name of every local footnote control. For example:

```ftl
# locales/en/site.ftl
footnote-marker = Footnote { $number }

# locales/de/site.ftl
footnote-marker = Fußnote { $number }
```

Heine resolves the message for the page locale through the ordinary Fluent
fallback chain and checks it as an authored relationship. A missing,
unformattable, or empty message reports the global setting that named it and
the relevant Fluent source when one exists. A
`[markdown.margin_footnotes]` table without any effective page setting is a
checked configuration error. If the content name is missing, non-Markdown,
segmented, or belongs to a page without a template, Heine likewise reports an
error at the authored setting.

`marker_message` is ordinary root-owned configuration and uses the normal
Fluent lookup rules. A future kit reference site may select a prefixed kit
message, such as `paper-footnote-marker`; a root resource may override that
entry under the ordinary kit message rules. Margin footnotes add no separate
kit message namespace or precedence rule.

Directories may provide the same complete default:

```toml
# content/en/guide/_directory.toml
[pages.footnotes]
content = "main"
```

The nearest complete table wins. Tables do not merge. A page or nested
descriptor may use `footnotes = false` only to suppress an otherwise effective
inherited setting. This avoids inactive configuration accumulating after a
directory default is removed. Using `footnotes = false` without an inherited
effective setting is a checked error at that field, rather than an advisory
warning: ineffective structural policy is an authored mistake that should be
removed, not a dormant fallback to preserve.

No footnote reference is required in a selected source. This permits a
directory default for pages that happen not to need notes.

## Template contract

The selected source must be rendered exactly once in each output document
through the existing source-aware function:

```tera
{% if page.footnotes %}
  <article class="has-margin-footnotes">
    {{ markdown(content=page.footnotes.content) }}
  </article>
{% endif %}
```

`page.footnotes` is `null` without an effective setting. Otherwise it exposes
only the selected declared content name:

```text
{ content: "main" }
```

The view lets a template opt into a surrounding layout class without exposing
pre-rendered note HTML or letting templates reconstruct source relationships.

After rendering, Heine checks the selected source's invocation count. Omitting
it leaves an effective setting without its promised rendered structure, while
rendering it twice duplicates generated identifiers. Both are template errors.
The same one-render rule applies to a source selected by a table of contents.
If the two features select the same source, their shared call satisfies both
requirements.

The ordinary `markdown` filter remains appropriate for arbitrary trusted
template text. If it receives the exact selected source, Heine reports a
template error at that call site. A string filter has no declared-content
identity, so it cannot apply the precomputed footnote plan reliably. This is
the same source-aware rendering boundary used by the table of contents.

Other Markdown features retain their own contracts. Search extracts the
original source-level text, including definition text, and never receives a
margin-presented HTML variant. Atom feeds use their authored summaries rather
than page-body Markdown. No other source-aware HTML renderer exists.

An effective table of contents and margin-footnote setting may select the same
Markdown document. A single `markdown(content="…")` call applies both heading
anchors and local footnote structure. A footnote marker never contributes to a
heading's ToC text or fragment identifier. Headings inside a footnote
definition are ancillary note content: they neither contribute to `page.toc`
nor receive a generated ToC anchor.

## Footnote graph and numbering

Authors use ordinary Markdown footnotes:

```markdown
Heine rejects ambiguous local times.[^time]

[^time]: During an autumn daylight-saving transition, the same wall-clock time
can name two distinct instants.
```

For the selected source, Heine records each footnote reference and definition
from the parsed Markdown event stream. It checks that:

- every reference has exactly one definition;
- every definition has at least one reference;
- definitions are unique; and
- a margin-footnote reference occurs in an eligible prose block.

A failure reports the relevant Markdown source span. Where a relationship has
two authored locations, the diagnostic names both. Heine follows the parser's
footnote-label comparison rules rather than inventing a second normalization
rule.

Numbers follow the source order of first references. A repeated reference to
the same definition retains its number. It nevertheless receives a distinct
local note instance at each location, so activating it never sends a reader to
an earlier paragraph or to an end-of-document section.

The selected source does not receive pulldown-cmark's ordinary endnote section.
Its definition body is rendered once for each local instance. This duplication
keeps each note beside the prose it qualifies rather than at one shared
destination elsewhere in the document. Definition
bodies retain the ordinary checked Markdown transformations, including internal
links, asset references, math, and syntax highlighting.

## Eligible prose blocks

The first version supports a reference in an ordinary Markdown paragraph or
Markdown heading. Normal non-interactive inline formatting, such as emphasis,
strong text, and strikethrough, remains eligible. A reference immediately
after inline code is likewise eligible, although footnote syntax inside inline
code is literal text. A marker remains directly after the relevant word or
punctuation. A heading retains any Heine-generated fragment identifier; its
generated control and note follow the heading they annotate in the DOM.

References in table cells, list items, block quotes, code blocks, HTML, links,
or images are errors for a selected margin-footnote source. A reference inside
a footnote definition is likewise an error in the first version. Raw HTML is
trusted Markdown, but its arbitrary nesting cannot be reconstructed safely
from Markdown events. Therefore a paragraph or heading that contains both raw
HTML and a reference is rejected. These contexts need container-specific output
rules to remain valid HTML and to keep a local note's placement understandable.
Ordinary Markdown footnotes outside a selected source retain their existing
rendering.

This boundary covers article and documentation prose, which is the motivating
use case, without pretending that every
Markdown location has a page margin. A later extension requires a
concrete use case and a complete structural rendering rule.

## Rendered structure and progressive presentation

For each eligible prose block with one or more references, Heine emits one
source-ordered wrapper. It contains the paragraph or heading, followed by an
independent native checkbox and note aside for each local instance:

```html
<div class="heine-annotated-block">
  <p>
    Heine rejects ambiguous local times.
    <sup class="heine-footnote-marker">
      <label for="__heine-footnote-toggle-1">1</label>
    </sup>
  </p>

  <input
    class="heine-footnote-toggle"
    id="__heine-footnote-toggle-1"
    type="checkbox"
    aria-label="Footnote 1"
    aria-controls="__heine-footnote-note-1"
    aria-describedby="__heine-footnote-note-1"
  >
  <aside class="heine-footnote" id="__heine-footnote-note-1" role="doc-footnote">
    <span class="heine-footnote-number" aria-hidden="true">1</span>
    <p>During an autumn daylight-saving transition, the same wall-clock time
    can name two distinct instants.</p>
  </aside>
</div>
```

The visible numeric label remains the marker's click target. A matching number
at the start of each note is visual association only, so it is hidden from
assistive technology. Heine obtains the checkbox's `aria-label` from the
site-wide `marker_message`, passing the displayed number as `$number`;
`aria-describedby` supplies the associated note text. Assistive technology
therefore receives the note body when it encounters the control, even when CSS
has visually collapsed that note. Note content is never gated behind a
discoverability step for non-visual readers. The checkbox's
checked state truthfully describes whether its local note is shown.
`aria-controls` additionally declares the control-to-note relationship for
assistive technology that recognizes it; it does not imply a JavaScript
runtime. The marker is a disclosure control, not a link: activating it changes
nearby content and does not navigate the document. A site or kit may visually
hide the checkbox only with a visible focus treatment for the real native
control. Styling its corresponding marker is an optional progressive
enhancement, not a guarantee of the generated structure. The control must
remain available to keyboard and assistive-technology users. The `aside` uses
the publishing-specific `doc-footnote` role for ancillary note content within
a work. Heine HTML-escapes the formatted Fluent value before placing it in the
attribute.

The exact generated IDs are implementation details. Ordinary and margin
footnotes share the reserved `__heine-footnote-…` prefix, which stays separate
from generated heading fragments and gives each generated control, note, and
ordinary definition a distinct role-specific ID. As with generated ToC anchors,
Heine does not inspect trusted template or raw HTML markup for manually
authored `id` collisions. The stable class names above are the styling contract
for sites and kits.

An identifier represents the local reference instance, not the displayed
footnote number. For example, two references to definition number `1` receive
different control and note IDs, while both visible markers remain `1` and both
localized control labels receive `$number = 1`. The two controls have the same
accessible name because they identify the same logical note,
just as repeated printed footnote markers do. Their distinct positions and
separate `aria-describedby` targets retain the local context without adding an
otherwise artificial occurrence number.

The default semantic output leaves every note visible. A site or kit may add
CSS such as:

```css
.heine-footnote-toggle:checked + .heine-footnote {
  display: block;
}

.heine-footnote {
  display: none;
}

@supports selector(.heine-annotated-block:has(.heine-footnote-toggle:focus-visible)) {
  .heine-footnote-toggle {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .heine-annotated-block:has(.heine-footnote-toggle:focus-visible)
    .heine-footnote-marker label {
    outline: 2px solid currentcolor;
    outline-offset: 2px;
  }
}

@supports selector(.heine-annotated-block:has(.heine-footnote-toggle:focus-visible)) {
  @media (min-width: 72rem) {
    .heine-annotated-block {
      display: grid;
      grid-template-columns: minmax(0, 44rem) minmax(14rem, 20rem);
      column-gap: 2rem;
    }

    .heine-annotated-block > p,
    .heine-annotated-block > h1,
    .heine-annotated-block > h2,
    .heine-annotated-block > h3,
    .heine-annotated-block > h4,
    .heine-annotated-block > h5,
    .heine-annotated-block > h6 {
      grid-column: 1;
    }

    .heine-footnote-toggle:checked + .heine-footnote {
      grid-column: 2;
    }
  }
}

@media print {
  .heine-annotated-block {
    display: block;
  }

  .heine-footnote-toggle {
    display: none;
  }

  .heine-footnote {
    display: block !important;
  }
}
```

The example is illustrative, not generated or required Heine CSS. Its order
matters: the note is hidden until its own marker selects it. At a wide
breakpoint it appears in the second grid column; otherwise it expands below
the relevant prose. Several notes in one block may remain expanded together.
Grid layout prevents expanded notes from overlapping later prose, but a tall
note can make its row taller than the associated paragraph and leave visible
space before the next block. That is the CSS-only trade-off for local
placement. A site or kit may choose a denser responsive layout,
including below-prose notes, but Heine does not attempt note-collision
avoidance.
For printing, the grid is removed and every note follows its annotated prose
in ordinary document flow. This makes a printed page or PDF readable without
depending on a page-margin layout. It does not promise that a long note and
its marker stay on the same physical printed page: forcing that constraint can
produce more disruptive blank space.
The focus enhancement is guarded: a browser without `:has()` keeps the native
checkbox visible and retains the below-prose disclosure layout, so keyboard
focus remains apparent and note disclosure still works.

No JavaScript, URL-fragment mutation, browser history entry, or distant scroll
is involved. Without the CSS, controls and their note content remain in logical
source order. Starter-site CSS must use a standard visually-hidden technique
for the controls, preserve visible focus indicators on markers, respect reduced
motion, and leave notes visible for printing.

## Resolution, performance, and builds

Page loading resolves the effective setting after descriptor inheritance. The
Markdown domain builds a compact footnote plan from the selected source:
definition locations, first-reference numbering, and local-reference facts.
It retains source ranges rather than pre-rendered HTML. Rendering parses the
selected source once with that plan, suppresses definition blocks from normal
flow, and renders their bodies into each local instance. The render boundary
records the selected-content invocation count before the completed output is
accepted.

Memory remains proportional to the selected Markdown source and its footnote
facts. Repeated references repeat only their rendered note output, which is a
direct consequence of preserving local reading. Heine does not scrape its own
HTML or perform a second source-to-HTML conversion after rendering.

Changing selected Markdown content re-renders its page in the existing quick
build model. Enabling, disabling, moving, or changing a page or descriptor
footnote setting is a relationship or configuration change and therefore
requires a full build. The development server's normal complete rebuild makes
such changes visible.

## Diagnostics

Configuration errors identify the `.page` or `_directory.toml` setting and
offer the appropriate remedy, such as enabling the Markdown extension,
selecting a declared unsegmented Markdown input, or suppressing an inherited
setting with `footnotes = false`. A missing global marker-message setting is
reported at the effective page or descriptor setting that requires it. An
invalid configured message identifies `markdown.margin_footnotes.marker_message`
and the related Fluent source when one exists. A suppression with nothing
inherited to suppress is likewise reported at its authored setting.

Footnote-graph and placement errors identify the original Markdown source.
Missing definitions name the reference. Duplicate definitions name the later
definition and related earlier definition. An unused definition identifies its
definition. Unsupported placement identifies the reference, explains that a
margin footnote needs a valid locally placeable prose wrapper, and names the
supported paragraph and heading contexts.

Template errors identify an invalid `markdown(content="…")` call, an omitted or
repeated selected-content rendering, or the accidental raw-filter rendering of
the selected source. No diagnostic depends on a rendered HTML position.

## Boundaries

The first version does not provide:

- a new Markdown note syntax or a generic directive language;
- a JavaScript runtime, browser-history behavior, or animated disclosure;
- automatic margin presentation for every Markdown document;
- notes in all possible Markdown structures;
- multiple independently selected Markdown sources, each with its own local
  footnote stream, on one page;
- a global footnote counter across several separately rendered content inputs;
- generated CSS, a required page sidebar, or a fixed breakpoint; or
- checks for `id` values authored in trusted template or raw HTML markup; or
- legal, citation, scholarly-style, or bibliographic interpretation of note
  content.

The ordinary endnote rendering remains available for every Markdown value not
selected by an effective `[footnotes]` setting.

## Rejected alternatives

### A separate `:::aside` Markdown directive

An aside block alone has no marker at the exact prose it qualifies. Adding an
inline reference and definition system would duplicate standard footnotes.
Ordinary footnote syntax already models the necessary source relationship.

### Raw HTML, content segments, or Tera2 components

These can produce a visual note but leave its relationship to prose implicit or
force prose into template-shaped fragments. They are presentation
workarounds, not an authored Markdown contract.

### URL fragments and `:target` CSS

Fragment targeting changes browser history and can scroll a reader to a
different position. A local native control expands the selected local note
without navigation.

### JavaScript disclosure controls

JavaScript could update button-specific ARIA state, but it would make basic
note access depend on a client runtime. A real checkbox exposes its own native
state and needs no script.

### A root-page sidebar or absolute positioning

A page-wide sidebar assumes one fixed layout and fails at narrow widths,
increased zoom, and many ordinary site designs. A local grid wrapper lets CSS
place each selected note beside its associated prose or beneath it.

## Implementation acceptance criteria

Implementation must include focused and fixture-site coverage for:

- page and descriptor configuration, inheritance, suppression, extension,
  selected-content, segmentation, global marker-message localization, and
  rendered-page errors with authored spans;
- standard source-order numbering, several references in one prose block, and
  repeated references that render local instances with the same number;
- missing, duplicate, and unreferenced definitions with relevant Markdown
  spans;
- paragraph and heading rendering, including coexistence with selected ToC
  heading anchors and footnote-definition headings excluded from that ToC;
- rejected references in unsupported structural contexts, including a footnote
  definition and raw-HTML-containing prose;
- multi-paragraph definition bodies containing lists, code blocks, and links,
  with valid sibling `aside` structure and ordinary checked Markdown
  transformations;
- source-aware `markdown(content="…")` rendering, exactly-one selected-content
  invocation, and raw-filter misuse;
- valid, distinct Heine-generated local control and note IDs for repeated
  references, matching visible note numbers, localized accessible control
  names, semantic note roles, and source-ordered no-CSS output;
- manual screen-reader checks of marker discovery, localized control naming,
  checked-state announcement, and local note text exposure, including a
  multi-paragraph note with a list and code block;
- quick and full-build behavior; and
- a responsive, keyboard-visible, print-safe starter-site example in every
  comparable authored-page template boundary where notes naturally belong,
  including a manual browser print-to-PDF check that notes follow their prose
  in readable document flow.

Public documentation, the reference, tutorial, glossary, and starter site must
be updated together when the feature is implemented.
