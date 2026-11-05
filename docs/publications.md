# Publication catalogs

A publication catalog lets templates present works from local BibTeX files,
including works that no page cites. Configure the files in `heine.toml`:

```toml
[publications]
bibliography = ["works.bib"]
extra_fields = ["code"]
```

Place `works.bib` in the site's `citations/` directory. A template can list
the entries in their authored order:

```tera
{% for work in publications() %}
  <p>{{ work.title }}{% if work.date %} ({{ work.date.text }}){% endif %}</p>
{% endfor %}
```

For a curated list, keep keys in site data and resolve each with
`publication(key=key)`. A BibTeX `ids` alias resolves to the canonical work.
An unknown key or a call without `[publications]` fails at the template call.
The full list contains every entry in the selected files, including works by
other authors and parent entries. Use a dedicated bibliography or curated keys
when a page should show only selected works.

Works expose `key`, `type`, `title`, `authors`, `editors`, `date`, `year`,
`journal`, `book_title`, `publisher`, `doi`, `url`, and `extra`. See the
[reference](reference.md#publication-catalog) for their shapes. The optional
`extra_fields` list selects additional lowercase BibTeX fields. Their values
are text, not checked URLs or asset IDs. A template can call `asset()` when a
field deliberately contains an asset ID. Heine checks that asset when the call
is rendered. The ordinary bibliography `url` field uses Heine's existing HTTP
and HTTPS checks; template escaping alone does not check a URL scheme.

Heine resolves all selected files together, including cross-references. An
invalid input fails the build even if a template displays only a few entries.
Heine converts common accents and some LaTeX commands, but unsupported
commands can remain in the plain text. It displays BibTeX `and others` as the
name `others`. A full build is required after catalog or bibliography changes;
quick builds do not track these changes completely.

An unparsed `date`, such as `date = {forthcoming}`, remains available as text.
The legacy `year` field must contain a year Heine can parse;
`year = {forthcoming}` causes a mapping error when the work is rendered.

`publication(key=...)` converts only the selected work. Each `publications()`
call constructs a view of every entry, so its time and memory grow with the
number and size of entries. Store the result in a template variable when using
it more than once. Sorting or reversing the list can require additional copies.

The starter site's German reading index lists the publication history of
*Nachtgedanken* from the same bibliography used by its citations.
