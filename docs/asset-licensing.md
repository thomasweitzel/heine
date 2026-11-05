# Published-artifact licensing

Use published-artifact licensing to publish rights information for a copied
file or a separate renderer resource. Heine publishes the declared license
texts and makes attribution information available to Tera templates. The site
author remains responsible for choosing declarations that fit each work and
its use.

The [tutorial's published-artifact licensing step](tutorial.md#11-add-published-artifact-licensing-and-attributions)
shows the same convention in a working site.

## License one copied asset

For an individual [copied asset](glossary.md#copied-asset), place a declaration
in a `.license` file beside it and supply the corresponding license text:

```text
content/assets/fonts/example.woff2
content/assets/fonts/example.woff2.license
LICENSES/OFL-1.1.txt
```

For example, the sidecar can contain:

```text
SPDX-FileCopyrightText: 2026 Example Font Authors
SPDX-License-Identifier: OFL-1.1
```

Heine copies the font and license text to the output. The sidecar is not
copied; templates can use its information to build an attribution page. This
follows a REUSE-compatible convention. For several files with the same facts,
or for renderer resources, use the shared declarations described below.

## License texts and sidecars

Put each license text directly in the optional project-root `LICENSES/`
directory. Its filename is an SPDX license or exception identifier, or a local
`LicenseRef-…`, followed by `.txt`. Heine copies it unchanged to
`<build.output_dir>/LICENSES/`. It never downloads, supplies, or rewrites
license texts:
the site author chooses and provides the text that backs each declaration.
Look up standard identifiers in the [SPDX License List](https://spdx.org/licenses/);
use its exact identifier as the filename stem.

Put a sidecar named `<asset filename>.license` next to each copied asset that
needs an attribution. It must contain at least one copyright notice and one
SPDX expression.

The `.license` suffix is reserved for this purpose: a file with that suffix
cannot itself be a copied asset.

Every SPDX license identifier and `WITH` exception in the expression requires
the corresponding `LICENSES/<identifier>.txt` file. A local declaration such
as `LicenseRef-Public-Domain` similarly requires
`LICENSES/LicenseRef-Public-Domain.txt`. Heine checks expressions with the
SPDX license list, diagnoses malformed sidecars at their authored value, and
rejects an orphaned sidecar rather than silently ignoring it.

This rule applies to every identifier named by a compound expression. For
example, `OFL-1.1 AND MIT` requires both `LICENSES/OFL-1.1.txt` and
`LICENSES/MIT.txt`; `OFL-1.1 OR MIT` requires those same two texts. Heine does
not choose one branch of an `OR` expression on the site's behalf.

The license-text directory is local to the site. Heine accepts
local `LicenseRef-…` values, but rejects `DocumentRef-…` and `AdditionRef-…`
forms: those refer to an external SPDX document or a non-standard exception
that this local-file model cannot identify or publish unambiguously.

## Shared declarations and required coverage

Use an adjacent sidecar for one copied asset with facts of its own. When
several files share facts, define one [licensing record](glossary.md#licensing-record)
and select it from an exact [licensing group](glossary.md#licensing-group):

```toml
[licensing]
required = true

[licensing.records.example-fonts]
SPDX-FileCopyrightText = "2000 Example Foundry"
SPDX-License-Identifier = "OFL-1.1"
SPDX-FileComment = "https://example.test/fonts"

[[licensing.groups]]
assets = [
  "assets/fonts/example-roman.woff2",
  "assets/fonts/example-bold.woff2",
]
licensing_record = "example-fonts"
```

`required = true` requires a declaration for every eligible published artifact:
every copied asset and every separate renderer resource in the current build.
Omit `required` when declarations remain optional. Writing
`required = false` is a checked error because omission already selects that
policy.

Each group lists literal published `output_path` values. They have no leading
slash and do not include `site.base_path`. Use the output identity shown by
`attributions()`, not an `asset()` ID, an `asset:` URL, or a public URL. For
example, a shared asset may be `assets/fonts/example.woff2`, while a German
locale overlay is `de/assets/fonts/example.woff2`.

Groups are exact lists, not directory rules or glob patterns. Adding a package
member means adding its output path to the group. If one member has different
facts, remove it from the group and give that asset its own sidecar. A group
and sidecar may not both declare one asset, even when their current values
match. Heine reports both locations rather than choosing a precedence rule.

Record fields use the same rules as sidecars. Copyright notices and SPDX
expressions may be one string or an array of strings. An array contains
separate authored SPDX tag values: it does not imply `AND` or `OR`; express
that relationship inside an SPDX expression. Unknown fields, empty groups,
duplicate members, unavailable paths, invalid expressions, and missing license
texts are checked errors.

Required coverage applies to every copied asset, including an unused file, a
binary file, a locale overlay, and an asset used only by a draft. It also
applies to each separate renderer resource that the current build publishes.
An ordinary build excludes draft pages, so their renderer resources are checked
only by a draft preview. It does not apply to declared content, templates,
data, deployment files, generated pages, embedded renderer markup, sidecars,
or license texts. A full build reports every uncovered copied asset in
deterministic output-path order. A quick build also checks the current asset
catalog, but may leave stale output from an earlier catalog after a deletion or
rename. Use a full build after changing `heine.toml`, deleting or renaming an
asset or sidecar, or changing a license text.

## Renderer resources

A renderer profile that can publish a separate resource may select a default
record. A request may override that choice when the particular resource has
different facts:

```toml
[licensing.records.organization-report]
SPDX-FileCopyrightText = "2026 Example Organization"
SPDX-License-Identifier = "CC-BY-4.0"

[renderers.report]
# renderer settings omitted
licensing_record = "organization-report"
```

```markdown
:::render[report label="Download the report"]
Renderer source
:::
```

Use `licensing_record="other-record"` on a `render` request only when it
publishes a separate resource. Heine rejects it for embedded output. When
strict coverage is enabled, Heine reports a missing record before it starts the
external renderer. If identical renderer bytes come from several requests,
every request must select the same record. One request cannot silently supply
facts for another.

Heine uses REUSE-compatible fields and sidecars, but groups are not
`REUSE.toml` annotations and successful coverage is not a claim of REUSE or
legal compliance.

## Template views

`asset()` returns `licensing` as `null` when no declaration exists. Otherwise it
contains `record`, `copyrights`, `expressions`, `comments`, and `license_texts`.
`record` is `null` for a sidecar. Each
license text has its `id`, base-path-aware `url`, and current-page-relative
`relative_url`.

```tera
{% set image = asset(id="blog/images/portrait.jpg") %}
<img src="{{ image.url }}" alt="Portrait">
{% if image.licensing %}
  <p>{{ image.licensing.copyrights | join(sep=", ") }};
     <a href="{{ image.licensing.license_texts[0].url }}">
       {{ image.licensing.expressions | join(sep="; ") }}
     </a></p>
{% endif %}
```

For a complete deterministic inventory, use `attributions()`. Each item has
`artifact` (`output_path`, `url`, `relative_url`, `absolute_url` when
configured, `kind`, and `media_type`) and the same `licensing` view.
`output_path` is the locale-qualified published path. For example,
`de/images/logo.svg`, rather than the locale-relative ID passed to `asset()`.
It is therefore unambiguous across the whole site and suitable as a key for
localized presentation data. An attribution page is an ordinary authored page,
so a site decides its route, language, and presentation:

```tera
<ul>
{% for item in attributions() %}
  <li><a href="{{ item.artifact.url }}">{{ item.artifact.output_path }}</a> -
      {{ item.licensing.copyrights | join(sep=", ") }}
      ({{ item.licensing.expressions | join(sep="; ") }})</li>
{% endfor %}
</ul>
```

For a renderer resource, `artifact.kind` is `"renderer_resource"` and
`artifact.media_type` is the profile's declared media type. Its `sources` list
names every Markdown request that produced the same bytes. Each source has its
page URL and locale, the declared Markdown input, whether the request produced
a link or image, and its label or alternative text. Copied assets have
`kind = "copied_asset"`, no media type, and an empty `sources` list. Heine does
not infer a copied file's media type from its filename.

A central inventory serves a site-wide attribution page. Some licenses or
information about a work's origin or authorship may require attribution near a
particular use; render that asset’s `licensing` view there as well. Heine
records the declared facts and links, rather than guessing legal presentation
requirements.
The stable published `output_path` also lets a localized template select its
own locale-specific presentation text while retaining the canonical declaration.

## Build behavior

License texts participate in normal output ownership and collision checks.
Full builds publish exactly the current `LICENSES/` tree and, when coverage is
required, establish that every eligible published artifact in the current
model has one declaration. Quick builds validate the current copied-asset
catalog and resources from the pages they render, but, like other quick-build
inputs, do not remove stale output after deletion or discover configuration
changes. Run a full build after changing licensing configuration or deleting
or renaming an asset, sidecar, or license text.
