//! Deterministic page indexes and relationship validation.

use std::collections::BTreeMap;

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use crate::{
    locale::Locale,
    site::page::{Page, PageId, PageLink, TranslationId},
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
    let mut groups: BTreeMap<TranslationId, BTreeMap<Locale, PageId>> = BTreeMap::new();
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
                    translationid: translationid.to_string(),
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

/// Adjacent members of an already ordered page relationship.
#[derive(Debug, Clone)]
pub(crate) struct Neighbors {
    pub(crate) before: Option<PageId>,
    pub(crate) after: Option<PageId>,
}

/// An immutable ordered page relationship with inexpensive neighbor lookup.
///
/// Both directory sibling ordering and collection ordering need the same
/// operation while rendering. Retaining each member's position avoids a
/// linear scan for every page rendered from a large collection.
#[derive(Debug, Clone, Default)]
pub(crate) struct OrderedPages {
    pages: Vec<PageId>,
    positions: BTreeMap<PageId, usize>,
}

impl OrderedPages {
    pub(crate) fn new(pages: Vec<PageId>) -> Self {
        let positions = pages
            .iter()
            .cloned()
            .enumerate()
            .map(|(position, id)| (id, position))
            .collect();
        Self { pages, positions }
    }

    pub(crate) fn neighbors(&self, page: &PageId) -> Option<Neighbors> {
        let position = *self.positions.get(page)?;
        Some(Neighbors {
            before: position
                .checked_sub(1)
                .map(|index| self.pages[index].clone()),
            after: self.pages.get(position + 1).cloned(),
        })
    }

    fn without(&self, page: &PageId) -> Vec<PageId> {
        self.pages
            .iter()
            .filter(|id| *id != page)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SiblingIndex {
    eligible: BTreeMap<String, OrderedPages>,
}

impl SiblingIndex {
    pub(crate) fn for_page(&self, directory: &str, page: &PageId) -> Vec<PageId> {
        self.eligible
            .get(directory)
            .map(|pages| pages.without(page))
            .unwrap_or_default()
    }

    pub(crate) fn neighbors(&self, directory: &str, page: &PageId) -> Option<Neighbors> {
        self.eligible.get(directory)?.neighbors(page)
    }
}

pub(super) fn resolve_siblings(pages: &BTreeMap<PageId, Page>) -> SiblingIndex {
    let mut eligible: BTreeMap<String, Vec<PageId>> = pages
        .iter()
        .filter(|(_, page)| page.path.is_some() && page.weight.is_some())
        .fold(BTreeMap::new(), |mut dirs, (id, page)| {
            dirs.entry(page.dir.clone()).or_default().push(id.clone());
            dirs
        });
    for ids in eligible.values_mut() {
        ids.sort_by_key(|id| (pages[id].weight, id.clone()));
    }
    SiblingIndex {
        eligible: eligible
            .into_iter()
            .map(|(directory, pages)| (directory, OrderedPages::new(pages)))
            .collect(),
    }
}
