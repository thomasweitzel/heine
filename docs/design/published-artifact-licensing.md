# Published-artifact licensing

## Status

Exploratory. This record proposes an extension of Heine's implemented
copied-asset licensing model. It also consolidates group declarations: records
would become the one configuration-level place where SPDX facts are authored,
and groups would select a record rather than repeat those fields. It does not
yet change configuration, template views, or coverage rules.

## Problem

Heine can already identify every copied asset that it will publish. A site may
attach REUSE-style SPDX facts with a sidecar or an exact licensing group, and
`required = true` makes an undeclared copied asset a build error.

That model ends at the copied-asset boundary. A renderer adapter can publish a
PDF, image, or resource-materialized SVG below `heine-renderers/`. Heine owns
that file's output path, bytes, and Markdown request, but it cannot attach a
licensing declaration to the file. Under the current contract, renderer
resources also fall outside required copied-asset coverage.

A documentation site may, for example, publish a CC-BY report PDF or a diagram
that incorporates licensed third-party icons. Its readers can download the
file independently of the page that introduced it. Today, the site has three
unsatisfactory choices: maintain a separate unchecked attribution list, treat
the result as an ordinary copied asset and give up the checked renderer
relationship, or pre-render it into `content/`. The last two options can
require committed build output or make every build depend on an external
renderer.

The missing relationship is narrow: an author needs to connect one renderer
resource that Heine publishes with one declared set of copyright and SPDX
facts. Named records provide that declaration without repeating facts across
groups and renderer requests.

## Current boundary

The implemented asset-licensing feature covers copied regular files below
`content/`, including unused files, binary files, draft-only files,
locale-specific overlays, and public kit assets. It publishes matching local
`LICENSES/` texts, exposes `asset().licensing`, and lists declared copied
assets through `attributions()`.

It does not cover declared Markdown inputs, page output, templates, data,
Fluent resources, deployment files, license texts, sidecars, generated pages,
feeds, sitemaps, search indexes, or renderer resources. That distinction is
intentional. A rendered page is a representation of the site's authored
content, not an independently acquired file to which Heine can infer rights.

An embedded renderer result also remains part of its page. Only a separate
renderer resource, such as a PDF download or image file, becomes a candidate
for this design.

## Two complementary responsibilities

This proposal does not make Heine a general repository-compliance tool.

### Repository sources

Projects that need source-tree coverage should use the
[REUSE tool](https://reuse.software/) alongside Heine. REUSE records licensing
facts for source code, Markdown, templates, renderer source bundles, and
other repository files. Its specification recommends file-local headers where
possible and adjacent `.license` files for files that cannot contain a header.
It can also generate an SPDX document for that source inventory.

Heine should continue to accept its own focused sidecars and exact groups
without claiming that they make a repository REUSE-compliant. In particular,
Heine must not adopt `REUSE.toml` glob rules: a broad pattern can assign facts
to a newly published file without the author reviewing that file.

### Published artifacts

Heine should track the files it publishes only when it has a precise,
author-controlled relationship to an independently usable artifact. The first
eligible artifact kinds are:

- a copied asset, which Heine already supports; and
- a separate renderer resource, whose bytes and digest-addressed output path
  Heine already owns.

This is an output inventory, not a statement that every output file is an
independently licensed work. It deliberately excludes generated HTML, feeds,
sitemaps, search indexes, and embedded renderer fragments.

## Candidate contract

### Reusable licensing records

The current group syntax repeats SPDX facts when more than one exact asset set
or renderer uses the same facts. A site should be able to name those facts
once in a root-owned record:

```toml
[licensing]
required = true

[licensing.records.organisation-report]
SPDX-FileCopyrightText = "2026 Example Organisation"
SPDX-License-Identifier = "CC-BY-4.0"
SPDX-FileComment = "Generated from the organisation's report source."

[[licensing.groups]]
assets = ["assets/reports/example.pdf", "assets/reports/example-data.csv"]
licensing_record = "organisation-report"
```

The table key is a unique, local record identifier. It uses lowercase ASCII
letters, digits, hyphens, and underscores. A group selects exactly one record.
Adjacent sidecars remain the natural declaration for one copied file and do
not need a record.

Choose the smallest declaration that describes the relationship. Use a sidecar
when one copied file needs facts of its own. Use a record and an exact group
when several copied files share facts. Use a record when copied files and
renderer resources share facts, even if the copied set has only one file. The
record then keeps one declaration attached to every authored cause of those
published artifacts.

Heine parses records with the same SPDX expression and local license-text
rules that it already applies to sidecars and groups. A record that names a
license identifier or exception still requires the matching
`LICENSES/<identifier>.txt` file. Unknown record IDs, empty fields, a group
without `licensing_record`, SPDX fields in a group, and an identifier outside
that character set are checked configuration errors. Every record must contain
at least one copyright notice and one SPDX expression. A named record may
remain unused, just as an unused renderer profile remains valid.

Records improve authoring without weakening review: groups still enumerate
their exact output paths. A record supplies facts; it never selects files.
Keeping SPDX fields only in records gives configuration one authoritative place
for those facts.

### Renderer-resource declarations

A renderer profile may select a record as its default for separate resources:

```toml
[renderers.typst-report]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.1"
inputs = ["body", "file", "bundle"]
result = "link"
media_type = "application/pdf"
extension = "pdf"
licensing_record = "organisation-report"
```

A profile may set that default when at least one of its declared result modes
publishes a separate resource. An embedded request does not use the default.
Heine rejects a profile default only when that profile can never publish a
separate resource.

A Markdown request may select another record when one profile legitimately
produces differently licensed work:

```markdown
:::render[typst-report licensing_record="partner-report" label="Download the report"]
= Report
:::
```

The request selection overrides the profile default. It does not accept SPDX
strings, copyright text, paths, or arbitrary template expressions. The
declaration must name one existing root licensing record, so a diagnostic can
point to both the request and the record when necessary.

`licensing_record` is valid only when the selected request publishes a
separate resource. Heine rejects it for embedded HTML, MathML, and SVG
results, and for any request whose selected result mode cannot write a
resource. A resource request may inherit a profile record, select its own
record, or have no declaration.

When `required = true`, the last case is an error at the renderer request
before Heine starts the external program. That keeps a missing declaration from
causing an avoidable process invocation. With optional coverage, it remains
publishable but does not appear in the attribution inventory, matching current
optional copied-asset behavior.

### One declaration for one output

Renderer resources use a digest-derived output path. Two requests can therefore
produce the same public resource. Heine must resolve declarations after it has
grouped those identical resources:

- no declaration produces one undeclared resource;
- one record produces one licensed resource;
- a mixture of declared and undeclared requests is a checked error; or
- different record IDs for the same resource are a checked error that labels
  each request selection or profile default.

The same resource cannot have different configured media types. That conflict
is also a checked error: one published file has one declared media type.

Even records with currently identical SPDX fields remain distinct declaration
sources. Treating them as interchangeable would allow their facts to drift
without a new review of the shared output.

The resource does not need a fictitious adjacent sidecar. Its stable authored
identity is the renderer request or profile field, not its changing digest
path.

### Coverage and output inventory

The existing `required = true` policy should require a declaration for every
eligible published artifact. A site that publishes a renderer resource must
therefore add a profile default or request-level record before the build can
publish that resource. This deliberately gives required coverage its complete
meaning: it cannot overlook an independently downloadable file that Heine
publishes.

Heine resolves copied assets and renderer resources into one deterministic
licensed-publication catalog before it validates the output plan. The catalog
does not need another hash or filesystem scan: renderer resources already have
validated bytes and output paths, while copied assets already have their
resolved output inventory.

`attributions()` should list every declared eligible artifact in a stable,
useful order: copied assets by output path, then renderer resources by their
first source page's output path and Markdown position. A changed resource
digest therefore does not reorder an attribution page. Its generic shape
should be:

```tera
{% for item in attributions() %}
  <a href="{{ item.artifact.url }}">{{ item.artifact.output_path }}</a>
  {{ item.artifact.relative_url }}
  {% if item.artifact.absolute_url %}
    {{ item.artifact.absolute_url }}
  {% endif %}
  {{ item.artifact.kind }}
  {{ item.artifact.media_type }}
  {{ item.licensing.record }}
  {{ item.licensing.expressions | join(sep="; ") }}
  {% for source in item.sources %}
    {{ source.kind }} {{ source.page.locale }}:
    <a href="{{ source.page.url }}">{{ source.text }}</a>
  {% endfor %}
{% endfor %}
```

`item.artifact.kind` initially has the values `copied_asset` and
`renderer_resource`; templates must not assume that this remains a closed set.
`output_path` is the exact public identity, not presentation text. It is
stable for copied assets but changes when a renderer's returned bytes change.
`media_type` is `null` for copied assets: Heine does not infer a media type
from a filename extension. `relative_url` has the same current-page-relative
meaning as the copied-asset attribution view. `absolute_url` is present only
when `site.origin` is configured, matching other template URL views.

Every renderer-resource item therefore also exposes one or more `sources`.
Each source identifies the page and declared Markdown input that requested the
resource, including its page URL and locale. Its `kind` is `link` or `image`,
and its text is the request's existing accessible text: `label` for a link or
`alt` for an image. Resource links already require a label, and resource images
already require an alt value, so the view needs no fallback text. These values
give an attribution page useful visible text and a link back to the page that
uses the resource. `media_type` identifies a renderer resource's configured
media type. Sources sort by page output path and then by their Markdown source
position.

`item.licensing.record` is the selected record's stable local identifier, or
`null` for a sidecar. A localized attribution page may use that key for shared
prose, while `sources` distinguish several resources that use the same record.
Copied-asset items expose an empty `sources` list.

`attributions()` is a site-wide inventory, not a locale-filtered view. A page
in one locale may therefore list a renderer resource whose sources exist only
in another locale. A template that needs a locale-specific presentation can
filter the deterministic `sources` list by `source.page.locale`.

The existing `asset()` helper remains copied-asset-only. Its `licensing` view
gains the same optional `record` identifier when a group selects a record;
sidecars still produce `null`. Since `item.asset` no longer accurately names
every attribution item, this candidate replaces it with `item.artifact` and
updates the starter kit and guides in the same change.

Heine records facts and URLs. It does not choose attribution wording or place
credits beside a particular use. An author whose license requires local credit
must use the available declaration facts in that page's template or prose.

## Source provenance remains authored

Heine cannot derive the license of a renderer result from its file extension,
renderer profile, source bundle, or installed program. A Typst source bundle
may contain first-party work, third-party fonts, or a template under different
terms. A renderer may also carry its own runtime dependencies that do not
become part of the returned resource.

The chosen record is therefore an authored assertion. A record comment can
identify an upstream source or relevant adaptation, but Heine does not
interpret that comment or determine whether the declaration is legally
correct. This preserves the same boundary as copied-asset sidecars.

## Other paths into a site

This proposal covers only output relationships Heine already owns.

Today, Markdown `:::render` requests are the only renderer-resource producer.
If a later feature can publish an independently usable generated resource, it
must use this same artifact catalog and licensing-record selection path rather
than add another attribution mechanism.

- A raw HTML `src`, literal CSS `url()`, JavaScript string, or third-party
  embed remains unchecked. Heine neither owns those bytes nor knows whether
  the URL identifies a durable resource.
- An external CSS or JavaScript builder should write final files before Heine
  reads them. When those files are copied below `content/`, current asset
  licensing already covers them. The exploratory
  [external asset-manifest design](external-asset-manifests.md) may later add
  another checked source-to-output relationship. If it does, it should select
  these same licensing records rather than create a second attribution system.
- Deployment files and ordinary generated documents remain outside this
  inventory. A project may license their source files through REUSE or state
  site-wide terms in an authored legal page.

## Starter site and documentation

The starter site must build immediately from the released executable and
starter-site archive. It must not require Typst, Java, Node, or another
external renderer merely to show licensing support.

The starter kit should therefore update its attribution template to consume
the generic `attributions()` shape. Its existing copied assets make that page
useful without a renderer. A focused fixture site should exercise a licensed
renderer resource with a controlled test renderer. The guide may show a real
Typst or PlantUML example, clearly stating its external prerequisite.

This keeps the downloadable starter site useful on every supported platform
while testing the new relationship where it belongs.

## Rejected alternatives

### Treat every managed output as an asset

Pages, feeds, sitemaps, search indexes, and deployment files have different
provenance and legal roles. Requiring one SPDX declaration for each would
create noise without establishing a useful relationship. It would also imply
that Heine can decide whether generated HTML is a separate licensed work.

### Let a `.license` sidecar name a future digest path

REUSE practice permits a sidecar for a build artifact that is not present in a
repository. That is useful for source-tree licensing, but a renderer resource
has no stable authored filename. Its digest path changes when its returned
bytes change. A request or profile record remains attached to the source that
causes the output, while a hypothetical sidecar would be easy to orphan.

### Import `REUSE.toml` annotations

REUSE annotations serve a broader source-tree purpose and may use globs,
precedence, and aggregation. Heine's output coverage instead needs an author
to enumerate each published asset or attach facts to one typed renderer
request. Importing those rules would create two competing ownership and
precedence systems.

### Scan rendered HTML, CSS, or JavaScript

Such scanning cannot reliably establish which URLs identify local resources,
external resources, dynamically constructed paths, or browser-only behavior.
It would turn licensing coverage into a best-effort guess, which is weaker
than Heine's existing checked model.

### Generate an SPDX document now

SPDX supports file and package licensing metadata, but a conforming document
needs an explicit document, package, relationship, checksum, and lifecycle
contract. Heine should not emit a partial SPDX-like report merely because it
has an attribution catalog. A later concrete compliance consumer may justify
a separately designed export.

## Prior art

[REUSE](https://reuse.software/spec/) provides the closest file-level prior
art: local copyright and SPDX tags, adjacent sidecars for non-commentable
files, and local license texts. Its older practices document explicitly allows
a sidecar for a build artifact that does not exist in the repository, while
the current REUSE guidance treats source-tree licensing and SPDX export as
separate responsibilities.

[SPDX](https://spdx.github.io/spdx-spec/v3.0.1/scope/) defines a larger data
model for software packages, individual files, provenance, and license
information. It is a useful future export target, not a reason to make
Heine's basic attribution view an SPDX document.

Documentation generators commonly distinguish publishable resources from
source-only material. For example,
[Antora](https://docs.antora.org/antora/latest/page/resource-id/) assigns
identities to publishable pages, attachments, and images, but leaves
licensing policy to the project. Heine can add value by checking declarations
against its final owned artifact inventory without taking responsibility for
the whole legal-compliance process.

## Acceptance criteria if implemented

- Named licensing records and groups that select them retain precise
  configuration spans and one shared SPDX and license-text resolver.
- Existing sidecars, optional coverage, kit assets, locale overlays, and
  copied-asset diagnostics retain their current behavior.
- Ordinary builds exclude draft pages before their renderer requests run. A
  draft renderer resource exists only in an explicit draft preview; required
  coverage applies when that preview publishes it. The renderer-adapter guide
  explains that an ordinary build can pass while a draft preview rejects an
  undeclared draft resource. Renderer resources use the shared
  `heine-renderers/` namespace, are not locale-qualified, and may have origins
  in several locales.
- Required coverage rejects a resource-capable request with no selected record
  before it starts an external process. After rendering, resource conflicts
  retain every request location and sort by their first source page and
  Markdown position.
- Profile defaults and request overrides resolve only named records. Invalid
  combinations, unknown records, a profile default for a profile that cannot
  publish a resource, and resource-inapplicable request options identify the
  authored field or directive option.
- Matching digest resources share one attribution item. Different records for
  one digest resource, or a mixture of declared and undeclared requests, fail
  with every relevant source location and explain why one request cannot set
  licensing facts for another. Differing media types for one resource fail
  with the same locations.
- `attributions()` exposes generic artifact information, stable order, license
  URLs, base-path-aware URLs, absolute URLs where configured, configured
  renderer media types, stable record identifiers, deterministic
  renderer-request sources, source kinds, and relative URLs. It remains a
  site-wide inventory. The starter kit renders copied assets through that new
  shape without needing a renderer.
- A fixture site verifies a renderer resource's declaration, required-coverage
  failure, optional-coverage omission, profile default, request override,
  output deduplication, mixed declared and undeclared requests, declaration
  conflict, media-type conflict, the draft-preview-only coverage failure, an
  accepted unused record, a group without `licensing_record`, a group with SPDX
  fields, and an unknown record.
- Full and quick builds retain their existing freshness rules. The guides name
  configuration, license text, renderer-profile, request, and record changes
  as changes that require a full build when quick mode cannot establish a
  complete current inventory.
- The tutorial, asset-licensing guide, renderer-adapter guide, reference,
  glossary, starter-site tour, and plugin design all describe the resulting
  boundary without claiming REUSE or legal compliance. The glossary defines
  both licensing record and published artifact. The guide shows a sidecar for
  one copied file, a record plus group for a shared copied-asset set, and a
  shared record for copied assets and renderer resources.
