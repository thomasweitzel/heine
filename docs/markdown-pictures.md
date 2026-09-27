# Color-scheme Markdown pictures

Use a [color-scheme picture](glossary.md#color-scheme-picture) when one
meaningful image has light and dark visual variants. Heine checks both copied
assets and emits a native HTML `picture`, so the browser selects the dark
source only when its `prefers-color-scheme: dark` media query matches.

```markdown
:::picture
alt = "Portrait of Heinrich Heine"
light = "asset:assets/images/heine-light.jpg"
dark = "asset:assets/images/heine-dark.jpg"
caption = "Heinrich Heine, shown in light and dark forms."
:::
```

This is ordinary block-level Markdown content. It works in declared Markdown
files rendered through either `markdown(content="…")` or the `markdown`
filter. It does not make Markdown a Tera2 template or introduce a generic
component language.

## Required fields

`alt`, `light`, `dark`, and `caption` use TOML strings, which keeps translated
text and punctuation unambiguous. `width` and `height` use TOML integers.

| Field     | Meaning                                                                                                               |
|-----------|-----------------------------------------------------------------------------------------------------------------------|
| `alt`     | The HTML alternative text. It may be empty only for a genuinely decorative image.                                     |
| `light`   | A checked `asset:<id>` reference for the ordinary fallback image.                                                     |
| `dark`    | A checked `asset:<id>` reference selected for a dark browser preference.                                              |
| `caption` | Optional visible plain text. When present, Heine wraps the picture in a native `figure` and emits it as `figcaption`. |
| `class`   | Optional non-empty HTML class-token string. Heine normalizes ASCII whitespace and rejects repeated tokens.            |

The two sources must be copied assets below `content/`. Heine rejects an
external URL, a raw public URL, a page reference, or an unknown asset ID at the
field that names it. This retains the usual asset relationship: locale
resolution, `base_path`, versioned URLs, dimensions, and copied-asset licensing
all remain coherent.

Use `caption` for a short visible description, credit, or context that belongs
to the image. It is plain text, not nested Markdown, so links and longer
source discussion remain ordinary prose following the figure. Alternative text
describes the image for nonvisual use; a caption supplies surrounding visible
context and should not merely repeat it.

Use ordinary Markdown images when there is one source:

```markdown
![Portrait of Heinrich Heine](asset:assets/images/heine-light.jpg)
```

Use two ordinary images when the alternatives communicate different
information. A color-scheme picture is for the same image in two visual forms,
not for changing the document's meaning with the selected scheme.

## Dimensions and layout

When Heine knows both source dimensions, it emits the light image's intrinsic
`width` and `height`. This lets the browser reserve layout space before either
image loads. The sources must have the same aspect ratio. If either source has
unknown dimensions, Heine omits automatic dimensions rather than claiming a
ratio it cannot establish.

An author may set integer HTML dimensions explicitly:

```markdown
:::picture
alt = "Heine wordmark"
light = "asset:assets/images/heine-light.svg"
dark = "asset:assets/images/heine-dark.svg"
width = 480
height = 120
:::
```

Both values must be positive. If one value is omitted and Heine knows the light
image's dimensions, it derives the missing value while preserving the ratio.
If its dimensions are unknown, provide both values. When intrinsic dimensions
are known, an authored pair must preserve the light image's ratio. These
attributes do not resize files or replace stylesheet layout rules. Responsive
sizing remains a site stylesheet concern.

## Rendered structure and other Markdown features

The preceding example renders the following native structure, with checked
URLs substituted for the source IDs:

```html
<figure>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="/assets/images/heine-dark.svg">
    <img src="/assets/images/heine-light.svg" width="480" height="120" alt="Heine wordmark">
  </picture>
  <figcaption>Heine wordmark in light and dark forms.</figcaption>
</figure>
```

Without `caption`, Heine emits the `picture` element alone.

When `class` is present, Heine adds its normalized class list to `picture`, and
also to `figure` when a caption creates one. Use `picture.<class>` or
`figure.<class>` when a rule belongs to one element only. A broad `.<class>`
selector matches both elements for a captioned picture.

The directive is a standalone Markdown block. Its opening line is exactly
`:::picture`, and its closing line is exactly `:::`. Fenced code remains
literal, so documentation can show the syntax without escaping it. A malformed
or unterminated block is a checked Markdown error.

Its `alt` text and optional caption each contribute once to a source-level
[search index](search.md). The source IDs and other directive fields do not. A
picture has no heading text, so it does not change a [table of contents](table-of-contents.md). It can appear
in a Markdown input selected for [margin footnotes](margin-footnotes.md);
the directive metadata does not become prose or footnote content.

The browser evaluates `prefers-color-scheme`; Heine does not implement a
site-owned appearance switch or client-side image selection.
