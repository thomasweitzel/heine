# Declared content

A page's `[content]` table gives its template named inputs. Most pages need one
unsegmented Markdown file and render it in one reading flow:

```toml
# content/en/article.page
[content]
main = "article.md"
```

```tera
<article>
  {{ page.content.main | markdown }}
</article>
```

Keep this form when the text belongs together. It is the clearest choice for a
page body, and it is required when that Markdown source supplies a table of
contents or margin footnotes.

Some pages need a different arrangement. Their prose still belongs in one
authored document, but the template needs to place named parts in different
locations. Segments provide that narrow form of structure.

## Choose the input that matches the content

| Need                                                                        | Use                               |
|-----------------------------------------------------------------------------|-----------------------------------|
| One normal reading flow                                                     | One unsegmented Markdown file     |
| One prose document whose named parts render in different template locations | Segments                          |
| Facts, records, flags, mappings, or repeatable structured values            | JSON or TOML                      |
| Site-wide shared values                                                     | A JSON or TOML file below `data/` |
| Independently authored documents                                            | Several declared content files    |

Use a declared JSON or TOML input for data owned by one page. A file below
`data/` provides data shared across templates. JSON and TOML provide structured
data. Segments provide structured text content. A segment remains authored
text, usually Markdown, rather than a field whose prose has been moved into a
data format.

## Render named prose parts

Start a declared text file with a segment marker. Each following marker names
the text that follows it:

```markdown
<!-- content/en/index.md -->
<!-- segment: lead -->
Heine checks relationships before it writes the site.

<!-- segment: callout -->
Internal page and asset references fail at their source when they are wrong.

<!-- segment: body -->
The remaining explanation belongs in the main article.
```

The page still declares one input:

```toml
# content/en/index.page
[content]
main = "index.md"
```

Its template can render each part where it belongs:

```tera
<section class="lead">
  {{ page.content.main.lead | markdown }}
</section>
<aside>
  {{ page.content.main.callout | markdown }}
</aside>
<article>
  {{ page.content.main.body | markdown }}
</article>
```

This keeps the page's prose together for editing while letting the template
choose its presentation. It does not create a general page-composition system:
the template still owns the structure and must name the parts it expects.

## Segment boundaries

Heine recognizes segments before it parses Markdown. A marker-shaped line
inside a fenced code block is therefore still a marker. Keep literal marker
examples in an unsegmented input, or phrase them differently.

Dotted names create nested template values. Repeating a name creates an
ordered array, which is useful when one authored document contains several
same-role text parts. The [reference](reference.md#declared-content-files)
defines the marker syntax and value shape.

Segments are not a Markdown syntax tree. A segmented Markdown input cannot be
the source selected for a table of contents or margin footnotes, because those
features need one unsegmented source rendered exactly once with
`markdown(content="…")`. Keep a normal page body unsegmented when it needs
either feature.
