# Roadmap

Heine grows through small, complete vertical features. A planned item is not a
compatibility promise or a commitment to a particular interface. Before work
starts, it needs a concrete use case, an issue, a design, tests, documentation,
and a cohesive starter-site example where one fits naturally.

## Next

- **Kits:** one selected, checked presentation layer with root-owned site
  structure and narrow, declared overrides. See the [design](design/kits.md).
- **Renderer adapters:** explicit Markdown constructs that a trusted external
  renderer turns into checked generated resources and replacements. This is
  the only planned plugin boundary. See the [design](design/plugins.md).

## Later

- **Absolute URLs:** expose `absolute_url` beside existing checked `url` and
  `relative_url` values when `site.origin` is configured.
- **Multilingual sitemap alternates:** opt in to checked `hreflang` XHTML
  annotations in sitemap entries for explicit authored `translationid` groups.
  Emit every known group member, including self, in deterministic locale order;
  partial groups remain partial. Do not infer counterparts from paths or cover
  generated pagination, collection, taxonomy, or Series pages. Account for
  alternate URLs and XML bytes when checking and splitting sitemap output.
  HTML-head annotations belong to a later template-facing companion, built on
  checked absolute page URLs; never inject them into arbitrary templates.
- **Draft publication state:** include draft pages consistently in an explicit
  preview build, while ordinary builds exclude them and every relationship
  derived from them.
- **Calendar archives:** add a narrow generated-resource consumer of one
  resolved collection when a concrete site establishes its bucket and route
  contract.
- **Markdown facts:** begin with deterministic Unicode-aware word counts.
  Reading time needs an explicit locale-aware policy, not an invented universal
  rate.
- **Collection-specific membership weights:** model a page's distinct curated
  position in more than one weight-ordered collection without overloading its
  directory sibling weight.
- **Required copied-asset licensing coverage:** an opt-in mode that requires a
  REUSE-style sidecar for every resolved copied asset Heine publishes,
  regardless of file type or whether a page links to it. It must retain the
  existing checked SPDX, copyright, and local license-text rules, report all
  missing sidecars deterministically, and include selected kit assets when
  kits exist. Design explicit shared licensing groups with this mode: a group
  names an exact non-empty asset list and one shared sidecar-style declaration,
  then resolves to individual asset facts. Do not add recursive directory
  inheritance or globs, which could silently cover a later unrelated asset. It is a
  provenance-coverage guarantee, not a claim of legal compliance: external
  URLs, inline media, raw HTML, generated output, and deployment configuration
  remain outside Heine's copied-asset inventory.
- **Dependency-aware incremental builds** and, if large sites justify it,
  streaming rendering into the staging tree while preserving atomic
  publication.
- **Release hygiene:** add Codeberg CI that runs the required formatting,
  Clippy, and test checks for every pull request. Before a release, verify
  `cargo package` and a release build. Decide a dependency policy for
  advisories, source provenance, duplicate versions, and acceptable licenses
  before automating those checks. Versioning, crate publication, and licensing
  metadata remain part of this work.

## Under consideration

### Redirect aliases

A host-level redirect is preferable whenever the deployment platform supports
one. Apache `.htaccess`, Netlify and Cloudflare Pages `_redirects`, and Vercel
configuration all have different formats and capabilities. Portable generated
HTML redirect pages work more widely but cannot provide an HTTP `301` or
`308` response.

Heine therefore does not yet own redirects. A future feature must preserve a
checked old-route-to-rendered-target relationship without assuming a hosting
format or treating a browser-level redirect as equivalent to a server redirect.
