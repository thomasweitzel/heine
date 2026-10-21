# Asset licensing

Use asset licensing when copied files need published rights information or an
attribution page. Heine follows a REUSE-compatible convention: a declaration
stays beside the [copied asset](glossary.md#copied-asset) it describes, or one
shared declaration names an exact asset set. Heine publishes the matching
license texts and exposes the declared information to Tera templates. The site
author remains responsible for whether those declarations fit a particular work
or use.

The [tutorial's asset-licensing step](tutorial.md#11-add-asset-licensing-and-attributions)
shows the same convention in a working site.

## Layout

Put each license text directly in the optional project-root `LICENSES/`
directory. Its filename is an SPDX license or exception identifier, or a local
`LicenseRef-…`, followed by `.txt`. Heine copies it unchanged to
`<build.output_dir>/LICENSES/`. It never downloads, supplies, or rewrites
license texts:
the site author chooses and provides the text that backs each declaration.
Look up standard identifiers in the [SPDX License List](https://spdx.org/licenses/);
use its exact identifier as the filename stem.

Put a sidecar named `<asset filename>.license` next to every copied asset that
needs an attribution. A sidecar is never copied as an asset. It must contain at
least one copyright notice and one SPDX expression:

```text
SPDX-FileCopyrightText: 2001-2021 The STIX Fonts Project Authors
SPDX-License-Identifier: OFL-1.1
SPDX-FileComment: https://github.com/stipub/stixfonts
```

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

Use an adjacent sidecar for one copied asset. When several files have exactly
the same licensing facts, such as a
family of font files or a first-party site asset set, a site may record one
[licensing group](glossary.md#licensing-group) in `heine.toml` instead:

```toml
[licensing]
required = true

[[licensing.groups]]
assets = [
  "assets/fonts/example-roman.woff2",
  "assets/fonts/example-bold.woff2",
]
SPDX-FileCopyrightText = "2000 Example Foundry"
SPDX-License-Identifier = "OFL-1.1"
SPDX-FileComment = "https://example.test/fonts"
```

`required = true` requires a declaration for every copied asset Heine
publishes. Omit `required` when declarations remain optional. Writing
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

All group fields use the same rules as sidecars. Copyright notices and SPDX
expressions may be one string or an array of strings. An array contains
separate authored SPDX tag values: it does not imply `AND` or `OR`; express
that relationship inside an SPDX expression. Unknown group fields, empty
groups, duplicate members, unavailable paths, invalid expressions, and missing
license texts are checked errors.

Required coverage applies to every copied asset, including an unused file, a
binary file, a locale overlay, and an asset used only by a draft. It does not
apply to declared content, templates, data, deployment files, generated
resources, sidecars, or license texts. A full build reports every uncovered
asset in deterministic output-path order. A quick build also checks the current
asset catalog, but may leave stale output from an earlier catalog after a
deletion or rename. Use a full build after changing `heine.toml`, deleting or
renaming an asset or sidecar, or changing a license text.

Heine uses REUSE-compatible fields and sidecars, but groups are not
`REUSE.toml` annotations and successful coverage is not a claim of REUSE or
legal compliance.

## Template views

`asset()` returns `licensing` as `null` when no declaration exists. Otherwise it
contains `copyrights`, `expressions`, `comments`, and `license_texts`. Each
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
`asset` (`output_path`, `url`, `relative_url`) and the same `licensing` view.
`output_path` is the locale-qualified published path. For example,
`de/images/logo.svg`, rather than the locale-relative ID passed to `asset()`.
It is therefore unambiguous across the whole site and suitable as a key for
localized presentation data. An attribution page is an ordinary authored page,
so a site decides its route, language, and presentation:

```tera
<ul>
{% for item in attributions() %}
  <li><a href="{{ item.asset.url }}">{{ item.asset.output_path }}</a> -
      {{ item.licensing.copyrights | join(sep=", ") }}
      ({{ item.licensing.expressions | join(sep="; ") }})</li>
{% endfor %}
</ul>
```

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
required, establish that every copied asset in that tree's current publication
model has one declaration. Quick builds validate the current copied-asset
catalog as well. They update present license texts and re-render pages when a
sidecar changes, but, like other quick-build inputs, do not remove stale
output after deletion. Run a full build after changing licensing configuration
or deleting or renaming an asset, sidecar, or license text.
