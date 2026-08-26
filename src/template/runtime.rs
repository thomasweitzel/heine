//! Private, validated services shared by Tera built-ins.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
    asset::AssetLinks,
    locale::Locale,
    localization::Localization,
    site::{
        PageCatalog, PageId, PageLinks, PageView, SiblingIndex, TagIndex, TagView,
        collection::CollectionView,
    },
};

use super::TemplateResources;

/// The only bridge from Tera's dynamic render state back to validated Heine
/// values. Built-ins use this instead of independently decoding context data.
#[derive(Clone)]
pub(super) struct TemplateRuntime {
    assets: AssetLinks,
    page_links: PageLinks,
    page_catalog: PageCatalog,
    collections: Arc<BTreeMap<Locale, BTreeMap<String, CollectionView>>>,
    siblings: Arc<BTreeMap<Locale, SiblingIndex>>,
    tags: Arc<BTreeMap<Locale, TagIndex>>,
    localization: Arc<Localization>,
    highlight_css: Option<String>,
}

impl From<TemplateResources> for TemplateRuntime {
    fn from(resources: TemplateResources) -> Self {
        let super::TemplateRelationships {
            page_links,
            page_catalog,
            collections,
            siblings,
            tags,
        } = resources.relationships;
        Self {
            assets: resources.asset_links,
            page_links,
            page_catalog,
            collections,
            siblings,
            tags,
            localization: resources.localization,
            highlight_css: resources.highlight_css,
        }
    }
}

impl TemplateRuntime {
    pub(super) fn assets(&self) -> &AssetLinks {
        &self.assets
    }

    pub(super) fn pages(&self) -> &PageLinks {
        &self.page_links
    }

    pub(super) fn page_view(&self, locale: &Locale, id: &str) -> Option<PageView> {
        self.page_catalog
            .view(locale, &PageId::from_source_path(std::path::Path::new(id)))
            .cloned()
    }

    pub(super) fn collection(&self, locale: &Locale, name: &str) -> Option<CollectionView> {
        self.collections.get(locale)?.get(name).cloned()
    }

    pub(super) fn collection_names(&self, locale: &Locale) -> Vec<String> {
        self.collections
            .get(locale)
            .map(|collections| collections.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn siblings(&self, locale: &Locale, directory: &str, page: &str) -> Vec<PageId> {
        self.siblings
            .get(locale)
            .map(|siblings| siblings.for_page(directory, page))
            .unwrap_or_default()
    }

    pub(super) fn tag(&self, locale: &Locale, name: &str) -> Option<TagView> {
        self.tags.get(locale)?.tag(name)
    }

    pub(super) fn tag_names(&self, locale: &Locale) -> Vec<String> {
        self.tags
            .get(locale)
            .map(TagIndex::names)
            .unwrap_or_default()
    }

    pub(super) fn localization(&self) -> &Localization {
        &self.localization
    }

    pub(super) fn highlight_css(&self) -> Option<&str> {
        self.highlight_css.as_deref()
    }

    pub(super) fn locale(&self, state: &tera::State) -> tera::TeraResult<Locale> {
        let code = self.runtime_string(state, "locale")?;
        code.parse()
            .map_err(|_| tera::Error::message("Heine render context has an invalid locale code"))
    }

    pub(super) fn page_dir(&self, state: &tera::State) -> tera::TeraResult<String> {
        self.runtime_string(state, "page_dir")
    }

    pub(super) fn root(&self, state: &tera::State) -> tera::TeraResult<String> {
        self.runtime_string(state, "root")
    }

    pub(super) fn page_id(&self, state: &tera::State) -> tera::TeraResult<String> {
        self.runtime_string(state, "page_id")
    }

    fn runtime_string(&self, state: &tera::State, field: &str) -> tera::TeraResult<String> {
        state
            .get::<tera::Value>("__heine_runtime")?
            .and_then(|runtime| runtime.get_from_path(field)?.as_str().map(str::to_owned))
            .ok_or_else(|| {
                tera::Error::message(format!("Heine internal render context has no {field}"))
            })
    }
}
