//! Deterministic page indexes and relationship validation.

use std::collections::BTreeMap;

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use thiserror::Error;

use crate::{
    locale::Locale,
    site::page::{Page, PageId, PageLink},
};

#[derive(Debug, Diagnostic, Error)]
#[error("translation ID {translationid:?} is declared more than once for locale {locale}")]
#[diagnostic(code(heine::i18n::duplicate_translationid))]
pub(crate) struct DuplicateTranslation {
    translationid: String,
    locale: Locale,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("duplicate declaration here")]
    span: SourceSpan,
    #[related]
    first: Vec<FirstTranslation>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("first declaration of this translation ID")]
struct FirstTranslation {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("first declaration here")]
    span: SourceSpan,
}

/// Relates explicitly authored pages across locales without guessing from
/// paths. Missing locales are deliberately normal; duplicate locale members
/// would make a language switcher ambiguous and therefore fail the build.
pub(super) fn resolve_translations(
    pages: &mut BTreeMap<Locale, BTreeMap<PageId, Page>>,
) -> Result<(), Box<DuplicateTranslation>> {
    let mut groups: BTreeMap<String, BTreeMap<Locale, PageId>> = BTreeMap::new();
    for (locale, locale_pages) in pages.iter() {
        for (id, page) in locale_pages {
            let Some(translationid) = &page.translationid else {
                continue;
            };
            if let Some(previous) = groups
                .entry(translationid.clone())
                .or_default()
                .insert(locale.clone(), id.clone())
            {
                let first = &locale_pages[&previous];
                return Err(Box::new(DuplicateTranslation {
                    translationid: translationid.clone(),
                    locale: locale.clone(),
                    source_code: page.source.clone(),
                    span: page.translationid_span.unwrap_or((0, 0).into()),
                    first: vec![FirstTranslation {
                        source_code: first.source.clone(),
                        span: first.translationid_span.unwrap_or((0, 0).into()),
                    }],
                }));
            }
        }
    }
    for members in groups.values() {
        let links = members
            .iter()
            .filter_map(|(locale, id)| {
                let page = pages.get(locale)?.get(id)?;
                page.path.as_ref().map(|path| {
                    (
                        locale.to_string(),
                        PageLink {
                            id: id.clone(),
                            path: path.as_str().to_owned(),
                        },
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        for (locale, id) in members {
            if let Some(page) = pages.get_mut(locale).and_then(|pages| pages.get_mut(id)) {
                page.translations = links
                    .iter()
                    .filter(|(other, _)| *other != &locale.to_string())
                    .map(|(locale, link)| (locale.clone(), link.clone()))
                    .collect();
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SiblingIndex {
    eligible: BTreeMap<String, Vec<PageId>>,
}

impl SiblingIndex {
    pub(crate) fn for_page(&self, directory: &str, page: &str) -> Vec<PageId> {
        let page = PageId::from_source_path(std::path::Path::new(page));
        self.eligible
            .get(directory)
            .into_iter()
            .flatten()
            .filter(|id| *id != &page)
            .cloned()
            .collect()
    }
}

pub(super) fn fix_siblings(pages: &mut BTreeMap<PageId, Page>) -> SiblingIndex {
    let mut eligible: BTreeMap<String, Vec<PageId>> = pages
        .iter()
        .filter(|(_, page)| page.path.is_some() && page.weight.is_some())
        .fold(BTreeMap::new(), |mut dirs, (id, page)| {
            dirs.entry(page.dir.clone()).or_default().push(id.clone());
            dirs
        });
    for ids in eligible.values_mut() {
        ids.sort_by_key(|id| (pages[id].weight, id.clone()));
        for (index, id) in ids.iter().enumerate() {
            if let Some(page) = pages.get_mut(id) {
                page.lighter = index.checked_sub(1).map(|index| ids[index].clone());
                page.heavier = ids.get(index + 1).cloned();
            }
        }
    }
    SiblingIndex { eligible }
}

/// A deterministic, locale-local tag index of rendered pages. Like
/// collections, it retains IDs rather than duplicating page views for every
/// render.
#[derive(Debug, Default)]
pub(crate) struct TagIndex {
    pages: BTreeMap<String, Vec<PageId>>,
}

impl TagIndex {
    pub(crate) fn tag(&self, name: &str) -> Option<TagView> {
        self.pages.get(name).map(|page_ids| TagView {
            name: name.into(),
            count: page_ids.len(),
            page_ids: page_ids.clone(),
        })
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.pages.keys().cloned().collect()
    }
}

/// The focused render-facing view returned by `tag()`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TagView {
    name: String,
    count: usize,
    page_ids: Vec<PageId>,
}

pub(super) fn tags_for(pages: &BTreeMap<PageId, Page>) -> TagIndex {
    let mut tags: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (id, page) in pages {
        if page.path.is_none() {
            continue;
        }
        for tag in &page.tags {
            tags.entry(tag.clone()).or_default().push(id.clone());
        }
    }
    for ids in tags.values_mut() {
        ids.sort();
    }
    TagIndex { pages: tags }
}
