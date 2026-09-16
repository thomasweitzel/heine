# Roadmap

Heine grows through complete vertical features. A planned item is not a
compatibility promise or a commitment to a particular interface. Before work
starts, it needs a concrete use case, an issue, a design, tests, documentation,
and a cohesive starter-site example where one fits naturally.

## Next

- **Kits:** one selected, checked presentation layer with root-owned site
  structure and narrow, declared overrides. See the [design](design/kits.md).
- **Release hygiene:** establish a repeatable local release procedure before
  publishing. It checks the required formatting, Clippy, and test commands,
  the packaged crate contents, and the optimized release binary. CI may later
  reproduce the same checks, but it is not a prerequisite for the first
  release. Decide a dependency policy for advisories, source provenance,
  duplicate versions, and acceptable licenses before automating those checks.
- **Renderer adapters:** explicit Markdown constructs that a trusted external
  renderer turns into checked generated resources and replacements. This is
  the only planned plugin boundary. See the [design](design/plugins.md).

## Later

- **Calendar archives:** add a narrow generated-resource consumer of one
  resolved collection when a concrete site establishes its bucket and route
  contract.
- **Collection-specific membership weights:** model a page's distinct curated
  position in more than one weight-ordered collection without overloading its
  directory sibling weight.
- **Dependency-aware incremental builds** and, if large sites justify it,
  streaming rendering into the staging tree while preserving atomic
  publication.

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
