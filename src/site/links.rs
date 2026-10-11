//! Locale-aware lookup of resolved rendered pages.
//!
//! This module owns the immutable catalog shared by Markdown and templates.
//! Keeping rendered views, output targets, and deliberately excluded drafts
//! together ensures every lookup observes one publication boundary.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use crate::{
    config::{BasePath, CanonicalUrls},
    locale::Locale,
    output::OutputPath,
};

use super::page::{Page, PageId, PageView};

/// The complete, locale-aware catalog of rendered pages.
///
/// Keeping template views and their output targets together makes it
/// impossible for the template boundary to observe two independently-built
/// page indexes. The catalog is constructed only after page relationships
/// have been resolved and is immutable for the lifetime of a build.
#[derive(Debug, Clone)]
pub(crate) struct PageCatalog {
    views: Arc<BTreeMap<Locale, BTreeMap<PageId, PageView>>>,
    targets: Arc<BTreeMap<Locale, BTreeMap<PageId, OutputPath>>>,
    excluded_drafts: Arc<BTreeMap<Locale, BTreeMap<PageId, PathBuf>>>,
}

impl PageCatalog {
    pub(super) fn from_pages(
        pages: BTreeMap<Locale, BTreeMap<PageId, Page>>,
        excluded_drafts: Arc<BTreeMap<Locale, BTreeMap<PageId, PathBuf>>>,
        canonical_urls: Option<&CanonicalUrls>,
    ) -> Self {
        let mut views = BTreeMap::new();
        let mut targets = BTreeMap::new();
        for (locale, pages) in pages {
            let mut locale_views = BTreeMap::new();
            let mut locale_targets = BTreeMap::new();
            for (id, page) in pages {
                if let Some(output) = page.output_path.clone() {
                    locale_targets.insert(id.clone(), output);
                }
                locale_views.insert(id, page.into_view(canonical_urls));
            }
            views.insert(locale.clone(), locale_views);
            targets.insert(locale, locale_targets);
        }
        Self {
            views: Arc::new(views),
            targets: Arc::new(targets),
            excluded_drafts,
        }
    }

    pub(crate) fn links(
        &self,
        current_locale: Locale,
        base_path: BasePath,
        canonical_urls: Option<CanonicalUrls>,
    ) -> PageLinks {
        PageLinks::from_targets(
            current_locale,
            base_path,
            canonical_urls,
            self.targets.clone(),
            self.excluded_drafts.clone(),
        )
    }

    pub(super) fn links_from_pages(
        pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
        current_locale: Locale,
        base_path: BasePath,
        canonical_urls: Option<CanonicalUrls>,
        excluded_drafts: Arc<BTreeMap<Locale, BTreeMap<PageId, PathBuf>>>,
    ) -> PageLinks {
        let targets = pages
            .iter()
            .map(|(locale, pages)| {
                (
                    locale.clone(),
                    pages
                        .iter()
                        .filter_map(|(id, page)| {
                            page.output_path
                                .as_ref()
                                .map(|output| (id.clone(), output.clone()))
                        })
                        .collect(),
                )
            })
            .collect();
        PageLinks::from_targets(
            current_locale,
            base_path,
            canonical_urls,
            Arc::new(targets),
            excluded_drafts,
        )
    }

    pub(crate) fn view(&self, locale: &Locale, id: &PageId) -> Option<&PageView> {
        self.views.get(locale)?.get(id)
    }

    pub(crate) fn output_path(&self, locale: &Locale, id: &PageId) -> Option<&OutputPath> {
        self.targets.get(locale)?.get(id)
    }
}

/// Locale-aware rendered-page lookup shared by Tera and Markdown. Page IDs
/// remain locale-local; callers opt into another locale explicitly.
#[derive(Clone)]
pub(crate) struct PageLinks {
    current_locale: Locale,
    base_path: BasePath,
    canonical_urls: Option<CanonicalUrls>,
    targets: Arc<BTreeMap<Locale, BTreeMap<PageId, OutputPath>>>,
    excluded_drafts: Arc<BTreeMap<Locale, BTreeMap<PageId, PathBuf>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PageLinkTarget {
    pub(crate) id: PageId,
    pub(crate) locale: Locale,
    pub(crate) output_path: OutputPath,
    pub(crate) url: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PageResolveError {
    #[error("page id must be a non-empty relative path without '.', '..', or backslashes")]
    InvalidId,
    #[error("locale {locale:?} is not configured")]
    InvalidLocale { locale: String },
    #[error("rendered page {id:?} was not found for locale {locale:?}")]
    NotFound { locale: String, id: String },
    #[error(
        "page {id:?} for locale {locale:?} is excluded because {path} declares `draft = true`; ordinary builds cannot link to drafts"
    )]
    DraftExcluded {
        locale: String,
        id: String,
        path: PathBuf,
    },
}

impl PageLinks {
    pub(crate) fn from_targets(
        current_locale: Locale,
        base_path: BasePath,
        canonical_urls: Option<CanonicalUrls>,
        targets: Arc<BTreeMap<Locale, BTreeMap<PageId, OutputPath>>>,
        excluded_drafts: Arc<BTreeMap<Locale, BTreeMap<PageId, PathBuf>>>,
    ) -> Self {
        Self {
            current_locale,
            base_path,
            canonical_urls,
            targets,
            excluded_drafts,
        }
    }

    pub(crate) fn with_locale(&self, current_locale: Locale) -> Self {
        Self {
            current_locale,
            base_path: self.base_path.clone(),
            canonical_urls: self.canonical_urls.clone(),
            targets: self.targets.clone(),
            excluded_drafts: self.excluded_drafts.clone(),
        }
    }

    pub(crate) fn base_path(&self) -> &BasePath {
        &self.base_path
    }

    pub(crate) fn canonical_urls(&self) -> Option<&CanonicalUrls> {
        self.canonical_urls.as_ref()
    }

    pub(crate) fn resolve(
        &self,
        id: &str,
        locale: Option<&str>,
    ) -> Result<PageLinkTarget, PageResolveError> {
        let requested = PageId::parse(id).ok_or(PageResolveError::InvalidId)?;
        let locale = self.resolve_locale(locale)?;
        let path = self
            .targets
            .get(&locale)
            .and_then(|pages| pages.get(&requested))
            .ok_or_else(|| {
                self.excluded_drafts
                    .get(&locale)
                    .and_then(|pages| pages.get(&requested))
                    .map_or_else(
                        || PageResolveError::NotFound {
                            locale: locale.to_string(),
                            id: requested.to_string(),
                        },
                        |path| PageResolveError::DraftExcluded {
                            locale: locale.to_string(),
                            id: requested.to_string(),
                            path: path.clone(),
                        },
                    )
            })?;
        Ok(PageLinkTarget {
            id: requested,
            locale,
            output_path: path.clone(),
            url: self.base_path.join(&path.url_path()),
        })
    }

    /// Looks up a relationship that has already been resolved inside the site
    /// model, without treating its page ID as new authored input.
    pub(crate) fn target(&self, locale: &Locale, id: &PageId) -> Option<PageLinkTarget> {
        let output_path = self.targets.get(locale)?.get(id)?.clone();
        Some(PageLinkTarget {
            id: id.clone(),
            locale: locale.clone(),
            url: self.base_path.join(&output_path.url_path()),
            output_path,
        })
    }

    /// Resolves an optional template locale against the finite configured
    /// locale set. Resource helpers use this shared validation before looking
    /// up their own locale-local values.
    pub(crate) fn resolve_locale(&self, locale: Option<&str>) -> Result<Locale, PageResolveError> {
        self.resolve_locale_for(&self.current_locale, locale)
    }

    /// Resolves an optional template locale against the configured locale set
    /// without constructing a locale-specialized link view.
    pub(crate) fn resolve_locale_for(
        &self,
        current_locale: &Locale,
        locale: Option<&str>,
    ) -> Result<Locale, PageResolveError> {
        let locale = match locale {
            Some(locale) => locale
                .parse()
                .map_err(|_| PageResolveError::InvalidLocale {
                    locale: locale.into(),
                })?,
            None => current_locale.clone(),
        };
        if self.targets.contains_key(&locale) {
            Ok(locale)
        } else {
            Err(PageResolveError::InvalidLocale {
                locale: locale.to_string(),
            })
        }
    }
}
