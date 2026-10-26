# Localized value formatting design

## Status

Implemented for decimal and temporal values. Package 1 provides deterministic
decimal formatting in `format_number()`, bare Fluent numeric placeables, and
Fluent `NUMBER()`. Package 2 preserves `format_date()` and adds
`format_time()`, `format_datetime()`, and Fluent `DATETIME()` for the site's
editorial time zone. Percentage and currency formatting remain separate
deferred proposals.

## Decision

Heine will add one locale-aware value-formatting domain for these values:

- decimal numbers;
- calendar dates;
- instants rendered as a time or a date and time.

Tera template functions and Fluent formatting functions will use the same
domain and compiled CLDR data. Tera accepts every checked canonical decimal.
Fluent accepts the checked subset that its current `f64` selector model can
represent exactly. Within that subset, the same canonical value, locale,
formatting intent, and editorial time zone produce the same text. The host
locale, host time zone, current clock, browser preferences, and network data do
not affect the result.

This feature adds a formatting boundary, not a general internationalization
framework or an arbitrary ICU interface. Heine continues to own locale
selection, Fluent-resource loading, root-over-kit message precedence, explicit
default-locale fallback, template rendering, and diagnostics.

[Percentage and currency formatting](percentage-and-currency-formatting.md)
remain separate deferred proposals. They need stable ICU4X APIs and their own
checked value contracts.

## Motivation

Fluent already gives templates strong localized-message composition and plural
selection. Its current value boundary is narrower: `t()` accepts strings,
numbers, and booleans, while `format_date()` separately turns a date-like
string into localized text. This makes a template choose between passing an
unformatted value into Fluent or formatting it before translation.

Preformatting a value before it enters a message loses useful translator
control. A translator can move the resulting string, but cannot choose a
shorter date or suppress digit grouping. Conversely, locale-neutral formatting
inside Fluent would allow a page to format the same value differently depending
on which template path produced it.

Fluent's model is to pass values into a message and let `NUMBER()` or
`DATETIME()` select their representation. It also permits applications to
provide a partially formatted value with defaults that a translation can
override. [Fluent functions](https://projectfluent.org/fluent/guide/functions.html)
describe both forms. ICU similarly models date and time formatting as an
instant-to-calendar conversion that depends on time zone, followed by a
calendar-to-text conversion that depends on locale. [ICU date and time
formatting](https://unicode-org.github.io/icu/userguide/format_parse/datetime/)
describes those independent mappings.

Heine can adopt that model without exposing the breadth of ICU number
formatting. ICU supports currencies, percentages, units, scientific notation,
compact notation, and more. [ICU number formatting](https://unicode-org.github.io/icu/userguide/format_parse/numbers/)
is evidence for a common formatter domain, not a reason for a broad public
language.

## Canonical value model

The formatter accepts values with deliberately distinct meaning:

| Value                        | Canonical representation                    | Meaning                                                         |
|------------------------------|---------------------------------------------|-----------------------------------------------------------------|
| Decimal number               | A checked integer or finite template number | A display quantity with no claim of decimal-place significance. |
| Calendar date                | `YYYY-MM-DD`                                | A date with no time, offset, or instant semantics.              |
| Instant                      | RFC 3339 offset date-time                   | One point in time.                                              |

This distinction is part of the public contract. In particular, a calendar date
is not a midnight instant and an instant is not a calendar date until Heine
applies an editorial time zone.

Template numeric values are JSON-like. Heine converts signed and unsigned
integers directly into its checked fixed-decimal representation, without
narrowing through another integer type or a float. A finite floating value
first becomes its shortest round-trippable decimal representation, including
scientific notation when necessary, then parses into that same fixed decimal.
`format_number()` may accept those values because it formats display
quantities. It rejects a non-finite float or a decimal outside the fixed
representation's checked bounds with a template diagnostic, never an overflow
or panic.

Fluent has a narrower numeric boundary. For a variable that participates in a
selector, Heine applies the selector's default decimal policy before it passes
the value into Fluent. A display-only variable retains its canonical decimal
so a surrounding `NUMBER()` call can choose the visible precision. In either
case, Heine converts the effective decimal to a finite `f64`, converts that
`f64` back through its shortest decimal text, and requires the same checked
decimal. A value that fails this round trip is a source-spanned `t()` error.
This rejects, for example, an integer that Fluent would collapse with a
neighboring integer. It does not weaken Tera's exact integer formatting.

A template number carries no authored decimal-place significance. In
particular, `1.0` has the same numeric value as `1`; Heine never infers that a
trailing zero must remain visible. An author who needs `1.0` sets
`minimum_fraction_digits=1`.

Without an explicit maximum, decimal formatting displays at most three
fractional digits and rounds half to even. This prevents ordinary binary
arithmetic, such as `0.1 + 0.2`, from exposing implementation precision as
`0.30000000000000004`. Authors who need more visible precision set
`maximum_fraction_digits` deliberately. A formatter pads only when
`minimum_fraction_digits` requests it, and that option defaults to zero.
Heine accepts from zero through 32,768 fractional digits. Its checked decimal
representation has 32,768 digit positions on each side of the decimal point,
which bounds both requested padding and rendered output. It never allocates
output from an unchecked template integer. All rounding happens on that fixed
decimal. Heine never rescales a template float, for example by computing
`value * 1000.0`, before it rounds.

The implicit maximum remains three when an author supplies only a larger
minimum. Heine rejects that incompatible request and names the explicit maximum
that would make it valid, rather than silently raising the maximum or producing
an unexpected number of padded digits.

Half-even rounding is deliberate. JavaScript's `Intl.NumberFormat` commonly
uses half-expand rounding, so a translator familiar with that API might expect
`2.5` at zero fractional digits to become `3`. Heine instead renders `2`, and
`3.5` as `4`, which avoids systematic upward bias.

## Locale and data ownership

Heine already validates configured BCP-47 locales and uses compiled ICU4X data
for date formatting and locale-aware collation. The formatter domain extends
that existing dependency family. It constructs formatter instances from the
locale currently being rendered and caches them only for that locale and a
checked option set.

The implementation must use bundled data, never operating-system locale
settings. Temporal conversion likewise uses Jiff's bundled IANA time-zone
database, never host zoneinfo. Cargo must disable Jiff default features and
enable only `std` and `tzdb-bundle-always`. A process-isolated regression test
must vary `TZ` and `TZDIR` while it formats an editorial timestamp, then assert
the same bundled-zone result. Required CI and release checks must also inspect
`cargo tree -e features -i jiff` and reject a Jiff feature set that enables
system time-zone lookup. A full build on two hosts therefore produces identical
formatting for the same Heine version and site input. Updating ICU4X, its
bundled CLDR data, or Jiff's bundled time-zone data can change localized
presentation. Such an update needs release-note review and locale fixtures that
make a deliberate change visible.

Heine accepts configured locales without BCP-47 extension sequences. A locale
such as `ar` is supported when configured, but `ar-u-nu-arab`, a transformed
extension such as `de-t-en`, and private use such as `en-x-site` are rejected
rather than silently ignored. Numbering-system selection remains Heine-owned
until a concrete site establishes a need for per-locale overrides.

Digit grouping is enabled by default. It inserts locale-appropriate separators
and group sizes, such as `1,234,567`, `1.234.567`, or `12,34,567`. Authors may
set `grouping=false` for dense tables or values intended to resemble an
identifier. The option never substitutes a manually selected separator.

## Tera functions

Heine follows its existing named-argument template-function convention:

```tera
{{ format_number(value=downloads) }}
{{ format_number(value=1234.567, maximum_fraction_digits=2) }}

{{ format_date(value=page.published, style="long") }}
{{ format_time(value=page.published, style="short") }}
{{ format_datetime(value=page.updated, date_style="medium", time_style="short") }}
```

The initial numeric option vocabulary is intentionally small:

- `grouping`, a boolean that defaults to `true`;
- `minimum_fraction_digits` and `maximum_fraction_digits`, non-negative
  integers from zero through 32,768, with a checked compatible range.

Heine may add significant digits only after a concrete site needs them. It does
not accept ICU number skeletons, arbitrary patterns, compact notation,
scientific notation, unit identifiers, or a generic `style` escape hatch.

`format_date()` retains its existing public behavior. It accepts a calendar
date or an RFC 3339 instant. When it receives an instant, Heine converts that
instant through `site.time_zone` before selecting its calendar date. This
preserves the publication-day guarantee for `page.published` and
`page.updated`.

`format_time()` and `format_datetime()` accept an RFC 3339 instant only. They
use `site.time_zone`, because the site owns its editorial presentation policy.
They reject calendar dates rather than inventing a midnight time.
`format_time()` accepts `style`; `format_datetime()` accepts `date_style` and
`time_style`, each with `short`, `medium`, or `long`. A combined value always
receives both a date and a time length. These styles do not implicitly add a
zone label. They map to ICU4X standard date and time field sets, whose ordinary
time fields exclude time-zone fields even at long length. The guide must state
that a rendered time implies the configured editorial time zone. Authors with
events in another zone must write that zone in their message until a later
event-time design introduces a checked zone label.

The two public formatting APIs use the same three style values but name their
arguments according to their host language:

| Presentation  | Tera function                                      | Fluent function                                  |
|---------------|----------------------------------------------------|--------------------------------------------------|
| Date          | `format_date(style)`                               | `DATETIME(dateStyle)`                            |
| Time          | `format_time(style)`                               | `DATETIME(timeStyle)`                            |
| Date and time | `format_datetime(date_style, time_style)`          | `DATETIME(dateStyle, timeStyle)`                 |

For an instant, `DATETIME(..., dateStyle: ...)` selects the calendar date
through `site.time_zone` exactly as `format_date()` does.

An RFC 3339 instant maps to exactly one local time in a named IANA zone,
including across daylight-saving transitions. Jiff applies the offset that was
in force at that instant. Heine therefore needs no ambiguity policy while it
formats instants. A future local-wall-time input type would need its own
separate policy for gaps and overlaps.

Heine does not yet support per-event time zones. An event zone without a
visible label would make a time such as “6:00 PM” ambiguous, and no current
site requires that broader model. A future event-time design must introduce an
explicit event zone and an appropriate visible zone label together.

## Fluent integration

Heine keeps direct ownership of `FluentBundle`. It does not adopt a general
Fluent loader or Tera bridge because those would duplicate Heine's source
discovery, root-over-kit precedence, fallback rule, and diagnostics.

For every resolved locale, Heine configures its Fluent bundle with:

- a locale-aware numeric placeable formatter and a Heine-owned `NUMBER()`
  function; and
- a Heine-owned `DATETIME()` function backed by the temporal formatter.

Fluent stores numeric values and selector keys as `f64`. Heine uses the
bundle's locale memoizer to recover the resolved locale in Fluent's formatter,
then uses the same thread-local formatter cache as its Tera functions. ICU4X
decimal formatters are not `Send + Sync`, so they cannot live in Fluent's
concurrent memoizer. Heine's synchronous renderer can cache them per thread,
locale, and checked option set without exposing process state to templates.

An ordinary numeric placeable such as `{ $count }` therefore renders with
Heine's locale-aware default decimal policy. `NUMBER()` changes only its
documented options. After Heine resolves the message, including explicit
default-locale fallback, it classifies every numeric `t()` argument against
that effective message and its reachable references. A Fluent selector then
chooses a plural category from its normalized value, while a display-only value
retains its canonical decimal so `NUMBER()` can choose visible precision:

```ftl
items = { $count ->
    [one] This release has { $count } item.
   *[other] This release has { $count } items.
}
```

```tera
{{ t(id="items", count=downloads) }}
```

The Fluent option names follow Fluent's established camel-case spelling, such
as `maximumFractionDigits`; Tera uses snake case. Both front ends convert into
the same internal option type. Fluent uses a deliberately closed vocabulary:

| Fluent value             | Formatter                  | Accepted options                                             |
|--------------------------|----------------------------|--------------------------------------------------------------|
| checked `f64` number     | placeable or `NUMBER()`    | `grouping`, `minimumFractionDigits`, `maximumFractionDigits` |
| calendar date or instant | `DATETIME()`               | `dateStyle`, `timeStyle`                                     |

Neither `dateStyle` nor `timeStyle` is a generic ICU escape hatch.
Fluent resource validation rejects an unknown formatting function, unknown
option, invalid literal option value, incompatible minimum and maximum digit
pair, or an option that does not apply to the chosen formatter. It also rejects
a numeric formatter that could disagree with a selector in the same message.
After `t()` resolves the selected message and every message or term it
references, directly or indirectly, it rejects the same mismatch. A formatting
option must be a literal, so resource loading can perform its checks. A dynamic
argument with the wrong value type fails when Heine formats the selected
message.

`DATETIME()` accepts only the two canonical temporal forms:

```ftl
published = Published { DATETIME($published, dateStyle: "long") }.
time = Starts at { DATETIME($start, timeStyle: "short") }.
updated = Updated { DATETIME($updated, dateStyle: "medium", timeStyle: "short") }.
```

```tera
{{ t(id="published", published=page.published) }}
{{ t(id="time", start=data.event.starts_at) }}
{{ t(id="updated", updated=page.updated) }}
```

Fluent-resource loading rejects a `DATETIME()` call that supplies neither
`dateStyle` nor `timeStyle`, as well as an unknown, duplicate, or incompatible
style option. A site therefore discovers a malformed message while the build
loads its resources, not only when a page happens to render the message.

It does not parse natural-language dates, locale-formatted dates, or arbitrary
strings that happen to resemble a date. A calendar-date input permits only a
date presentation through `dateStyle`. An instant permits date, time, or
combined presentation according to the supplied style fields. `DATETIME()`
requires at least one of them. Instant presentation consistently uses
`site.time_zone`. A runtime calendar date paired with `timeStyle`, such as
`DATETIME($published, timeStyle: "short")`, is a render-time error: resource
loading can validate the option but cannot know that variable's eventual
temporal form.

### Effective message locale

Tera formatters use the locale of the page they render. A message that resolves
through Heine's explicit fallback uses the locale of the Fluent bundle that
supplied that message, including for number formatting and plural selection.
This keeps a fallback message's prose, grammar, and digits coherent. The
Tera-and-Fluent equivalence guarantee therefore applies when both operations
use the same effective formatting locale.

### Fractional plural selection

Fluent plural categories depend on a decimal's visible fraction digits as well
as its numeric value. A numeric selector uses the default decimal policy:
maximum three fraction digits, minimum zero, and half-even rounding. Heine
selects a category from that canonical rounded decimal before it converts the
value to Fluent's checked `f64` subset. For example, `2.0004` selects as `2`
under the default policy. Exact-number variant keys follow the same
normalization and round-trip rule as selector values. An unsafe key is a
resource-load error rather than an accidental match for a neighboring number.

When a message or attribute selects on `$count`, every `NUMBER($count, ...)`
occurrence must use the selector's effective digit policy. This rule also
applies to every message or term that it references, directly or indirectly,
with its variable bindings. `NUMBER()` may change grouping, omit digit options,
or repeat the default maximum of three and minimum of zero. The resource loader
rejects another minimum or maximum digit setting in the selected message. When
a reference introduces that setting, the checked `t()` call rejects it because
it could display plural operands that disagree with the selector. This rejects
`NUMBER($count, minimumFractionDigits: 1)` beside a `$count` selector instead
of silently producing English text such as “1.0 item”. The first package does
not provide per-selector precision. A template author who needs a different
display precision must deliberately pass a separate display argument. A
translator cannot create that argument from an FTL resource.

Fixtures cover `1`, `1.0`, a bare selector after default rounding, an
explicitly padded non-selector placeable that retains more than three digits, a
rejected selector-formatting mismatch across a message reference, a selector
next to a grouping-only `NUMBER()` placeable, and rejected unsafe exact-number
keys.

## Diagnostics

Every formatter boundary remains checked:

- Tera formatting functions identify their call site and explain invalid
  values, unknown options, incompatible options, or a missing `site.time_zone`.
- `t()` identifies its call site when an argument cannot become a supported
  Fluent formatting value, including a selector mismatch that a referenced
  message or term introduces, or a decimal that cannot round-trip through
  Fluent's `f64` boundary. That diagnostic also names the Fluent declaration
  that introduces the incompatible presentation. For example, a large integer
  diagnostic explains that Fluent cannot distinguish it from a neighboring
  integer and recommends formatting it with Tera's `format_number()` instead.
- Fluent resource loading identifies the FTL source and expression for an
  unknown Heine formatting function, invalid literal option, or direct
  selector and numeric-presentation mismatch. It also identifies an unsafe
  exact-number variant key.
- Formatting a selected message identifies both the Tera call and the FTL
  message when a dynamic argument has an incompatible type or temporal form.

Heine must not silently fall back to a locale-neutral representation, host
settings, UTC, or an arbitrary parser. A numeric value used as a Fluent
selector remains a number even when the message renders it with locale-specific
digits and separators.

## Boundaries

This design does not add:

- unit, compound-unit, scientific, compact, spell-out, range, or list
  formatting;
- relative dates or times;
- alternative calendars;
- arbitrary ICU patterns or skeletons;
- browser-selected or host-selected time zones;
- per-event time zones;
- localized time-zone names;
- natural-language date parsing; or
- a generic template mechanism for carrying arbitrary internal Rust types.

Those capabilities may become useful, but each has independent semantic and
diagnostic questions. They remain outside this feature until a concrete site
needs them.

## Implementation packages

1. **Decimal numbers and Fluent `NUMBER()`.** Add the private value types,
   locale data loading, formatter caching, and option validation together with
   `format_number()`, localized Fluent numeric placeables, and Fluent
   `NUMBER()`. Cover plural selection, grouping, rounding, invalid options,
   fallback-message formatting, and representative `en`, `de`, `fr`, and
   Bangla-digit output. Exercise `1`, `1.0`, and explicit minimum precision in
   Tera output, plus the deliberate half-even rule in Fluent selectors. Reject
   a numeric presentation that could disagree with a selector for the same
   variable, including through a message or term reference. Verify that a
   display-only `NUMBER()` value retains precision beyond the selector default.
   Reject a decimal or exact variant key that cannot round-trip through
   Fluent's `f64` boundary. Cover direct conversion in Tera for `i64::MIN`,
   `i64::MAX`, and `u64::MAX`, ordinary binary-float values, and finite floats
   whose shortest representation uses scientific notation.
2. **Temporal formatting.** Completed. `format_date()` retains its calendar
   date contract; `format_time()`, `format_datetime()`, and Fluent
   `DATETIME()` format RFC 3339 instants through the editorial zone. Focused
   tests cover calendar-date rejection, daylight-saving boundaries in both
   directions, date-only, time-only, and combined field sets, matching Tera
   and Fluent output, and long time output without a zone label. The existing
   isolated editorial-zone regression continues to vary `TZ` and `TZDIR`.

### Starter-site example

The decimal package should update the starter kit's existing localized post
metadata to use `NUMBER()` for word counts and reading-time values. The Math,
Markdown, and Media post should then add a short “Localized values” section
that explains the displayed facts and shows a compact `format_number()`
template example. The page must not place Tera expressions inside Markdown or
branch on that post's identity. Existing post metadata is a real template
boundary that naturally demonstrates Fluent formatting; the section gives
readers an authored example without turning the starter site into a feature
catalog.

Each public package updates the focused internationalization guide, reference,
glossary where terminology becomes durable, starter site where a cohesive
example fits, changelog, and fixture sites.

## Acceptance criteria

- Tera and Fluent produce identical locale-appropriate number, date, time, and
  datetime output for equivalent Fluent-safe inputs, options, and effective
  formatting locale. Tera continues to format checked decimal values that
  Fluent rejects at its `f64` boundary.
- Full builds are deterministic across host locale and time-zone settings.
- Numeric Fluent plural selection uses the checked rounded decimal and its
  default visible fraction precision, never localized output text. Heine
  rejects a sibling `NUMBER()` result that could use different plural operands.
- Numeric conversion and rounding never narrow a 64-bit integer, rescale a
  binary float, overflow, or panic.
- Calendar dates and instants remain distinct. Time and datetime formatting
  never invent an instant from a calendar date.
- The temporal formatter consistently uses `site.time_zone` for reader-facing
  presentation of instants, including date-only `DATETIME()` presentation.
- Formatting an instant chooses the zone offset in force at that instant,
  including across daylight-saving transitions.
- The bundled-zone regression test produces the same editorial result with
  varied `TZ` and `TZDIR` settings.
- Required CI and release checks reject a Jiff feature graph that permits
  system time-zone lookup.
- Locale configuration rejects every BCP-47 extension sequence rather than
  accepting one that Heine cannot honor.
- Unknown, incompatible, or malformed formatter input produces a useful,
  source-spanned diagnostic at every authored boundary involved.
- Fluent-resource loading rejects a `DATETIME()` call without a date or time
  style before any page renders it.
- The public formatter vocabulary remains limited to the documented value
  kinds and options.
