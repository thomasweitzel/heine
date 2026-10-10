# Generated color-scheme pictures

## Status

Implemented. This record defines the renderer-adapter color-scheme picture
contract.

## Context

Heine already models a light and dark form of one copied image with
[`:::picture`](../markdown-pictures.md). The directive checks both image
assets, requires one alternative text, preserves their common presentation
role, and emits a native HTML `picture` element. The browser selects the dark
source only when `prefers-color-scheme: dark` matches.

A renderer resource has the same presentation need when a tool such as Typst,
Mermaid, Graphviz, or a chart generator produces fixed-colour SVG, PNG, or
WebP output. Two independent `:::render` requests cannot express that they
are visual alternatives of one semantic image. They would emit two independent
`img` elements, retain two unrelated source spans, and expose neither result
to `:::picture`, whose inputs are deliberately copied assets.

The browser selects image candidates after Heine has built the page. Heine
therefore needs a checked representation of a pair of generated image
resources, rather than a Markdown mechanism for passing generated URLs between
unrelated constructs.

## Decision

Add a renderer result shape named `picture`. It represents exactly one
meaningful image in two visual forms: `light` and `dark`. A selected request
has one checked input, one required `alt`, and optionally one caption, one
pair of presentation dimensions, and one list of CSS classes. It produces two
checked generated image resources and one native `picture` replacement.

`picture` is limited to image media types. It is not a generic appearance
variant mechanism for every renderer result.

The result stays separate from `image`:

- `image` returns one generated resource and emits `img`.
- `picture` returns checked light and dark resources and emits `picture` with
  one fallback `img`.

This distinction makes the semantic presentation contract visible in profile
configuration and avoids making a one-image result conditionally return a
different HTML structure.

## Profile contract

A picture profile declares `result = "picture"`, one `image/*` media type,
and one safe filename extension. It has both light and dark placement
variants. The variants are fixed renderer invocations, not Markdown options.

```toml
[renderers.typst_diagram]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.1"
inputs = ["body"]
input_extension = "typ"
result = "picture"
media_type = "image/svg+xml"
extension = "svg"

[renderers.typst_diagram.block.light]
arguments = [
  "compile", "{input_file}", "{output_file}",
  "--format", "svg",
  "--input", "scheme=light",
]

[renderers.typst_diagram.block.dark]
arguments = [
  "compile", "{input_file}", "{output_file}",
  "--format", "svg",
  "--input", "scheme=dark",
]
```

The profile author chooses how the renderer receives that fixed appearance
input. The Typst example uses `sys.inputs`; another tool may use its own
fixed command-line setting. Heine does not define a renderer-specific theme
API, inspect the source for colour choices, or transform the returned files.

Each supported placement must declare both appearances. A profile may support
only blocks, only inline requests, or both, subject to the existing placement
rules. Stream and workspace protocols retain their current input and
placeholder rules for each appearance variant.

## Markdown and replacement

The request keeps the existing image-resource fields:

```markdown
:::render[typst_diagram alt="Architecture diagram" caption="Request path" class="diagram align-center"]
#import "diagram.typ": diagram
#diagram(scheme: sys.inputs.at("scheme"))
:::
```

`alt` remains required. `caption`, `width`, and `height` retain the existing
picture meanings and validation rules. `class` follows the shared contract
below. Markdown does not name the light or dark resource. Those are
profile-owned fixed representations of the one request.

`class` is one non-empty TOML string containing one or more HTML class tokens.
Heine splits tokens at ASCII whitespace, normalizes every separator to one
space, preserves authored token order, and rejects a repeated token after that
normalization. It follows the HTML class-attribute model, not CSS selector
grammar: a token may require CSS escaping when a site selects it, and authors
remain responsible for choosing selector-friendly names. Heine HTML-escapes
every accepted token. The field provides a site-owned presentation hook, not an
arbitrary HTML-attribute mechanism. The same field is added to copied-asset
`:::picture` blocks, so a site can use one CSS rule for copied and generated
picture results:

```markdown
:::picture
alt = "Architecture diagram"
light = "asset:images/architecture-light.svg"
dark = "asset:images/architecture-dark.svg"
class = "diagram align-center"
:::
```

The class list applies to every host-owned structural element that represents
the picture result: `picture` always, and `figure` as well when the request has
a caption. This gives CSS stable hooks for image selection and the complete
captioned unit. Authors should select `picture.<class>` or `figure.<class>`
when a rule belongs to only one of those roles; a broad `.<class>` selector
matches both elements for a captioned result. This duplication is deliberate:
it lets a `picture.<class>` selector work whether the picture stands alone or
is nested in a captioned figure. It also means a class intended for only one
role needs an element-qualified selector.

A caption emits a block-level `figure`, so `caption` is valid only for a block
renderer-picture request. Heine rejects a caption on an inline request at that
option. An inline picture request may still use `alt`, `class`, and dimensions.

The host emits this structure, with its checked digest URLs substituted:

```html
<figure class="diagram align-center">
  <picture class="diagram align-center">
    <source media="(prefers-color-scheme: dark)" srcset="/heine-renderers/sha256-<dark>.svg">
    <img src="/heine-renderers/sha256-<light>.svg" alt="Architecture diagram">
  </picture>
  <figcaption>Request path</figcaption>
</figure>
```

Without `caption`, the host emits `picture` alone. The browser owns the media
query decision, just as it does for copied-asset pictures. Heine does not add
a site-owned appearance preference, JavaScript selection, or automatic colour
transformation.

The light resource is the fallback `img` source. Browsers that support the
dark media query select the dark `source`; browsers that do not receive the
light fallback.

## Checked invariants

Both appearances must succeed for one request to succeed. For each result,
Heine checks the same invariants as an ordinary generated image resource:

- it is non-empty;
- it has the profile's declared image media type and extension;
- SVG has an `svg` document element and is well-formed XML;
- its generated path is a digest-addressed claim below `heine-renderers/`.

When Heine can determine both variants' intrinsic dimensions, the two
representations must have the same aspect ratio and the host emits the light
variant's dimensions. If it cannot determine both variants' dimensions, it
emits no automatic dimensions without authored dimensions. When the light
variant has known dimensions, an author may provide one positive `width` or
`height` and Heine derives the other from that fallback image. When both
variants have known dimensions, every authored dimension must preserve their
shared ratio. When the light variant has no known dimensions, one authored
dimension is insufficient: the author must provide both, and Heine cannot
check their ratio. A known dark size alone does not permit derivation because
the fallback `img` uses the light resource. The host must not claim dimensions
or a ratio it cannot check.

The ratio comparison is exact, using the same integer cross-multiplication
rule as copied-asset pictures. A minor renderer-induced size difference still
fails because it can change layout when the browser selects the other
appearance. Authors must stabilize the renderer's visual frame, for example by
setting an explicit diagram or page size, or provide explicit dimensions that
Heine accepts without ratio verification when intrinsic dimensions are unknown.

The two appearances must represent the same information. Heine cannot infer
that property from bytes, so the profile and request syntax make it explicit.
Documentation must state that a light and dark picture changes presentation,
not meaning. Matching aspect ratios are the only automated relationship check:
two different images with matching dimensions still pass. Distinct information
belongs in separate ordinary images or separate renderer requests.

## Execution, cache, and output ownership

Heine runs the light and dark variants separately from the same checked input.
Each cache key includes the appearance role, selected placement, fixed variant
arguments, profile identity, requested output form, and complete checked
input. `--refresh-renderers` refreshes both variants. A cache hit for one
appearance does not excuse a failed or missing other appearance.

On a cold cache and during refresh, a picture request costs two renderer
invocations. That cost is deliberate: automatic recolouring of one fixed
result cannot reliably preserve diagram meaning or contrast. Independent cache
entries avoid repeated invocations when an unchanged variant remains valid.

The current synchronous renderer pipeline runs the two appearances
sequentially. Parallel execution is deliberately undecided: it would need a
separate design for deterministic diagnostic ordering, process-resource limits,
cache publication, and the one-reusable-workspace-slot-per-profile invariant.

The host retains the originating page, content source, directive span, and
appearance role for both generated-resource claims. Identical bytes may still
share one digest-addressed output where their media type and extension agree.
Collision diagnostics retain every contributing request and appearance.

A full build publishes only the current complete pair inventory. A quick
build may retain superseded generated resources until the next full build,
under the existing renderer-resource rule.

Workspace execution still accepts exactly one primary output file per variant.
The profile does not create a resource set or permit a renderer to write a
directory of theme-specific output files.

## Deliberate exclusions

### Paired HTML or MathML

HTML and MathML are embedded document fragments, not image candidates. A
light and dark pair would duplicate IDs, links, interactive elements, and
accessibility content in the page. There is no native HTML equivalent of
`picture` that selects one fragment before it enters the document.

An embedded renderer that needs theme-aware presentation must return one
semantic fragment with stable classes. The site stylesheet owns its colours
through its normal tokens, custom properties, and appearance rules. An SVG
that can contain its own responsive CSS should likewise remain one SVG result
rather than become a generated picture pair. Prefer that self-theming SVG form
when the renderer can deliberately produce it. Use a generated picture pair
when the renderer produces fixed-colour SVG or raster output.

### Arbitrary Markdown attributes and embedded-renderer classes

`class` is available for copied and generated picture results because Heine
owns their structural HTML. It is also available for embedded SVG renderer
results because Heine checks one `svg` document element and can add the list to
that root without changing document structure. The list gives site CSS a stable
hook for a deliberately monochrome SVG, including one that follows
`currentColor` in light and dark prose.

This does not introduce a general Markdown attributes dialect, arbitrary
`style` declarations, or arbitrary HTML attributes. An embedded HTML or MathML
renderer owns its returned fragment, so Heine does not inject a class into it
or guess which nested element should receive one. SVG resources remain `img`
elements and use the existing image-resource contract.

### Linked and arbitrary resources

A PDF, archive, audio file, video file, or other download does not normally
change because a page uses dark presentation. If two files carry a meaningful
difference, authors must expose two explicit labelled links. Heine must not
silently choose a download based on a browser preference.

### Automatic recolouring and post-processing

Heine does not invert, recolour, or inject CSS into renderer output. Such a
transformation cannot preserve the meaning of diagrams, photographs, charts,
brand colours, and accessibility contrast in general. A root class on one
embedded SVG is different: it lets the site choose CSS for a renderer output
that the profile author deliberately designed as monochrome. A renderer profile
may invoke a trusted external wrapper after explicit permission, but the host
continues to validate only the declared final result.

The profile intentionally repeats the complete fixed argument vector for each
appearance. Heine expands only complete path placeholders and does not
interpolate values into arbitrary arguments, because command-string templating
would weaken the process boundary. A shared-argument facility needs a separate
concrete design: tool argument order can be significant, so a simplistic common
prefix or `{scheme}` interpolation would not reliably remove duplication.

### A site-owned appearance switch

The existing copied-asset picture contract uses the browser's
`prefers-color-scheme` query. A manual appearance switch would need a separate
site presentation contract that defines persistence, initial rendering,
template integration, and how image selection follows it. Renderer pictures
must not introduce that unrelated feature implicitly.

### Further variant axes

This design owns only light and dark image representations selected by the
browser's colour-scheme media feature. A contrast preference, reduced-motion
alternative, pixel-density representation, locale-specific image, or another
variant axis may have a different number of alternatives, fallback behavior,
accessibility meaning, or native HTML mapping. It requires a separate concrete
design record rather than a new value added to this profile contract.

## Alternatives considered

### Two independent renderer requests

This duplicates source and accessibility metadata, cannot prove that the
outputs are one image, and emits two independent replacements. It also makes
output and cache ownership harder to diagnose. A single request with a typed
pair preserves the actual relationship.

### Let `:::picture` name renderer URLs

Digest URLs are host-derived results, not authored identifiers. Exposing them
to Markdown would couple authored text to execution order and blur the
boundary between copied assets and generated resources. The host should
compose the checked pair before Markdown rendering completes.

### Extend `image` with optional appearances

One configuration value would sometimes return `img` and sometimes `picture`.
The explicit `picture` result is easier to read, validate, document, and
extend without ambiguous result semantics.

### Render two HTML fragments and hide one with CSS

CSS hiding is not native resource selection. Both fragments remain in the DOM
and can conflict through IDs, controls, focus order, media loads, and
accessibility. One adaptive HTML fragment is the correct model.

## Acceptance criteria

Implementation must include:

- profile and request diagnostics for every incomplete or incompatible pair;
- stream and workspace fixture coverage for successful and failing pairs;
- cache, refresh, output-collision, quick-build, and watcher coverage,
  including the light and dark roles where they share an otherwise identical
  renderer result;
- generated HTML checks for `picture`, `source`, fallback `img`, figure,
  caption, accessibility text, dimensions, base paths, and class placement
  with and without a caption; class-token normalization, duplicate rejection,
  and HTML escaping; inline-caption diagnostics; and embedded SVG root-class
  placement with cached renderer output remaining undecorated;
- dimension coverage for matching known variants, incompatible variant ratios,
  authored dimensions that disagree with a known shared ratio, derivation from
  a known light variant when the dark size is unknown, and explicit dimensions
  when the light size is unknown;
- real-tool coverage with Typst rendering a light and dark SVG pair;
- reference, glossary, focused-guide, design-record, and changelog updates;
- a starter-site example only if a coherent real site image benefits from two
  visual forms.
