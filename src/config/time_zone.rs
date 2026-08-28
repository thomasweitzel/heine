//! Deterministic editorial time-zone configuration.

use jiff::tz::TimeZone;

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
    pub(crate) fn zone(&self) -> &TimeZone {
        &self.zone
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
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
