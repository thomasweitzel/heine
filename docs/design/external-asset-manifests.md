# External asset manifests

## Status

Exploratory. The only candidate under consideration is consuming a checked
external-builder manifest that maps logical entry-point asset IDs to final
fingerprinted outputs. Heine fingerprinting copied assets itself is out of
scope. This record does not define configuration, template behavior, or an
implementation commitment.

## Problem

A long-lived immutable cache is useful only when a changed representation gets
a new URL. Content fingerprints give an asset such a URL, while rendered HTML
continues to use a stable URL and points to the current assets.

The tempting small feature is a template helper that appends a digest query to
one URL. Zola's `get_url(..., cachebust=true)` follows that model. It is useful
for the one file named by the call, but it does not understand the file's
dependencies. A stylesheet can still refer to a stable font, image, or imported
stylesheet URL. If the asset behind that stable URL changes, immutable caching
can leave a reader with a mixture of old and new resources. The [Zola Sass
discussion](https://zola.discourse.group/t/cache-busting-for-sass-url-statements/1542)
shows this problem with an `@font-face` URL.

The Web's usual solution is an asset graph. A build tool identifies imports and
references, writes content-addressed filenames, rewrites the graph to use those
filenames, and emits an entry-point manifest. The builder establishes final
dependency filenames, rewrites references to them, and only then derives the
final bytes and fingerprint of each referencing asset. When configured to emit
a manifest, Vite, for example, rewrites CSS `url()` references and JavaScript
asset imports while producing hashed output and a manifest for the
HTML-producing host. See Vite's [asset
handling](https://vite.dev/guide/assets) and [backend integration
guide](https://vite.dev/guide/backend-integration).

That work belongs to a tool that understands the relevant languages. It does
not belong to a static-site generator that otherwise copies CSS, JavaScript,
fonts, and binary files unchanged.

## Cache policy follows resource identity

Rendered documents and assets have different public identities.

| Resource                              | URL identity              | Typical cache policy                |
|---------------------------------------|---------------------------|-------------------------------------|
| Authored pages and generated listings | Stable public route       | Revalidate or use a modest lifetime |
| Canonical URLs, feeds, and sitemaps   | Stable public route       | Revalidate or use a modest lifetime |
| Manifest-selected assets              | Fingerprinted file URL    | Long-lived `immutable` caching      |
| Ordinary copied assets                | Stable file URL           | Revalidate or use a modest lifetime |

Pages, pagination, taxonomy and Series listings, feeds, canonical URLs, and
sitemaps must retain stable URLs. Their routes are public identity: readers,
internal links, search indexes, and deployment configuration refer to them.
Fingerprinting them would make a content update look like a new public
document. They should instead lead the browser to the current fingerprinted
assets.

The cache headers remain deployment policy. A fingerprinted filename makes a
long cache lifetime safe; it does not itself send an HTTP header. The [MDN HTTP
caching guide](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Caching)
describes the usual pairing of changed asset URLs with long `immutable` caching
and revalidating HTML.

### Fingerprinted filenames need retention

The candidate supports only final outputs that an external builder has
fingerprinted in filenames. It does not construct query-string cache keys. A
filename gives the representation its own path, lets old and new files coexist,
and does not depend on an intermediary including a query parameter in its cache
key. Query parameters are valid cache keys, but a CDN or proxy can be configured
to ignore them.

That coexistence makes retention a deployment concern. A deployment must publish
new assets before, or atomically with, documents that name them. It must also
retain older fingerprinted files for an explicit policy period: a browser or CDN
may still hold earlier HTML, and durable references in Atom entries, newsletters,
social previews, citations, or bookmarks can name an older asset for much longer.
Rolling back to earlier HTML has the same requirement: its fingerprinted assets
must still exist. Heine's full builds atomically publish only the current
managed output tree and remove stale local files. That tree is not an archive,
and Heine cannot make a deployment system retain prior public assets.

## Ownership boundary

The deciding factor is not an asset's extension. It is who can correctly
resolve every relevant reference to it.

### External asset graphs

CSS, JavaScript modules, fonts imported by CSS, images imported by CSS or
JavaScript, source maps, and split bundles normally form an external build
graph. A graph-aware external tool must:

1. produce the final files before Heine reads them;
2. fingerprint the files it owns;
3. rewrite graph references to the final filenames; and
4. emit enough manifest data for an HTML-producing host to select entry
   resources and their required dependencies.

Heine must not parse or rewrite CSS `url()`, CSS `@import`, JavaScript imports,
source maps, SVG-internal references, or runtime-generated URLs. Doing so would
make Heine an incomplete asset pipeline and would still fail for dynamic
references that a static scan cannot establish.

During development, the external builder may use a disk-backed watch build and
write its current assets and manifest before Heine reads them. Heine does not
coordinate an external development server, proxy Hot Module Replacement, or
substitute development-only URLs. Those behaviors would make a Heine build
depend on another server and a browser runtime rather than the files it checks.

### Ordinary copied assets

A PDF, image, archive, media file, or other binary remains at its stable copied
URL unless an external graph-aware builder owns it. Heine does not need to
parse the file's bytes. That does not make its URL safe to rename: downloads
and images can have inbound links, citations, bookmarks,
email references, or deployment configuration just as pages do.

Raw HTML, literal CSS URLs, literal JavaScript strings, and externally
constructed paths remain outside Heine's checked relationship. Heine must not
silently rename an ordinary copied asset, duplicate it under a fingerprinted
name, or use a filename glob to select a generated replacement. A glob cannot
describe an asset graph, and stale generated files would make its selection
ambiguous.

### External URLs

Heine must not fetch, hash, or rewrite external URLs. It does not own their
bytes, cannot establish their cache semantics, and could break signed URLs,
provider-specific transformation URLs, or query-bearing API endpoints. Authors
use the external provider's versioned URL and, where appropriate, Subresource
Integrity separately.

## Candidate future boundary

If a concrete site needs this integration, the future contract should model a
logical entry-point asset ID and its checked external-builder output, not a
generic URL transformation:

```text
logical entry-point asset ID
    -> checked final fingerprinted output
```

For example:

```text
site
    -> assets/build/site-D7r9.css
```

Entry-point IDs form a separate typed namespace from ordinary asset IDs. A
manifest entry cannot shadow an ordinary copied asset, even when the two names
have identical text. The final outputs must already exist below a declared
builder-output root when Heine resolves the relationship. Heine would validate
safe paths, target existence, uniqueness, locale and kit layering,
deterministic ordering, and output collisions. Integrity metadata would cover
the final bytes that Heine reads. The fingerprinted filename remains the
external builder's claim: Heine cannot prove that a builder's filename digest
represents those bytes, or that a manifest is current when its target file
still exists.

The manifest must name the complete checked set of builder outputs that Heine
publishes for an entry point. Heine must not copy a builder-output root
wholesale: watch builds and non-cleaning builders can leave stale fingerprints,
source maps, or partial artifacts there. That set must therefore include the
entry resource and every required stylesheet, module preload, image, font, or
other final dependency.

A plain `asset()` lookup remains a one-to-one lookup for an ordinary copied
asset. It exposes the existing base-path-aware `url`, current-output-relative
`relative_url`, and canonical `absolute_url` views. Markdown `asset:`
references remain in that ordinary asset domain.

An entry point needs a separate typed template view. It may contain one primary
module or stylesheet together with ordered lists of stylesheets, module
preloads, and other typed preloads. That view must hide the external tool's
manifest schema without turning `asset()` into a bundle-graph API or giving a
manifest authority to inject arbitrary HTML. It should also align with the
deferred [renderer output dependencies](renderer-output-dependencies.md)
record, which has the same template-boundary problem.

The manifest format remains deliberately undecided. If a concrete Vite use case
comes first, a narrow integration with Vite's native manifest may be the
least-friction design. If several builders need one stable host boundary, a
small Heine-owned typed manifest may be justified instead. Heine must not accept
arbitrary JSON as an unchecked URL map in either case. It reads final resource
bytes itself, computes any integrity metadata from those bytes, and exposes only
the typed facts that templates need.

## Non-goals

A future feature must not add:

- a CSS, Sass, JavaScript, image, or general asset-processing pipeline;
- a command runner, watcher, or package-manager integration for external
  builders;
- rewriting of raw HTML, arbitrary template strings, or external URLs;
- content-addressed URLs for pages, listings, feeds, canonical metadata, or
  sitemaps; or
- a generic deployment or HTTP-header configuration system.

Sites continue to run their asset builder before Heine. Their deployment
configuration continues to set cache headers.

## Questions that need a concrete use case

Before accepting a feature, a concrete site must establish answers to these
questions:

- Which manifest format and entry-point dependency facts does the site need?
- Does the concrete use case justify one tool's native manifest, or a stable
  cross-tool manifest boundary?
- Where does the external builder write its final assets and non-published
  manifest input?
- What builder-side publication protocol ensures that the manifest and its
  final assets come from the same completed build?
- How does a template request all required stylesheets, scripts, and preloads
  for an entry without receiving arbitrary HTML from the manifest?
- How does the manifest express the complete dependency set that Heine may
  publish for each entry point?
- Can the site's development workflow use a disk-backed builder watch mode,
  without a Heine proxy or Hot Module Replacement integration?
- How do locale overlays, public kit assets, licensing sidecars, integrity
  metadata, and generated renderer resources interact with a mapped target?
- Which diagnostics identify a missing entry, unsafe path, or incompatible
  duplicate mapping?
- Which deployment retention policy keeps prior fingerprinted assets available
  for cached documents and durable references?

The implementation must begin only when those answers serve a real site. It
must include focused tests for stable document URLs, base paths, relative and
absolute asset views, manifest failures, output collisions, and the external
tool ordering boundary. When integrity metadata is enabled, tests must show
that Heine derives it from the final bytes it reads, not a manifest claim. They
must not claim that Heine can verify an external builder's filename fingerprint
or detect an existing but outdated manifest target. They must also show that an
entry-point ID cannot collide with a plain asset ID, an unreferenced stale file
below the builder-output root is not published, and Heine publishes every
dependency that an entry point declares.
