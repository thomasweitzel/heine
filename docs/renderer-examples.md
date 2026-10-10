# Reproducible renderer examples

This guide builds three end-to-end recipes with two external renderers in an
existing Heine site:

- Typst produces transparent light and dark SVG mathematics through one
  `:::render` request.
- Typst produces one embedded, monochrome SVG that follows the surrounding
  text color.
- KaTeX produces embedded display mathematics with a MathML representation and
  uses self-hosted, site-owned CSS and fonts.

The examples use external programs. Heine does not install them and executes
them only when the build command includes `--allow-external-renderers`.
Run every command from the site root unless the command says otherwise. A block
request begins with `:::render[profile]` and ends with `:::`. An inline request
has the form ``:render[profile]`body` `` and stays on one line. Heine replaces
either request with the profile's checked result.

## Choose a recipe

Choose the output form before configuring a renderer. The form determines what
the browser receives, how the result responds to a color scheme, and what
accessibility information it can carry.

| When you need                                                | Use                   | Result and trade-off                                                                                   |
|--------------------------------------------------------------|-----------------------|--------------------------------------------------------------------------------------------------------|
| Independently designed light and dark artwork                | Typst `picture`       | Two SVG resources. The browser selects one with `prefers-color-scheme`; a cold build runs Typst twice. |
| Monochrome visual mathematics that follows surrounding prose | Embedded Typst SVG    | One SVG that CSS colors with `currentColor`; it is visual output, not semantic mathematics.            |
| Mathematical structure for assistive technology              | KaTeX HTML and MathML | One embedded HTML result with MathML. It requires Node.js plus self-hosted CSS and fonts.              |

## Before you begin

Install Heine and create a site as described in the [first-site guide](../README.md#first-site).
The examples add renderer profiles to that site's `heine.toml`, Markdown to a
page below `content/`, and CSS to the stylesheet loaded by the selected kit or
site template.

External renderers are a deliberate trust boundary. Read the
[renderer-adapter guide](renderer-adapters.md) before adapting a command that
you did not write or inspect. In particular, keep every program, argument,
file input, and bundle input fixed in `heine.toml`. Markdown supplies only the
checked request body and declared inputs.

### When a build refuses to run a renderer

Each recipe needs `--allow-external-renderers`. Without it, Heine stops at the
request before it reads a cached result or starts the configured program. The
diagnostic includes:

```text
external renderer permission is required here
help: rerun with --allow-external-renderers after reviewing the selected renderer profile
```

Each profile records the renderer version in `cache_identity`. That value is
part of Heine's cache key. Replace the example value with the exact version
that you install, and change it whenever a renderer or a relevant local
environment changes.

## Typst light and dark SVG mathematics

This example uses one Typst source to produce two transparent SVG resources.
The profile supplies `scheme=light` and `scheme=dark`; the source uses that
input only to select its foreground color. Heine emits a native `picture`
element, so the browser chooses the dark resource when its color-scheme media
query matches.

### Install Typst

Download a release archive for your platform from Typst's
[official releases](https://github.com/typst/typst/releases), extract it, and
put its executable directory on `PATH`. The Typst project also documents
package-manager installation, `brew install typst` on macOS,
`winget install --id Typst.Typst` on Windows, and
`cargo install --locked typst-cli` for Rust users. Package-manager versions can
lag behind the current release.

Check the installed command before configuring Heine:

```bash
typst --version
typst compile --help
```

The example below uses Typst 0.15.1. Keep its `cache_identity` synchronized
with the first command's output.

### Configure the profile

Add this profile to `heine.toml`:

```toml
[renderers.typst_picture]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.1"
inputs = ["body"]
input_extension = "typ"
result = "picture"
media_type = "image/svg+xml"
extension = "svg"

[renderers.typst_picture.block.light]
arguments = [
  "compile", "{input_file}", "{output_file}",
  "--format", "svg", "--input", "scheme=light",
]

[renderers.typst_picture.block.dark]
arguments = [
  "compile", "{input_file}", "{output_file}",
  "--format", "svg", "--input", "scheme=dark",
]
```

`result = "picture"` requires exactly the `light` and `dark` invocation
blocks. The generated SVGs must have matching intrinsic aspect ratios when
Heine can determine them. A cold build executes Typst once per appearance;
later builds reuse each checked cache entry independently.

### Add the request

Add this block to a Markdown page:

```markdown
## Dynamically rendered with Typst

:::render[typst_picture alt="The Gaussian integral and a Fourier-transform identity" class="math-diagram"]
#let foreground = if sys.inputs.at("scheme") == "dark" { white } else { black }

#set page(width: auto, height: auto, margin: 0.35em, fill: none)
#set text(size: 1.75em, fill: foreground)

#align(center)[
  #stack(
    spacing: 0.65em,
    [$ integral_0^infinity e^(-x^2) dif x = sqrt(pi) / 2 $],
    [$ cal(F)[e^(-a x^2)](xi) = sqrt(pi / a) e^(-pi^2 xi^2 / a), quad a > 0 $],
  )
]
:::
```

The zero page fill keeps the SVG transparent. Do not add a background color to
make the dark text visible. The browser shows the SVG over the page's own light
or dark background.

### Size the picture in the site stylesheet

Add these rules to the stylesheet your site already loads:

```css
picture.math-diagram {
  display: block;
  text-align: center;
}

picture.math-diagram img {
  display: inline-block;
  width: min(100%, 52rem);
  height: auto;
}
```

The image remains proportional, does not exceed the available inline width,
and centers within the prose column. Adjust `52rem` to choose a different
maximum width.

### Build and inspect it

Build with the required permission:

```bash
heine --allow-external-renderers
```

Heine publishes two digest-addressed SVG files below
`public/heine-renderers/` and replaces the Markdown block with a `picture`
element. Load the rendered page in a browser, then switch the operating system
or browser between light and dark color schemes. The formulas should retain a
transparent background while their foreground changes.

If Typst reports an error, Heine shows the full Markdown request and preserves
Typst's output beneath it. The isolated input file named by Typst's diagnostic
is a temporary workspace file, not the Markdown page itself. The failure
rejects the build; Heine does not publish an inline error replacement.

## Typst embedded SVG mathematics that follows prose colour

For deliberately monochrome SVG mathematics, one embedded result can follow
the surrounding text colour in both schemes. Each request runs Typst once,
rather than once for each appearance as a `picture` result does. This is useful
for a short visual formula, but not for a multi-colour diagram or a formula
that needs a semantic MathML representation. Do not use this visual result as
the only representation of mathematics that readers must be able to interpret
with assistive technology.

Add a separate SVG profile. A `picture` profile cannot embed one SVG because
its contract always produces two generated resources.

```toml
[renderers.typst_foreground_svg]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.1"
inputs = ["body"]
input_extension = "typ"
result = "svg"
materializations = ["embedded"]

[renderers.typst_foreground_svg.inline]
arguments = ["compile", "{input_file}", "{output_file}", "--format", "svg"]

[renderers.typst_foreground_svg.block]
arguments = ["compile", "{input_file}", "{output_file}", "--format", "svg"]
```

`materializations = ["embedded"]` permits this SVG profile to replace
Markdown directly, rather than publish an SVG resource. The `inline` and
`block` tables independently enable the two Markdown request forms. They use
the same Typst invocation here, but remain separate because a profile declares
each supported placement explicitly. Omit either table when the site needs
only the other form.

Use the profile in prose and give the checked root SVG a site-owned class. An
inline request body stays on one Markdown line:

```markdown
Euler's identity is :render[typst_foreground_svg class="typst-inline-math"]`#page(width: auto, height: auto, margin: 0pt, fill: none)[$ e^(i pi) + 1 = 0 $]`.
```

The same profile supports a larger block formula. This example includes a
square root and a fraction, which also exercise Typst's stroked rule paths:

```markdown
:::render[typst_foreground_svg class="typst-block-math"]
#set page(
  width: auto,
  height: auto,
  margin: 0.25em,
  fill: none,
)
#set text(size: 1.75em)

#align(center)[
  $ cal(F)[e^(-a x^2)](xi)
      = sqrt(pi / a) e^(-pi^2 xi^2 / a), quad a > 0 $
]
:::
```

For the Typst 0.15.1 output generated by this source, visible glyphs are SVG
`use` elements with fixed `fill` attributes. Fraction rules are direct,
unfilled `path` children of the root SVG with fixed `stroke` attributes. The
site stylesheet can override both presentation attributes with the inherited
`currentColor` value:

```css
svg.typst-inline-math {
  block-size: 1.15em;
  inline-size: auto;
  vertical-align: -0.16em;
}

svg.typst-block-math {
  display: block;
  max-inline-size: 100%;
  block-size: auto;
  inline-size: auto;
  margin-block: 1rem;
}

svg.typst-inline-math use,
svg.typst-block-math use {
  fill: currentColor;
}

svg.typst-inline-math > path[fill="none"][stroke],
svg.typst-block-math > path[fill="none"][stroke] {
  stroke: currentColor;
}
```

The direct-child selector reflects the verified Typst 0.15.1 structure. Inspect
the generated SVG after upgrading Typst and keep the selector narrow. Do not
use a broad `[fill]` or `[stroke]` selector: it can recolour intentional
multi-colour artwork or turn `fill="none"` into a visible fill. If a future
formula leaves one glyph unrecoloured, inspect whether Typst emitted that glyph
as a raw `path` rather than a `use` element before changing the selector. The
class gives the site a styling hook; Heine does not recolour the SVG itself. Use
an HTML-and-MathML renderer, such as KaTeX, when the equation needs semantic
mathematical content for assistive technology.

## KaTeX embedded display mathematics

This example uses KaTeX to turn TeX into embedded HTML and MathML. It is
intentionally a different result type from the Typst example: the renderer
returns markup, while KaTeX's fixed stylesheet and self-hosted fonts determine
its visual layout. The generated markup includes MathML for assistive
technology and a visual HTML representation for browsers.

### Install Node.js and KaTeX

Install a supported Node.js release using your operating system's package
manager or the [official Node.js downloads](https://nodejs.org/en/download).
Confirm that Node and npm are available:

```bash
node --version
npm --version
```

Choose an absolute directory that the account building the site can write. The
POSIX commands below use `/opt/katex`; creating that directory typically
requires administrator rights. A home-directory location is often more
suitable for an individual developer or CI account. The Windows commands use
the current account's local application-data directory through
`$env:LOCALAPPDATA`. Keep the selected location consistent in the commands and
profile.

The current renderer-adapter contract does not resolve renderer arguments from
the site root. A relative `tools/katex` path therefore works only when Heine
happens to be invoked from that directory.

Install the pinned KaTeX package, inspect its supported flags, and verify that
the CLI accepts the HTML-and-MathML output form. On Linux, macOS, or WSL, run:

```bash
npm install --prefix /opt/katex katex@0.19.0
node /opt/katex/node_modules/katex/cli.js --version
node /opt/katex/node_modules/katex/cli.js --help
printf 'x^2' | node /opt/katex/node_modules/katex/cli.js --format htmlAndMathml >/dev/null
```

On native Windows, run the equivalent commands in PowerShell:

```powershell
$katexRoot = "$env:LOCALAPPDATA\heine\katex"
npm install --prefix $katexRoot katex@0.19.0
node "$katexRoot/node_modules/katex/cli.js" --version
node "$katexRoot/node_modules/katex/cli.js" --help
'x^2' | node "$katexRoot/node_modules/katex/cli.js" --format htmlAndMathml | Out-Null
$katexRoot -replace '\\', '/'
```

The last command prints the slash-normalized absolute path to use in
`heine.toml`. KaTeX declares `cli.js` as its command-line entry point. Calling
that file directly avoids npm's platform-specific `.bin` shims.

Use the reported version in the profile's `cache_identity`. The rest of this
guide uses 0.19.0 as a concrete example. The final command verifies the
`htmlAndMathml` value used by the profile, rather than assuming that the CLI
and JavaScript API accept the same format names.

### Copy the stylesheet and fonts into the site

KaTeX's distribution stylesheet refers to its `fonts/` directory with relative
URLs. Copy `katex.min.css` and the complete directory together below the
ordinary copied-asset root:

```bash
install -d content/assets/katex/fonts
cp /opt/katex/node_modules/katex/dist/katex.min.css content/assets/katex/
cp -R /opt/katex/node_modules/katex/dist/fonts/. content/assets/katex/fonts/
```

On native Windows, run this in PowerShell from the site root:

```powershell
$katexRoot = "$env:LOCALAPPDATA\heine\katex"
New-Item -ItemType Directory -Force -Path 'content/assets/katex/fonts' | Out-Null
Copy-Item "$katexRoot/node_modules/katex/dist/katex.min.css" 'content/assets/katex/katex.min.css'
Copy-Item "$katexRoot/node_modules/katex/dist/fonts/*" 'content/assets/katex/fonts/'
```

The resulting structure must remain intact:

```text
content/assets/katex/
├── katex.min.css
└── fonts/
    ├── KaTeX_*.woff2
    ├── KaTeX_*.woff
    └── KaTeX_*.ttf
```

The CSS resolves `fonts/...` relative to its own published URL. It therefore
works below a non-root `base_path` without a CDN or URL rewriting. Keep the
WOFF and TTF files too when copying the upstream distribution, unless the
site's browser-support policy deliberately permits removing those fallbacks.

### Load the stylesheet only on pages that use KaTeX

KaTeX's CSS is a stable package asset, not a renderer-generated dependency.
Until Heine has a checked renderer-to-template dependency contract, let the
site template make that ordinary presentation choice explicit. `extra.katex`
is a site-owned convention in this example, not a built-in Heine setting.

Add this to each authored page's `.page` file that contains a KaTeX request:

```toml
extra.katex = true
```

Then add this guarded link to the shared base template, after the site's main
stylesheet:

```tera
{% if page and page.extra.katex is defined and page.extra.katex %}
  {% set katex = asset(id="assets/katex/katex.min.css") %}
  <link rel="stylesheet" href="{{ katex.url }}"{% if katex.integrity %} integrity="{{ katex.integrity }}" crossorigin="anonymous"{% endif %}>
{% endif %}
```

The initial `page` guard matters because generated taxonomy, pagination, feed,
and series pages do not have authored page metadata. The `is defined` test
keeps pages without `extra.katex` valid, and the final value test loads KaTeX
only for `true`. The stylesheet's relative font URLs remain correct because
the stylesheet and `fonts/` directory retain their sibling layout.

### Configure the profile

Add this profile to `heine.toml`:

```toml
[renderers.katex_html]
protocol = "stream"
program = "node"
timeout_seconds = 10
cache_identity = "katex-0.19.0"
inputs = ["body"]
result = "html"

[renderers.katex_html.block]
arguments = [
  "/opt/katex/node_modules/katex/cli.js",
  "--display-mode",
  "--format", "htmlAndMathml",
  "--max-size", "20",
  "--max-expand", "1000",
]
```

On native Windows, replace the first argument with the normalized path that
the installation command printed, followed by `/node_modules/katex/cli.js`.
For example, an account named `Ada` would use
`C:/Users/Ada/AppData/Local/heine/katex/node_modules/katex/cli.js`.

KaTeX reads the request body from standard input and writes one HTML and MathML
fragment to standard output, so it needs no wrapper. The fixed `max-size` and
`max-expand` limits constrain malformed or unexpectedly expansive TeX. Do not
add `--trust` unless the authored TeX deliberately needs KaTeX features that
emit trusted HTML.

### Keep a team profile portable

The absolute `cli.js` path above pins one local Node.js installation. A team
can instead provision the exact KaTeX command outside Heine and put it on
`PATH` in every developer and CI environment. Replace `program = "node"` with
`program = "katex"`, then remove the `cli.js` path from the beginning of the
`arguments` list. The remaining fixed arguments stay unchanged.

This keeps `heine.toml` portable without treating a renderer path as a
site-relative file or interpolating environment variables. The external setup
must still install the version named by `cache_identity` and copy the matching
KaTeX stylesheet and fonts into the site. A toolchain file, container image, or
CI setup can own that provisioning; it remains outside Heine's renderer
boundary.

### Add the request

Add this block to the Markdown content named by the `.page` file:

```markdown
## Dynamically rendered with KaTeX

:::render[katex_html]
\mathcal{F}[e^{-a x^2}](\xi)
  = \sqrt{\frac{\pi}{a}} e^{-\pi^2 \xi^2 / a},
    \qquad a > 0
:::
```

KaTeX display mode centers the formula. Its stylesheet uses `currentColor`, so
the formula inherits the page foreground and follows light and dark themes
without separate renderer invocations.

### Build and inspect it

Build with the required permission:

```bash
heine --allow-external-renderers
```

The page contains a `katex-display` element with MathML and visual KaTeX HTML,
rather than a generated resource. Inspect the rendered page in a narrow
viewport and in both color schemes. Check that the page loads
`katex.min.css` and only the needed local font files, with no third-party font
requests.

KaTeX deliberately keeps display mathematics unwrapped. Add this optional rule
to the site's stylesheet when a wide formula should scroll horizontally rather
than widen the page:

```css
.katex-display {
  max-inline-size: 100%;
  overflow-x: auto;
  overflow-y: hidden;
}
```

The rule keeps KaTeX's vertical layout intact while permitting a horizontal
scrollbar only when a formula does not fit.

## Updating or removing these examples

After updating Typst, Node.js, KaTeX, fonts, or renderer arguments, update
the affected `cache_identity` and run a normal build. Use
`--refresh-renderers` with the permission flag when you deliberately need to
discard reusable renderer cache entries:

```bash
heine --allow-external-renderers --refresh-renderers
```

To remove the Typst example, delete its Markdown request, profile, and
dedicated stylesheet rules. To remove the KaTeX example, delete its Markdown
request, profile, page `extra.katex` value, template link, and copied KaTeX
assets. Generated files below `public/heine-renderers/` disappear on the next
full build when no remaining renderer result owns them. The renderer cache
below `.heine/cache/renderers/` is local build state and can be removed when
you no longer want its entries.
