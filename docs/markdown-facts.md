# Word counts and reading time

Use `page.markdown` when a template should show a word count or reading-time
estimate without guessing which content file is a page's primary body. Every
declared Markdown input exposes a deterministic Unicode word count:

```tera
{{ page.markdown.main.word_count }}
```

`page.content.main` remains the value rendered through `markdown`; the parallel
`page.markdown.main` view contains facts derived from that original source.

## Enable an estimate

Reading time is optional because it depends on a site's audience and language.
Choose one positive words-per-minute rate for each configured locale:

```toml
[reading_time.words_per_minute]
en = 236
de = 260
```

The value then becomes a whole-minute estimate, rounded up:

```fluent
reading-time = about { $minutes ->
   [one] one minute
  *[other] { $minutes } minutes
} read
```

```tera
{% if page.markdown.main.reading_time %}
  <p>{{ t(id="reading-time", minutes=page.markdown.main.reading_time) }}</p>
{% endif %}
```

Keep the label in Fluent. A template may present the count, the estimate, both,
or neither. Without `[reading_time]`, `reading_time` is `null`; `word_count`
remains available. A zero-word input has an estimate of zero minutes when the
policy is enabled.

## Choosing a rate

A reading-time value is an estimate, not a claim about a reader. Text
difficulty, familiarity, age, and whether someone is studying rather than
reading all affect it. The following non-normative starting points are mean
silent-reading rates reported by a review of the available studies. They apply
to the studies' word units and populations, not to every site or reader.

| Locale | Words per minute |
|--------|-----------------:|
| `ar`   |              181 |
| `zh`   |              260 |
| `nl`   |              228 |
| `en`   |              236 |
| `fi`   |              195 |
| `fr`   |              214 |
| `de`   |              260 |
| `he`   |              224 |
| `it`   |              285 |
| `ko`   |              226 |
| `es`   |              278 |
| `sv`   |              218 |

The [review and meta-analysis by Marc Brysbaert](https://biblio.ugent.be/publication/8647789)
is the source for these values. It reports substantial variation, including an
English non-fiction range of roughly 175 to 300 words per minute. Start with a
relevant value, then adjust it when you know more about the site's readers.

Heine does not supply these values automatically. A multilingual site must
choose every configured locale's rate, so an English convention never becomes
an implicit policy for another language.

## What Heine counts

The count comes from parsed Markdown before templates render it. It includes
prose in headings, paragraphs, quotations, lists, tables, links, image
alternatives, and footnote definitions. It excludes Markdown syntax, URLs,
comments, recognized math expressions, and fenced or indented code blocks.
Inline code is counted. Code blocks are excluded because comprehension time,
rather than reading speed, normally determines how long they take to understand.

Trusted raw HTML contributes its textual content, but not markup, attributes,
URLs, comments, or `script`, `style`, and `template` contents. A color-scheme
picture contributes its authored alternative text and caption. Each segmented
Markdown input is counted once as its original source, independent of how a
template renders its segments.

Heine uses Unicode default word segmentation. This is deterministic across
hosts, but it is not a language-specific lexical tokenizer. Scripts such as
Chinese, Japanese, Thai, Lao, and Khmer may need tailored segmentation for a
linguistic word count. Choose rates for the unit Heine actually reports rather
than copying a benchmark based on a different tokenizer.

See the [reference](reference.md#markdown-facts) for configuration and template
field constraints.
