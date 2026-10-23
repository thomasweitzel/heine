# Heine starter site

This directory is a complete multilingual example rather than a required
project layout. Build it from this directory with `heine`, or from the
repository root with `cargo run -- --root starter-site`.

## What this site demonstrates

Open `public/my-project/index.html` after a build, then follow the links as a
reader would. The table connects each visible result to the smallest useful
place in the source. Use the linked guide when you want the full contract,
rather than copying an example without understanding its limits.

| When you need to…                                                                                          | Visit in the built site                                                              | Start in the source                                                                                               | Read next                                                                                                                                           |
|------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| publish one site in more than one language                                                                 | the English home page and its German counterpart under `de/`                         | `heine.toml`, then `content/en/` and `content/de/`                                                                | [Internationalization and localization](../docs/internationalization.md)                                                                            |
| define pages with separate metadata and Markdown                                                           | `/my-project/blog/explicit-composition/`                                             | `content/en/blog/explicit-composition/index.page` and `main.md`                                                   | [Declared content](../docs/declared-content.md)                                                                                                     |
| list posts, split a long list into pages, and let readers browse tags                                      | `/my-project/blog/`, its later pager, and the tag listings                           | `[collections.posts]`, `[taxonomies.tags]`, `content/en/blog/index.page`, and the post descriptors                | [Collections](../docs/collections.md), [pagination](../docs/pagination.md), and [taxonomies](../docs/taxonomies.md)                                 |
| keep unfinished work in a local preview                                                                    | the draft post when building with `heine serve --drafts`                             | `content/en/blog/draft-publication/index.page`                                                                    | [Drafts](../docs/drafts.md)                                                                                                                         |
| build a reading sequence separate from a blog                                                              | `/my-project/heine/` and its next and previous links                                 | `[series]` in `heine.toml` and `content/en/heine/*/index.page`                                                    | [Series](../docs/series.md)                                                                                                                         |
| add a site-owned browser search page                                                                       | `/my-project/search/`                                                                | `[search.site]` in `heine.toml`, `templates/search.tera`, and `kits/starter/assets/js/search.js`                  | [Search indexes](../docs/search.md)                                                                                                                 |
| give feed readers and search engines structured discovery files                                            | `/my-project/atom.xml` and `/my-project/sitemap.xml`                                 | `[feeds.posts]`, `[sitemap]`, and `[site] origin` in `heine.toml`                                                 | [Atom feeds](../docs/atom-feeds.md) and [sitemaps](../docs/sitemap.md)                                                                              |
| present ordinary Markdown with a table of contents, footnotes, code, MathML, media, and right-to-left text | `/my-project/blog/mathml-audio/` and `/my-project/heine/life/`                       | their `index.page` and `main.md` files, plus `[markdown]` in `heine.toml`                                         | [Table of contents](../docs/table-of-contents.md), [margin footnotes](../docs/margin-footnotes.md), and [Markdown facts](../docs/markdown-facts.md) |
| wrap analyzed Markdown in a reusable presentation component or show light and dark image variants          | the note and portrait on `/my-project/heine/`                                        | `content/en/heine/main.md` and `kits/starter/templates/components.tera`                                           | [Markdown block components](../docs/markdown-components.md) and [color-scheme Markdown pictures](../docs/markdown-pictures.md)                      |
| format numbers, dates, and times for the page language                                                     | the metadata on the Math, Markdown, and Media post                                   | `content/en/blog/mathml-audio/main.md`, `kits/starter/templates/post.tera`, and `kits/starter/locales/*/site.ftl` | [Localized values](../docs/localized-values.md)                                                                                                     |
| use a kit while keeping content and site policy in the project root                                        | every page, especially the shared layout and navigation                              | `[kit]` in `heine.toml`, root `templates/`, and `kits/starter/`                                                   | [Kits](../docs/kits.md)                                                                                                                             |
| publish host files and published-artifact licensing information                                            | `/my-project/robots.txt`, `/my-project/.htaccess`, and `/my-project/legal/licenses/` | `deploy/`, `LICENSES/`, licensing settings in `heine.toml`, and `data/*/attributions.toml`                        | [Deployment files](../docs/deployment-files.md) and [published-artifact licensing](../docs/asset-licensing.md)                                      |

Some details deliberately remain site policy. The local audio player uses raw
HTML because browser controls and preload behavior are not Markdown features.
The search script is one dependency-free browser implementation, not a search
engine supplied by Heine. It shows contextual passages from the matching
Markdown sections, including two passages when a result matches in separate
parts of a page. Treat the example as a working starting point, then keep only
the parts your site needs.

The search form controls its excerpt window with `data-context-before` and
`data-context-after`. The example uses 80 and 160 Unicode characters.
It favors title matches, then Markdown sections. Equal scores keep Heine's
page-ID order. Each result is one link to its strongest matching section, or
to the page when that section has no heading anchor. A title-only match shows
only its title.

`kits/starter/assets/js/search.js` is the canonical source for the maintained
starter, documentation, and Zolarwind search clients. Those independently
deployable sites keep complete copied versions. Edit the starter-kit file
first, then replace each copy as a whole. Its leading comment defines the
required search-page hooks, optional controls, and semantic result classes;
site CSS owns their appearance.

## Starter kit

The example selects its bundled `starter` kit through `[kit]` in
`heine.toml`. The kit owns the shared layout, interface messages, stylesheet,
font, flags, search icon, and search script. This root keeps its content,
site data, deployment files, legal declarations, and adapter templates. Each
adapter explicitly extends one public `kit/starter/…` template.

`kits/starter/site/` is the kit's copyable reference-site snapshot. It shows
the root inputs a new site receives after installing the kit. It is not a
second runtime input tree: Heine ignores it while building this starter site.
See the [kit guide](../docs/kits.md) for the package and customization model.

## Reusing the starter material

The starter site's authored content, configuration, templates, stylesheets,
scripts, and example assets are available under MIT. The `LICENSES/MIT.txt`
text records that licence. Files with their own declarations, including the
STIX font and historical portraits, retain those terms. Keep those declarations
with redistributed or modified starter material.

## Custom 404 page

The site renders a default-locale `404.html` page and a German counterpart.
Configure the host to use the default-locale page for missing paths. Heine
produces the document, but the host controls the HTTP response for a missing
request.

## Color-scheme image pairs

The English Heinrich Heine guide uses a native Markdown `picture` block for
distinct light and dark artwork:

```markdown
:::picture
alt = "Portrait of Heinrich Heine"
light = "asset:assets/images/heine-light.jpg"
dark = "asset:assets/images/heine-dark.jpg"
caption = "Heinrich Heine in light and dark forms."
:::
```

The block resolves both IDs during the build, preserving checked links,
base-path handling, and versioned URLs. The caption makes the result a native
HTML `figure` with `figcaption`; without it, the result is a `picture` alone.
The dark source is selected only when the browser reports
`prefers-color-scheme: dark`; the light image is the ordinary fallback. This is
browser-preference behavior, not a site-owned appearance switch.

The example's portraits both measure 480 by 540 pixels, so Heine emits their
dimensions and the browser can reserve space. A pair with different proportions
is rejected because selection must not shift the layout. When either image has
unknown dimensions, provide `width` and `height` in the Markdown block when a
stable layout matters.
