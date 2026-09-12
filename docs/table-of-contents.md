# Table of contents

A page can expose a table of contents from one declared Markdown document.
Heine derives its headings and fragment identifiers from that source, while
the template decides where and how to show the navigation.

## Start with one page

Choose the declared content name, not its filename, and render it with the
source-aware `markdown()` function:

```toml
# content/en/guide/install.page
template = "guide.tera"
title = "Install Heine"

[content]
main = "main.md"

[toc]
content = "main"
min_level = 2
max_level = 3
```

An effective ToC requires a rendered authored page. Its selected declaration
must be one unsegmented `.md` file, rendered exactly once with
`markdown(content="…")`.

Navigation labels are interface text, so keep them in Fluent even for a page
that currently exists in one language:

```ftl
# locales/en/site.ftl
toc-label = Table of contents
```

Add the corresponding message to every configured locale.

```tera
{# templates/guide.tera #}
{% extends "base.tera" %}

{% block content %}
  <article>
    <h1>{{ page.title }}</h1>
    {% if page.toc and page.toc.headings %}
      <nav aria-label="{{ t(id="toc-label") }}">
        <ol>
          {% for heading in page.toc.headings %}
            <li class="toc-depth-{{ heading.depth }}">
              <a href="#{{ heading.fragment }}">{{ heading.text }}</a>
            </li>
          {% endfor %}
        </ol>
      </nav>
    {% endif %}
    {{ markdown(content="main") }}
  </article>
{% endblock %}
```

The selected `main.md` may contain headings at every level. When the template
renders it with `markdown(content="main")`, Heine adds IDs to all of them, but
only levels 2 and 3 appear in `page.toc.headings` here. A template can use the
numeric `depth` class for indentation without constructing a fragile nested
list.

`page.toc` is `null` when a page has no effective setting. It is an object
with the selected `content` name and a flat source-order `headings` array. Each
heading has `level`, `depth`, `text`, and `fragment` fields.

If the same content is also selected for [margin footnotes](margin-footnotes.md),
this one `markdown(content="…")` call supplies both the heading
anchors and the local notes.

## Apply one default to a directory

Documentation sections often use the same primary document. A descriptor can
provide that recurring policy for its descendants:

```toml
# content/en/guide/_directory.toml
[pages]
template = "guide.tera"

[pages.toc]
content = "main"
min_level = 2
max_level = 3
```

The closest descriptor's complete ToC table is the default. A page can provide
its own complete `[toc]` table to replace it. The tables do not merge, so a
page that changes the heading range repeats `content` as well.

When one inherited page should not have a ToC, use this page-level opt-out:

```toml
toc = false
```

It is permitted only when it suppresses an inherited setting. The same
`toc = false` form under `[pages]` suppresses an inherited directory default
for a nested subtree. This prevents inactive configuration from accumulating.

## Anchors and rendering

Heine derives fragments from parsed Markdown text. It uses NFC-normalized,
Unicode-lowercase text, preserves letters and numbers, turns punctuation and
spaces into hyphens, and adds deterministic suffixes for duplicates. For
example, two `## Introduction` headings receive `introduction` and
`introduction-2`. A heading with no usable letter or number begins with
`section` instead.

Raw HTML headings are not Markdown headings, so they neither appear in the
ToC nor receive generated IDs. Heine also does not inspect manually authored
HTML IDs for collisions.

Use `markdown(content="…")` exactly once for the selected source. The familiar
filter still renders arbitrary text:

```tera
{{ data.notice | markdown }}
```

If a ToC-enabled page omits its selected source, renders it more than once, or
passes the exact source through `| markdown`, Heine reports a template error.
This keeps every generated fragment attached to one rendered heading. A
transformed copy cannot be recognized reliably, so do not use it as the source
of an enabled ToC.

The full field and template contract is in the
[reference](reference.md#page), including errors for missing, non-Markdown,
segmented, and unrendered-page cases. The
[starter site](../starter-site/) includes a compact guide section using this
pattern in normal document flow, which works without a fixed sidebar on narrow
screens.
