//! Deterministic locale-aware presentation of checked calendar values.

use std::{cell::RefCell, collections::BTreeMap};

use fluent_bundle::{FluentArgs, FluentValue};
use icu_datetime::{
    DateTimeFormatter,
    fieldsets::{T, YMD, YMDT},
    input::{Date, DateTime, Time},
    options::TimePrecision,
};
use jiff::Timestamp;
use thiserror::Error;

use crate::{config::EditorialTimeZone, locale::Locale};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Style {
    Short,
    Medium,
    Long,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Presentation {
    Date(Style),
    Time(Style),
    DateTime { date: Style, time: Style },
}

impl Presentation {
    /// Checks that an authored value has the temporal meaning this presentation
    /// requires without constructing localized output.
    pub(crate) fn validate(
        self,
        editorial_time_zone: Option<&EditorialTimeZone>,
        value: &str,
    ) -> Result<(), String> {
        match self {
            Self::Date(_) => parse_date(editorial_time_zone, value).map(|_| ()),
            Self::Time(_) | Self::DateTime { .. } => {
                parse_instant(editorial_time_zone, value).map(|_| ())
            }
        }
    }

    pub(crate) fn format(
        self,
        locale: &Locale,
        editorial_time_zone: Option<&EditorialTimeZone>,
        value: &str,
    ) -> Result<String, String> {
        match self {
            Self::Date(style) => format_date(locale, editorial_time_zone, value, style),
            Self::Time(style) => format_time(locale, editorial_time_zone, value, style),
            Self::DateTime { date, time } => {
                format_datetime(locale, editorial_time_zone, value, date, time)
            }
        }
    }
}

impl Style {
    pub(crate) fn parse(value: &str, name: &str) -> Result<Self, String> {
        match value {
            "short" => Ok(Self::Short),
            "medium" => Ok(Self::Medium),
            "long" => Ok(Self::Long),
            _ => Err(format!(
                "{name} must be \"short\", \"medium\", or \"long\", not {value:?}"
            )),
        }
    }
}

#[derive(Debug, Error)]
#[error("could not load date and time formatting data for locale {locale:?}: {message}")]
struct FormatError {
    locale: String,
    message: String,
}

/// Prebuilt ICU formatters for one validated locale.
struct Formats {
    dates: BTreeMap<Style, DateTimeFormatter<YMD>>,
    times: BTreeMap<Style, DateTimeFormatter<T>>,
    datetimes: BTreeMap<(Style, Style), DateTimeFormatter<YMDT>>,
}

impl Formats {
    fn for_locale(locale: &Locale) -> Result<Self, FormatError> {
        let locale = locale
            .to_string()
            .parse::<icu_locale::Locale>()
            .map_err(|error| FormatError {
                locale: locale.to_string(),
                message: error.to_string(),
            })?;
        let load_date = |field_set| {
            DateTimeFormatter::try_new(locale.clone().into(), field_set).map_err(|error| {
                FormatError {
                    locale: locale.to_string(),
                    message: error.to_string(),
                }
            })
        };
        let dates = [
            (Style::Short, load_date(YMD::short())?),
            (Style::Medium, load_date(YMD::medium())?),
            (Style::Long, load_date(YMD::long())?),
        ]
        .into_iter()
        .collect();
        let times = [Style::Short, Style::Medium, Style::Long]
            .into_iter()
            .map(|style| {
                DateTimeFormatter::try_new(locale.clone().into(), time_field_set(style))
                    .map(|formatter| (style, formatter))
                    .map_err(|error| FormatError {
                        locale: locale.to_string(),
                        message: error.to_string(),
                    })
            })
            .collect::<Result<_, _>>()?;
        let mut datetimes = BTreeMap::new();
        for date_style in [Style::Short, Style::Medium, Style::Long] {
            for time_style in [Style::Short, Style::Medium, Style::Long] {
                let field_set = datetime_field_set(date_style, time_style);
                let formatter = DateTimeFormatter::try_new(locale.clone().into(), field_set)
                    .map_err(|error| FormatError {
                        locale: locale.to_string(),
                        message: error.to_string(),
                    })?;
                datetimes.insert((date_style, time_style), formatter);
            }
        }
        Ok(Self {
            dates,
            times,
            datetimes,
        })
    }
}

fn datetime_field_set(date_style: Style, time_style: Style) -> YMDT {
    let date = match date_style {
        Style::Short => YMD::short(),
        Style::Medium => YMD::medium(),
        Style::Long => YMD::long(),
    };
    if time_uses_seconds(time_style) {
        date.with_time_hms()
    } else {
        date.with_time_hm()
    }
}

/// A short time is an hour-and-minute presentation. Medium and long time
/// presentations retain seconds, matching the same styles in a combined value.
fn time_uses_seconds(style: Style) -> bool {
    !matches!(style, Style::Short)
}

fn time_field_set(style: Style) -> T {
    match style {
        Style::Short => T::short(),
        Style::Medium => T::medium(),
        Style::Long => T::long(),
    }
    .with_time_precision(if time_uses_seconds(style) {
        TimePrecision::Second
    } else {
        TimePrecision::Minute
    })
}

thread_local! {
    /// ICU formatters contain non-Send data. Heine renders synchronously, so
    /// one cache per rendering thread avoids repeated construction without
    /// making formatter state visible to templates.
    static FORMATS: RefCell<BTreeMap<Locale, Formats>> = const { RefCell::new(BTreeMap::new()) };
}

pub(crate) fn format_date(
    locale: &Locale,
    editorial_time_zone: Option<&EditorialTimeZone>,
    value: &str,
    style: Style,
) -> Result<String, String> {
    let date = parse_date(editorial_time_zone, value)?;
    with_formats(locale, |formats| {
        formats.dates[&style].format(&date).to_string()
    })
}

pub(crate) fn format_time(
    locale: &Locale,
    editorial_time_zone: Option<&EditorialTimeZone>,
    value: &str,
    style: Style,
) -> Result<String, String> {
    let time = parse_instant(editorial_time_zone, value)?.time;
    with_formats(locale, |formats| {
        formats.times[&style].format(&time).to_string()
    })
}

pub(crate) fn format_datetime(
    locale: &Locale,
    editorial_time_zone: Option<&EditorialTimeZone>,
    value: &str,
    date_style: Style,
    time_style: Style,
) -> Result<String, String> {
    let datetime = parse_instant(editorial_time_zone, value)?;
    with_formats(locale, |formats| {
        formats.datetimes[&(date_style, time_style)]
            .format(&datetime)
            .to_string()
    })
}

/// Fluent only passes strings across this function boundary. Resource loading
/// checks the static call shape first; this function checks the dynamic value
/// that a template supplies when the selected message renders.
pub(crate) fn format_fluent(
    locale: &Locale,
    editorial_time_zone: Option<&EditorialTimeZone>,
    positional: &[FluentValue<'_>],
    named: &FluentArgs<'_>,
) -> Result<String, String> {
    let [FluentValue::String(value)] = positional else {
        return Err("DATETIME() requires exactly one date or RFC 3339 instant argument".into());
    };
    let date_style = fluent_style(named, "dateStyle")?;
    let time_style = fluent_style(named, "timeStyle")?;
    fluent_presentation(date_style, time_style)?.format(locale, editorial_time_zone, value)
}

fn fluent_style(named: &FluentArgs<'_>, name: &str) -> Result<Option<Style>, String> {
    match named.get(name) {
        Some(FluentValue::String(value)) => Style::parse(value, name).map(Some),
        Some(_) => Err(format!("DATETIME() option {name} must be a style string")),
        None => Ok(None),
    }
}

pub(crate) fn fluent_presentation(
    date_style: Option<Style>,
    time_style: Option<Style>,
) -> Result<Presentation, String> {
    match (date_style, time_style) {
        (Some(date), Some(time)) => Ok(Presentation::DateTime { date, time }),
        (Some(style), None) => Ok(Presentation::Date(style)),
        (None, Some(style)) => Ok(Presentation::Time(style)),
        (None, None) => Err("DATETIME() requires dateStyle or timeStyle".into()),
    }
}

fn with_formats<T>(locale: &Locale, format: impl FnOnce(&Formats) -> T) -> Result<T, String> {
    FORMATS.with(|formats| {
        let mut formats = formats.borrow_mut();
        let formats = formats
            .entry(locale.clone())
            .or_insert(Formats::for_locale(locale).map_err(|error| error.to_string())?);
        Ok(format(formats))
    })
}

/// A date is either an authored ISO calendar date or the editorial local date
/// of an RFC 3339 instant. Time-bearing presentation takes the stricter path
/// below and never invents midnight for the first form.
fn parse_date(
    editorial_time_zone: Option<&EditorialTimeZone>,
    value: &str,
) -> Result<Date<icu_calendar::Iso>, String> {
    let calendar = if value.contains('T') {
        let timestamp = parse_timestamp(value)?;
        match editorial_time_zone {
            Some(time_zone) => time_zone.calendar_date(timestamp),
            None => value
                .get(..10)
                .map(str::to_owned)
                .ok_or_else(|| invalid_date(value))?,
        }
    } else {
        value.to_owned()
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

fn parse_instant(
    editorial_time_zone: Option<&EditorialTimeZone>,
    value: &str,
) -> Result<DateTime<icu_calendar::Iso>, String> {
    let timestamp = parse_timestamp(value).map_err(|_| {
        format!(
            "time value {value:?} must be an RFC 3339 offset date-time; calendar dates do not have a time"
        )
    })?;
    let time_zone = editorial_time_zone.ok_or_else(|| {
        "time formatting requires site.time_zone; add an IANA time-zone name under [site]"
            .to_owned()
    })?;
    let local = time_zone.civil_datetime(timestamp);
    let date = Date::try_new_iso(
        i32::from(local.year()),
        u8::try_from(local.month()).map_err(|_| invalid_date(value))?,
        u8::try_from(local.day()).map_err(|_| invalid_date(value))?,
    )
    .map_err(|_| invalid_date(value))?;
    let time = Time::try_new(
        u8::try_from(local.hour()).map_err(|_| invalid_date(value))?,
        u8::try_from(local.minute()).map_err(|_| invalid_date(value))?,
        u8::try_from(local.second()).map_err(|_| invalid_date(value))?,
        u32::try_from(local.subsec_nanosecond()).map_err(|_| invalid_date(value))?,
    )
    .map_err(|_| invalid_date(value))?;
    Ok(DateTime { date, time })
}

fn parse_timestamp(value: &str) -> Result<Timestamp, String> {
    value.parse::<Timestamp>().map_err(|_| invalid_date(value))
}

fn invalid_date(value: &str) -> String {
    format!(
        "date value {value:?} must be a four-digit ISO calendar date or RFC 3339 offset date-time"
    )
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

    fn berlin() -> EditorialTimeZone {
        EditorialTimeZone::try_from("Europe/Berlin".to_owned()).expect("known IANA time zone")
    }

    #[test]
    fn formats_iso_dates_for_the_requested_locale() {
        assert_eq!(
            format_date(&"en".parse().unwrap(), None, "2026-08-14", Style::Long).unwrap(),
            "August 14, 2026"
        );
        assert_eq!(
            format_date(&"de".parse().unwrap(), None, "2026-08-14", Style::Long).unwrap(),
            "14. August 2026"
        );
    }

    #[test]
    fn preserves_the_editorial_publication_day() {
        let locale = "en".parse().unwrap();
        assert_eq!(
            format_date(
                &locale,
                Some(&berlin()),
                "2026-02-10T23:00:00Z",
                Style::Long,
            )
            .unwrap(),
            "February 11, 2026"
        );
    }

    #[test]
    fn retains_an_offset_timestamps_calendar_date_without_an_editorial_time_zone() {
        assert_eq!(
            format_date(
                &"en".parse().unwrap(),
                None,
                "2026-08-14T00:30:00+02:00",
                Style::Long,
            )
            .unwrap(),
            "August 14, 2026"
        );
    }

    #[test]
    fn rejects_noncanonical_calendar_forms() {
        for value in ["2026-8-14", "-0001-02-03", "2026-08-14Tnot-a-time"] {
            assert!(parse_date(None, value).is_err(), "{value}");
        }
    }

    #[test]
    fn formats_time_and_datetime_through_the_editorial_zone() {
        let locale = "en".parse().unwrap();
        let zone = berlin();
        let value = "2026-03-29T00:30:15Z";
        assert_eq!(
            format_time(&locale, Some(&zone), value, Style::Medium).unwrap(),
            "1:30:15\u{202f}AM"
        );
        assert_eq!(
            format_datetime(&locale, Some(&zone), value, Style::Long, Style::Short).unwrap(),
            "March 29, 2026 at 1:30\u{202f}AM"
        );
    }

    #[test]
    fn applies_the_offset_in_force_on_each_side_of_a_dst_boundary() {
        let locale = "en".parse().unwrap();
        let zone = berlin();
        assert_eq!(
            format_time(&locale, Some(&zone), "2026-03-29T00:30:00Z", Style::Short).unwrap(),
            "1:30\u{202f}AM"
        );
        assert_eq!(
            format_time(&locale, Some(&zone), "2026-03-29T01:30:00Z", Style::Short).unwrap(),
            "3:30\u{202f}AM"
        );
        assert_eq!(
            format_time(&locale, Some(&zone), "2026-10-25T00:30:00Z", Style::Short).unwrap(),
            "2:30\u{202f}AM"
        );
        assert_eq!(
            format_time(&locale, Some(&zone), "2026-10-25T01:30:00Z", Style::Short).unwrap(),
            "2:30\u{202f}AM"
        );
    }

    #[test]
    fn rejects_calendar_dates_for_time_bearing_presentations() {
        let locale = "en".parse().unwrap();
        let error = format_time(&locale, Some(&berlin()), "2026-08-14", Style::Short)
            .expect_err("calendar dates have no time");
        assert!(error.contains("calendar dates do not have a time"));
    }

    #[test]
    fn requires_an_editorial_time_zone_for_time_bearing_presentations() {
        let error = format_time(
            &"en".parse().unwrap(),
            None,
            "2026-08-14T09:30:00Z",
            Style::Short,
        )
        .expect_err("time presentation requires an editorial time zone");
        assert!(error.contains("site.time_zone"));
    }

    #[test]
    fn long_time_does_not_add_a_zone_name() {
        let output = format_time(
            &"en".parse().unwrap(),
            Some(&berlin()),
            "2026-08-14T09:30:00Z",
            Style::Long,
        )
        .unwrap();
        assert!(!output.contains("Berlin"));
        assert!(!output.contains("GMT"));
        assert!(!output.contains("UTC"));
    }

    #[test]
    fn editorial_time_zone_uses_bundled_data() -> Result<(), Box<dyn std::error::Error>> {
        const CHILD: &str = "HEINE_EDITORIAL_TIME_ZONE_CHILD";

        if std::env::var_os(CHILD).is_some() {
            assert_bundled_berlin_output();
            return Ok(());
        }

        let nonexistent_tzdir = std::env::temp_dir().join(format!(
            "heine-no-zoneinfo-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let status = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "localization::temporal::tests::editorial_time_zone_uses_bundled_data",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("TZ", "Pacific/Kiritimati")
            .env("TZDIR", nonexistent_tzdir)
            .status()?;
        assert!(status.success(), "isolated time-zone test failed");
        Ok(())
    }

    fn assert_bundled_berlin_output() {
        let locale = "en".parse().unwrap();
        let zone = berlin();
        assert_eq!(
            format_datetime(
                &locale,
                Some(&zone),
                "2026-10-25T01:30:00Z",
                Style::Long,
                Style::Short,
            )
            .unwrap(),
            "October 25, 2026 at 2:30\u{202f}AM"
        );
    }
}
