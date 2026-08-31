//! Private, validated services shared by Tera built-ins.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use crate::{
    asset::AssetLinks,
    locale::Locale,
    localization::Localization,
    site::{
        FeedCatalog, FeedView, PageCatalog, PageId, PageLinks, PageTermView, PageView, Pager,
        PaginationCatalog, SiblingIndex, TaxonomyCatalog, TaxonomyIndexView,
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
    collection_neighbors: Arc<crate::site::collection::CollectionNeighborIndex>,
    taxonomies: Arc<TaxonomyCatalog>,
    pagers: PaginationCatalog,
    feeds: Arc<FeedCatalog>,
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
            collection_neighbors,
            taxonomies,
            pagers,
            feeds,
        } = resources.relationships;
        Self {
            assets: resources.asset_links,
            page_links,
            page_catalog,
            collections,
            siblings,
            collection_neighbors,
            taxonomies,
            pagers,
            feeds,
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

    pub(super) fn page_view(&self, locale: &Locale, id: &PageId) -> Option<&PageView> {
        self.page_catalog.view(locale, id)
    }

    pub(super) fn page_terms(
        &self,
        locale: &Locale,
        id: &PageId,
        root: &str,
    ) -> BTreeMap<String, Vec<PageTermView>> {
        self.taxonomies
            .page_terms(locale, id, self.page_links.base_path(), root)
    }

    pub(super) fn taxonomy_index(
        &self,
        locale: &Locale,
        name: &str,
        root: &str,
    ) -> Result<Option<TaxonomyIndexView>, String> {
        let name = crate::config::TaxonomyName::parse(name.to_owned())?;
        Ok(self
            .taxonomies
            .index(locale, &name, self.page_links.base_path(), root))
    }

    pub(super) fn taxonomy_names(&self, locale: &Locale) -> Vec<String> {
        self.taxonomies.names(locale)
    }

    pub(super) fn collection(&self, locale: &Locale, name: &str) -> Option<&CollectionView> {
        self.collections.get(locale)?.get(name)
    }

    pub(super) fn collection_names(&self, locale: &Locale) -> Vec<String> {
        self.collections
            .get(locale)
            .map(|collections| collections.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn siblings(&self, locale: &Locale, directory: &str, page: &PageId) -> Vec<PageId> {
        self.siblings
            .get(locale)
            .map(|siblings| siblings.for_page(directory, page))
            .unwrap_or_default()
    }

    pub(super) fn sibling_neighbors(
        &self,
        locale: &Locale,
        directory: &str,
        page: &PageId,
    ) -> Option<crate::site::Neighbors> {
        self.siblings.get(locale)?.neighbors(directory, page)
    }

    pub(super) fn collection_neighbors(
        &self,
        locale: &Locale,
        name: &str,
        page: &PageId,
    ) -> Option<crate::site::Neighbors> {
        self.collection_neighbors.neighbors(locale, name, page)
    }

    pub(super) fn pager(&self, state: &tera::State) -> tera::TeraResult<Option<&Pager>> {
        let locale = self.locale(state)?;
        if state
            .get::<tera::Value>("__heine_runtime")?
            .and_then(|runtime| runtime.get_from_path("pagination_page")?.as_u64())
            .is_none()
        {
            return Ok(None);
        }
        let output_path = crate::output::OutputPath::try_new(PathBuf::from(
            self.runtime_string(state, "output_path")?,
        ))
        .map_err(|_| tera::Error::message("Heine render context has an invalid output path"))?;
        Ok(self.pagers.pager(&locale, &output_path))
    }

    pub(super) fn feed(&self, locale: &Locale, name: &str, root: &str) -> Option<FeedView> {
        self.feeds
            .view(locale, name, root, self.page_links.base_path())
    }

    pub(super) fn feed_names(&self, locale: &Locale) -> Vec<String> {
        self.feeds.names(locale)
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

    pub(super) fn page_id(&self, state: &tera::State) -> tera::TeraResult<PageId> {
        self.authored_page_id(state)?
            .ok_or_else(|| tera::Error::message("this operation requires an authored page render"))
    }

    fn authored_page_id(&self, state: &tera::State) -> tera::TeraResult<Option<PageId>> {
        let value = state
            .get::<tera::Value>("__heine_runtime")?
            .and_then(|runtime| {
                runtime
                    .get_from_path("page_id")?
                    .as_str()
                    .map(str::to_owned)
            });
        let Some(value) = value else {
            return Ok(None);
        };
        PageId::parse(&value).map(Some).ok_or_else(|| {
            tera::Error::message("Heine internal render context has an invalid page ID")
        })
    }

    pub(super) fn markdown_content<'a>(
        &'a self,
        state: &tera::State,
        name: &str,
    ) -> tera::TeraResult<(&'a str, Option<&'a crate::markdown::TocPlan>)> {
        let locale = self.locale(state)?;
        let id = self.page_id(state)?;
        let page = self.page_view(&locale, &id).ok_or_else(|| {
            tera::Error::message("Heine render context has no current authored page")
        })?;
        let content = page.content_markdown(name).ok_or_else(|| {
            tera::Error::message(format!(
                "Markdown content {name:?} is not declared on the current page; use a declared unsegmented .md content name"
            ))
        })?;
        let toc = page.toc().filter(|toc| toc.content() == name);
        Ok((content, toc))
    }

    pub(super) fn is_selected_toc_source(
        &self,
        state: &tera::State,
        input: &str,
    ) -> tera::TeraResult<bool> {
        let locale = self.locale(state)?;
        let Some(id) = self.authored_page_id(state)? else {
            return Ok(false);
        };
        let page = self.page_view(&locale, &id).ok_or_else(|| {
            tera::Error::message("Heine render context has no current authored page")
        })?;
        Ok(page
            .toc()
            .and_then(|toc| page.content_markdown(toc.content()))
            .is_some_and(|content| content == input))
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
