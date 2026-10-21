# Renderer adapters

Use a renderer adapter when an external program should turn one marked
Markdown request into a diagram, formula, download, or other result. Each
request names one configured profile. The profile fixes the program, inputs,
output form, timeout, and cache identity; Heine checks the request and
publishes the result it returns.

This guide assumes that you already have a working Heine site. For complete
Typst SVG and KaTeX HTML walkthroughs, including installation, stylesheets,
accessibility, and verification, read the [reproducible renderer
examples](renderer-examples.md). That guide is the maintained source for those
external-tool recipes.

## Requests, profiles, and trust

Choose an inline request for a short one-line input:
``:render[profile]`body` ``. Choose a block request for a larger input. It
normally begins with `:::render[profile]` and ends with `:::`. When its body
contains a literal `:::` line, use a longer opening fence and an otherwise
empty closing fence of the same length or longer. Each request selects one
profile from the root `heine.toml`. Block requests must be ordinary top-level
Markdown blocks. Heine rejects a request in a list item or block quote, or a
paragraph continuation, until it defines how that structure should contain the
replacement.

A profile's `protocol` controls how Heine supplies the checked input. A
`stream` profile receives one input on standard input and writes one result to
standard output. A `workspace` profile receives staged files in a temporary
directory and writes its declared primary result file there. `inputs` lists the
forms a profile accepts: an authored body, a named file, or a named source
bundle. `result` declares what the renderer returns, such as embedded
HTML, SVG, or MathML, an image, a link, or a light and dark picture. Only an
SVG profile uses `materializations` to choose whether it embeds SVG or
publishes it as a resource.

External renderers are a trust boundary. A permitted program and any wrapper
script run with the operating-system authority of the account that starts
Heine. A workspace restricts the files that Heine supplies and receives; it is
not a sandbox against a malicious executable. Embedded HTML, SVG, and MathML
also have the browser-output authority of trusted project templates. Review the
program, every fixed argument, and every declared file or bundle input before
you grant permission.

Every build that selects a renderer request needs explicit permission:

```bash
heine --allow-external-renderers
```

Without that flag, Heine stops at the request before reading a cache entry or
starting a process:

```text
external renderer permission is required here
help: rerun with --allow-external-renderers after reviewing the selected renderer profile
```

Heine does not discover or install a renderer. Check the program yourself
before building, for example with `katex --version`, `typst --version`, or
`java -version`. The profile's `cache_identity` is your declared version or
environment identity. Change it after changing a renderer or a meaningful part
of its environment.

When a renderer fails, Heine labels the complete authored request and presents
the program's diagnostic output separately. Heine preserves that output as the
renderer emitted it. For a workspace request with a body, paths and line
numbers in that output refer to the isolated staged input.

An embedded SVG request may add a normalized `class` list to its checked root
element. This lets site CSS style the SVG without a wrapper. HTML and MathML
results remain renderer-owned fragments, so they do not accept `class`.

## PlantUML SVG image

Install Java and download a PlantUML JAR. Replace `/absolute/path/plantuml.jar`
with the absolute path to that JAR. The profile produces an SVG resource, so
the Markdown request must provide alternative text:

```toml
[renderers.plantuml]
protocol = "stream"
program = "java"
timeout_seconds = 20
cache_identity = "plantuml-1.2026.8"
inputs = ["body", "file"]
result = "svg"
materializations = ["resource"]

[renderers.plantuml.block]
arguments = ["-jar", "/absolute/path/plantuml.jar", "--svg", "-pipe"]
```

```markdown
:::render[plantuml alt="Browser request and application response"]
@startuml
Browser -> Application: request
Application --> Browser: response
@enduml
:::
```

Heine verifies that the returned document has an SVG root element, publishes
the original bytes as `heine-renderers/sha256-<digest>.svg`, and replaces the
request with an `img` element. The URL includes the configured `site.base_path`.

To use a checked page-owned file instead of an inline diagram, declare it in
the adjacent `.page` file:

```toml
[render.architecture]
file = "diagrams/architecture.puml"
```

Then select it from a block request with an empty body:

```markdown
:::render[plantuml source="architecture" alt="Application architecture"]
:::
```

The declared file is renderer input, not a copied asset.

## Typst PDF download

Install the `typst` command-line program. A workspace profile receives an
isolated input, output, and work directory. Heine names the selected primary
output `result.pdf` from the configured result extension, so Typst can infer
the format from its destination filename:

```toml
[renderers.typst-report]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.1"
inputs = ["body", "file", "bundle"]
input_extension = "typ"
result = "link"
media_type = "application/pdf"
extension = "pdf"

[renderers.typst-report.block]
arguments = ["compile", "{input_file}", "{output_file}"]
```

Heine keeps one reusable workspace slot for each workspace profile during a
build session. It clears that slot before and after each request. If a platform
lock prevents cleanup, Heine keeps a successful result but reports the retained
path; later requests retry cleanup instead of allocating unbounded temporary
directories.

```markdown
:::render[typst-report label="Download the one-page report (PDF)"]
= Report title

This PDF was generated while building the site.
:::
```

Heine replaces the request with an `a` element whose visible text is the
required `label`. The PDF receives a digest-addressed URL below
`heine-renderers/`, so changed bytes receive a new public URL.

For a multi-file Typst project, declare a checked source bundle in the page:

```toml
[render.report]
root = "reports/quarterly"
entry = "main.typ"
```

The profile can use `{input_dir}` instead of `{input_file}` when its fixed
arguments need the staged bundle directory. Heine copies only the declared
bundle files, hashes every checked file into the cache key, and rejects an
extra renderer output file.

## Light and dark generated images

`result = "picture"` represents one meaningful image in fixed light and dark
forms. The profile owns both renderer invocations; a Markdown request supplies
one alternative text and optional picture presentation fields. Heine checks
both resources, emits a native `picture`, and uses the light source as its
fallback. Read the [generated-picture design](design/renderer-pictures.md) for
the complete contract.

For complete local Typst SVG and KaTeX HTML walkthroughs, including
installation, profiles, stylesheets, accessibility, and test commands, see
[reproducible renderer examples](renderer-examples.md).

## Build and development behavior

The first permitted build runs each selected renderer. Later builds reuse a
typed cache entry when the profile, request form, and complete checked input
are unchanged. Use this command when deliberately checking the installed
tools or after an external change not represented by `cache_identity`:

```bash
heine --allow-external-renderers --refresh-renderers
```

At the start of each build session, Heine removes incomplete cache files from
an interrupted atomic write. It does not scan the complete cache for every
request, and it does not evict valid entries automatically. Remove `.heine/`
when you want to discard the private cache completely.

`heine serve --allow-external-renderers` applies the same permission to every
complete watched rebuild. Renderer source files and bundles live below
`content/`, so changes trigger a rebuild. A `--quick` build can update a
selected request after a present body or declared input changes, but it remains
incomplete: change profiles, request structure, source declarations, names, or
paths with a full build.

When identical renderer bytes produce the same digest-addressed resource,
Heine publishes that resource once. If another producer claims its path,
Heine's collision report identifies every Markdown request that contributed to
the shared resource.

Read the [reference](reference.md#renderer-adapters) for every field and the
[renderer-adapter design](design/plugins.md) for the trust and output-ownership
boundaries.
