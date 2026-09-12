# Copied-asset licensing coverage design

## Status

This is the accepted and implemented design for copied-asset licensing coverage
and shared declarations. It extends Heine's existing optional
REUSE-compatible asset-sidecar support. The [asset-licensing guide](../asset-licensing.md)
and [reference](../reference.md#licenses-and-asset-sidecars) define the current
user-facing contract.

## Decision

Heine will let a site require a checked licensing declaration for every copied
asset it publishes:

```toml
[licensing]
required = true
```

With this opt-in enabled, each copied asset must receive exactly one resolved
declaration. An adjacent `.license` sidecar remains the normal declaration for
one asset. A `[[licensing.groups]]` table may instead declare one shared set of
SPDX facts for an exact non-empty list of published asset paths:

```toml
[[licensing.groups]]
assets = [
  "assets/fonts/et-book-roman-line-figures.woff2",
  "assets/fonts/et-book-display-italic.woff2",
  "assets/fonts/et-book-bold-line-figures.woff2",
]
SPDX-FileCopyrightText = "2000 Edward Tufte"
SPDX-License-Identifier = "OFL-1.1"
SPDX-FileComment = "https://github.com/edwardtufte/et-book"
```

The group uses the same SPDX field names and rules as an adjacent sidecar. A
group is a single declaration applied to several enumerated copied assets, not
a new license-file format and not a recursive filesystem policy.

Without `required = true`, individual sidecars and groups remain optional.
They attach the same licensing facts to their named assets, while an asset with
no declaration continues to expose `licensing = null`. With the setting, an
uncovered asset is a checked build error.

`required` has one accepted authored value. Omitting it selects optional
declarations; `required = false` is a checked error explaining that omission
already selects that policy. The resolved configuration uses an explicit
optional-or-required coverage enum rather than retaining a raw boolean.

This design uses three related terms consistently:

- a **declaration source** is one authored adjacent sidecar or licensing
  group;
- a **resolved declaration** is the normalized licensing record assigned to
  one copied asset; and
- **coverage** is the policy that permits, or requires, a resolved declaration
  for every copied asset.

## Motivation

The existing model adds a sidecar when an asset needs attribution or licensing
facts. That is appropriate for an
ordinary personal site, but it cannot demonstrate that all copied output has a
declaration. A missed font, image, video, PDF, or binary file remains
publishable without any signal.

Some public-sector and organizational publication policies require a complete
inventory of copied material. Heine can check that narrow property because it
already owns the final copied-asset inventory, including locale overlays and
the distinction between declared page content and copied files. The resulting
guarantee is precise: every copied asset Heine publishes has exactly one
resolved declaration containing one or more copyright notices and one or more
SPDX license expressions.

That guarantee is not legal advice or a declaration of REUSE, SPDX, copyright,
or license compliance. Heine cannot determine whether a declaration is true,
whether a license permits the intended use, whether a required attribution is
adequately presented, or whether an external URL, inline resource, generated
file, deployment configuration, or raw HTML asset has been declared.

Several files from one upstream font or image package commonly share exactly
the same facts. Requiring mechanically duplicated sidecars makes that
relationship hard to maintain. A shared declaration addresses that case, but broad
patterns would weaken coverage: a newly added unrelated file could inherit a
license without an author consciously reviewing it. Heine therefore models a
group as an exact visible list.

## Scope and asset identity

Coverage applies only to the resolved copied-asset inventory. It includes every
regular file below `content/` that Heine copies to managed output, regardless of
extension, whether a page currently links to it, or whether a page is a draft.
It includes shared assets and locale-specific overlays according to their
actual published output paths.

Coverage does not apply to declared page content, `.page` files,
`_directory.toml`, data, templates, Fluent resources, deployment files,
license texts below `LICENSES/`, sidecars themselves, generated pages, or
development-server resources. Those are separate input or output domains.

Any future feature that contributes copied assets to the managed publication
inventory is subject to the root site's coverage policy. Its design must state
the copied output identity used for coverage and group membership, as well as
its declaration ownership rules. This preserves one declaration per published
asset without prematurely adding a second, feature-specific precedence system.

Group `assets` values name literal published copied-asset paths, without a
leading slash and without `site.base_path`. This is the same whole-site
identity exposed as `item.asset.output_path` by `attributions()`. It is chosen
over locale-relative `asset()` IDs because a configuration table must be
unambiguous across shared assets and locale overlays:

```toml
[[licensing.groups]]
assets = [
  "assets/fonts/shared.woff2",
  "de/assets/fonts/localized.woff2",
]
SPDX-FileCopyrightText = "Example Foundation"
SPDX-License-Identifier = "OFL-1.1"
```

The paths name the actual output filenames, including a source version suffix
such as `styles.v2.css`. They never name a versionless
`latest()` lookup, an `asset:` URL, a filesystem path, or an eventual URL with
percent encoding.

Copied assets retain source-derived output filenames. Optional integrity
digests describe bytes but do not rename assets. If Heine later gains
content-addressed copied-asset filenames, that feature must first define a
stable licensing identity rather than treating changing digest names as group
members.

Every group path must pass ordinary portable output-path validation and resolve
to one current copied asset. A valid-looking path that currently resolves to a
page, a license text, generated output, or nothing is a checked error at the
group member. This keeps the declaration tied to the output inventory rather
than permitting stale claims.

## Authored configuration

`[licensing]` is optional. Its accepted fields are `required` and `groups`:

```toml
[licensing]
required = true

[[licensing.groups]]
assets = ["assets/images/logo.svg", "assets/images/logo-dark.svg"]
SPDX-FileCopyrightText = ["2026 Example Studio"]
SPDX-License-Identifier = ["CC-BY-4.0"]
SPDX-FileComment = ["Adapted for this example site"]
```

Each SPDX field accepts one non-empty string or a non-empty array of non-empty
strings. Arrays preserve their authored order in the resolved per-asset facts.
The comment field is optional. The copyright and license-expression fields are
required. Repeated values are preserved rather than deduplicated, matching the
existing sidecar declaration model.

Each `SPDX-License-Identifier` value is one independently authored SPDX tag
value. An array does not imply `AND` or `OR` between its values. An author who
means either relationship writes it in one SPDX expression, such as
`MIT AND Apache-2.0` or `MIT OR Apache-2.0`.

A group `assets` array must be non-empty and contain distinct literal paths.
Its declarations receive the same parsing as an adjacent sidecar:

- every expression is parsed by the SPDX parser;
- every identifier and `WITH` exception needs its matching local
  `LICENSES/<identifier>.txt` text;
- local `LicenseRef-…` identifiers are accepted when their corresponding text
  exists; and
- document-qualified `LicenseRef` values and all `AdditionRef` values remain
  unsupported by Heine's local license-text model.

The group setting is global. It is not directory policy,
page metadata, template data, or an input a template can create. A page cannot
make an asset licensed merely by linking to it.

`[licensing]` and every `[[licensing.groups]]` table reject unknown fields,
like the rest of `heine.toml`. In particular, REUSE configuration fields such
as `path`, `precedence`, and `aggregate` are neither accepted nor ignored.

## Resolution and exclusivity

Heine first discovers the complete copied-asset catalog and then resolves
individual sidecars and groups into one `AssetLicensing` record per output
asset. It uses one shared parser and record constructor for both declaration
forms so that SPDX validation, local license-text lookup, template facts, and
diagnostics cannot diverge.

Configuration loading retains the licensing groups' source text and individual
value spans, but does not resolve their members immediately. Resolution occurs
only after the copied-asset and license-text catalogs both exist. It can then
check every literal group path against the real inventory, retain exact TOML
locations for diagnostics, and apply the same SPDX and license-text rules as a
sidecar.

An asset may receive one declaration source only:

- an adjacent `asset.ext.license` sidecar attaches to its one copied sibling;
- a group attaches to each exact asset named in its `assets` list;
- two groups must not name the same asset; and
- an asset named by a group must not also have an adjacent sidecar.

There is no precedence rule, merge behavior, fallback, or override mechanism.
Such rules would make it difficult to tell which declaration a published asset
actually has. A conflict names both declaration locations and the affected
output asset; it does not claim an arbitrary declaration order.

This rule applies even when a sidecar and group happen to contain the
same facts. Two declarations for one output asset create two sources that can
later drift apart. When one member of an otherwise uniform asset set has
different facts, its author removes that asset from the group's exact list and
gives that asset an adjacent sidecar. The group then remains an accurate
statement about its members, while the exceptional asset visibly owns its
different declaration.

An optional site may use groups to avoid duplicated sidecars. A required site
uses the same resolution pass, followed by one complete coverage check. The
check considers resolved output assets, not source-file counts. If a shared
source is published once for several locale lookups, it needs one declaration
for that one output. If a locale overlay produces another output path, that
output needs its own individual or grouped declaration.

Required coverage is vacuously satisfied when the resolved inventory contains
no copied assets. A group is still never empty: an authored group always makes
a concrete claim and must resolve every listed member.

After successful resolution, every declared asset has the existing
`asset().licensing` view and appears in `attributions()`. The view has no
`group` field: templates need the facts that apply to an asset, not the
authoring mechanism that supplied them. An uncovered asset remains `null` only
when coverage is optional. Required coverage guarantees non-null licensing for
every copied asset before templates render.

## Authoring and maintenance

Use an adjacent sidecar for one asset. A group is for copied assets that have
one shared declaration, such as the files of one upstream font family or a
maintained first-party asset set. It is not a shortcut for licensing a
directory or every asset of
one kind.

Groups trade some maintenance for reviewable coverage. Adding a
same-licensed package member means adding its literal
published path to that package's group. Adding an asset with different facts
means giving it a sidecar instead. Renaming or deleting an asset leaves a
checked unavailable group member; adding an uncovered asset while coverage is
required produces the complete missing-coverage diagnostic. These outcomes
keep the configuration synchronized with the actual output inventory without
making a location in the source tree imply a license.

Groups are optional declarations when `required` is omitted. Their contents
are never optional configuration: malformed SPDX fields, unavailable paths,
duplicate membership, and declaration conflicts are checked errors in both
coverage modes. Optional coverage only permits an otherwise valid copied asset
to have no declaration.

## Diagnostics

All configuration and declaration failures remain checked build errors.
Heine retains source spans for every TOML group value and each sidecar field.

The following failures identify the authored value and provide an actionable
remedy:

- `required = false` explains that the setting is opt-in and should be
  omitted;
- an empty group, duplicate path within one group, unsafe path, or group path
  that is not a current copied output asset points to that group member;
- missing copyright or SPDX fields, empty values, invalid SPDX expressions,
  unsupported references, and missing license texts point to the corresponding
  TOML value or sidecar value;
- a sidecar with no copied sibling remains an orphan-sidecar error; and
- duplicate group membership or group-and-sidecar membership names both
  declaration locations and the affected output asset.

The same checks cover common boundary cases. A group cannot name its `.license`
sidecar, a declared Markdown input, a rendered page, generated output, or a
license text as though it were a copied asset. A shared source published at one
path needs one declaration, while a locale overlay published at another path
needs its own declaration. A draft-only asset is still copied and therefore
covered. A source version suffix, such as `styles.v2.css`, remains part of the
literal group path.

When required coverage finds omissions, Heine reports the full deterministic
set rather than failing at the first asset. Entries are ordered by their
published output paths and name each asset's source file and output path. The
remedy is to add an adjacent sidecar or name the asset in one explicit group.
This is a coverage diagnostic, not a claim that adding arbitrary text makes an
asset legally usable.

## Build modes, watching, and output

Coverage is part of constructing the resolved copied-asset catalog. It runs
before template rendering and output writing in every build mode. A new asset
cannot be copied by `heine --quick` while required coverage is enabled unless
it has a resolved declaration.

Each `heine --quick` invocation reloads configuration and checks the current
catalog. It retains the general incompleteness of quick builds: it can leave
deleted copied assets and their old output behind, and it does not safely
refresh every relationship-derived output. It therefore guarantees only that
every copied asset in the current resolved catalog has a declaration when
coverage is required. It cannot prove that its physical output tree is exactly
the current publication tree.

A full build additionally replaces the managed output tree with exactly the
current resolved publication model. A full build, or the development server's
normal complete rebuild, is required after deleting or renaming an asset,
sidecar, group member, or license text, or after changing `heine.toml`. The
development server watches all of these inputs and publishes only a
successfully resolved catalog.

License texts continue to be copied unchanged under `LICENSES/` and participate
in normal output ownership and collision checks. A group changes no license
text output, and no separate group file is published. Full builds atomically
replace the managed output tree. This feature does not create a server route,
new generated resource, or template-owned output.

## REUSE relationship and boundaries

REUSE supports `REUSE.toml` annotations that may use globs, multiple nested
files, and precedence rules. That is valuable for repository-wide compliance,
especially where a project cannot place sidecars beside every upstream file.
Heine uses REUSE-compatible SPDX fields, local license texts, and adjacent
sidecars, but its groups are not `REUSE.toml` annotations and do not establish
REUSE compliance. Heine does not implement that general
repository model.

Heine reuses familiar SPDX fields, adjacent sidecars for individual binary
assets, local `LICENSES/` texts, and strict expression parsing. Its group
model rejects:

- glob patterns, directory-wide declarations, and recursive inheritance;
- nested licensing configuration;
- precedence, aggregation, overrides, and implicit fallback;
- declarations for files outside the copied-asset inventory;
- automatic license-text downloads or synthesis;
- a compliance badge, generated legal notice, or a claim of legal sufficiency;
- CSS, HTML, JavaScript, Markdown, or template reference analysis; and
- automatic attribution placement or wording.

The first four exclusions preserve the core guarantee: coverage is visible,
exact, and checked against what Heine actually publishes. The final exclusions
keep general legal analysis and presentation decisions outside a static-site
generator's narrow asset relationship model.

## Documentation, starter site, and acceptance criteria

Implementation must update the asset-licensing guide, reference, glossary, and
tutorial so that ordinary optional sidecars remain easy to introduce before
the strict policy is explained. The guide must link to the SPDX License List
and distinguish a checked declaration inventory from legal advice. It
must show literal `output_path` group members and call out common mistakes:
leading slashes, `site.base_path`, URLs, and locale-relative `asset()` IDs do
not name group members.

The starter site should use one shared asset set, such as a font family or
maintained first-party assets, to demonstrate one explicit group. It should
enable required coverage only after every starter-site copied asset has a
declaration. It must keep its
existing attribution page accurate without adding a group-specific presentation
branch.

The implementation is complete only when focused and fixture-site tests cover:

- optional individual sidecars and optional groups producing identical
  per-asset template facts;
- `required = true` covering every copied asset, including non-image binary
  assets, unused assets, shared assets, and locale overlays;
- every uncovered asset appearing in deterministic required-coverage output;
- malformed and redundant `required` values with exact configuration spans;
- empty, duplicate, unsafe, unavailable, overlapping, and sidecar-conflicting
  group members with all relevant source locations;
- group declaration reordering leaving every successful resolved declaration
  unchanged;
- adding an unrelated copied asset leaving existing resolved declarations
  unchanged, while producing one additional uncovered asset when coverage is
  required;
- repeated SPDX fields, arrays, invalid expressions, compound expressions,
  local references, unsupported document-qualified or addition references, and
  missing required license texts for groups;
- unchanged `asset()` and `attributions()` views regardless of whether a
  sidecar or group supplied the record;
- normal, quick, and development-server behavior after asset, sidecar, group,
  configuration, and license-text changes; and
- a complete starter-site build and its attribution output.
