# Citations

[Citations](glossary.md#citation) connect Markdown references to works in local
BibTeX files. Heine
checks citation keys while it analyzes the selected Markdown source, formats
citations with a CSL style, and lets the page template place the reference
list. It does not fetch bibliography entries or styles.

Put `references.bib` and an independent CSL style, such as `numeric.csl`, in
`citations/` at the site root. Select them in the page file:

```toml
template = "page.tera"
content.main = "article.md"

[citations]
content = "main"
bibliography = ["references.bib"]
style = "numeric.csl"
```

Write references in the selected Markdown:

```markdown
One account [@smith2024] describes the result.
Another compares two works [@smith2024; @jones2023].
A page can be identified with [@smith2024, p. 12].
```

The page template renders the selected Markdown and places its bibliography:

```tera
{{ markdown(content="main") }}
<h2>References</h2>
{{ bibliography() }}
```

The template must render the selected source and place `bibliography()` once.
The bibliography contains the cited works. Its surrounding heading belongs to
the template. Citation links target the bibliography section. In a Markdown
heading, the table of contents displays the formatted citation; the heading
ID remains derived from the authored Markdown.

To apply the same setting to a directory, use `[pages.citations]` in its
`_directory.toml`. A nearer complete table replaces it; `citations = false`
suppresses an inherited setting. `content` is resolved separately for each
page. A dependent style needs `parent_style`, a path to its local independent
parent. A `locale` value overrides the page locale for CSL formatting.

Citation groups use semicolons. Locators accept `p.`, `pp.`, `chap.`, and
`chapter`. A numeric group with a locator expands instead of collapsing.
Other collapsed groups with locators are rejected. Prefixes, suffixes, and
note styles are not supported. Citation rules that can reach `ibid`,
`ibid-with-locator`, or `near-note` are rejected. A locator containing periods
is rejected when a reachable citation rule can strip periods from a locator.
Inline and fenced code, raw HTML contents, and renderer directive bodies remain
literal; use `\[@smith2024]` to write citation-shaped prose literally. An
unclosed bracket such as `[@smith2024` remains literal Markdown. Citations in ordinary
and margin footnotes and in Markdown block components are supported.

Bibliography URLs must be absolute HTTP or HTTPS URLs without credentials.
Write URL characters literally, for example `&` rather than `\&`; use URL
percent encoding where the URL requires it.
The current bibliography parser cannot preserve consecutive hyphens in URLs;
Heine reports them so an author can use a verified percent-encoded URL.
The CSL output adapter rejects unsupported output and URLs. The generated
bibliography is a list with `csl-bibliography` and `csl-entry` classes. It
also exposes `csl-first` and `csl-content` classes and layout data attributes
for site styles. CSL styles may supply their own numbers; the starter kit hides
the list markers and spaces entries apart.
Formatted text can also use `csl-small-caps` and `csl-light`.
A full build is required after changes to citation settings, bibliographies, or
styles. Quick builds do not track these changes completely.

The starter site's German reading of *Nachtgedanken* cites its newspaper and
book publications. See its page under `starter-site/content/de/lesen/`, the
site's `citations/` directory, and `templates/reading-citations.tera` for a
complete example.
