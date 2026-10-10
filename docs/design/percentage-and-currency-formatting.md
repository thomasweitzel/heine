# Percentage and currency formatting design

## Status

Deferred. This record preserves decisions for two later localized
value-formatting packages. Neither formatter is part of Heine's public
contract yet.

## Why these packages wait

ICU4X currently exposes locale-aware percentage and currency formatting through
experimental APIs. Heine will not make an experimental API part of its public
template interface. It also will not reconstruct CLDR patterns
from decimal formatting and hard-coded symbols: percentage placement and
spacing, currency symbols, default fraction digits, and localized names belong
to locale data. The [ICU4X 2.3 changelog](https://github.com/unicode-org/icu4x/blob/main/CHANGELOG.md)
records the currency API changes that make this boundary material.

When stable ICU4X formatters become available, the implementation must first
reassess this record against their API and data model. It must not treat this
record as authorization to depend on the current experimental modules.

## Percentage

A future `format_percent()` accepts a checked numeric ratio. For example,
`0.725` represents 72.5 percent. The formatter owns scaling, digit selection,
sign placement, and spacing through CLDR data. It does not concatenate a
decimal result with a percent sign.

The Fluent formatting API, plural behavior after scaling, and accepted options
remain undecided until the stable formatter determines what Heine can support
as one checked contract.

## Currency

Money has a stronger value contract than an ordinary decimal display quantity.
A future `format_currency()` accepts a quoted ASCII decimal string and an
explicit uppercase ISO 4217 code. It does not accept a binary floating-point
amount, infer a currency from locale, or permit a monetary value as a Fluent
plural selector.

The formatter uses the currency's standard fraction digits by default, such as
zero for JPY and three for KWD. Explicit options may later request different
rounding or padding. Heine uses half-even rounding unless the stable formatter
makes a stronger currency-specific rule necessary.

Tera values are JSON-like, so a future Fluent monetary argument uses one
documented map rather than a Rust-only value:

```tera
{{ t(id="price", price={"amount": data.product.price, "currency": "EUR"}) }}
```

The map has exactly `amount` and `currency` keys. A numeric `amount` must
produce a source-spanned diagnostic that tells the author to quote the exact
decimal string. Other objects and arrays remain unsupported. Heine labels the
narrowest source span that Tera retains, preferably the map expression and
otherwise the enclosing `t()` call.

The corresponding Fluent message treats the value as one monetary fact:

```ftl
price = Price: { $price }.
```

## Future acceptance criteria

Before either package ships, its implementation must show that:

- formatting uses stable ICU4X APIs and bundled CLDR data;
- a percentage uses locale-owned placement and spacing;
- a currency amount preserves its exact quoted decimal input;
- a currency uses standard fraction digits by default and never derives its
  code from locale;
- malformed values, unknown codes, and unsupported options produce useful,
  source-spanned diagnostics; and
- Tera and Fluent use one shared checked formatting domain where their host
  value representations permit it.
