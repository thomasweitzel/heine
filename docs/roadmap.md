# Roadmap

Heine grows through complete vertical features. A planned item is not a
compatibility promise or a commitment to a particular interface. Before work
starts, it needs a concrete use case, an issue, a design, tests, documentation,
and a cohesive starter-site example where one fits naturally.

## Later

- **Kit distribution:** publish a small, curated kit catalogue only after kit
  authors have a practical way to version, test, and maintain their own
  repositories. Heine's checked kit contract does not require a central
  registry or package resolver.

- **Calendar archives:** add a narrow generated-output feature for one resolved
  collection when a concrete site establishes its bucket and route contract.
- **Collection-specific membership weights:** model a page's distinct curated
  position in more than one weight-ordered collection without overloading its
  directory sibling weight.
- **Dependency-aware incremental builds** and, if large sites justify it,
  streaming rendering into the staging tree while preserving atomic
  publication.

## Under consideration

### Localized value formatting

[Localized values](localized-values.md) now give Tera and Fluent one
deterministic presentation path for decimal numbers, calendar dates, and
editorial-zone times and date-times. The
[localized-value-formatting design](design/localized-value-formatting.md)
still excludes units, relative time, browser-selected time zones, and
per-event time zones. The separate [percentage and currency
design](design/percentage-and-currency-formatting.md) preserves those
deliberate later extensions until ICU4X provides stable APIs.

### Redirect aliases

A host-level redirect is preferable whenever the deployment platform supports
one. Apache `.htaccess`, Netlify and Cloudflare Pages `_redirects`, and Vercel
configuration all have different formats and capabilities. Portable generated
HTML redirect pages work more widely but cannot provide an HTTP `301` or
`308` response.

Heine therefore does not yet own redirects. A future feature must preserve a
checked old-route-to-rendered-target relationship without assuming a hosting
format or treating a browser-level redirect as equivalent to a server redirect.

### External asset manifests

A per-URL cache-busting helper cannot update the CSS, JavaScript, font, image,
and split-bundle references that form an external asset graph. Heine therefore
does not yet fingerprint copied assets or rewrite asset URLs. A future feature
needs a concrete external-builder use case and must preserve the boundary in
the [external-asset-manifest record](design/external-asset-manifests.md): the
builder owns graph rewriting and final fingerprinted files, while Heine may
validate a narrow manifest-to-checked-asset relationship.

### Published-artifact licensing

Copied assets already support REUSE-style sidecars, exact licensing groups,
local license texts, and optional required coverage. Renderer resources remain
outside that model even though Heine owns their final bytes and output paths.
The [published-artifact licensing design](design/published-artifact-licensing.md)
explores a narrow extension: reusable declared licensing records and coverage
for separate renderer resources, without treating pages or arbitrary URLs as
assets or claiming legal compliance.
