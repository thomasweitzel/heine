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

### Localized value formatting

The proposed [localized value-formatting design](design/localized-value-formatting.md)
would give Tera exact checked decimal formatting and Fluent a checked numeric
subset compatible with its current selector model, alongside dates, times, and
date times. It deliberately excludes a generic ICU formatting language, units,
relative time, and browser-selected time zones. The first implementation
package would establish locale-aware decimal formatting and Fluent `NUMBER()`.
The separate [percentage and currency design](design/percentage-and-currency-formatting.md)
preserves those deliberate later extensions until ICU4X provides stable APIs.

### Redirect aliases

A host-level redirect is preferable whenever the deployment platform supports
one. Apache `.htaccess`, Netlify and Cloudflare Pages `_redirects`, and Vercel
configuration all have different formats and capabilities. Portable generated
HTML redirect pages work more widely but cannot provide an HTTP `301` or
`308` response.

Heine therefore does not yet own redirects. A future feature must preserve a
checked old-route-to-rendered-target relationship without assuming a hosting
format or treating a browser-level redirect as equivalent to a server redirect.
