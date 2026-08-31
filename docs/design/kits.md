# Kits design

## Status

This is an accepted implementation design for a future Heine feature. Kits are
not implemented yet, so this document is not part of the current user-facing
contract.

## Decision

Heine will provide **kits**, not supposedly interchangeable themes. A kit is a
complete site design with two separate purposes:

- its runtime presentation resources supply templates, assets, and kit UI
  messages to a site that selects it;
- its `site/` directory is a complete, copyable ordinary Heine site that shows
  the content structure, configuration, and data conventions the design was
  made for.

The copied site root is owned by the user. A kit is updated or replaced without
editing its files; the user overrides supported kit resources in normal root
directories. Changing kits is not promised to preserve content conventions and
may require a deliberate migration.

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
  user's site root.
- **Kit resource**: the selected kit's corresponding runtime input.

## Layout

An installed kit has exactly this top-level layout:

```text
kits/paper/
├── kit.toml
├── templates/
├── assets/
├── locales/
├── LICENSES/          # only when kit assets need license texts
└── site/
```

`site/` has the ordinary root shape. It may contain `heine.toml`, `content/`,
`data/`, `templates/`, `locales/`, `deploy/`, `LICENSES/`, and any applicable
subdirectories. Files below it are examples and initial root overrides, not
kit runtime inputs.

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
looks like this. The root `templates/post.tera` is an intentional override of
the kit template with the same logical path.

```text
my-site/
├── heine.toml
├── content/
│   └── assets/
│       └── css/
│           └── paper.css          # optional root asset override
├── data/
├── templates/
│   └── post.tera                  # root template override
├── locales/
├── deploy/
├── LICENSES/
└── kits/
    └── paper/
        ├── kit.toml
        ├── templates/
        │   └── post.tera          # overridden by the root file above
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

Heine does not initially provide a package resolver, an installer, or a
`heine new` command. Copying is intentionally visible. A later convenience
command may automate this procedure without changing its semantics.

## Manifest and kit settings

`kit.toml` is a manifest, not another `heine.toml`. It contains:

```toml
name = "paper"
version = "1.0.0"
heine = ">=0.2.0, <0.3.0"
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

The site's optional `[kit].version` value is a separate semantic-version
requirement. Heine checks it against the installed kit's `version`. If the
site omits this requirement, any installed kit version that is compatible with
the running Heine version is accepted. An unsatisfied site requirement is a
checked build error at the authored site setting, with the installed kit
version as related context.

The manifest's `[public]` table is the kit's supported integration API. A kit
author uses semantic versioning for this surface: removing or renaming a
public template, asset, component, message prefix, or extra setting requires a
major version increment. Adding a public member is a minor version change.
Adding an optional extra setting with a manifest default is also a minor
version change: sites that do not override it receive that default. Correcting
behavior without changing the contract is a patch version change.

The values below manifest `[extra]` are defaults and the complete schema for
the site's `[kit.extra]` table. Tables merge recursively down to existing leaf
values. Arrays replace the manifest array as a whole and never concatenate or
merge by index. A manifest schema array must be non-empty and contain values
of one TOML type; every replacement value must have that type. Unknown keys,
missing intermediate tables, type changes, empty schema arrays, and
incompatible array element types are errors at the authored setting. This one
explicit, checked configuration overlay is the only configuration merging kits
perform.

## Runtime sources and precedence

Only one kit may be selected. It contributes these runtime domains:

| Domain              | Kit location             | Root location       | Rule                                                                                                        |
|---------------------|--------------------------|---------------------|-------------------------------------------------------------------------------------------------------------|
| Templates           | `kits/<name>/templates/` | `templates/`        | Root wins only for a manifest-declared public template path.                                                |
| Copied assets       | `kits/<name>/assets/`    | `content/assets/`   | Both use IDs beginning with `assets/`; root wins only for a manifest-declared public asset ID.              |
| Fluent UI messages  | `kits/<name>/locales/`   | `locales/`          | Root messages may override kit messages with the kit's declared prefix.                                     |
| Asset license texts | `kits/<name>/LICENSES/`  | `LICENSES/`         | Root wins for the same logical license-text path.                                                           |
| Components          | not applicable           | no replacement path | Defined within kit templates; root templates may invoke declared public components, but cannot replace one. |

Root resources with no kit counterpart continue to work normally. A root
resource that collides with a private kit template or asset is a build error
showing both locations and the kit's declared public alternatives. Components
have no root override path: root templates may invoke only
manifest-declared public kit components.

Public template and asset paths are literal logical paths in the first version.
Glob patterns are deliberately unsupported: a public override surface is a
small compatibility API, and a newly added file must not become public merely
because it matches a broad pattern.

The asset rule intentionally maps `kits/<name>/assets/css/paper.css` and
`content/assets/css/paper.css` to the same logical asset ID,
`assets/css/paper.css`. Other regular files in `content/` remain ordinary
site-only copied assets and cannot shadow a kit asset accidentally.
`content/assets/` is Heine's existing root-site asset domain; a kit uses its
shorter `assets/` presentation-resource root. Templates address either source
through the shared logical asset ID, for example
`asset(path="assets/css/paper.css")`, rather than through a source-tree path.

Kit templates are reachable by their ordinary logical path through the
root-first lookup and by an unambiguous original namespace:

```tera
{% extends "kit/paper/base.tera" %}
```

The namespaced form always selects the kit original. A root override may use
it to extend a kit template rather than replace it. A root template may name
only a public kit template through this namespace; kit templates may also name
their own private originals. Kit components must use the kit-name namespace,
for example `paper.card`, and only manifest-declared public components are
available to root templates.

When a site needs different component markup, it overrides the public kit
template that invokes the component, using the extension mechanism above when
appropriate, and calls a root-owned component or writes the required markup
there. A kit author should expose a public template or documented template
block when that form of customization is expected.

## Localization

Kit messages are only for kit interface text. Every kit Fluent message and
term must use the manifest's message prefix; Heine checks this when it loads
the kit. The prefix prevents message collisions and keeps kit-internal term
references visibly distinct from site terminology. Heine also checks that
every component defined by a kit uses its kit-name namespace. These are build
errors at the kit-authored definition, not informal authoring conventions.
The lookup order for a message is:

1. root resource for the current locale;
2. kit resource for the current locale;
3. root resource for the configured default locale;
4. kit resource for the configured default locale;
5. a checked build error at the template call site.

This preserves Heine's locale fallback while allowing a site to replace kit
wording for a specific locale. The kit never contributes locale configuration;
the root site remains the source of configured locales and text direction.

If no kit is selected, or a different kit is selected, a root template that
names `kit/<name>/...` fails at that template reference with a clear migration
diagnostic. A normal unqualified template lookup continues to report its
missing template at the authored page or template reference.

## Assets and licensing

Kit assets follow the same copied-asset and REUSE-style sidecar rules as root
assets. A sidecar belongs to the asset from the source layer that won lookup.
If a root asset overrides a kit asset, the kit sidecar does not carry over.

A root `LICENSES/<path>` file overrides the selected kit's same logical
license-text path, even when the bytes differ. This lets a site correct a kit
license text without modifying a vendored or submodule kit. Heine exposes the
winning asset and license facts through the existing attribution view; it does
not make a legal-compliance claim.

## Source discovery, output, and development server

- Heine ignores every kit `site/` directory during source discovery, rendering,
  and watching.
- It reads only the selected kit's manifest, templates, assets, locales, and
  attached license inputs.
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

## Intentional boundaries

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

## Rejected alternatives

### A visual theme without a reference site

This leaves content requirements as informal documentation or template errors.
The required `site/` tree is a more reliable and inspectable statement of a
kit's intended structure.

### A full runtime overlay of kit and root configuration, content, and data

Merging would make page ownership, declared-content lookup, arrays, collection
configuration, metadata requirements, and diagnostics ambiguous. It would
also make a kit update silently alter site content. The narrow presentation
overlay avoids a second composition system.

### A universal content-role or mapping language

No finite role schema covers arbitrary sites. Such a system would either fail
to support real content or grow into a competing template/query language.
Explicit Tera composition remains the escape hatch for site-specific needs.

### Components and site-owned adapters as the whole feature

Components are useful inside kits, but they do not communicate a complete
design's required content shape. They would provide a visual toolkit, not a
coherent starting site.

### Multiple kits or kit components

Multiple precedence layers make template, asset, message, configuration, and
license resolution order-dependent. One selected kit keeps the mental model
small. Revisit composition only after a concrete use case justifies it.

### Glob-based public override declarations

Globs would shorten a large manifest, but they would also expose future files
accidentally and make the kit's compatibility surface less visible. Literal
paths keep each public override deliberate and make semantic-version changes
auditable.

### Treating kits as freely swappable

A kit can be overridden and updated independently, but another kit may expect
different content and data conventions. Changing kits is therefore a deliberate
migration, not a build-time switch that Heine claims will preserve a site.

## Implementation acceptance criteria

Implementation must include focused and fixture-site coverage for:

- selection, missing-kit, name, manifest, kit-version, Heine-compatibility,
  extra-schema, and symlink errors with authored spans;
- copying a kit reference site into a root and building it successfully;
- root precedence for declared public templates, assets, messages, and license
  texts;
- errors for private template and asset collisions, with both locations;
- namespaced kit-template extension, missing-selected-kit diagnostics, and
  component namespace isolation;
- locale fallback across root and kit messages;
- message-prefix and component-namespace enforcement at kit-authored spans;
- kit asset integrity, image metadata, licensing, attributions, and output
  collisions;
- source discovery and watcher behavior for selected, non-selected, and
  `site/` kit paths;
- full and quick-build behavior, including the existing quick-build limits.

Public documentation, the reference, tutorial, glossary, and a natural starter
example must be updated only when the feature is implemented.
