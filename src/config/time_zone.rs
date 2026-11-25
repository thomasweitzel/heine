//! Deterministic editorial time-zone configuration.

use jiff::{
    Timestamp,
    civil::DateTime,
    tz::{AmbiguousOffset, TimeZone},
};

/// A named IANA time zone resolved from Heine's bundled database.
#[derive(Debug, Clone)]
pub(crate) struct EditorialTimeZone {
    name: String,
    zone: TimeZone,
}

impl PartialEq for EditorialTimeZone {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for EditorialTimeZone {}

impl EditorialTimeZone {
    /// Returns the calendar date that this instant has in the site's
    /// editorial time zone. Templates use this for reader-facing dates while
    /// feeds and sitemaps retain the same instant in UTC.
    pub(crate) fn calendar_date(&self, timestamp: Timestamp) -> String {
        self.zone.to_datetime(timestamp).date().to_string()
    }

    /// Returns the local civil date-time for an instant in the editorial zone.
    /// Formatting code consumes this display value only after the timestamp has
    /// been resolved through the site's bundled zone database.
    pub(crate) fn civil_datetime(&self, timestamp: Timestamp) -> jiff::civil::DateTime {
        self.zone.to_datetime(timestamp)
    }

    /// Resolves an authored offsetless TOML date-time without consulting host
    /// timezone data. Callers retain their own source span and field context.
    pub(crate) fn resolve_local(
        &self,
        field: &str,
        value: &toml::value::Datetime,
    ) -> Result<Timestamp, String> {
        let local = value
            .to_string()
            .parse::<DateTime>()
            .map_err(|_| format!("field `{field}` contains an invalid local date-time"))?;
        let ambiguous = self.zone.to_ambiguous_timestamp(local);
        match ambiguous.offset() {
            AmbiguousOffset::Unambiguous { .. } => ambiguous.unambiguous().map_err(|_| {
                format!(
                    "field `{field}` could not be resolved in site.time_zone {:?}",
                    self.name
                )
            }),
            AmbiguousOffset::Gap { before, after } => Err(format!(
                "field `{field}` does not exist in site.time_zone {:?}: the clock moves from {before} to {after}; choose a local time outside that gap",
                self.name
            )),
            AmbiguousOffset::Fold { before, after } => Err(format!(
                "field `{field}` occurs twice in site.time_zone {:?}: the clock moves from {before} to {after}; choose a local time outside that repeated interval",
                self.name
            )),
        }
    }
}

impl TryFrom<String> for EditorialTimeZone {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let zone = TimeZone::get(&value)
            .map_err(|_| format!("site.time_zone {value:?} is not a known IANA time-zone name"))?;
        Ok(Self { name: value, zone })
    }
}

#[cfg(test)]
mod tests {
    use super::EditorialTimeZone;
    use jiff::Timestamp;

    #[test]
    fn accepts_known_iana_zone_names_only() {
        assert!(EditorialTimeZone::try_from("Europe/Berlin".to_owned()).is_ok());
        assert!(EditorialTimeZone::try_from("not/a-time-zone".to_owned()).is_err());
    }

    #[test]
    fn civil_conversion_matches_zoned_conversion_at_transitions_and_boundaries() {
        for name in ["Europe/Berlin", "Pacific/Kiritimati", "America/New_York"] {
            let zone = EditorialTimeZone::try_from(name.to_owned()).unwrap();
            for timestamp in [
                Timestamp::MIN,
                Timestamp::MAX,
                "2026-03-29T00:59:59Z".parse().unwrap(),
                "2026-03-29T01:00:00Z".parse().unwrap(),
                "2026-10-25T00:59:59Z".parse().unwrap(),
                "2026-10-25T01:00:00Z".parse().unwrap(),
            ] {
                let expected = timestamp.to_zoned_datetime(zone.zone.clone()).datetime();
                assert_eq!(
                    zone.civil_datetime(timestamp),
                    expected,
                    "{name}: {timestamp}"
                );
                assert_eq!(zone.calendar_date(timestamp), expected.date().to_string());
            }
        }
    }

    #[test]
    fn timestamp_resolution_matches_zoned_resolution() {
        for name in ["Europe/Berlin", "Pacific/Kiritimati", "America/New_York"] {
            let zone = EditorialTimeZone::try_from(name.to_owned()).unwrap();
            for local in [
                "2026-01-15T12:00:00",
                "2026-03-29T02:30:00",
                "2026-10-25T02:30:00",
                "2026-03-08T02:30:00",
                "2026-11-01T01:30:00",
                "0001-01-01T00:00:00",
                "9999-12-31T23:59:59",
            ] {
                let datetime = local.parse().unwrap();
                let direct = zone.zone.to_ambiguous_timestamp(datetime);
                let zoned = zone.zone.to_ambiguous_zoned_datetime(datetime);
                assert_eq!(direct.offset(), zoned.offset(), "{name}: {local}");
                assert_eq!(
                    direct.unambiguous().ok(),
                    zoned.unambiguous().ok().map(|value| value.timestamp()),
                    "{name}: {local}"
                );
            }
        }
    }
}
