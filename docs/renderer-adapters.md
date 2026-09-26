# Renderer adapters

A renderer adapter lets one explicit Markdown request run one configured
external program and return either checked embedded markup or one generated
resource. Heine owns the request syntax, input boundary, timeout, cache, and
published output. The external program remains a tool you install and choose.

This guide starts with three working profile shapes. Each example assumes its
program is already available on `PATH`, except PlantUML, whose JAR path must
be replaced with the local absolute path. First add the profile to the root
`heine.toml`, then add the Markdown request, and build with explicit
permission:

```bash
heine --allow-external-renderers
```

Heine does not discover or install a renderer. Check the program yourself
before building, for example with `katex --version`, `typst --version`, or
`java -version`. The profile's `cache_identity` is your declared version or
environment identity. Change it after changing a renderer or a meaningful part
of its environment.

## KaTeX embedded mathematics

Install the KaTeX command-line program so `katex` is available on `PATH`.
This profile fixes all tool options and accepts only authored TeX bodies:

```toml
[renderers.katex]
protocol = "stream"
program = "katex"
timeout_seconds = 10
cache_identity = "katex-0.16.27"
inputs = ["body"]
result = "html"

[renderers.katex.inline]
arguments = ["--format", "htmlAndMathml"]

[renderers.katex.block]
arguments = ["--display-mode", "--format", "htmlAndMathml"]
```

Use it inline or as a block:

```markdown
The area of a circle is :render[katex]`\pi r^2`.

:::render[katex]
\int_0^1 x^2\,dx = \frac{1}{3}
:::
```

KaTeX writes HTML to standard output. Heine checks UTF-8, replaces the request
with that trusted embedded HTML, and caches the checked result below
`.heine/cache/renderers/v1/`.

## PlantUML SVG image

Install Java and download a PlantUML JAR. Replace `/absolute/path/plantuml.jar`
with the absolute path to that JAR. The profile produces an SVG resource, so
the Markdown request must provide alternative text:

```toml
[renderers.plantuml]
protocol = "stream"
program = "java"
timeout_seconds = 20
cache_identity = "plantuml-1.2025.7"
inputs = ["body", "file"]
result = "svg"
materializations = ["resource"]

[renderers.plantuml.block]
arguments = ["-jar", "/absolute/path/plantuml.jar", "-tsvg", "-pipe"]
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
isolated input, output, and work directory. Typst writes its PDF only to
Heine's selected primary output file:

```toml
[renderers.typst-report]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.0"
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

Read the [reference](reference.md#renderer-adapters) for every field and the
[renderer-adapter design](design/plugins.md) for the trust and output-ownership
boundaries.
