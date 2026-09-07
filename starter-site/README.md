# Heine starter site

This directory is a complete multilingual example rather than a required
project layout. Build it from this directory with `heine`, or from the
repository root with `cargo run -- --root starter-site`.

## Color-scheme image pairs

`templates/components.tera` provides `ui.color_scheme_image` for a guide that
has distinct light and dark artwork. The English Heinrich Heine guide supplies
the two copied-asset IDs and its localized alternative text through this page
metadata:

```toml
[extra.color_scheme_image]
light = "assets/images/heine-light.jpg"
dark = "assets/images/heine-dark.jpg"
alt = "Portrait of Heinrich Heine"
```

`guide.tera` resolves both IDs with `asset()` before it invokes the component.
That keeps the links checked and preserves Heine's base-path and versioned-URL
behavior. The component renders a native HTML `picture`: the dark source is
selected only when the browser reports `prefers-color-scheme: dark`; the light
image is the ordinary fallback. This is browser-preference behavior, not a
site-owned appearance switch.

Keep paired images at the same aspect ratio when possible. The example's
portraits both measure 480 by 540 pixels, so the component emits the light
fallback's dimensions and the browser can reserve the required space. A pair
with different proportions can shift the layout when the selected source
changes; use matched artwork or reserve space in site CSS instead.
