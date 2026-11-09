# Draft publication design

## Status

Implemented. The focused [drafts guide](../drafts.md) and
[reference](../reference.md#page) define the current user-facing contract.
This record retains the design rationale and accepted boundaries.

## Decision

Heine gives an authored rendered page one explicit unpublished state:

```toml
# content/en/blog/new-article.page
draft = true
template = "post.tera"
title = "An unfinished article"
```

Omitting `draft` means that the page is published. `draft = false` is a checked
error because it has no effect. `draft` belongs only in a `.page` file. It is
not inherited from `_directory.toml`, and a descriptor that declares it is a
checked error.

Ordinary `heine` builds and ordinary `heine serve` runs exclude drafts. The
development server gains an explicit `--drafts` option:

```text
heine serve --root my-site --drafts
```

That command is a local preview. It includes drafts in the same resolved site
model as published pages, then performs the server's existing complete build
and live-reload loop. Heine does not add a `--drafts` option to ordinary
`heine` builds in this version. A command intended for publication must not
also be a convenient way to generate a preview tree containing unpublished
pages.

Draft exclusion happens before page relationships are resolved. Collections,
links, indexes, and generated resources in an ordinary build therefore do not
remember hidden pages.

## Motivation

An unfinished page needs two states. During writing, an author needs to
see it in its real collection, navigation, taxonomy, Series, feed, and search
context. During an ordinary build, that page must not reach generated output
or leak into any relationship derived from it.

Filtering only rendering is insufficient. It would leave a draft in a post
index, a taxonomy term count, an Atom feed, a sitemap, or a search document.
Filtering each relationship and generated output separately would duplicate
policy and eventually miss one. Heine instead resolves one active page set for
the selected build mode. All existing features use only that set.

Drafts are not scheduled publication. Heine does not compare a wall clock to a
date, infer publication from a filename, or provide a generic visibility
language. The only question this feature answers is whether a page belongs to
the ordinary published site now.

## Authored declaration

`draft` is a top-level `.page` field with a single accepted authored value:

```toml
draft = true
```

It is not part of `[extra]`, metadata requirements, a collection
membership, or directory policy. A directory cannot safely decide that every
descendant is unpublished because individual pages need to move independently
between writing and publication.

The following are checked errors at their authored fields:

```toml
draft = false  # omit the field for a published page
draft = "yes" # draft must be a boolean
```

The diagnostic for `false` explains that the field accepts only `true` and
that omission selects ordinary publication. A `draft` key in
`_directory.toml` explains that draft state is page-local rather than
inheritable.

## Build modes

| Invocation             | Active pages              | Intended use                          |
|------------------------|---------------------------|---------------------------------------|
| `heine`                | Published pages only      | Ordinary complete build               |
| `heine --quick`        | Published pages only      | Existing incomplete local update path |
| `heine serve`          | Published pages only      | Ordinary local development            |
| `heine serve --drafts` | Published and draft pages | Local preview of unfinished work      |

`heine serve --drafts` uses the configured output directory, just as ordinary
serving does. Its output is local development output, not a second publication
format. After previewing, run a normal complete `heine` build before deploying
from that directory. The normal full build replaces the preview
tree and removes draft output.

`--drafts` composes with the existing `serve` options such as `--host`,
`--port`, and `--clean-output`. It does not weaken their constraints. `--quick`
continues not to apply to `serve`.

## Resolution model

Source discovery still visits the complete project tree. Heine continues to
check source-root safety, symbolic-link restrictions, path identity and
normalization collisions, TOML syntax, global configuration, directory
descriptors, copied assets, deployment files, and license sidecars regardless
of a page's draft state. A draft must not become a way to hide an unsafe,
ambiguous, or accidentally published project input.

Heine then reads the top-level `draft` declaration from every syntactically
valid `.page` file before page-specific resolution:

- an ordinary build retains only pages without `draft = true`;
- `heine serve --drafts` retains every page; and
- page-specific validation runs only for retained pages.

Before that filtering step, a draft's `[content]` table still reserves its
declared inputs from copied-asset discovery. Every value in that table must be
a string naming a safe path below the page's locale content root. Heine records
the resulting relative path even when the file does not yet exist. It does not
read, parse, or require the file until the draft is retained for preview. A
missing `[content]` table claims no files, so adjacent regular files remain
ordinary copied assets. Content-name template-identifier checks remain
page-specific and therefore run only in a preview that retains the draft.

An excluded draft that would have an effective margin-footnote setting counts
as use of `markdown.margin_footnotes`. This prevents the global
marker-message setting from becoming spuriously unused merely because its only
current users are drafts. An ordinary build does not load that draft's
Markdown, assign note numbers, or format its marker message. A preview checks
all of those relationships normally. Sites without such a draft retain the
existing checked unused-configuration error.

Consequently, an ordinary build may contain an unfinished draft
whose declared content file does not yet exist, whose final template is not
written, whose Markdown is incomplete, or whose metadata would not yet meet a
directory requirement. The preview build checks those facts normally. This is
the boundary: a production build excludes a draft while it is
still in progress, but malformed TOML and a malformed publication declaration
cannot silently escape checking.

Every retained page follows the existing loader and resolver without a second
draft-specific branch. It receives its usual output target, template view, and
relationships. A previewed draft exposes `page.draft = true`; every published
page exposes `page.draft = false`. Heine does not inject a draft banner or
otherwise alter rendered markup. A site template may use that page fact when a
preview needs a visible cue.

## Relationship and generated-output effects

The resolved page catalog is the only source for page links and derived
relationships. Therefore an ordinary build treats a draft exactly as a page that
does not exist in its locale:

- `page()` and Markdown `page:` links to it are checked missing-page errors;
- `page_exists()` returns `false` for its otherwise valid ID;
- translation groups simply omit it, so a published counterpart may have a
  partial language-navigation group;
- sibling navigation, collections, collection neighbors, taxonomies, and
  Series omit it;
- pagination derives only from the remaining collection members;
- feeds omit its entry, sitemaps omit its page URL, and search indexes omit its
  document; and
- templates and generated-resource views cannot enumerate or link to it.

Existing empty-output rules remain intact. A configured locale feed still uses
its checked empty-feed fallbacks, a configured search index still writes its
empty document, and configured taxonomy and Series indexes still render in
every locale. An authored paginated listing still has its first, empty route.
By contrast, a taxonomy term route and an individual Series listing disappear
when their last active member disappears. Sitemap inventory follows those
existing generated-resource facts, never a special draft-only rule.

In a `serve --drafts` preview, the same page participates in all of those
relationships normally. A collection may require its `published` value, a feed
may require its authors and summary, and a template may require its declared
content. Preview does not relax those existing contracts. It simply selects a
larger active catalog before they are checked.

Copied assets remain independent of draft membership. Declared draft content
is reserved from that asset inventory, but every other regular file below
`content/` is a published input, not a child owned by one page. A site may use
an asset from a later published page. Heine copies and license-checks those
assets according to the existing asset contract in every build mode. A draft
must not be used to conceal an asset that is not yet suitable for public
output: do not place such an asset below `content/` until it is ready to
publish.

## Quick builds and the development server

Draft state is a resolved-site relationship boundary. A full ordinary build,
or the development server's complete rebuild, is required after adding,
removing, or changing `draft = true`.

Quick builds retain their existing incomplete-output contract. In particular,
if a previously published page becomes a draft, `heine --quick` can leave its
old output and derived listings in place. If a draft becomes published, a
quick build cannot claim to update all relationships that now include it.
This is not a separate draft exception: quick builds already do not remove
stale output or resolve configuration, relationship, renamed, and deleted
inputs. Documentation must make this consequence explicit where it introduces
draft previewing.

The watched server always performs complete builds. Saving a draft while
running `heine serve --drafts` re-resolves the preview site and reloads only
after successful publication. Saving it while ordinary `heine serve` runs
removes it from the active catalog on the next complete build.

## Diagnostics

Draft diagnostics follow the ordinary authored-source rules:

- the `draft` value, its type, and redundant `false` form identify the
  `.page` source and field span;
- a descriptor-level use identifies its `_directory.toml` field and explains
  the page-local rule;
- a published page that targets an excluded draft reports the link or template
  call site and names the target's draft source path. Its diagnostic explains
  that a local preview can include the target only when it has an effective
  template; and
- preview-only failures name the draft's actual authored template, content,
  metadata, Markdown, or relationship source rather than a generic preview
  wrapper.

Heine does not issue a warning merely because a draft exists. Drafts are an
authored state. A preview that exposes an incomplete draft fails
normally and keeps the previous successful development-server output in place.

## Boundaries

- No directory-level, collection-level, taxonomy-level, or time-based draft
  policy exists.
- No ordinary-build `--drafts` flag exists in the first version.
- No separate preview output root, deployment target, or draft URL namespace
  exists. The development server is explicitly local-only.
- No template-wide `preview` mode flag exists. `page.draft` is sufficient for
  a page template to label its own preview without giving unrelated templates
  a second global execution mode.
- Drafts do not suppress independent source-tree, asset, license, deployment,
  configuration, or descriptor checks.
- Heine does not preserve a stale draft preview when a normal full build is
  requested. A full build replaces the entire managed output tree.

## Starter site and documentation

The starter site should not add an unfinished article merely to
show an invisible ordinary-build feature. Its published content remains a
site rather than a catalog of flags. Fixture-site tests will
exercise both ordinary and preview catalogs.

The public documentation needs:

- a concise draft-publication guide with the authored declaration, preview
  command, output-directory warning, and full-build requirement;
- command and `.page` reference rows, including the accepted-value rule and
  `page.draft` template fact;
- an update to the development-server guide explaining `serve --drafts`; and
- glossary wording only if a durable public term is needed beyond “draft”.

## Acceptance criteria

- A page accepts only `draft = true`; `false`, non-booleans, and descriptor
  declarations report authored spans.
- An ordinary complete build excludes a syntactically valid incomplete draft
  without checking page-specific template, content, metadata, Markdown, or
  relationship facts.
- A draft's declared safe content inputs remain excluded from copied assets in
  an ordinary build, while their missing files remain permissible. Malformed
  draft content declarations report their authored fields rather than risking
  accidental asset publication.
- A draft-only effective margin-footnote setting keeps its global marker
  configuration active in an ordinary build without loading the draft
  source. A preview validates its complete marker and footnote relationships.
- Ordinary builds retain all independent source-tree, asset, licensing,
  deployment, configuration, descriptor, and TOML-syntax checks.
- An ordinary build exposes no draft output or page view, and drafts are
  absent from links, `page_exists()`, translation navigation, sibling and
  collection navigation, taxonomies, Series, pagination, feeds, sitemaps, and
  search indexes, while each configured feature retains its established
  empty-output behavior.
- A configured empty feed, search document, taxonomy index, Series index, or
  authored paginated listing remains present after drafts remove its final
  member. A taxonomy term route and individual Series listing disappear after
  their final active member is drafted.
- A published page's ordinary Markdown or template link to a draft fails at
  the link call site.
- `heine serve --drafts` includes the same draft in all page relationships,
  checks it fully, and exposes `page.draft = true`.
- A normal complete build after preview removes preview-only draft
  output.
- Quick-build tests demonstrate that draft-state transitions require a full
  build and cannot claim complete output.
- Command, guide, reference, glossary when needed, fixture coverage, and the
  changelog are updated with the implemented public contract.
