# Markdown block components

Use a Markdown block component when a site or kit component needs to wrap
authored Markdown at one point in a document. The body remains ordinary
Markdown, so its headings, checked links, search text, word count, and
footnotes stay part of the page.

```markdown
:::component[site.note kind = "warning"]
## Back up before upgrading

Copy the existing data first. See the [installation guide](page:guide/install).
:::
```

The component receives the rendered body through Tera's `body` value:

```tera
{% component site.note(kind: string) %}
<aside class="note note-{{ kind }}">
  {{ body }}
</aside>
{% endcomponent site.note %}
```

Components normally use the three-colon opening and closing fences shown
above. When the body contains a literal `:::` line, use a longer opening fence
and an otherwise empty closing fence of the same length or longer. Components
need a non-empty body and must occur as an ordinary top-level Markdown block.
They cannot nest or occur in a list, quote, table,
footnote definition, or segmented source. Heine rejects directive-looking
forms in a list, quote, or paragraph continuation until it can define their
structural behavior without changing their meaning later.

Arguments follow the component name and use TOML scalar values. They can be
strings, finite numbers, or booleans. They are literal values, not Tera
expressions. A component therefore reads page and site facts through its
declared implicit arguments, just like a component called from a template.

## Body contract

Components keep their Markdown body visible by default. A disclosure or
similar component that may hide its body declares:

```tera
{"heine.body_may_hide": true}
```

Do not add this metadata to an ordinary component. When present, the value must
be literal `true`; `false` is an error. A body for a component that may hide
it cannot contain headings, because table-of-contents and search links should
not lead readers into hidden content.

The component must preserve `{{ body }}` exactly once. Heine checks that
contract during the build. It does not inspect the component's HTML, so the
component remains responsible for valid HTML and for any literal URLs or IDs
it creates.

## What stays checked

Heine analyzes the body together with the surrounding Markdown before it
renders the component. `page:` and `asset:` links, heading IDs, ordinary
footnotes, selected margin footnotes, search text, word count, and reading
time therefore use the same page-wide rules as surrounding prose. A footnote
definition itself must stay outside the component body.

Render a source containing a block component with `markdown(content="…")`.
The `markdown` filter cannot apply the page's component plan.

## When not to use one

Use ordinary raw HTML for one static element. Use a page template and declared
content segments for data-driven lists or components that do not wrap authored
Markdown. Block components cannot add CSS or JavaScript to the document head,
run Tera control flow in Markdown, or create new Markdown structure after the
page analysis pass.
