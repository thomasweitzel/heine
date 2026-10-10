//! Locale-aware, deterministic ordering for authored human-language text.
//!
//! Every feature that reads the localized site model receives a configured
//! BCP-47 locale.
//! This module keeps ICU collation and Heine's literal tie-breaker in one
//! place, so collections and taxonomies cannot accidentally adopt different
//! ordering contracts.

use std::cmp::Ordering;

use crate::locale::Locale;

/// Compares text with the locale's ICU collation, then its literal spelling.
///
/// The literal tie-breaker makes equal collation keys deterministic. A
/// configured locale is already validated as BCP-47; retaining the literal
/// fallback here nevertheless keeps a build deterministic if ICU cannot
/// construct a collator for a future accepted locale value.
pub(super) struct TextCollation {
    collator: Option<icu_collator::CollatorBorrowed<'static>>,
}

impl TextCollation {
    pub(super) fn for_locale(locale: &Locale) -> Self {
        let collator = locale
            .to_string()
            .parse::<icu_locale::Locale>()
            .ok()
            .and_then(|locale| {
                icu_collator::Collator::try_new(
                    icu_collator::CollatorPreferences::from(locale),
                    Default::default(),
                )
                .ok()
            });
        Self { collator }
    }

    pub(super) fn compare(&self, left: &str, right: &str) -> Ordering {
        self.collator
            .as_ref()
            .map(|collator| collator.compare(left, right))
            .unwrap_or_else(|| left.cmp(right))
            .then(left.cmp(right))
    }
}
