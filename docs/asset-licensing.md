# Asset licensing

Heine supports a small, REUSE-compatible convention for declaring the rights
information of copied assets. It keeps licensing beside the asset it describes,
publishes the matching license texts, and exposes the declared information to
ordinary Tera templates. It does not determine whether a declaration satisfies
the law applicable to a particular work or use.

## Layout

Put each license text directly in the optional project-root `LICENSES/`
directory. Its filename is an SPDX license or exception identifier, or a local
`LicenseRef-…`, followed by `.txt`. Heine copies it unchanged to
`public/LICENSES/`.

Put a sidecar named `<asset filename>.license` next to every copied asset that
needs an attribution. A sidecar is never copied as an asset. It must contain at
least one copyright notice and one SPDX expression:

```text
SPDX-FileCopyrightText: 2001–2021 The STIX Fonts Project Authors
SPDX-License-Identifier: OFL-1.1
SPDX-FileComment: https://github.com/stipub/stixfonts
```

The `.license` suffix is reserved for this purpose: a file with that suffix
cannot itself be a copied asset.

Every SPDX license identifier and `WITH` exception in the expression requires
the corresponding `LICENSES/<identifier>.txt` file. A local declaration such
as `LicenseRef-Public-Domain` similarly requires
`LICENSES/LicenseRef-Public-Domain.txt`. Heine validates expressions with the
SPDX license list, diagnoses malformed sidecars at their authored value, and
rejects an orphaned sidecar rather than silently ignoring it.

The license-text directory is deliberately local to the site. Heine accepts
local `LicenseRef-…` values, but rejects `DocumentRef-…` and `AdditionRef-…`
forms: those refer to an external SPDX document or a non-standard exception
that this small local-file model cannot identify or publish unambiguously.

## Template views

`asset()` returns `licensing` as `null` when no sidecar exists. Otherwise it
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
`output_path` is the locale-qualified published path—for example,
`de/images/logo.svg`—rather than the locale-relative ID passed to `asset()`.
It is therefore unambiguous across the whole site and suitable as a key for
localized presentation data. An attribution page is an ordinary authored page,
so a site decides its route, language, and presentation:

```tera
<ul>
{% for item in attributions() %}
  <li><a href="{{ item.asset.url }}">{{ item.asset.output_path }}</a> —
      {{ item.licensing.copyrights | join(sep=", ") }}
      ({{ item.licensing.expressions | join(sep="; ") }})</li>
{% endfor %}
</ul>
```

A central inventory is useful for a site-wide attribution page. Some licenses
or a work’s provenance may require attribution near a particular use; render
that asset’s `licensing` view there as well. Heine deliberately records the
declared facts and links, rather than guessing legal presentation requirements.
The stable published `output_path` also lets a localized template select its
own locale-specific presentation text while retaining the canonical declaration.

## Build behavior

License texts participate in normal output ownership and collision checks.
Full builds publish exactly the current `LICENSES/` tree. Quick builds update
present license texts and re-render pages when a sidecar changes, but—like
other quick-build inputs—do not remove stale output after deletion.
