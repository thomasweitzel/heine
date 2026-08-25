//! Locale-aware template built-ins.

use std::{cell::RefCell, collections::BTreeMap};

use icu_datetime::{DateTimeFormatter, fieldsets::YMD, input::Date};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{locale::Locale, localization::Arguments};

use super::runtime::TemplateRuntime;

pub(super) fn register(tera: &mut tera::Tera, runtime: &TemplateRuntime) {
    let translation_runtime = runtime.clone();
    tera.register_function(
        "t",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            let id = kwargs.must_get::<&str>("id")?;
            let attribute = kwargs.get::<&str>("attribute")?;
            let mut arguments = Arguments::new();
            for (key, value) in kwargs.iter() {
                let Some(key) = key.as_str() else {
                    return Err(tera::Error::message(
                        "translation argument names must be strings",
                    ));
                };
                if matches!(key, "id" | "attribute") {
                    continue;
                }
                if let Some(value) = value.as_i64() {
                    arguments.set(key.to_owned(), value);
                } else if let Some(value) = value.as_f64() {
                    arguments.set(key.to_owned(), value);
                } else if let Some(value) = value.as_bool() {
                    arguments.set(key.to_owned(), value.to_string());
                } else if let Some(value) = value.as_str() {
                    arguments.set(key.to_owned(), value.to_owned());
                } else {
                    return Err(tera::Error::message(format!(
                        "translation argument {key:?} must be a string, number, or boolean"
                    )));
                }
            }
            translation_runtime
                .localization()
                .format(
                    &translation_runtime.locale(state)?,
                    id,
                    attribute,
                    &arguments,
                )
                .map(Into::into)
                .map_err(tera::Error::message)
        },
    );

    let date_runtime = runtime.clone();
    tera.register_function(
        "format_date",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            let value = kwargs.must_get::<&str>("value")?;
            let style = kwargs.get::<&str>("style")?.unwrap_or("medium");
            let locale = date_runtime.locale(state)?;
            format_date(&locale, value, style)
                .map(Into::into)
                .map_err(tera::Error::message)
        },
    );
}

#[derive(Debug, Error)]
#[error("could not load date formatting data for locale {locale:?}: {message}")]
pub(super) struct DateFormatError {
    locale: String,
    message: String,
}

/// Prebuilt ICU formatters for one validated locale.
struct DateFormats {
    styles: DateStyles,
}

#[derive(Clone)]
struct DateStyles {
    short: DateTimeFormatter<YMD>,
    medium: DateTimeFormatter<YMD>,
    long: DateTimeFormatter<YMD>,
}

impl DateFormats {
    fn for_locale(locale: &Locale) -> Result<Self, DateFormatError> {
        let icu_locale = locale
            .to_string()
            .parse::<icu_locale::Locale>()
            .map_err(|error| DateFormatError {
                locale: locale.to_string(),
                message: error.to_string(),
            })?;
        let load = |style| {
            DateTimeFormatter::try_new(icu_locale.clone().into(), style).map_err(|error| {
                DateFormatError {
                    locale: locale.to_string(),
                    message: error.to_string(),
                }
            })
        };
        Ok(Self {
            styles: DateStyles {
                short: load(YMD::short())?,
                medium: load(YMD::medium())?,
                long: load(YMD::long())?,
            },
        })
    }

    fn format(&self, value: &str, style: &str) -> Result<String, String> {
        let date = parse_iso_date(value)?;
        let formatted = match style {
            "short" => &self.styles.short,
            "medium" => &self.styles.medium,
            "long" => &self.styles.long,
            _ => {
                return Err(format!(
                    "date style must be \"short\", \"medium\", or \"long\", not {style:?}"
                ));
            }
        };
        Ok(formatted.format(&date).to_string())
    }
}

thread_local! {
    /// ICU's compiled formatter owns non-Send data. Heine builds
    /// synchronously, so this preserves Tera's Send + Sync callback contract
    /// while avoiding repeated formatter construction.
    static DATE_FORMATS: RefCell<BTreeMap<Locale, DateFormats>> = const { RefCell::new(BTreeMap::new()) };
}

fn format_date(locale: &Locale, value: &str, style: &str) -> Result<String, String> {
    DATE_FORMATS.with(|formats| {
        let mut formats = formats.borrow_mut();
        let formats = formats
            .entry(locale.clone())
            .or_insert(DateFormats::for_locale(locale).map_err(|error| error.to_string())?);
        formats.format(value, style)
    })
}

/// `page.date` is validated before rendering. The public helper deliberately
/// accepts that same contract: a four-digit ISO calendar date or RFC 3339
/// offset date-time. This keeps date-only values strict and predictable.
fn parse_iso_date(value: &str) -> Result<Date<icu_calendar::Iso>, String> {
    let calendar = match value.contains('T') {
        true => OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|_| {
                format!(
                    "date value {value:?} must be a four-digit ISO calendar date or RFC 3339 offset date-time"
                )
            })?
            .date()
            .to_string(),
        false => value.to_owned(),
    };
    let bytes = calendar.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[0..4]
            .iter()
            .chain(bytes[5..7].iter())
            .chain(bytes[8..10].iter())
            .all(u8::is_ascii_digit)
    {
        return Err(format!(
            "date value {value:?} must use the four-digit ISO calendar-date form YYYY-MM-DD"
        ));
    }
    let year = calendar[0..4]
        .parse()
        .map_err(|_| format!("date value {value:?} has an invalid year"))?;
    let month = parse_component(&calendar[5..7], "month", value)?;
    let day = parse_component(&calendar[8..10], "day", value)?;
    Date::try_new_iso(year, month, day)
        .map_err(|_| format!("date value {value:?} has an invalid calendar date"))
}

fn parse_component<T>(component: &str, name: &str, value: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    component
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then_some(component)
        .and_then(|component| component.parse().ok())
        .ok_or_else(|| format!("date value {value:?} has an invalid {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_iso_dates_for_the_requested_locale() {
        let formats = DateFormats::for_locale(&"en".parse().unwrap()).unwrap();
        assert_eq!(
            formats.format("2026-08-14", "long").unwrap(),
            "August 14, 2026"
        );
        let german = DateFormats::for_locale(&"de".parse().unwrap()).unwrap();
        assert_eq!(
            german.format("2026-08-14", "long").unwrap(),
            "14. August 2026"
        );
    }

    #[test]
    fn rejects_unknown_styles_and_non_dates() {
        let formats = DateFormats::for_locale(&"en".parse().unwrap()).unwrap();
        assert!(formats.format("2026-08-14", "full").is_err());
        assert!(formats.format("not a date", "long").is_err());
    }

    #[test]
    fn accepts_the_page_date_model_and_rejects_ambiguous_calendar_forms() {
        assert!(parse_iso_date("0001-02-03").is_ok());
        assert!(parse_iso_date("2026-08-14T09:30:00+02:00").is_ok());
        assert!(parse_iso_date("2026-8-14").is_err());
        assert!(parse_iso_date("-0001-02-03").is_err());
        assert!(parse_iso_date("2026-08-14Tnot-a-time").is_err());
    }
}
