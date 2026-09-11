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

### Added

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

- **Development directory redirects.** `heine serve` now redirects an existing
  generated directory requested without its trailing slash to the slash form,
  retaining query parameters. This gives browsers the correct base URL for
  relative references while keeping directory-index serving a local preview
  convenience.
