//! Exact decimal values and their deterministic locale presentation.

use std::{cell::RefCell, collections::BTreeMap, fmt};

#[cfg(test)]
use std::str::FromStr;

use fixed_decimal::{Decimal, FloatPrecision};
use icu_decimal::{
    DecimalFormatter,
    options::{DecimalFormatterOptions, GroupingStrategy},
};
use thiserror::Error;

use crate::locale::Locale;

/// The largest fractional precision that `fixed_decimal` can represent.
pub(crate) const MAX_FRACTION_DIGITS: u16 = 32_768;

/// A checked decimal value that has not yet acquired locale-specific text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LocalizedNumber(Decimal);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    grouping: bool,
    minimum_fraction_digits: u16,
    maximum_fraction_digits: u16,
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("number must be finite")]
    NonFinite,
    #[error("number is outside Heine's checked decimal range")]
    OutOfRange,
    #[error("{name} must be an integer from 0 through {MAX_FRACTION_DIGITS}")]
    FractionDigits { name: &'static str },
    #[error("minimum_fraction_digits must not exceed maximum_fraction_digits")]
    IncompatibleFractionDigits,
    #[error("could not load decimal formatting data for locale {locale:?}: {message}")]
    Locale { locale: String, message: String },
    #[error(
        "Fluent cannot distinguish this number from a neighboring value; format it with format_number() in the template instead"
    )]
    FluentPrecision,
}

impl LocalizedNumber {
    pub(crate) fn from_i64(value: i64) -> Self {
        Self(value.into())
    }

    pub(crate) fn from_u64(value: u64) -> Self {
        Self(value.into())
    }

    pub(crate) fn from_f64(value: f64) -> Result<Self, Error> {
        if !value.is_finite() {
            return Err(Error::NonFinite);
        }
        Decimal::try_from_f64(value, FloatPrecision::RoundTrip)
            .map(Self)
            .map_err(|_| Error::OutOfRange)
    }

    #[cfg(test)]
    fn from_str(value: &str) -> Result<Self, Error> {
        Decimal::from_str(value)
            .map(Self)
            .map_err(|_| Error::OutOfRange)
    }

    /// Converts this value through Fluent's unavoidable `f64` selector model.
    /// The round trip proves that Fluent will not silently alter its identity.
    pub(crate) fn fluent_value(&self) -> Result<f64, Error> {
        let text = self.0.to_string();
        let value = text.parse::<f64>().map_err(|_| Error::FluentPrecision)?;
        if !value.is_finite() {
            return Err(Error::FluentPrecision);
        }
        let recovered = Self::from_f64(value).map_err(|_| Error::FluentPrecision)?;
        (recovered == *self)
            .then_some(value)
            .ok_or(Error::FluentPrecision)
    }

    pub(crate) fn prepared(&self, options: Options) -> Decimal {
        let mut decimal = self.0.clone();
        decimal.round(-i32::from(options.maximum_fraction_digits) as i16);
        decimal.pad_end(-i32::from(options.minimum_fraction_digits) as i16);
        decimal
    }

    pub(crate) fn normalized(&self, options: Options) -> Self {
        Self(self.prepared(options))
    }
}

impl Options {
    pub(crate) const fn defaults() -> Self {
        Self {
            grouping: true,
            minimum_fraction_digits: 0,
            maximum_fraction_digits: 3,
        }
    }

    pub(crate) fn new(
        grouping: bool,
        minimum_fraction_digits: u16,
        maximum_fraction_digits: u16,
    ) -> Result<Self, Error> {
        if minimum_fraction_digits > MAX_FRACTION_DIGITS {
            return Err(Error::FractionDigits {
                name: "minimum_fraction_digits",
            });
        }
        if maximum_fraction_digits > MAX_FRACTION_DIGITS {
            return Err(Error::FractionDigits {
                name: "maximum_fraction_digits",
            });
        }
        if minimum_fraction_digits > maximum_fraction_digits {
            return Err(Error::IncompatibleFractionDigits);
        }
        Ok(Self {
            grouping,
            minimum_fraction_digits,
            maximum_fraction_digits,
        })
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::defaults()
    }
}

struct Formats {
    grouped: DecimalFormatter,
    ungrouped: DecimalFormatter,
}

impl Formats {
    fn for_locale(locale: &Locale) -> Result<Self, Error> {
        let locale = locale
            .to_string()
            .parse::<icu_locale::Locale>()
            .map_err(|error| Error::Locale {
                locale: locale.to_string(),
                message: error.to_string(),
            })?;
        let load = |grouping_strategy| {
            DecimalFormatter::try_new(locale.clone().into(), {
                let mut options = DecimalFormatterOptions::default();
                options.grouping_strategy = Some(grouping_strategy);
                options
            })
            .map_err(|error| Error::Locale {
                locale: locale.to_string(),
                message: error.to_string(),
            })
        };
        Ok(Self {
            grouped: load(GroupingStrategy::Auto)?,
            ungrouped: load(GroupingStrategy::Never)?,
        })
    }
}

thread_local! {
    /// ICU's formatter is deliberately thread-local because its compiled data
    /// is not `Send`. Heine renders synchronously and reuses it per locale.
    static FORMATS: RefCell<BTreeMap<Locale, Formats>> = const { RefCell::new(BTreeMap::new()) };
}

pub(crate) fn format(
    locale: &Locale,
    number: &LocalizedNumber,
    options: Options,
) -> Result<String, Error> {
    FORMATS.with(|formats| {
        let mut formats = formats.borrow_mut();
        let formats = formats
            .entry(locale.clone())
            .or_insert(Formats::for_locale(locale)?);
        let decimal = number.prepared(options);
        let formatter = if options.grouping {
            &formats.grouped
        } else {
            &formats.ungrouped
        };
        Ok(formatter.format(&decimal).to_string())
    })
}

impl fmt::Display for LocalizedNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_grouped_decimal_values_for_multiple_locales() {
        let value = LocalizedNumber::from_str("1234567.895").unwrap();
        assert_eq!(
            format(&"en-US".parse().unwrap(), &value, Options::default()).unwrap(),
            "1,234,567.895"
        );
        assert_eq!(
            format(&"de-DE".parse().unwrap(), &value, Options::default()).unwrap(),
            "1.234.567,895"
        );
        assert_eq!(
            format(&"fr".parse().unwrap(), &value, Options::default()).unwrap(),
            "1\u{202f}234\u{202f}567,895"
        );
        assert_eq!(
            format(&"bn".parse().unwrap(), &value, Options::default()).unwrap(),
            "১২,৩৪,৫৬৭.৮৯৫"
        );
    }

    #[test]
    fn applies_half_even_rounding_and_requested_padding() {
        let value = LocalizedNumber::from_str("2.345").unwrap();
        let options = Options::new(true, 2, 2).unwrap();
        assert_eq!(
            format(&"en".parse().unwrap(), &value, options).unwrap(),
            "2.34"
        );
        let value = LocalizedNumber::from_str("2.355").unwrap();
        assert_eq!(
            format(&"en".parse().unwrap(), &value, options).unwrap(),
            "2.36"
        );
    }

    #[test]
    fn fluent_boundary_rejects_inexact_large_integers() {
        assert!(LocalizedNumber::from_i64(i64::MIN).fluent_value().is_err());
        assert!(LocalizedNumber::from_u64(u64::MAX).fluent_value().is_err());
        assert_eq!(
            LocalizedNumber::from_i64(9_007_199_254_740_992)
                .fluent_value()
                .unwrap(),
            9_007_199_254_740_992.0
        );
    }

    #[test]
    fn preserves_all_supported_integer_digits_for_direct_template_formatting() {
        let locale = "en".parse().unwrap();
        assert_eq!(
            format(
                &locale,
                &LocalizedNumber::from_i64(i64::MIN),
                Options::default()
            )
            .unwrap(),
            "-9,223,372,036,854,775,808"
        );
        assert_eq!(
            format(
                &locale,
                &LocalizedNumber::from_i64(i64::MAX),
                Options::default()
            )
            .unwrap(),
            "9,223,372,036,854,775,807"
        );
        assert_eq!(
            format(
                &locale,
                &LocalizedNumber::from_u64(u64::MAX),
                Options::default()
            )
            .unwrap(),
            "18,446,744,073,709,551,615"
        );
    }

    #[test]
    fn accepts_the_documented_fraction_digit_boundary() {
        let options = Options::new(false, MAX_FRACTION_DIGITS, MAX_FRACTION_DIGITS).unwrap();
        let output = format(
            &"en".parse().unwrap(),
            &LocalizedNumber::from_i64(1),
            options,
        )
        .unwrap();
        assert_eq!(output.len(), usize::from(MAX_FRACTION_DIGITS) + 2);
        assert!(output.starts_with("1."));
    }

    #[test]
    fn accepts_scientific_notation_from_a_template_float() {
        let value = LocalizedNumber::from_f64(1.25e-7).unwrap();
        assert_eq!(value.to_string(), "0.000000125");
    }
}
