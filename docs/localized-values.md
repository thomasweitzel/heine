# Localized numbers

Sites often need to show a count or measured value in prose. Writing commas,
spaces, or decimal separators in a template makes that presentation belong to
one language. `format_number()` keeps the value numeric until Heine formats it
for the locale currently being rendered.

```tera
{{ format_number(value=1234567.89) }}
```

For example, this renders as `1,234,567.89` in `en-US` and
`1.234.567,89` in `de-DE`. The formatter uses Heine's bundled ICU data, never
the operating system locale, so the same checked build produces the same text
on every machine.

## Template numbers

`format_number()` accepts a finite template number and these optional keyword
arguments:

```tera
{{ format_number(
  value=data.measurement,
  grouping=false,
  minimum_fraction_digits=2,
  maximum_fraction_digits=2,
) }}
```

Grouping defaults to `true`. The minimum defaults to `0`; the maximum defaults
to `3`. Heine rounds to the requested maximum with round-half-to-even and then
pads to the requested minimum. Both fraction-digit values must be integers
from `0` through `32768`, and the minimum must not exceed the maximum.

Heine converts floating-point inputs through their shortest round-trippable
decimal text before it rounds them. It does not multiply a binary float to
implement decimal precision. Integers remain exact at the template boundary.

## Fluent messages

Pass a number directly to `t()`. A bare Fluent placeable formats it in the
locale of the Fluent bundle that supplied the resolved message:

```ftl
post-count = { $count ->
   [one] One post
  *[other] { $count } posts
}
```

```tera
{{ t(id="post-count", count=posts.count) }}
```

Use Fluent's built-in `NUMBER()` when the translation needs a different
presentation:

```ftl
download-size = Download size: { NUMBER($size, maximumFractionDigits: 1) }
```

`NUMBER()` accepts the three checked decimal presentation options
`useGrouping`, `minimumFractionDigits`, and `maximumFractionDigits`. It rejects
other options instead of silently ignoring them. Heine formats both a bare
placeable and `NUMBER()` with the same bundled ICU data as `format_number()`.

Fluent uses `f64` for its selector engine. Before `t()` passes a numeric value
to Fluent, Heine verifies that the number survives that conversion exactly. A
large integer that Fluent cannot distinguish from a neighboring integer is a
template error. Format such a value directly with `format_number()` instead.

When a message falls back to the default locale, its numbers follow that
message's locale as well. An English fallback message therefore does not gain
German separators merely because the page is German.

## Plural messages

Fluent selects plural variants from its numeric value before it writes locale
digits or separators. Keep a selector's numeric presentation at the default
policy when the same value appears through `NUMBER()`. If a message needs a
differently rounded display value, the template author should pass that value
as a separate argument with the intended meaning.

Heine currently formats decimal values only. Percentages, currencies, units,
compact notation, and relative dates have distinct semantics and remain out of
scope. The [deferred percentage and currency record](design/percentage-and-currency-formatting.md)
explains why those APIs wait for stable formatter support.
