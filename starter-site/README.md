# Heine starter site

This directory is a complete multilingual example rather than a required
project layout. Build it from this directory with `heine`, or from the
repository root with `cargo run -- --root starter-site`.

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
scripts, and example assets are available under MIT. The licence text is
[`LICENSES/MIT.txt`](LICENSES/MIT.txt). Files with their own declarations,
including the STIX font and historical portraits, retain those terms. Keep
those declarations with redistributed or modified starter material.

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
