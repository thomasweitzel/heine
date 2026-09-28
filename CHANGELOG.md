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

### Changed

- **Starter-material licensing.** Heine remains EUPL-1.2-or-later, while the
  copyable starter kit and reference-site material are now explicitly MIT.
  Bundled third-party fonts and artwork retain their declared terms. The new
  licensing notes explain that building an independently authored site does not
  place it under Heine's licence.

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
  surface. Kits include copyable reference sites, while the consuming root
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
  labelled `heine-table-scroll` regions for parsed Markdown tables, allowing
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
