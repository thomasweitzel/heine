# Citations

[Citations](glossary.md#citation) connect Markdown references to works in local
BibTeX files. Heine checks citation keys in the selected Markdown source,
formats citations with a Citation Style Language (CSL) style, and lets the
page template place the reference list. It does not fetch bibliography entries
or styles.

Create `citations/references.bib` at the site root:

```bibtex
@book{smith2024,
  author = {Smith, Alice},
  title = {An Example Study},
  year = {2024}
}

@book{jones2023,
  author = {Jones, Robin},
  title = {Another Study},
  year = {2023}
}
```

Copy the starter site's `citations/reading.csl` to your site's `citations/`
directory. This independent numeric style works with the example. Select the
files in the page file:

```toml
template = "page.tera"
content.main = "article.md"

[citations]
content = "main"
bibliography = ["references.bib"]
style = "reading.csl"
```

Write references in `article.md`, the selected Markdown input:

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

Heine uses terms defined by the CSL style and its bundled CSL locale data. The
bundled data covers a finite set of locales, not every locale Heine accepts.
Heine looks for the selected locale and an applicable language fallback; a
term it cannot find can fall back to US English. These terms do not come from
the site's Fluent messages. The style decides which terms to display and
whether to use their short or long forms.

Citation groups use semicolons. Locators accept `p.`, `pp.`, `chap.`, and
`chapter` in every locale. These identify the kind of locator; the CSL style
and locale determine its displayed label. A numeric group with a locator
expands instead of collapsing.
Other collapsed groups with locators are rejected. Prefixes, suffixes, and
note styles are not supported. Citation rules that can reach `ibid`,
`ibid-with-locator`, or `near-note` are rejected. A locator containing periods
is rejected when a reachable citation rule can strip periods from a locator.
Inline and fenced code, raw HTML contents, and renderer directive bodies remain
literal; use `\[@smith2024]` to write citation-shaped prose literally. An
unclosed bracket such as `[@smith2024` remains literal Markdown. Citations in ordinary
and margin footnotes and in Markdown block components are supported.

Heine's fixture tests exercise representative APA and IEEE styles. Passing
those examples does not establish conformance for every use of either style.
In particular, citations can fail to distinguish different authors with the
same surname in author-date citations. For example, works by Alice Smith and
Bob Smith from 2020 can both appear as “Smith, 2020,” with neither given name
in the bibliography. Heine does not currently detect this. Review the rendered
citations and their reference entries when authors share a surname, especially
when their works have the same year. See the
[upstream name-disambiguation issue](https://github.com/typst/hayagriva/issues/470)
for its status.

BibTeX `ids` aliases identify the same work as its key. Keys and aliases must
be unique across the selected bibliography files; Heine reports both
declarations if they conflict.

Bibliography URLs must be absolute HTTP or HTTPS URLs without credentials.
Write URL characters literally, for example `&` rather than `\&`; use URL
percent encoding where the URL requires it.
The bibliography parser changes consecutive hyphens within a single BibTeX
value fragment. Heine reports them in URLs so an author can use a verified
percent-encoded URL. Hyphens assembled from separate fragments are preserved.
Heine rejects unsupported CSL output and URLs. The generated
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
