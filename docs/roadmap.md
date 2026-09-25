# Roadmap

Heine grows through complete vertical features. A planned item is not a
compatibility promise or a commitment to a particular interface. Before work
starts, it needs a concrete use case, an issue, a design, tests, documentation,
and a cohesive starter-site example where one fits naturally.

## Next

- **Renderer adapters:** complete workspace renderers and generated-resource
  outputs for the existing trusted external-renderer boundary. Stream
  renderers already provide checked embedded replacements. See the
  [design](design/plugins.md).

## Later

- **Kit distribution:** publish a small, curated kit catalogue only after kit
  authors have a practical way to version, test, and maintain their own
  repositories. Heine's checked kit contract does not require a central
  registry or package resolver.

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
