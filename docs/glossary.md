# Glossary

This glossary defines Heine's stable public vocabulary. A guide introduces a
term in context before relying on it; this page is a concise place to return
when reading configuration, templates, diagnostics, or the reference.

## Site and output

### Base path

The absolute URL-directory prefix below which a site is deployed, configured
as `site.base_path`. For example, `/guide/` prefixes Heine's base-path-aware
`url` values; it does not change the source layout.

### Full build

A normal Heine build. It creates and checks a complete replacement output
tree before publishing it, so renamed and deleted source files disappear from
generated output.

### Draft

A page declared with `draft = true`. Ordinary builds exclude it before
resolving page relationships; `heine serve --drafts` includes it in a local
preview. Draft state is page-local and has no inherited `false` form. See the
[drafts guide](drafts.md).

### Generated output

The files Heine writes below `build.output_dir`, `public/` by default. It is a
managed tree: do not maintain unrelated files there by hand.

### Managed output

Generated output that Heine has marked as its own. Heine will replace it on a
full build. `--clean-output` explicitly adopts an existing non-empty output
directory once.

### Output path

A checked path relative to generated output, such as `blog/post.html`.
Pages, copied assets, deployment files, license texts, and generated resources
all make output claims; Heine rejects collisions before writing.

### `__heine/`

A URL and output-path namespace reserved for Heine’s development-server
endpoints. It is never published as site output. Pages, copied assets,
deployment files, and generated resources cannot claim a path inside it,
preventing a site from colliding with the server’s own routes.

### Quick build

An incomplete in-place update selected with `--quick`. It
updates changed present inputs, but does not detect every relationship,
configuration, template, rename, or deletion change. Use a full build after
those changes.

### Relative URL

A URL from one generated page to another generated resource. Heine exposes it
as `relative_url`; unlike `url`, it does not begin with `site.base_path`.

### Site root

The project directory containing `heine.toml`, `content/`, and `templates/`.
Optional roots such as `data/`, `locales/`, `deploy/`, and `LICENSES/` also
live beside them.

### Site origin

The scheme, host, and optional port configured as `site.origin`, for example
`https://example.org`. It has no deployment path and is configuration-only.
Heine combines it with the base path when a generated document or template
needs an absolute canonical URL.

### Site URL

The canonical root URL exposed as `site.url` when `site.origin` is configured.
It combines the origin and `site.base_path`, including the trailing slash. A
site with origin `https://example.org` and base path `/guide/` has site URL
`https://example.org/guide/`.

### Sitemap

An opt-in XML discovery document generated from Heine's rendered-output
inventory. It lists canonical page URLs for search engines; it does not affect
whether a page is rendered or reachable. A page can opt out with
`sitemap = false`; a descriptor's `[pages] sitemap = false` establishes a
recursive, non-overridable boundary for its descendant pages.

### Atom feed

An opt-in XML syndication document generated once per locale from one
configured collection. It contains chronological page summaries and canonical
page links. Feed entry requirements are checked during the build; see the
[Atom-feed guide](atom-feeds.md).

### Search index

An opt-in locale-local generated JSON document selected from one or more
collections. Heine supplies checked source-derived page facts and output URLs;
the site supplies the browser search library, ranking, and interface. See the
[search-index guide](search.md).

### Publication chronology

Optional page `published` and `updated` local date-times. They must include a
calendar date and time without an offset; Heine resolves them through the
site-wide [editorial time zone](#editorial-time-zone). `published` records when
a page became public, while `updated` records a later change the author regards
as substantive and cannot precede `published`.

### Editorial time zone

The site-wide IANA zone in `site.time_zone` used to resolve authored local
`published` and `updated` times. Heine uses its bundled time-zone database, so
the resulting UTC timestamps do not depend on the build machine.

## Authored files and inputs

### Asset sidecar

A file named `<asset filename>.license` beside a copied asset. It declares
REUSE-style SPDX copyright and licensing facts for that asset; it is not itself
published as an asset.

### Licensing group

One explicit `[[licensing.groups]]` declaration in `heine.toml`. It assigns
the same SPDX facts to an exact non-empty list of copied-asset output paths.
It is an alternative to individual sidecars for an upstream or
first-party asset set, not a directory rule, glob, or template-facing object.

### Copied asset

A regular file below `content/` that is neither a `.page` file nor a file
declared by a page's `[content]` table. Heine copies it to its corresponding
generated output path. Templates use `asset()` to link to one safely.

### Color-scheme picture

A standalone Markdown `:::picture` block that names checked light and dark
variants of one image, plus its alternative text and, when needed, a plain-text
caption. Heine emits a native HTML `picture`, or a `figure` with `figcaption`;
the browser selects its dark source from
`prefers-color-scheme: dark`. See [color-scheme Markdown pictures](markdown-pictures.md).

### Declared content file

A file named by a page's `[content]` table. Heine reads it as page content but
does not publish it as an asset.

### Segment

A named text part in a declared content file that begins with a segment marker.
Segments let a template render prose from one authored document in several
named locations. See [declared content](declared-content.md).

### Markdown fact

A deterministic value derived from one declared Markdown input, currently its
Unicode `word_count` and optional locale-policy `reading_time`. Facts appear in
`page.markdown.<name>` without changing the renderable `page.content.<name>`
value. See [word counts and reading time](markdown-facts.md).

### Deployment file

A regular file below optional `deploy/`. Heine copies it unchanged to the
matching output-root path. Deployment files are for host configuration and
verification, not for files a page links through `asset()`.

### Directory descriptor

A non-published `_directory.toml` file below a locale tree. It provides the
directory's metadata and recursive defaults or requirements for descendant
pages.

### License text

A text file in optional `LICENSES/`, named for an SPDX identifier or local
`LicenseRef-…`. Heine publishes it below `<build.output_dir>/LICENSES/` and
checks it against every resolved asset licensing declaration that names it.

### Locale tree

The `content/<locale>/` directory containing one configured locale's pages,
declared content, and locale-specific assets. The default locale has unprefixed
URLs; other locale trees receive a URL prefix.

### Locale data

Structured data loaded from `data/<locale>/` for the locale currently being
rendered. Templates access it through `locale_data`; it is separate from
shared `data` and has no fallback.

### Page file

A `.page` TOML file below a locale tree. It declares a page's template,
metadata, optional content inputs, and relationships. A rendered page normally
becomes one output file.

### Page directory

The locale-relative directory containing a page file. Templates receive it as
`page.dir`; the locale-root directory is the empty string. `page.subdirs` is
the sorted list of its immediate source subdirectories.

### Summary

Optional authored page or directory metadata used for a short description in a
listing. It is written as `summary` in TOML and exposed as `page.summary`;
Heine never extracts it from Markdown.

### Table of contents

An optional, page-owned view of headings from one selected declared Markdown
file. Configure it with `[toc]` on a page or `[pages.toc]` in a directory
descriptor. Templates read `page.toc` and render the selected source with
`markdown(content="…")`, which gives its headings matching fragment IDs.

### Margin footnote

An optional, page-owned presentation of ordinary Markdown footnotes from one
selected declared Markdown input. Heine checks every reference and definition,
then renders each reference as a local note instance after its containing
paragraph or heading. A template and its CSS decide the visual margin or
below-prose layout. See [margin footnotes](margin-footnotes.md).

### Authors

An optional non-empty list of human names written as `authors = ["Name"]` on
a page or inherited from `[pages] authors` in a directory descriptor. Atom
feeds require every selected entry to resolve authors; the feed checks that
independently of directory requirements.

### Weight

An authored integer used for sibling or collection ordering. `weight = false`
excludes a non-index page from sibling ordering; collection weight ordering
requires every member to have a numeric weight.

### Sibling navigation

The deterministic sequence of rendered, numeric-weight pages in one page
directory and locale. It is ordered by weight, then page ID. `siblings()`
returns the other IDs in the same sequence, while `sibling_neighbors()`
returns the immediate `before` and `after` pages. It is independent of
collections.

### Template

A Tera2 file below `templates/` that renders a page or another named
generated resource. Its logical template name is its path below `templates/`,
such as `post.tera`.

### Versioned asset

A copied asset with `.v<positive integer>` before its extension, such as
`styles.v2.css`. `asset(id="assets/styles.css")` and the `latest` filter select
the highest available version. This is a naming convention for asset selection,
not a separate directory or build step.

## Identity and links

### Asset ID

The literal path used to identify a copied asset within its locale's content
tree, without a leading slash; for example, `css/site.css`. `asset()` checks
this ID and returns URLs, integrity metadata, and image dimensions when known.

### Page ID

The literal path of a `.page` file relative to its locale tree, without the
`.page` extension; for example, `blog/first-post`. `page()` checks this ID
and resolves an exact rendered page.

### URL

The absolute-path form Heine exposes as `url`. It begins with the configured
base path, such as `/guide/blog/first-post.html`; it is not a full site origin.

### Absolute URL

A canonical public URL exposed as `absolute_url` on a known page, asset, or
generated-resource view when `site.origin` is configured. It combines the
origin, base path, and that view's checked output path. See [absolute URLs](absolute-urls.md).

## Localization

### Counterpart

A rendered page in another locale explicitly connected by the same
`translationid`. Heine never infers counterparts from matching filenames or
directories.

### Default locale

The configured locale whose generated URLs are not prefixed by a locale code.
Every site has exactly one default locale.

### Locale

A configured canonical BCP-47 language identifier, such as `en`, `de`, or
`ar`. It has a configured writing direction and one required content source
tree.

### Translation ID

An opaque page metadata value, `translationid`, that explicitly groups known
counterpart pages across locales. A group may be partial but contains at most
one page for each locale.

### Hreflang sitemap alternate

An XHTML sitemap annotation that names every sitemap-included counterpart of a
page, including itself, with its locale code and canonical URL. Heine emits it
only when `[sitemap] hreflang = true` is configured.

## Listings

### Collection

A configured, locale-local ordered set of rendered pages. Pages opt in through
membership; a collection itself does not generate output.

### Collection neighbors

The immediate `before` and `after` members around the current authored page
in one collection's resolved locale-local order. Templates obtain them with
`collection_neighbors(name="…")`; this is separate from directory sibling
navigation.

### Listing page

An authored rendered page that reads a collection. It may enable pagination,
in which case its own output is the first slice and Heine creates later pages.

### Pager

One generated later output of a paginated listing. It uses the authored first
listing page's template model plus its pagination view.

### Taxonomy

A configured classification built from one collection. It turns literal terms
assigned by member pages into generated taxonomy-index and term-listing pages.

### Series

A configured, locale-local ordered reading sequence. Pages declare a positive
position for each Series they join. A Series produces an index and listing
pages, but does not select a collection or alter taxonomy, feed, or search
membership. See the [Series guide](series.md).

### Term

A literal taxonomy value such as `Rust` or `Static sites`. A term is both its
display name and one route component; Heine does not silently slugify it.

## Template boundary

### Filter

A Tera operation applied with `|`, such as `value | markdown`. Heine provides
filters for Markdown rendering and selected checked resource operations.

### Render-facing value

Data Heine makes available to Tera, such as `page`, `data`,
`locale`, or `pagination`. Templates do not receive mutable internal site
state or arbitrary filesystem access.

### Template function

A named Heine operation called from a Tera template, such as `asset()`,
`page()`, `collection()`, `t()`, or `format_date()`. Fixed arguments are
checked at the template call site.
