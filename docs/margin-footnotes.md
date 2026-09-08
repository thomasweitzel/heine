# Margin footnotes

Margin footnotes present ordinary Markdown footnotes beside the paragraph or
heading they qualify. Heine checks the Markdown relationship and supplies
semantic, local note markup. Your template and CSS decide whether a note opens
below prose or in a wide-screen margin.

This is an opt-in presentation for one declared Markdown input on a page. It
does not add another Markdown syntax or change the ordinary endnote
presentation of unselected Markdown sources. Every ordinary `[^label]`
reference still needs a matching definition, as described in the
[reference](reference.md#markdown).

## Start with one page

Enable the existing Markdown extension and name a Fluent message for the
accessible note controls. Heine supplies its `$number` argument when it formats
that message:

```toml
# heine.toml
[markdown]
extensions = ["footnotes"]

[markdown.margin_footnotes]
marker_message = "footnote-marker"
```

```ftl
# locales/en/site.ftl
footnote-marker = Footnote { $number }
```

Select one declared, unsegmented Markdown input on a rendered page. An
unsegmented input is a declared Markdown file without `<!-- segment: … -->`
markers:

```toml
# content/en/guide/chronology.page
template = "guide.tera"

[content]
main = "main.md"

[footnotes]
content = "main"
```

Use standard Markdown references and definitions in `main.md`:

```markdown
Heine rejects ambiguous local times.[^time]

[^time]: During an autumn daylight-saving transition, one wall-clock time can
name two different instants.
```

Render the selected source exactly once with the source-aware function. The
template may use `page.footnotes` to add a layout class without receiving
pre-rendered note HTML:

```tera
<article{% if page.footnotes %} class="has-margin-footnotes"{% endif %}>
  {{ markdown(content="main") }}
</article>
```

Heine replaces the normal endnote section for that source with a numeric marker
and an independent local note instance after its containing prose block. A
repeated reference keeps its number but receives another local instance, so it
does not send a reader back to an earlier paragraph.

The generated structure stays local to that block. The native checkbox is the
interactive control, its localized accessible name comes from `marker_message`,
and the note is both its controlled and described content. The visible number
inside the note repeats the marker's number, helping readers associate notes
when several are open:

```html
<div class="heine-annotated-block">
  <p>Heine rejects ambiguous local times.<sup class="heine-footnote-marker">
    <label for="__heine-footnote-toggle-1">1</label>
  </sup></p>
  <input class="heine-footnote-toggle" id="__heine-footnote-toggle-1"
    type="checkbox" aria-label="Footnote 1" aria-controls="__heine-footnote-note-1"
    aria-describedby="__heine-footnote-note-1">
  <aside class="heine-footnote" id="__heine-footnote-note-1" role="doc-footnote">
    <span class="heine-footnote-number" aria-hidden="true">1</span>
    <p>During an autumn daylight-saving transition, one wall-clock time can
      name two different instants.</p>
  </aside>
</div>
```

The identifiers are implementation details. Heine reserves the
`__heine-footnote-…` family for generated ordinary and margin-footnote
fragments, so its generated headings and footnotes cannot claim one another's
IDs. Trusted raw HTML and templates remain author-owned: Heine does not parse
them to check manually chosen `id` values against that reserved family.

A selected source does not need a footnote. This makes a directory default
useful for a documentation section whose individual pages need notes only
occasionally.

## Add presentation CSS

Heine deliberately does not generate site styling. Without CSS, the native
checkbox controls and their note asides remain visible in source order. This
is a usable baseline.

The following small example hides notes until their own marker is selected,
then places an open note below its paragraph. It preserves a focus indication
when the browser supports `:has()`:

```css
.heine-footnote {
  display: none;
}

.heine-footnote-toggle:checked + .heine-footnote {
  display: block;
}

.heine-footnote-number {
  float: inline-start;
  margin-inline-end: 0.5em;
}

@supports selector(.heine-annotated-block:has(.heine-footnote-toggle:focus-visible)) {
  .heine-footnote-toggle {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .heine-annotated-block:has(.heine-footnote-toggle:focus-visible)
    .heine-footnote-marker label {
    outline: 2px solid currentcolor;
    outline-offset: 2px;
  }
}

@media print {
  .heine-footnote-toggle {
    display: none;
  }

  .heine-footnote {
    display: block !important;
  }
}
```

At a sufficiently wide breakpoint, a site can make `.heine-annotated-block` a
two-column grid and put `.heine-footnote-toggle:checked + .heine-footnote` in
the second column. The note stays in ordinary flow, so several open notes do
not overlap later prose. A tall note can leave space before the next block;
that is the deliberate CSS-only trade-off for dependable local placement.
For printing, keep every note visible after its annotated prose instead of
trying to force a page-margin layout.

The starter site contains a complete responsive and print-safe example.

## What Heine checks

For a selected source, every reference needs exactly one definition, every
definition needs a reference, and definitions must be unique. Errors identify
the original Markdown location; duplicate definitions also identify the first
one.

The first version accepts a reference in an ordinary Markdown paragraph or
heading. It supports ordinary inline formatting around the marker. References
in a list, table, block quote, code block, raw HTML, link, image, or another
footnote definition are errors. A paragraph or heading containing both raw
HTML and a reference is also rejected because Heine cannot safely reconstruct
arbitrary HTML nesting into the local wrapper.

Definition bodies may contain normal block Markdown, including links, lists,
code blocks, math, and copied-asset references. Their headings are ancillary
note content, so they do not appear in a table of contents and receive no
generated heading anchor.

The selected input must be rendered once with `markdown(content="…")`. A
missing call leaves no promised local-note structure; two calls duplicate
generated IDs. Heine reports either as a template error. The `markdown` filter
is still useful for arbitrary text, but it rejects the exact selected source.

## Directory defaults

Documentation pages often use one main Markdown input. A descriptor can select
it for a whole subtree:

```toml
# content/en/guide/_directory.toml
[pages.footnotes]
content = "main"
```

The nearest complete table wins. Tables do not merge. A page or nested
descriptor may write `footnotes = false` only to suppress an inherited setting.
Writing that value when nothing is inherited is a checked error, because it is
inactive structural policy rather than a useful fallback.

Likewise, `[markdown.margin_footnotes]` is a checked error unless at least one
rendered page has an effective setting. The named Fluent message is checked in
every locale that uses it, with the ordinary default-locale fallback.

## Table of contents and builds

A table of contents and margin footnotes may select the same Markdown input.
One `markdown(content="…")` call then supplies both heading anchors and local
notes. A marker never contributes to a heading's visible ToC text or fragment.

Changing the selected Markdown file participates in the normal quick rebuild
behavior. Adding, removing, or changing a page or descriptor `footnotes`
setting changes a resolved relationship, so use a full build. `heine serve`
normally performs the complete rebuild required for that change.

For exact fields and template values, see the [reference](reference.md). The
[glossary](glossary.md#margin-footnote) defines the durable term used here.
