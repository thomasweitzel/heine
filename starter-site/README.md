# Heine starter site

This directory is a complete multilingual example rather than a required
project layout. Build it from this directory with `heine`, or from the
repository root with `cargo run -- --root starter-site`.

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
