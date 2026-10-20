# How Heine builds a site

Heine builds a model of a site before it writes the site. That model gives
pages, assets, translations, collections, and generated files stable identities
such as page and asset IDs, plus checked relationships. Templates present those
resolved facts. They do not create new pages, routes, or output files.

This guide explains that boundary. It is an overview of Heine's current
architecture, not a replacement for the [reference](reference.md) or a guide
to its Rust modules.

## The build pipeline

Each full build follows one direction:

```text
authored inputs
    -> discovery and source checks
    -> resolved Site model and output claims
    -> Markdown analysis and derived page facts
    -> template rendering
    -> staged output
    -> atomic publication
```

Heine first discovers the inputs below the site root, assigns them their
roles, and rejects unsafe paths and symbolic links. It then resolves the
relationships that need knowledge of the complete site.

That step creates the stable Site model and a checked plan for every output
Heine owns. An output claim records that one page, copied asset, deployment
file, or generated resource owns one output path. A collision or a missing
relationship fails at its authored location before Heine publishes a partial
site.

Markdown analysis then derives page facts such as heading anchors, word counts,
reading-time estimates, and search text. It also resolves checked Markdown
links and assets. Templates render only after that analysis. They receive
deliberate views of the resolved model, such as a page, an asset, a collection,
or a taxonomy. They can choose how to present those views, but they cannot
invent output ownership or change the relationships that Heine has already
resolved.

Finally, a full build writes the complete managed output to a staging tree. It
publishes that tree only after every render succeeds, so a failed build leaves
the previous published output intact.

## What a full build guarantees

A successful full build has resolved the relationships Heine owns, rejected
output collisions, rendered every planned page and generated resource, and
published a complete replacement of its managed output tree.

That guarantee has deliberate limits. Heine does not verify external URLs,
literal URLs or manually chosen IDs in trusted raw HTML, references in CSS or
JavaScript, or the `#fragment` suffix after a checked `page:` link. It checks
the page part of `page:guide/install#section`, then preserves `#section` for
the browser.

## What belongs to each domain

Heine keeps the main authored roles separate because they have different
validation and output behavior:

- **Content** holds pages, declared Markdown inputs, and copied assets. A page
  declaration chooses its inputs. Declared inputs are not copied.
- **Configuration, locale trees, and the selected kit** establish site-wide
  behavior, localized identity, and the available presentation boundary.
- **Data** supplies global structured values to templates. It is not published
  and does not provide arbitrary filesystem access.
- **Templates and Fluent messages** describe presentation. Templates receive
  resolved views, and messages hold reader-facing text.
- **Generated resources** consume resolved facts. Listings, feeds, sitemaps,
  search documents, and renderer resources each have an explicit output claim.
- **Output-attached files** include deployment files and optional license
  declarations. They attach to the copied output inventory without becoming
  pages or template-controlled output.

This separation lets a template link to a known page without creating one, and
lets Markdown refer to a checked asset without gaining unrestricted file
access.

## Checked relationships

Heine checks relationships that need knowledge of the complete site. These
include a `page:` Markdown link naming a rendered page, an `asset:` URL or
`asset()` call naming a copied asset, locale counterparts, collection
membership, and generated routes that must not collide with other output.

For example, when Markdown contains `[Install](page:guide/install)`, Heine
resolves `guide/install` against the current locale's page catalog during
Markdown analysis. A missing page reports the link's authored source location.
A known page renders with its resolved URL.

Ordinary literal web URLs remain ordinary content. Heine does not pretend that
it can verify arbitrary external URLs or rewrite all HTML, CSS, and JavaScript
references. When an author needs a checked internal relationship, they use the
corresponding typed Markdown form or template function.

## Markdown, templates, and external tools

Markdown is prose by default. Tera2 runs in templates, where its inputs and
output role are explicit. This lets authors write literal `{{ ... }}` and
`{% ... %}` in Markdown, including code examples, without creating a second
templating phase or changing Markdown-derived facts after resolution.

Some Markdown constructs have a narrow structural contract. `:::picture`
connects one authored construct to checked assets, and `:::render[...]`
connects one request to a checked renderer result. They do not become a general
extension or plugin system.

External CSS and JavaScript bundling, media processing, remote fetching,
deployment, and provider integration remain ordinary external tools. An
external tool must produce its final asset before Heine reads it. Processing
Heine-managed output afterwards can invalidate versioned URLs or integrity
metadata.

## Quick builds and the development server

`heine --quick` trades completeness for a faster edit cycle. It updates known
changed inputs in place, but it can retain stale output and cannot reliably
discover every relationship, configuration, template, rename, or deleted-input
change. Use a full build before deployment and whenever a change can affect the
wider site model.

The development server performs complete rebuilds before it publishes a new
generation. It therefore gives local preview the same resolved-model behavior
as a full build.

## Where to continue

The [tutorial](tutorial.md) shows how a site grows from a small page into a
multilingual blog. The [reference](reference.md) defines every current field,
template value, function, and constraint. Focused guides explain each domain
in detail.
