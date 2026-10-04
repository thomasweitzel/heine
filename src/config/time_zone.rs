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
        timestamp.to_zoned(self.zone.clone()).date().to_string()
    }

    /// Returns the local civil date-time for an instant in the editorial zone.
    /// Formatting code consumes this display value only after the timestamp has
    /// been resolved through the site's bundled zone database.
    pub(crate) fn civil_datetime(&self, timestamp: Timestamp) -> jiff::civil::DateTime {
        timestamp.to_zoned(self.zone.clone()).datetime()
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
        let ambiguous = self.zone.to_ambiguous_zoned(local);
        match ambiguous.offset() {
            AmbiguousOffset::Unambiguous { .. } => ambiguous
                .unambiguous()
                .map(|zoned| zoned.timestamp())
                .map_err(|_| {
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

    #[test]
    fn accepts_known_iana_zone_names_only() {
        assert!(EditorialTimeZone::try_from("Europe/Berlin".to_owned()).is_ok());
        assert!(EditorialTimeZone::try_from("not/a-time-zone".to_owned()).is_err());
    }
}
