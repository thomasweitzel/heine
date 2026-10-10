# Markdown facts design

## Status

Implemented. The focused [word counts and reading-time guide](../markdown-facts.md)
and [reference](../reference.md#markdown-facts) define the current public
contract. This record retains the design rationale and accepted boundaries.

## Decision

Heine will derive two source-level facts for every declared Markdown input:

- `word_count`, a deterministic count of readable Markdown text; and
- `reading_time`, an optional whole-minute estimate derived from that count.

Facts are exposed beside, rather than inside, the existing renderable content
values:

```tera
{{ page.markdown.main.word_count }}

{% if page.markdown.main.reading_time %}
  {{ t(id="reading-time", minutes=page.markdown.main.reading_time) }}
{% endif %}
```

`page.content.main` remains the original declared content value. It may be a
string or a segmented structure, so it must not acquire derived fields.
`page.markdown` contains only declared `.md` inputs, keyed by the same simple
content names as `page.content`.

## Motivation

Word count is a useful stable fact for article and documentation templates.
Estimated reading time can be useful too, but it is not an objective property
of a page. Reading speed depends on language, reader, text type, and purpose.
Heine therefore derives a count itself but requires a site to choose the
reading-rate policy used for each of its locales.

A page can declare several named content inputs. Treating `main` as an
implicit whole-page body would make one convention a framework rule and would
silently combine independent content such as an article, an appendix, and a
sidebar. Facts remain attached to their actual Markdown source instead.

## Authored configuration

Word counts require no configuration. Reading time is opt-in:

```toml
[reading_time.words_per_minute]
en = 236
de = 260
```

Omitting `[reading_time]` leaves every `reading_time` field as `null`. The
table is enabled only when it supplies one positive integer for every
configured locale, and no key for an unconfigured locale. Locale keys use the
configured BCP-47 locale spelling exactly.

There is no implicit site-wide rate and no fallback from one locale to another.
An English rate must not silently become a German, Arabic, or Chinese rate.
The configuration represents the site's estimate policy, not a claim about an
individual reader.

For a nonzero count, Heine computes minutes with ceiling division. A source
with zero counted words has `word_count = 0` and `reading_time = 0`; it does
not become a one-minute document merely because the feature is enabled.

Templates own all human-facing presentation. They use Fluent for labels such
as "5 min read" and may show a word count, an estimate, both, or neither.

## Counted source text

The facts are derived while parsing the original declared Markdown source,
before templates render it. They do not depend on whether a template renders
the source, how often it renders it, or where its segments appear.

Heine counts Unicode words from readable text in source order. This includes
ordinary prose in paragraphs, headings, quotations, lists, table cells, link
labels, image alternatives, and footnote-definition bodies. A repeated
footnote reference does not repeat its definition's contribution.

The count excludes Markdown delimiters and other syntax, URLs, comments,
generated HTML, recognized block-math and inline-math expressions, and fenced
code blocks. Code can be important content, but its comprehension time is not
a words-per-minute estimate. Inline code remains ordinary inline readable text
and contributes according to the same Unicode word rule as surrounding prose.

Trusted raw HTML contributes its parsed textual content, not its markup,
attributes, comments, URLs, or the contents of `script`, `style`, and
`template` elements. This uses the same conservative source-level boundary as
search extraction. A `:::picture` caption and its image alternative contribute
as authored text when present; generated image markup does not.

Segment markers do not contribute text. A segmented declared Markdown file is
parsed as its one original source, so its facts cover all of its segments once
in source order.

## Unicode and language scope

Heine uses Unicode default word segmentation for `word_count`. The resulting
integer is deterministic and independent of the host locale. It is a useful
common baseline for scripts with ordinary word boundaries, but it is not a
universal linguistic tokenizer. Chinese, Japanese, Thai, Lao, Khmer, and
similar writing systems need language-specific tailoring for a lexical word
count.

That limitation is material for reading-time policy. A site enabling reading
time chooses a rate for the exact unit Heine counts in each configured locale;
it must not copy a published WPM figure that used incompatible word-boundary
rules. Heine does not silently substitute character counts or use a different
algorithm for a selected script in this version.

The user guide includes non-normative, source-linked starting points where the
research measures comparable silent reading. They are examples for
configuration, never built-in defaults or a complete list of languages.

## Resolution and template model

During page loading, Heine retains each original declared Markdown source for
existing source-level features. The Markdown domain derives one immutable
fact record for each such input. A rendered `PageView` serializes those records
through a separate `markdown` map, while `content` keeps its current rendering
contract unchanged.

The records are page-local and locale-local. A translated page has facts from
its own source, even if it shares a `translationid` with a page in another
locale. Generated taxonomy, Series, pager, feed, and search pages have no
authored declared Markdown input and therefore have no invented facts.

Drafts follow the existing active-page model. An ordinary build does not load
or expose a draft's page facts; `heine serve --drafts` resolves them normally.

## Diagnostics

Reading-time configuration is a checked `heine.toml` relationship:

- a zero, negative, non-integer, duplicate, missing, or unconfigured-locale
  rate identifies its authored value or key;
- an enabled table that omits a configured locale names the missing locale and
  the configuration table; and
- an absent `[reading_time]` is not an error, because reading time is optional.

No page-level configuration selects a fact, and no template call is required.
There are therefore no source-specific configuration spans or render-count
requirements comparable to the table-of-contents and margin-footnote features.

## Boundaries

- Heine does not choose a universal reading rate or claim a duration is
  accurate for every reader.
- It does not count fenced code as ordinary reading material or attempt to
  estimate code-comprehension time.
- It does not use template output, rendered HTML, stylesheet layout, or a
  browser measurement to derive these facts.
- It does not provide a generic readability score, reading-level classifier,
  language detector, or locale-specific lexical tokenizer.
- It does not aggregate named Markdown inputs into an implicit page-wide total.
- A future locale-aware tokenizer may require a separate design. It must not
  silently redefine existing `word_count` values.

## Acceptance criteria

- Every declared Markdown input exposes a deterministic `word_count` in
  `page.markdown`; non-Markdown inputs do not appear there.
- Repeated template rendering, segmentation, and different templates leave
  facts unchanged.
- Prose, footnote definitions, raw HTML text, links, image alternatives, math,
  code fences, and `:::picture` captions follow the documented inclusion
  rules.
- `word_count` uses the documented Unicode segmentation behavior across
  locales and hosts.
- Reading time is `null` without `[reading_time]`, and otherwise uses one
  checked positive rate for every configured locale.
- Ceiling rounding and the zero-word case are covered by focused tests.
- Draft preview and ordinary-build exclusion preserve the existing draft
  boundary.
- The guide, reference, glossary where warranted, starter site, and changelog
  document the implemented public behavior.
