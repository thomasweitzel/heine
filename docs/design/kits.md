# Kits design

## Status

Kits are implemented. The [kit guide](../kits.md) and
[reference](../reference.md) define the current user-facing contract; this
record explains the design decisions behind it.

## Decision

Heine provides **kits**, not supposedly interchangeable themes. A kit is a
complete site design with two separate purposes:

- its runtime presentation resources supply templates, assets, and kit UI
  messages to a site that selects it;
- its `site/` directory is a complete, copyable ordinary Heine site that shows
  the content structure, configuration, and data conventions the design was
  made for.

The copied site root is owned by the user. A kit is updated or replaced without
editing its files. Root templates adapt public kit originals through explicit
namespaced references; root assets, messages, and license texts retain their
normal value-overlay behavior. Changing kits is not promised to preserve
content conventions and may require migration.

## Motivation

Conventional SSG themes often include templates that assume particular paths,
front-matter keys, collections, taxonomies, and data shapes. Their expected
site structure is usually documented informally or provided as demo content.
The `themes/` name nevertheless suggests that users can freely replace one
theme with another.

Heine cannot make arbitrary complete site designs interchangeable without
inventing a universal content model. Such a model would be incomplete and
would constrain the explicit content composition Heine supports today. Kits
instead make the design's conventions concrete in one buildable reference
tree, while keeping the runtime layering narrow and predictable.

## Terminology

- **Site**: an ordinary Heine project rooted at a `heine.toml` file.
- **Kit**: one optional selected package below `kits/<name>/`.
- **Kit reference site**: the required `site/` directory inside a kit. It is
  copied into a new project root; it is never read as a build-input layer.
- **Root resource**: a template, asset, message, or legal input owned by the
  user's site root. An ordinary template name addresses this source only.
- **Kit resource**: the selected kit's corresponding runtime input. A kit
  template has a `kit/<name>/` namespace.

## Layout

An installed kit's runtime layout requires these paths:

```text
kits/paper/
├── kit.toml
├── templates/
├── assets/
├── locales/
├── LICENSES/          # only when kit assets need license texts
└── site/
```

Other top-level files, such as a `README.md`, changelog, or test support files,
are package metadata. Heine ignores them.

`site/` has the ordinary root shape. It may contain `heine.toml`, `content/`,
`data/`, `templates/`, `locales/`, `deploy/`, `LICENSES/`, and any applicable
subdirectories. Its templates are root-owned adapters or site-specific
templates; every file below `site/` is reference-site input, not a kit runtime
input.

A site without a kit has its existing ordinary layout unchanged:

```text
my-site/
├── heine.toml
├── content/
├── data/
├── templates/
├── locales/
├── deploy/
└── LICENSES/
```

After installing `paper` and copying its reference site, the combined project
looks like this. The root `templates/post.tera` is an adapter that explicitly
extends the public kit original, `kit/paper/post.tera`.

```text
my-site/
├── heine.toml
├── content/
│   └── assets/
│       └── css/
│           └── paper.css          # optional root asset override
├── data/
├── templates/
│   └── post.tera                  # root adapter for kit/paper/post.tera
├── locales/
├── deploy/
├── LICENSES/
└── kits/
    └── paper/
        ├── kit.toml
        ├── templates/
        │   └── post.tera          # original, named kit/paper/post.tera
        ├── assets/
        │   └── css/
        │       └── paper.css      # overridden only when declared public
        ├── locales/
        ├── LICENSES/
        └── site/                  # reference only, never runtime input
```

## Creating a kit-based site

To start from `paper`, a user installs it below `kits/paper/`, copies the
contents of `kits/paper/site/` into the project root, and builds from that
root. The copied configuration selects the installed kit:

```toml
[kit]
name = "paper"
version = "^1.0" # optional SemVer requirement
```

The reference site's `[kit].name` must equal its containing manifest's `name`.
It therefore selects the kit being distributed, never another kit. A reference
site cannot compose kits through its own copied configuration.

The reference `site/heine.toml` records ordinary root-owned configuration. It
has no runtime status inside the kit. After copying `site/` into a project
root, it becomes that project's ordinary `heine.toml`; the copied site, rather
than the kit runtime layer, owns every setting outside the checked
`[kit.extra]` overlay.

This includes external renderer profiles. A reference site may contain
Markdown `render` requests and declare the profiles they select in its copied
`heine.toml`. The site owner then owns those commands and may adapt them for
their system. A selected kit's runtime resources never add, replace, or run a
profile in an existing root configuration.

Heine does not initially provide a package resolver, an installer, or a
`heine new` command. Copying remains visible. A later convenience
command may automate this procedure without changing its semantics.

## Developing and packaging a kit

A kit author develops runtime resources in their final locations below
`kits/<name>/`. The surrounding ordinary site root is the live integration
harness: its root templates are adapters that name kit originals, and its
content, data, and configuration exercise the kit as a user would.

When preparing a distributable kit, the author copies the ordinary site's
Heine inputs into `kits/<name>/site/`: `heine.toml`, `content/`, `data/`,
`templates/`, `locales/`, `deploy/`, `LICENSES/`, and applicable input
subdirectories. The copy excludes `kits/` itself, generated output, VCS data,
and unrelated workspace files. Kit-owned runtime templates, assets, messages,
and license texts already remain in their final kit locations, so their
references do not need to change during packaging.

`site/` is a release snapshot, not a live mirror of the authoring root. Kit
tests copy its inputs into a temporary ordinary site root, install the
kit at that root's `kits/<name>/` path, and build the result. This checks that
the copied reference site is complete without giving `site/` runtime-input
status or introducing a special development mode. When a kit changes its
assumed site structure, configuration, or reference example, its author
updates the snapshot in the same change. Heine cannot compare an authoring
root with `site/`: an ordinary site root is expected to diverge from its
starting point.

When a reference site exercises an external renderer, its kit test installs
that renderer and grants the corresponding explicit Heine command-line
permission with `--allow-external-renderers --refresh-renderers`. A missing or
failing renderer is a test failure. Heine never skips the request or
substitutes prior output.

## Manifest and kit settings

`kit.toml` is a manifest, not another `heine.toml`. It contains:

```toml
name = "paper"
version = "1.0.0"
heine = ">=0.9.0, <0.10.0"
message_prefix = "paper-"

[public]
templates = ["base.tera", "post.tera"]
assets = ["assets/css/paper.css"]
components = ["paper.card"]

[extra]
accent = "violet"
show_search = true
```

The kit name is a simple identifier and must equal the directory name. It
forms the template namespace and the default component namespace. `version`
is a semantic version. `heine` is a semantic-version requirement for the
Heine release that loads the kit. The message prefix is non-empty and prevents
accidental collision between kit UI messages and site messages.

Semantic-version requirements use the `semver` crate's requirement syntax,
which follows Cargo's familiar comparator and caret conventions. Before
loading any kit runtime resource, Heine always checks its running version
against the manifest's `heine` requirement. An unsatisfied requirement is a
checked build error that identifies both the running Heine version and the
required range.
The diagnostic explains that the site needs a kit release compatible with the
running Heine version, or a Heine release compatible with the installed kit; it
does not suggest changing a compatibility declaration without establishing
that the kit actually supports the resulting version.

This check verifies a kit's declared compatibility with the running Heine
version. It cannot prove that the kit works with every Heine release in its
declared range; the kit author establishes that broader promise through
testing and release discipline.

The site's optional `[kit].version` value is a separate semantic-version
requirement. Heine checks it against the installed kit's `version`. If the
site omits this requirement, any installed kit version that is compatible with
the running Heine version is accepted. An unsatisfied site requirement is a
checked build error at the authored site setting, with the installed kit
version as related context.

The kit's supported integration API consists of the manifest's `[public]`
members, every prefixed kit Fluent message and term, and the `[extra]` schema.
A kit author uses semantic versioning for this public interface. Removing or
renaming a public template, asset, component, kit message or term, or extra
setting requires a major version increment. The message prefix and every
kit-defined message or term using it are part of that interface: a root site
may override a specific existing entry, so removing or renaming that entry is
breaking.
Adding a public member is a minor version change. Adding an optional extra
setting with a manifest default is also a minor version change: sites that do
not override it receive that default. Correcting behavior without changing the
contract is a patch version change.

The values below manifest `[extra]` are defaults and the complete schema for
the site's `[kit.extra]` table. Tables merge recursively down to existing leaf
values. Arrays replace the manifest array as a whole and never concatenate or
merge by index. A manifest schema array must be non-empty and contain values
of one TOML type; every replacement value must have that type. Unknown keys,
missing intermediate tables, type changes, empty schema arrays, and
incompatible array element types are errors at the authored setting. TOML does
not preserve an element type for an empty array, so v1 does not invent one or
use sentinel values. A later explicit schema extension may add typed empty
arrays when a concrete kit requires them. This one explicit, checked
configuration overlay is the only configuration merging kits perform.

Every manifest declaration is checked against the selected kit before runtime
loading begins. A declared public template, asset, or component must name one
existing kit resource exactly once. Template and asset paths use the existing
portable relative-path rules; an asset declaration must use its `assets/`
logical ID. A misleading manifest is an error at its authored declaration,
rather than a later missing-resource failure in the consuming site. Every kit
asset not listed under `[public].assets` is private.

## Runtime sources and precedence

Only one kit may be selected. It contributes these runtime domains:

| Domain              | Kit location             | Root location                                 | Rule                                                                                                                                                                                                                                             |
|---------------------|--------------------------|-----------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Templates           | `kits/<name>/templates/` | `templates/`                                  | Kit originals use `kit/<name>/…`; ordinary names address root templates only. Root templates may name public kit originals explicitly.                                                                                                           |
| Copied assets       | `kits/<name>/assets/`    | `content/assets/`, `content/<locale>/assets/` | Public kit assets use their declared shared `assets/…` ID. A root shared asset may overlay one; locale-local root assets retain their ordinary locale lookup and output behavior. Private kit assets use `kit/<name>/assets/…` and are kit-only. |
| Fluent UI messages  | `kits/<name>/locales/`   | `locales/`                                    | A present root entry may replace an existing prefixed kit message or term.                                                                                                                                                                       |
| Asset license texts | `kits/<name>/LICENSES/`  | `LICENSES/`                                   | A present root text wins for the same logical license-text path.                                                                                                                                                                                 |
| Components          | not applicable           | no replacement path                           | Defined within kit templates; root templates may invoke declared public components, but cannot replace one.                                                                                                                                      |

Root resources with no kit counterpart continue to work normally. A private
kit asset cannot collide with a root asset because its identity and published
path are namespaced. It can still collide with a page, deployment file, or
generated resource that claims that published path, under Heine's ordinary
output-collision checks. Components have no root override path: root templates
may invoke only manifest-declared public kit components. A root component
definition in the selected kit's component namespace is a build error, so
invocation can never quietly become replacement.

Public template and asset paths are literal logical paths in the first version.
Glob patterns are unsupported: the public interface must remain visible,
and a newly added file must not become public merely because it
matches a broad pattern.

The asset rule promotes a manifest-declared kit asset such as
`kits/paper/assets/css/paper.css` to the public logical ID and published path
`assets/css/paper.css`. `content/assets/css/paper.css` may then replace it.
A root `content/<locale>/assets/css/paper.css` file retains its ordinary
locale-local behavior: it publishes at `<locale>/assets/css/paper.css` and that
locale's asset lookup finds it before the shared public kit asset. A public kit
asset is otherwise available through ordinary root asset lookups, including
`asset()`, Markdown `asset:` URLs, and `:::picture` sources.
A private kit asset such as `kits/paper/assets/images/mark.svg` instead has the
logical ID and published path `kit/paper/assets/images/mark.svg`. Root
templates and Markdown cannot address that private ID through `asset()` or an
`asset:` URL, including through a `:::picture` source; kit templates can. As
elsewhere, an authored raw URL is not an asset lookup and is not checked. This
keeps internal kit assets out of the root asset namespace while preserving
relative URLs between private kit assets.
“Private” describes the checked kit API, not browser access: the file is a
published static resource, and its namespaced output path makes
its kit ownership visible. Other regular files in `content/` remain ordinary
site-only copied assets.

Heine does not parse or rewrite CSS. CSS `url(...)` values are ordinary raw
stylesheet URLs, so a kit author must make them match the published layout.
This is the same trade-off as any raw URL in a site: it permits standard CSS
without a second parser, but an incorrect URL cannot become a build diagnostic.

Template source is never inferred from a fallback. An ordinary template name,
such as `post.tera`, resolves only below root `templates/`. A kit original uses
its unambiguous namespace:

```tera
{% extends "kit/paper/base.tera" %}
```

The namespaced form always selects the kit original. Every root-authored
template setting, including `.page`, `_directory.toml`, and generated-resource
template names in `heine.toml`, names a root template only. The copied
reference site therefore provides root adapters such as:

```tera
{% extends "kit/paper/post.tera" %}
```

If that root adapter is removed, its ordinary template selection fails at the
authored page or directory setting. It never falls back to a kit template. A
root template may name only a public kit original through the namespace. Kit
templates name their own originals, including private ones, through the same
namespace in `extends` and `include` references, so root resources cannot
silently alter kit-internal composition. Kit templates cannot name root
templates. The `kit/` template namespace is globally reserved, including for a
site that currently selects no kit, and root templates cannot define resources
beneath it. Kit components must use the kit-name namespace, for example
`paper.card`, and only manifest-declared public components are available to
root templates.

When a site needs different component markup, it changes the root adapter for
the public kit template that invokes the component, using the extension
mechanism above when appropriate, and calls a root-owned component or writes
the required markup there. A kit author should expose a public template or
documented template block when that form of customization is expected.
Private kit partials are not root customization points. For a structural
change, a root adapter overrides a documented block in its
public parent template. Replacing the adapter outright remains possible, but
is a larger, consciously site-owned presentation change.

## Localization

Kit messages are only for kit interface text. Every kit Fluent message and
term must use the manifest's message prefix; Heine checks this when it loads
the kit. The prefix prevents message collisions and keeps kit-internal term
references visibly distinct from site terminology. A root entry using that
prefix must correspond to an existing kit message or term, so a spelling error
in an intended override is diagnosed instead of becoming a dormant root
message. All existing prefixed kit messages and terms are replaceable: Fluent
is reserved for human-facing interface text, not machine-readable state or
hidden kit configuration. Root-owned interface text uses ordinary, non-kit
IDs. Heine also checks that every component defined by a kit uses its kit-name
namespace. These are build errors at the kit-authored or root-authored
definition, not informal authoring conventions.

The lookup order for a message is:

1. root resource for the current locale;
2. kit resource for the current locale;
3. root resource for the configured default locale;
4. kit resource for the configured default locale;
5. a checked build error at the template call site.

For each requested locale, Heine resolves every message and term identity with
that precedence, then creates one Fluent bundle from the winning definitions.
It does not try separate root and kit Fluent lookup passes. Consequently, a
root-overridden message may reference an unmodified kit term, including one
obtained through the default-locale fallback.

This preserves Heine's locale fallback while allowing a site to replace kit
wording for a specific locale. The kit never contributes locale configuration;
the root site remains the source of configured locales and text direction.

If no kit is selected, or a different kit is selected, a root template that
names `kit/<name>/...` fails at that template reference with a migration
diagnostic. A normal unqualified template lookup continues to report its
missing template at the authored page or template reference. Other root
templates and resources remain ordinary root inputs; removing `[kit]` does not
make a root file erroneous merely because it originated in a copied reference
site.

## Assets and licensing

Kit assets and license inputs follow the same copied-asset, REUSE-style
sidecar, source-root safety, and portable-path rules as their root equivalents.
A sidecar belongs to the asset from the source layer that won lookup. If a root
asset overrides a kit asset, the kit sidecar does not carry over.

Selected kit assets join the same resolved asset inventory as root assets.
The existing `attributions()` template view therefore exposes their winning
licensing facts with every other copied asset. Heine does not generate a
separate attribution document.

The root site's `[licensing]` configuration applies to that combined inventory.
A root `[[licensing.groups]]` declaration may name a selected kit asset by its
literal published output path, whether the asset is public or private. It is
then that asset's one declaration source. A group and an adjacent sidecar that
applies to the resolved asset conflict, just as two root declaration sources
do; Heine never chooses one by precedence. This lets a strict root site cover
an otherwise undeclared kit asset without creating a second kit-specific
licensing policy.

A root `LICENSES/<path>` file overrides the selected kit's same logical
license-text path, even when the bytes differ. This lets a site correct a kit
license text without modifying a vendored or submodule kit. Heine exposes the
winning asset and license facts through the existing attribution view; it does
not make a legal-compliance claim. License-text replacement is not manifest
gated: a kit must not make a local legal correction impossible. License-text
paths are independent of copied-asset IDs, so the public or private status of
the asset that cites a text does not namespace or otherwise alter that text's
logical `LICENSES/` path.

Assets, Fluent entries, and license texts are value overlays. Removing a root
value resumes the selected kit's corresponding value, whether the removal is a
user change or corrects an earlier mistake. Templates differ because selecting
a page's structural rendering entry point must never silently change source.

## Source discovery, output, and development server

- Before runtime loading, Heine checks that the selected kit has a regular
  `site/` directory and that `site/heine.toml` selects its containing kit. It
  reads no other reference-site input at that point.
- Heine ignores every kit `site/` directory during source discovery, rendering,
  and watching.
- Apart from that package-completeness check, it reads only the selected kit's
  manifest, templates, assets, locales, and attached license inputs.
- Those runtime kit roots reject symbolic links and obey the existing UTF-8,
  portable-path, collision, output-ownership, integrity, and attribution
  rules.
- A change to a selected kit runtime input triggers the same complete rebuild
  required for a root template, asset, locale, or license-input change.
- A change below `kits/<name>/site/` triggers no build because it is not a
  runtime input.
- Non-selected kits are ignored. Their invalid files cannot affect a build.
- Changing `[kit]`, its version requirement, or the selected name requires a
  full build; quick mode retains its existing configuration-change limitation.

## Boundaries

Kits do not contribute or merge these root domains at build time:

- `heine.toml`, except for the checked `[kit.extra]` overlay described above;
- `content/` pages or declared content;
- `data/`;
- `deploy/`;
- root legal declarations other than license inputs attached to selected kit
  assets.

Kits do not imply multiple-kit composition, package resolution, arbitrary
remote loading, a plugin API, a generic transformation language, or a
universal semantic content-role model.

Kit templates do not participate in unqualified root-first fallback. A site
selects root-owned template entry points, and root adapters name kit originals
only where the site depends on them.

## Rejected alternatives

### A visual theme without a reference site

This leaves content requirements as informal documentation or template errors.
The required `site/` tree states a kit's intended structure in files that can
be inspected and built.

### A full runtime overlay of kit and root configuration, content, and data

Merging would make page ownership, declared-content lookup, arrays, collection
configuration, metadata requirements, and diagnostics ambiguous. It would
also make a kit update silently alter site content. The narrow presentation
overlay avoids a second composition system.

### A universal content-role or mapping language

No finite role schema covers arbitrary sites. Such a system would either fail
to support real content or grow into a competing template/query language.
Explicit Tera composition remains the escape hatch for site-specific needs.

### Root-first template fallback

Letting an ordinary root name fall back to a kit template would make deleting a
root adapter silently change a page's rendering contract. Separate root and
kit template namespaces make template ownership visible and turn a missing root
entry point into a checked error at its authored selection.

### Components and site-owned adapters as the whole feature

Components can be used inside kits, but they do not communicate a complete
design's required content shape. They would provide a visual toolkit, not a
site structure.

### Multiple kits or kit components

Multiple precedence layers make template, asset, message, configuration, and
license resolution order-dependent. Selecting one kit preserves one root-over-kit
precedence layer. Revisit composition only after a concrete use case justifies it.

### Glob-based public declarations

Globs would shorten a large manifest, but they would also expose future files
accidentally and make the kit's public interface less visible. Literal
paths make each public member visible and make semantic-version changes
auditable.

### Treating kits as freely swappable

A kit can be overridden and updated independently, but another kit may expect
different content and data conventions. Changing kits is therefore a migration,
not a build-time switch that Heine claims will preserve a site.

## Implementation acceptance criteria

Implementation must include focused and fixture-site coverage for:

- selection, missing-kit, name, manifest, site-kit version and
  Heine-kit compatibility requirements, extra-schema, reference-site
  self-selection, and symlink errors with authored spans;
- malformed semantic versions and requirements in both `heine.toml` and
  `kit.toml`, plus rejection of an incompatible kit before Heine reads any of
  its runtime resources;
- materializing a kit reference site into a root beside its kit and building
  it successfully;
- root-only template selection, public namespaced kit-template extension, and
  a missing root adapter that cannot fall back to a kit original;
- documented-block customization of a public kit template, with private kit
  partials inaccessible to root templates;
- root precedence for declared public assets, messages, and license texts,
  including ordinary locale-local asset lookup over a public kit asset;
- public and private kit asset identities, including public use through
  `asset()`, Markdown `asset:` URLs, and `:::picture` sources; root rejection
  of every private asset lookup; and the published private output path;
- manifest declarations for missing, duplicate, or unsafe public templates,
  assets, and components;
- errors for inaccessible private kit templates, kit templates that reference
  root templates, and root definitions that attempt to occupy the globally
  reserved `kit/` namespace;
- missing-selected-kit diagnostics and component namespace isolation,
  including a root definition that attempts to occupy the selected kit
  namespace and a root call to an existing non-public kit component;
- locale fallback across root and kit messages and terms, including a
  root-overridden message that references a kit term;
- message-prefix and component-namespace enforcement at the responsible kit
  or root-authored spans;
- kit asset integrity, image metadata, licensing, strict root coverage and
  licensing groups, group-and-sidecar conflicts across root and kit sources,
  attributions, and output collisions;
- source discovery and watcher behavior for selected, non-selected, and
  `site/` kit paths;
- full and quick-build behavior, including the existing quick-build limits.

Public documentation, the reference, tutorial, glossary, and a starter example
must be updated only when the feature is implemented.
