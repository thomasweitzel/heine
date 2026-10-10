# Renderer output dependencies

## Status

Deferred. This record does not add configuration, Markdown syntax, template
values, or renderer behavior. It preserves a future extension boundary for a
second concrete renderer that needs files in addition to its primary result.

## Context

A renderer adapter currently returns one checked primary result: embedded HTML,
SVG, MathML, an image, a link, or a color-scheme picture. The host owns that
result's resource output and Markdown replacement.

Some embedded HTML renderers also need supporting files. MathJax CommonHTML
first motivated this record: its layout and glyph metrics need CSS, and its
fonts may need font resources. A site can handle that requirement today by
generating a complete static stylesheet outside Heine and loading it as an
ordinary site-owned stylesheet.

MathJax can disable adaptive CSS and generate rules for all supported CHTML
elements and glyphs. That makes one stylesheet independent of the current TeX
corpus, but MathJax describes the result as extremely large and recommends
adaptive CSS for normal use. A site that deliberately accepts the transfer cost
may still use that all-formulas stylesheet as a normal global asset. Heine does
not need a new renderer result to support it.

The KaTeX example in the [reproducible renderer guide](../renderer-examples.md)
has a different, simpler shape. Its fixed package stylesheet and sibling font
directory are ordinary copied site assets, and a page's `extra.katex` value
lets its template opt in to the stylesheet. That is deliberate site-template
policy, not a checked renderer-output dependency: Heine cannot yet verify that
every KaTeX request has the matching page value.

MathJax SVG is a useful alternative for sites that prefer image-like resources,
but it does not replace CHTML automatically. The output form is an authored
presentation and accessibility choice, not a renderer-adapter policy.

## Problem

The static global-stylesheet workflow is simple but can load renderer support
on pages that do not use the renderer. A future renderer may instead produce a
primary embedded result together with one or more checked resources that only
the consuming page needs.

Adding a renderer-specific `stylesheet` field would not solve the general
problem. A renderer can need zero, one, or many supporting files. Other
renderers may need CSS, ES modules, images, fonts, or another host-understood
resource role. A site-owned page value, such as the KaTeX example's
`extra.katex`, remains useful for known static assets but cannot establish the
checked renderer-to-template relationship this design considers.

## Future direction

If a second concrete renderer establishes the need, extend renderer results
with an ordered, deduplicated list of typed output dependencies:

```text
renderer result
  ├─ primary replacement: embedded HTML
  └─ output dependencies: zero or more typed resource facts
```

Each dependency would describe a host-understood loading operation, a checked
generated resource, and its scope. Likely operations include a stylesheet, a
module script, a module preload, or a typed preload. The exact enum remains
deliberately undecided until a concrete renderer shows which operations have
distinct host behavior. A font or image is a resource category, not by itself
a request for head markup: CSS normally loads fonts, and an image normally
belongs in the primary HTML result.

A dependency never carries raw HTML or an arbitrary `link` or `script` string.
The first implementation should support only the operation required by the
concrete renderer that justifies it. It must not add module or preload support
merely because those operations fit the eventual model.

Scope would state which rendered documents need the dependency, for example a
consuming page or every page in a site. It would not grant a renderer authority
to write into a document head. Heine would collect the facts while resolving a
page and render them only through the checked template slot described below.

This follows the useful part of build-manifest practice without turning Heine
into an asset bundler. Vite manifests use unbounded `css`, `assets`, and import
lists per entry, while the backend renders the corresponding HTML tags. Heine
would likewise retain host ownership of output paths, URLs, integrity metadata,
and template markup. See Vite's [backend integration guide](https://vite.dev/guide/backend-integration)
for that manifest pattern.

## Required properties of a future design

### Typed, bounded facts

The adapter would return a finite list of structured dependency descriptions.
Heine would write their bytes as generated resources, validate media type and
extension for the declared operation, assign digest-addressed URLs, and reject
unsupported operation and scope combinations.

The protocol must not become an arbitrary-files channel. In particular, a
renderer must not select a template, write output directly, name an arbitrary
HTML element, or provide a literal tag for Heine to inject.

### Deterministic ordering and deduplication

Template output needs a stable order. Heine should preserve renderer-declared
dependency order within one result and page request order across results. It
should deduplicate equal dependency *uses* at the scope where they would
otherwise produce duplicate markup. The first contributing request retains the
output position; every contributing request remains available for diagnostics.

Equality must use checked resource identity and complete loading semantics, not
an untrusted filename or bytes alone. The same digest used as a stylesheet and
a preload is two distinct uses. A future operation may add checked attributes, such
as stylesheet media or preload destination; those attributes also belong to
the identity. The future design must specify whether an incompatible repeated
use is an error or two ordered uses.

### Deliberate template boundary

Templates, not renderer results and not Markdown, own document-head structure.
Required dependencies cannot, however, be an optional page list that a
template can silently omit. That would let a build publish an embedded result
without the stylesheet or module that makes it work.

The future design should introduce one narrow host-rendered dependency slot.
A page template places that slot deliberately within its `head`; Heine renders
only checked, ordered dependency uses there. The template therefore owns
whether the document has a head and where the slot appears, while Heine owns
the safe `link`, `script`, or preload markup that each typed operation
requires. During rendering, Heine must report an error when a page has a
required dependency but its selected template did not invoke the slot.

The exact Tera2 spelling remains undecided. It might be a named function or a
dedicated render-facing value, but it must be observable by the host so the
missing-slot error is checked. It must not be raw HTML whose contents Heine
cannot inspect. The design must cover every comparable template boundary
that renders an authored page. A feature that works only in one starter-kit
template would not meet Heine's template contract.

### Cache, output, and diagnostics

Dependency bytes and their checked descriptions belong in the renderer cache
identity. A cache entry cannot be valid if it restores a primary replacement
without all of its dependencies. Full builds must include the complete current
dependency inventory; quick builds retain the existing generated-resource
limitations.

Output ownership and collision reports remain with Heine. Renderer diagnostics
must retain the originating Markdown span, profile, dependency operation, and
scope. A failed dependency rejects its entire primary result rather than
leaving a page with a broken implicit requirement.

### Security and resource policy

Every loading operation needs an explicit safe media-type, extension, and HTML
mapping policy before implementation. Stylesheets and modules are executable
or active browser inputs, so a future design must define integrity and
cross-origin behavior together with the existing asset/template boundary. A
font or image resource may be publishable without implying that the dependency
slot should emit a head element for it.

## Alternatives rejected for now

### One special stylesheet beside an HTML result

This fixes one MathJax-shaped case but fails as soon as a renderer needs two
stylesheets, a module, a font, or no dependencies. It would create a public
exception without a durable domain model.

### Markdown or page options that inject tags

An author should not write a renderer-generated `<link>` or `<script>` through
Markdown. Such options would duplicate renderer knowledge in content and
would bypass the template boundary that owns document structure.

### A generic renderer-owned head fragment

A raw head fragment that Heine cannot inspect would make ordering, escaping,
integrity, CSP policy, and output ownership impossible to check. It is a
broader hook system, not a renderer-adapter result.

### Implementing this for MathJax alone

The existing static stylesheet approach is adequate for MathJax today. It
would be premature to add a general result protocol with only one motivating
renderer and no agreed role or scope set.

## Trigger for reconsideration

Revisit this record when another renderer has a concrete, checked need for
supporting resources, or when the manual MathJax stylesheet workflow proves
insufficient for a real site. At that point, create an issue with the exact
renderer outputs, desired page scope, template behavior, cache effects,
resource policy, diagnostics, tests, documentation, and starter-site impact.

Until then, keep the adapter's one-primary-result contract and treat
renderer-supporting stylesheets as ordinary site-owned assets.
