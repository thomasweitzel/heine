# Changelog

All notable user-facing changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Until 1.0, Heine uses `0.y.0` for deliberate public contract additions or
changes that can require an author migration. It uses `0.y.z` for
backward-compatible fixes and documentation improvements. Each incompatible
change names its smallest migration.

## Maintaining this file

Update the `Unreleased` section in the same change that alters a user's
authored input, generated output, command behavior, supported environment, or
documented public contract. Group related work into one entry: a changelog
describes the difference a user experiences, not each commit or file changed.
Do not record internal refactors, routine dependency updates, test-only work,
or starter-site visual adjustments unless they change that public experience.

Use a non-empty `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, or
`Security` subsection. Start every entry with a bold, short name, then state
what changed and why it matters. When an existing site, template, command, or
integration needs adjustment, label the smallest complete remedy as
**Migration:**. An intentional tightening that makes formerly accepted
authored input fail is incompatible and needs that guidance too.

At release, review `Unreleased` for omissions, move its contents unchanged
under `## [<version>] - YYYY-MM-DD`, and begin a new empty `Unreleased`
section above it. The heading version must match `Cargo.toml`; released entries
are corrected only to repair an error or explain a missed material migration.

## [Unreleased]

### Fixed

- **Asset and renderer edge cases.** Version-like asset names without a base
  name are copied normally, and very large renderer timeouts no longer cause
  an overflow.
- **Renderer diagnostics and cleanup.** Heine now bounds retained renderer
  standard error while continuing to drain it, and attempts to terminate and
  reap the direct renderer if writing its input fails.
- **Output recovery diagnostics.** If publication fails and Heine cannot
  restore the previous output, the error now identifies its backup and both
  filesystem failures.

## [0.12.1] - 2026-11-07

### Fixed

- **MathML escaping.** Less-than signs, ampersand operators, and custom operator
  names are escaped in generated MathML, preserving their meaning and preventing
  operator names from being interpreted as HTML.
- **Root indices.** The reference now explains how to group a multi-part root
  index to avoid incorrect output from the built-in TeX parser.

### Changed

- **Syntax highlighting.** The updated highlighter improves highlighting for
  some CSS constructs and changes how some code fragments are colored.

## [0.12.0] - 2026-11-04

### Added

- **Publication catalogs.** Templates can list and select works from local
  BibTeX files, sharing bibliography inputs with page citations.

### Fixed

- **Bibliography aliases.** Conflicting keys and aliases now fail with both
  declarations instead of resolving a citation to the wrong work.
  **Migration:** Remove or rename each conflicting alias.
- **Bibliography URLs.** An assembled URL no longer fails because unrelated
  bibliography text contains consecutive hyphens.

## [0.11.0] - 2026-11-01

### Added

- **Citations.** Pages can cite local BibTeX entries with an in-text CSL style
  and place a checked bibliography from their template. Directory defaults can
  share input selections across pages.

### Changed

- **Starter-site search excerpts.** The maintained search client now uses an
  80/160 preferred context and bounds word-boundary expansion at 100/200 code
  points, so an unusually long token cannot produce an unbounded excerpt.

### Fixed

- **Renderer placement.** Renderer output before a Markdown component or margin
  footnote no longer invalidates the later source plan.
- **Component headings.** A Setext heading at the start of a Markdown component
  body no longer includes the component fence in its table-of-contents text or ID.
- **Component footnote checks.** Margin footnotes now use the same Markdown
  structure as component rendering, so errors identify the rendered document's
  footnote relationships and retain locations in the authored source.

## [0.10.1] - 2026-10-25

### Fixed

- **Search text boundaries.** Nested Markdown blocks and image alternatives
  now remain separate words in search indexes, word counts, and reading-time
  estimates.

## [0.10.0] - 2026-10-21

### Changed

- **Published-artifact licensing.** Named licensing records now provide one
  SPDX declaration for exact copied-asset groups and separate renderer
  resources. Required coverage, `attributions()`, and `asset().licensing`
  now expose that shared published-artifact inventory.

## [0.9.9] - 2026-10-21

### Changed

- **Starter-site tour.** The starter-site README now maps each demonstrated
  feature to its rendered route, authored files, and focused guide.

### Fixed

- **Literal colon-directive syntax.** Directive-looking text in Markdown code,
  raw-HTML blocks, and HTML attributes now remains literal.

### Changed

- **Block directive fences.** `picture`, block `render`, and block
  `component` directives now share one fence rule: an opening fence has at
  least three colons and its closing fence may be longer. Heine reserves
  directive-looking forms in a list, quote, or paragraph continuation rather
  than giving them an accidental meaning.
  **Migration:** Move a block directive out of a list item or block quote, or
  begin it after a paragraph break. Write literal syntax as inline or fenced
  code.

## [0.9.8] - 2026-10-20

### Added

- **Markdown block components.** Authored Markdown can now use a checked
  `:::component[...]` block to pass one analyzed Markdown body to a named Tera
  component. Headings, checked links, search text, and footnotes remain part
  of the page while the component controls only presentation around its body.

## [0.9.7] - 2026-10-16

### Fixed

- **Nested margin-footnote diagnostics.** Heine now explains that a footnote
  definition cannot contain another footnote reference, instead of suggesting
  that links in definitions are unsupported.

## [0.9.6] - 2026-10-11

### Added

- **Template relationship catalogs.** Templates can now use `collections()`
  and `taxonomies()` to inspect every configured relationship in the current
  or an exact locale. `collection()` now accepts `locale=`, while `taxonomy()`
  exposes the resolved ordered term catalog that generated taxonomy indexes
  already receive.

### Fixed

- **Localized-value rendering performance.** Heine now analyzes each Fluent
  message's number and date-time requirements while it loads localization
  resources, rather than repeating that analysis for every `t()` call. Large
  sites therefore avoid redundant Fluent syntax-tree walks while rendering.

- **Decimal precision diagnostics.** `format_number()` and Fluent `NUMBER()`
  now explain when a requested minimum fraction precision exceeds the implicit
  default maximum of three, including the explicit maximum that resolves it.

## [0.9.5] - 2026-10-04

### Added

- **Generated-listing sitemap boundaries.** Taxonomy definitions and the
  global Series configuration can now use `sitemap = false` to exclude their
  complete generated listing families, including every derived pager, from a
  configured sitemap. Sitemap configuration can also use
  `include_pagers = false` to omit only derived pager pages while retaining
  every listing's first page.

- **Localized dates and times.** Templates can now use `format_time()` and
  `format_datetime()`, while Fluent messages can use `DATETIME()`. They format
  RFC 3339 instants with the configured editorial time zone and bundled ICU
  data; calendar dates remain deliberately date-only values.

- **Localized decimal numbers.** Templates can now use `format_number()` and
  Fluent messages can use bare numeric placeables or `NUMBER()`. Both use the
  supplying locale and bundled ICU data, while checked diagnostics reject a
  numeric argument that Fluent cannot represent without changing its value.

### Fixed

- **Fluent function diagnostics.** Misspelled Fluent functions now fail while
  Heine loads the resource and identify the function in the `.ftl` source,
  rather than failing later at an unrelated `t()` template call.

- **Editorial time-zone data.** Builds now always use Heine's bundled IANA
  time-zone database. A machine's `TZDIR` or installed zoneinfo files can no
  longer change publication-date resolution or other editorial-time-zone
  output.

## [0.9.4] - 2026-09-30

### Changed

- **Workspace renderer output names.** Workspace renderers now receive a
  host-owned primary output filename with the checked result extension, such
  as `result.svg` or `result.pdf`. This supports file-oriented tools that infer
  their format from the destination filename while preserving Heine's exact
  one-file output rule.

- **Starter-material licensing.** Heine remains EUPL-1.2-or-later, while the
  copyable starter kit and reference-site material are now explicitly MIT.
  Bundled third-party fonts and artwork retain their declared terms. The new
  licensing notes explain that building an independently authored site does not
  place it under Heine's license.

### Fixed

- **Editorial publication dates.** `format_date()` now converts RFC 3339
  timestamps into the configured editorial time zone before it selects a
  calendar date. A post published at local midnight therefore displays its
  authored publication day while its machine-readable timestamp remains UTC.

- **Plain code highlighting.** Unlabelled fenced and indented Markdown code
  blocks now use the configured plain-code highlighter. They therefore receive
  the same generated light and dark code styling as named language blocks,
  while explicitly unknown language names retain the configured diagnostic or
  plain fallback policy.

### Security

- **Private vulnerability reporting.** The repository now provides a security
  policy with a private reporting address and the checked boundary for
  externally permitted renderers.

## [0.9.0] - 2026-09-27

### Added

- **Embedded SVG renderer classes.** Embedded SVG renderer requests may now
  attach checked CSS class tokens to the returned root `svg` element. This lets
  site CSS style intentionally monochrome inline SVG, including with
  `currentColor`, without changing cached renderer output or adding a wrapper.

- **Generated color-scheme renderer pictures.** A `result = "picture"`
  profile now runs fixed light and dark renderer variants from one Markdown
  request and emits one native `picture` element with digest-addressed image
  resources. Picture requests support checked captions, dimensions, and shared
  `:::picture` CSS classes.

- **Renderer result caching.** Successful stream and workspace results,
  including generated resources and source bundles, now use typed,
  integrity-checked cache entries. `--refresh-renderers` refreshes every
  selected request, and Heine removes incomplete cache artifacts once when a
  renderer build session starts.

- **Generated renderer resources.** SVG, image, and linked-resource requests
  now publish one digest-addressed output below `heine-renderers/` and replace
  their Markdown request with checked semantic HTML. Identical renderer bytes
  share one generated file, and collision reports retain every Markdown request
  that produced the shared resource. Full builds remove superseded resources
  with the previous managed output tree.

- **Workspace renderer inputs.** Embedded renderer requests can now run a
  configured workspace profile with a checked body, declared file, or declared
  bundle staged into an isolated temporary layout. Heine expands only complete
  workspace placeholders, accepts one checked primary output file, and reuses
  one cleaned workspace slot per profile for the build session.

- **Stream renderer adapters.** Root-owned renderer profiles can now turn
  explicit Markdown requests into checked embedded HTML, SVG, or MathML by
  running a configured stream program only with
  `--allow-external-renderers`. Successful results use an integrity-checked
  local cache, and `--refresh-renderers` deliberately re-runs them.
  **Migration:** no existing Markdown changes; configure a profile and pass
  the explicit permission flag for each build that may execute it.

- **Checked kits.** A site may select one local kit with a manifest-declared
  public template, asset, component, message, and presentation-settings
  interface. Kits include copyable reference sites, while the consuming root
  retains content and configuration ownership. This makes a complete design
  reusable without treating unrelated site structures as interchangeable
  themes.

- **Multilingual sitemap alternates.** Sites may opt into
  `sitemap.hreflang = true` to emit checked, locale-sorted XHTML alternate
  sets for explicit, sitemap-included `translationid` counterparts. This keeps
  search-engine language discovery tied to the same authored relationships as
  Heine's language-switcher data.

- **Checked absolute URLs.** Configuring `site.origin` now exposes the
  base-path-aware canonical root as `site.url` and `absolute_url` on known
  page, asset, and generated-resource views. This supports canonical and
  metadata markup without exposing a raw origin or accepting unchecked
  template URL construction. **Migration:** configure an HTTP(S)
  `site.origin` before a template reads either new field.

- **Markdown word counts and reading-time estimates.** Every declared Markdown
  input now exposes a deterministic Unicode `word_count` through
  `page.markdown.<name>`. Sites may opt into checked locale-specific
  `reading_time` estimates with `[reading_time.words_per_minute]`, keeping the
  chosen policy separate from the source fact.

- **Color-scheme Markdown pictures.** A standalone `:::picture` block now
  connects checked light and dark copied assets to one native HTML `picture`,
  with an optional semantic `figure` caption. It keeps Markdown documentation
  and prose independent of template markup while preserving source-spanned
  asset diagnostics, intrinsic layout dimensions, and source-level search text.

- **Semantic responsive Markdown tables.** Sites can opt into localized,
  labeled `heine-table-scroll` regions for parsed Markdown tables, allowing
  site CSS to provide keyboard-accessible narrow-screen horizontal scrolling
  without changing native table semantics. Trusted raw HTML tables remain
  author-owned. **Migration:** no existing table configuration changes. To add
  responsive regions, configure `[markdown.tables].scroll_message` and define
  that Fluent message in the default locale, with ordinary locale fallback or
  translations as appropriate.

- **Copied-asset licensing coverage.** Sites can opt into
  `licensing.required = true` to require one checked REUSE-compatible
  declaration for every copied asset Heine publishes. Exact
  `[[licensing.groups]]` declarations let a coherent upstream or first-party
  asset set share facts without duplicated sidecars, while conflicts and
  incomplete coverage name their authored or copied paths deterministically.

- **Draft publication previews.** Pages may declare `draft = true`; ordinary
  builds exclude them before resolving links and generated relationships, while
  `heine serve --drafts` previews them in their real site context. This lets an
  unfinished page remain private without making published navigation, feeds,
  indexes, or sitemaps inconsistent.

  Search documents now expose each entry's `draft` fact and therefore use
  schema format `2`.
  **Migration:** update a search client that checks `format` to accept `2` and
  its boolean `draft` field.

- **Checked Series.** Rendered pages can join one or more named, locale-local
  reading sequences with independently checked positions, generated listings,
  pagination, and deliberate template-controlled navigation. This gives a
  reading order a first-class relationship without overloading collections,
  taxonomies, or page weight.

- **Margin-footnote presentation.** A page can opt one declared Markdown input
  into local, semantic footnotes that CSS presents beside their prose on wide
  screens and below it elsewhere. This keeps references near their context
  without making JavaScript or a new Markdown notation part of the contract.

- **Accessible Markdown task lists.** Task-list checkboxes now carry their
  authored checked state and accessible labels. This preserves ordinary
  Markdown authoring while making the generated control understandable beyond
  its visual checkmark.

### Changed

- **Compact pager windows.** `pagination.navigation.window` now retains only
  the first and last page plus the configured number of adjacent pages on each
  side of the current page. Values range from `1` through `3`, which keeps the
  generated navigation compact and gives `window` one purpose.
  **Migration:** change values `4` or `5` to `3` or less; update templates or
  snapshots that expected neighboring links beside the first or last page.

- **Search result fragments.** Search-document format `3` replaces each
  document's single `text` string with ordered `sections`, each carrying text
  and an existing table-of-contents fragment when one applies. This lets a
  browser link a matching result to its nearest rendered heading without
  guessing from HTML.
  **Migration:** update a search client to index `sections[].text`; join those
  strings when it needs one page-wide field, and append a non-null section
  fragment to the document URL when it presents a section-level result.

- **Exactly rendered table-of-contents sources.** A page with an effective
  table of contents must render its selected Markdown source exactly once with
  `markdown(content="…")`. This keeps generated heading fragments unique and
  makes fragment-aware search results point at a real rendered location.
  **Migration:** replace an omitted or repeated selected-source render with
  one `markdown(content="…")` call in the page's template.

- **Explicit sibling-weight opt-out.** `weight = false` now excludes a page
  from sibling ordering, replacing the former magic string
  `weight = "None"`. This makes the authored opt-out match Heine's other
  boolean exclusions and keeps the resolved ordering model finite.
  **Migration:** replace every `weight = "None"` with `weight = false`.

- **Checked ordinary footnote references.** A `[^label]` reference without a
  definition now fails at its Markdown source instead of appearing as literal
  text. This preserves the checked relationship promised by the `footnotes`
  extension. **Migration:** add `[^label]: ...`, remove the reference, or
  write `\[^label]` when literal text is intended.

- **Reserved generated footnote fragments.** Ordinary and margin-footnote
  fragments now use the `__heine-footnote-...` namespace, avoiding collisions
  with generated heading fragments. **Migration:** if authored raw HTML,
  templates, CSS, or inbound links depended on an old generated footnote ID,
  update them to the new generated output; avoid the reserved namespace for
  future authored IDs.

### Fixed

- **Renderer failure diagnostics.** External renderer output now appears
  separately from Heine's request label. Workspace body diagnostics identify
  that renderer paths and line locations refer to an isolated staged input.

- **GNU release rehearsal.** The Ubuntu 22.04 GNU builder now records its
  observed glibc requirement inside the container that owns the build output.
  This lets the following artifact upload read the result without a
  cross-container filesystem-permission failure.

- **Rust 1.88 release checks.** The declared minimum supported Rust toolchain
  now passes the same lint gate used by release builds. Local quality commands
  explicitly select Rust 1.88.0, so contributors run the supported-toolchain
  gate before publication.

- **Draft-reference diagnostic paths.** Errors for a published template or
  Markdown page reference to an excluded draft now name the project-relative
  draft source, rather than the machine-specific absolute path. This keeps
  authored relationship diagnostics portable and focused on the site source.

- **Literal fenced footnotes.** Fenced code examples containing `[^label]` no
  longer trigger an ordinary-footnote missing-definition diagnostic. This keeps
  documented Markdown source literal while retaining checked references in
  prose.

- **Development directory redirects.** `heine serve` now redirects an existing
  generated directory requested without its trailing slash to the slash form,
  retaining query parameters. This gives browsers the correct base URL for
  relative references while keeping directory-index serving a local preview
  convenience.
