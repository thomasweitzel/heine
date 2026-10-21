# Kits

Use a kit when its complete reference site is a useful starting point for your
site's presentation. A kit supplies one site design, while the site root keeps
ownership of content, configuration, data, deployment files, and local
adaptations. Its required reference site shows the structure and conventions it
expects.

Keep an ordinary site when you want to own every template and presentation
resource. A kit has a specific contract, rather than being a generic theme that
Heine can swap between arbitrary sites.

## Install a kit and copy its reference site

Heine does not download or resolve kits. Install one below `kits/<name>/`, then
copy its `site/` directory into the project root. The copied configuration
selects the installed kit.

```text
my-site/
├── heine.toml
├── content/
├── templates/
└── kits/
    └── paper/
        ├── kit.toml
        ├── templates/
        ├── assets/
        ├── locales/
        ├── LICENSES/
        └── site/
```

The copied `site/` directory is an ordinary site root. It may contain
`heine.toml`, `content/`, `data/`, `templates/`, `locales/`, `deploy/`, and
`LICENSES/`. It does not participate in a build after copying. Heine checks
only that every selected kit has a regular `site/heine.toml` file that selects
the kit containing it.

```toml
# heine.toml
[kit]
name = "paper"
version = "^1.0"
```

`name` is required. `version` is optional, but it gives the site an additional
check against an unintended installed kit release. Heine also checks the
kit's declared compatible Heine-version range before it reads the kit's
templates, assets, messages, or license texts.

Only one kit may be selected. Removing `[kit]` restores an ordinary site, but
any root template that extends a kit template then fails at that reference.

## Describe a kit

Each kit has a `kit.toml` manifest beside its runtime directories. It identifies
the package, states which Heine releases may load it, reserves its Fluent
message prefix, and declares its public interface.

```toml
# kits/paper/kit.toml
name = "paper"
version = "1.0.0"
heine = ">=0.9.0, <0.10.0"
message_prefix = "paper-"

[public]
templates = ["base.tera", "post.tera"]
assets = ["assets/css/paper.css"]
components = ["paper.card"]
```

`name` must match the kit directory. `version` is the kit's semantic version,
and `heine` is the semantic-version requirement for the Heine binary that loads
it. `message_prefix` is non-empty and namespaces every Kit Fluent message and
term. The `[public]` lists are exact: a root site may extend only listed
templates, resolve only listed kit assets through ordinary `asset()` calls, and
invoke only listed components. The [reference](reference.md) defines the
root-owned `[kit]` selection and settings; the [design record](design/kits.md)
explains the compatibility contract.

The package directory, its manifest, and its runtime directories must be
ordinary paths, not symbolic links. This keeps the selected package's source
ownership visible while Heine loads it.

## Adapt public templates

The kit owns its original templates below `kits/paper/templates/`. Their names
always begin with `kit/paper/`; root template names never fall back to them.
The kit manifest declares which originals are public. A copied root adapter can
extend one of those public templates and override its Tera blocks.

```tera
{# templates/post.tera #}
{% extends "kit/paper/post.tera" %}

{% block content %}
  <p>Published by the site.</p>
{% endblock content %}
```

This keeps the customization boundary visible. A root template cannot extend a
private kit partial, and a kit template cannot reach into a root template.
When a kit does not expose a block or public component for the change you need,
override the relevant public template or change the kit itself.

The `kit/` template namespace is reserved even without a selected kit. Do not
create root templates below `templates/kit/`.

## Use kit settings

A kit can declare a small schema of presentation settings in its manifest. The
site may override only those declared keys through `[kit.extra]`. Tables merge
into declared tables; scalar values and arrays replace their defaults.

```toml
# kits/paper/kit.toml
[extra]
accent = "violet"

[extra.navigation]
compact = false
```

```toml
# heine.toml
[kit]
name = "paper"

[kit.extra]
accent = "indigo"

[kit.extra.navigation]
compact = true
```

Kit templates receive the resolved value as `kit.extra`:

```tera
{% if kit.extra.navigation.compact %}
  <nav class="compact">…</nav>
{% endif %}
```

Heine rejects an undeclared key or a value whose type differs from the kit's
default. This preserves a small, checked customization contract rather than
merging arbitrary site configuration into a kit.

## Assets, messages, and components

The manifest's `[public]` table states a kit's supported public interface.
It can declare templates, copied assets, and Tera components.

```toml
[public]
templates = ["base.tera", "post.tera"]
assets = ["assets/css/paper.css"]
components = ["paper.card"]
```

A declared public asset has its ordinary shared `assets/…` ID. A root shared
asset at the same ID overlays it, and a locale-local root asset continues to
use the normal locale lookup. Private kit assets use `kit/paper/assets/…` and
remain available only to kit templates. They are still published, so their
output URLs intentionally show that kit-owned namespace.

Kit Fluent messages and terms use the manifest's prefix, such as `paper-`.
Root Fluent files may replace existing prefixed entries, which lets a site
adjust kit interface wording without editing the kit. Root templates may call
only components declared public by the kit. Components remain kit-owned and
cannot be replaced from the root.

Root `LICENSES/` files and licensing declarations work across root and kit
assets. A root license text with the same logical path replaces the kit text.
See [asset licensing](asset-licensing.md) for the copied-asset rules.

## Develop and update a kit

Develop kit runtime resources in their final `kits/<name>/` locations. Use the
surrounding ordinary root as an integration site: its root templates extend
public kit originals, and its content and data exercise the design as a user
would.

When preparing a release, copy the root's Heine inputs into `kits/<name>/site/`.
Copy `heine.toml`, `content/`, `data/`, `templates/`, `locales/`, `deploy/`,
`LICENSES/`, and other applicable input directories. Do not copy `kits/`,
generated output, VCS data, or unrelated workspace files. The reference site
is a release snapshot, so update it whenever the kit's expected root structure
or example changes.

Kit runtime inputs follow the same rebuild rules as their root counterparts.
A selected kit template, Fluent resource, or license-input change needs a full
build. A selected present asset change can participate in the existing quick
asset update. Changes below `kits/<name>/site/` and changes in non-selected
kits do not trigger a build because they are not runtime inputs.

The [kit design record](design/kits.md) explains the compatibility and
ownership decisions behind this contract.
