# Drafts

A [draft](glossary.md#draft) page can stay out of ordinary publication while
it is unfinished:

```toml
# content/en/blog/new-article.page
draft = true
template = "post.tera"
title = "An unfinished article"
```

Run the development server with `--drafts` to see that page in its real site
context:

```bash
/path/to/heine serve --root /path/to/my-site --drafts
```

The preview includes draft pages in page links, collections, pagination,
taxonomies, Series, feeds, sitemaps, and search indexes. `page.draft` is
`true` for a draft and `false` for a published page, so a template can add a
local preview treatment when it has a real reason to do so.

## Publication boundary

An ordinary `heine` build, `heine --quick`, and `heine serve` exclude every
page with `draft = true` before Heine resolves page relationships. A published
page that links to a draft is therefore a checked error, rather than a link to
an unpublished output file. The diagnostic names the target's source path and
its `draft = true` state. It also explains that a local `heine serve --drafts`
preview can include the target when the draft has an effective template.

Removing a final active member also removes its derived taxonomy term or Series
listing. Configured empty outputs keep their usual behavior.

A future `published` value records chronology, not a release schedule. It does
not withhold a page from a build; use `draft = true` until the page should
participate in publication.

`heine serve --drafts` writes the configured output directory, not a separate
preview tree. Before deployment, run a normal complete `heine` build. Its
staged publication replaces preview output and removes draft files.

`heine --quick` remains an incomplete local update. After changing `draft`,
run a full build: a quick build can retain old page files and relationships
from before that publication change.

## While writing

Drafts may be incomplete. In an ordinary build, Heine still
checks source-root safety, TOML syntax, the `draft` declaration, directory
policy, copied assets, deployment files, and licensing. It also reserves paths
named by a draft's `[content]` table, so an unfinished source file is not
mistakenly copied as a public asset.

The page's template, declared content files, Markdown, metadata requirements,
and relationships are checked when the draft is included in a `--drafts`
preview. It prevents an unfinished article from becoming a completed page in
normal publication.

`draft` accepts only one authored value: `true`. Omit it to publish the
page. `draft = false` and a `draft` key in `_directory.toml` are checked errors
because draft state is page-local, not inherited.

The starter site includes an unpublished English blog post dated 17 August
2026. It is absent from the ordinary generated site and is available when you
run its server with `--drafts`.

See the [reference](reference.md#page) for the exact field contract and the
[development-server guide](development-server.md) for local-server behavior.
