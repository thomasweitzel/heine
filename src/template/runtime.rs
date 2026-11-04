//! Private, validated services shared by Tera built-ins.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    asset::AssetLinks,
    locale::Locale,
    localization::{Arguments, Localization},
    site::{
        FeedCatalog, FeedView, PageCatalog, PageId, PageLinks, PageTermView, PageView, Pager,
        PaginationCatalog, SearchCatalog, SearchView, SeriesCatalog, SeriesView, SiblingIndex,
        TaxonomyCatalog, TaxonomyView, collection::CollectionView,
    },
};

use super::TemplateResources;

/// One declared Markdown source and the render-time facts Heine keeps outside
/// its authored text.
pub(super) struct MarkdownContent<'a> {
    pub(super) source: &'a str,
    pub(super) replacements: &'a [crate::markdown::RendererReplacement],
    pub(super) toc: Option<&'a crate::markdown::TocPlan>,
    pub(super) footnotes: Option<&'a crate::site::FootnotesView>,
    pub(super) has_components: bool,
    pub(super) citations: Option<&'a crate::site::CitationView>,
}

/// The only bridge from Tera's dynamic render state back to validated Heine
/// values. Built-ins use this instead of independently decoding context data.
#[derive(Clone)]
pub(super) struct TemplateRuntime {
    assets: AssetLinks,
    renderer_attributions: Arc<Vec<crate::asset::RendererAttribution>>,
    page_links: PageLinks,
    page_catalog: PageCatalog,
    collections: Arc<BTreeMap<Locale, BTreeMap<String, CollectionView>>>,
    siblings: Arc<BTreeMap<Locale, SiblingIndex>>,
    collection_neighbors: Arc<crate::site::collection::CollectionNeighborIndex>,
    taxonomies: Arc<TaxonomyCatalog>,
    pagers: PaginationCatalog,
    feeds: Arc<FeedCatalog>,
    search: Arc<SearchCatalog>,
    series: Arc<SeriesCatalog>,
    publications: Option<crate::site::PublicationCatalog>,
    localization: Arc<Localization>,
    highlight_css: Option<String>,
    table_scroll_message: Option<String>,
    editorial_time_zone: Option<crate::config::EditorialTimeZone>,
    source_aware_renders: Arc<Mutex<Vec<SourceAwareRender>>>,
    prepared_markdown: Arc<Mutex<BTreeMap<String, String>>>,
    bibliography: Arc<Mutex<Option<BibliographyRender>>>,
    publication_error: Arc<Mutex<Option<crate::site::PublicationMappingError>>>,
}

#[derive(Debug)]
struct BibliographyRender {
    html: String,
    marker: String,
    calls: usize,
}

#[derive(Debug)]
pub(super) struct SourceAwareRender {
    pub(super) kind: SourceAwareRenderKind,
    pub(super) calls: usize,
    content: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum SourceAwareRenderKind {
    TableOfContents,
    MarginFootnotes,
    MarkdownComponents,
    Citations,
}

impl SourceAwareRenderKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::TableOfContents => "table-of-contents",
            Self::MarginFootnotes => "margin-footnote",
            Self::MarkdownComponents => "block-component",
            Self::Citations => "citation",
        }
    }
}

impl SourceAwareRender {
    pub(super) fn exact_render_message(&self) -> String {
        if self.calls == 0 {
            format!(
                "{} source {:?} was not rendered with markdown(content={:?})",
                self.kind.name(),
                self.content,
                self.content,
            )
        } else {
            format!(
                "{} source {:?} was rendered more than once with markdown(content={:?})",
                self.kind.name(),
                self.content,
                self.content,
            )
        }
    }
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
            search,
            series,
            publications,
        } = resources.relationships;
        Self {
            assets: resources.asset_links,
            renderer_attributions: resources.renderer_attributions,
            page_links,
            page_catalog,
            collections,
            siblings,
            collection_neighbors,
            taxonomies,
            pagers,
            feeds,
            search,
            series,
            publications,
            localization: resources.localization,
            highlight_css: resources.highlight_css,
            table_scroll_message: resources.table_scroll_message,
            editorial_time_zone: resources.editorial_time_zone,
            source_aware_renders: Arc::new(Mutex::new(Vec::new())),
            prepared_markdown: Arc::new(Mutex::new(BTreeMap::new())),
            bibliography: Arc::new(Mutex::new(None)),
            publication_error: Arc::new(Mutex::new(None)),
        }
    }
}

impl TemplateRuntime {
    pub(super) fn record_publication_error(
        &self,
        error: crate::site::PublicationMappingError,
    ) -> tera::Error {
        let message = error.to_string();
        if let Ok(mut slot) = self.publication_error.lock() {
            *slot = Some(error);
        }
        tera::Error::message(message)
    }

    pub(super) fn take_publication_error(&self) -> Option<crate::site::PublicationMappingError> {
        self.publication_error
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }
    pub(super) fn publications(&self) -> Option<&crate::site::PublicationCatalog> {
        self.publications.as_ref()
    }
    pub(super) fn begin_bibliography(
        &self,
        citations: Option<&crate::site::CitationView>,
    ) -> Result<(), String> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        use sha2::{Digest, Sha256};
        let mut current = self
            .bibliography
            .lock()
            .map_err(|_| "bibliography state is unavailable".to_owned())?;
        *current = citations.map(|value| {
            let digest = Sha256::digest(value.bibliography.as_bytes());
            let marker = format!(
                "<!--heine-bibliography-{}-->",
                URL_SAFE_NO_PAD.encode(digest)
            );
            BibliographyRender {
                html: value.bibliography.clone(),
                marker,
                calls: 0,
            }
        });
        Ok(())
    }

    pub(super) fn bibliography(&self) -> tera::TeraResult<String> {
        let mut current = self
            .bibliography
            .lock()
            .map_err(|_| tera::Error::message("bibliography state is unavailable"))?;
        let value = current.as_mut().ok_or_else(|| {
            tera::Error::message("bibliography() requires citations on this page")
        })?;
        value.calls += 1;
        Ok(format!("{}{}{}", value.marker, value.html, value.marker))
    }

    pub(super) fn finish_bibliography(&self, mut rendered: String) -> Result<String, String> {
        let mut current = self
            .bibliography
            .lock()
            .map_err(|_| "bibliography state is unavailable".to_owned())?;
        let Some(value) = current.take() else {
            return Ok(rendered);
        };
        if value.calls != 1 {
            return Err("bibliography() must be called exactly once".into());
        }
        let mut markers = rendered.match_indices(&value.marker);
        let first = markers.next().map(|(index, _)| index);
        let second = markers.next().map(|(index, _)| index);
        if markers.next().is_some()
            || first.zip(second).is_none_or(|(first, second)| {
                rendered.get(first + value.marker.len()..second) != Some(value.html.as_str())
            })
        {
            return Err("bibliography output must appear intact exactly once".into());
        }
        if let (Some(first), Some(second)) = (first, second) {
            rendered.replace_range(second..second + value.marker.len(), "");
            rendered.replace_range(first..first + value.marker.len(), "");
        }
        Ok(rendered)
    }

    pub(super) fn begin_prepared_markdown(&self) -> Result<(), String> {
        let mut prepared = self
            .prepared_markdown
            .lock()
            .map_err(|_| "Markdown preparation state became unavailable".to_owned())?;
        prepared.clear();
        Ok(())
    }

    pub(super) fn prepare_markdown(&self, name: String, rendered: String) -> Result<(), String> {
        self.prepared_markdown
            .lock()
            .map_err(|_| "Markdown preparation state became unavailable".to_owned())?
            .insert(name, rendered);
        Ok(())
    }

    pub(super) fn prepared_markdown(&self, name: &str) -> tera::TeraResult<Option<String>> {
        self.prepared_markdown
            .lock()
            .map_err(|_| tera::Error::message("Markdown preparation state became unavailable"))
            .map(|prepared| prepared.get(name).cloned())
    }

    pub(super) fn finish_prepared_markdown(&self) -> Result<(), String> {
        self.prepared_markdown
            .lock()
            .map_err(|_| "Markdown preparation state became unavailable".to_owned())?
            .clear();
        Ok(())
    }
    pub(super) fn editorial_time_zone(&self) -> Option<&crate::config::EditorialTimeZone> {
        self.editorial_time_zone.as_ref()
    }

    pub(super) fn table_scroll_label(
        &self,
        state: &tera::State,
    ) -> tera::TeraResult<Option<String>> {
        let Some(message) = &self.table_scroll_message else {
            return Ok(None);
        };
        let locale = self.locale(state)?;
        self.localization
            .format(&locale, message, None, &Arguments::new())
            .map(Some)
            .map_err(tera::Error::message)
    }

    pub(super) fn assets(&self) -> &AssetLinks {
        &self.assets
    }

    pub(super) fn attributions(&self, root: &str) -> Vec<crate::asset::AttributionView> {
        self.assets.attributions(root, &self.renderer_attributions)
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
        self.taxonomies.page_terms(
            locale,
            id,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
            root,
        )
    }

    pub(super) fn taxonomy(
        &self,
        locale: &Locale,
        name: &str,
        root: &str,
    ) -> Result<Option<TaxonomyView>, String> {
        let name = crate::config::TaxonomyName::parse(name.to_owned())?;
        Ok(self.taxonomies.view(
            locale,
            &name,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
            root,
        ))
    }

    pub(super) fn taxonomy_names(&self, locale: &Locale) -> Vec<String> {
        self.taxonomies.names(locale)
    }

    pub(super) fn taxonomies(&self, locale: &Locale, root: &str) -> Vec<TaxonomyView> {
        self.taxonomies.views(
            locale,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
            root,
        )
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

    pub(super) fn collections(&self, locale: &Locale) -> Vec<&CollectionView> {
        self.collections
            .get(locale)
            .map(|collections| collections.values().collect())
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
        self.feeds.view(
            locale,
            name,
            root,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
        )
    }

    pub(super) fn feed_names(&self, locale: &Locale) -> Vec<String> {
        self.feeds.names(locale)
    }

    pub(super) fn search(&self, locale: &Locale, name: &str, root: &str) -> Option<SearchView> {
        self.search.view(
            locale,
            name,
            root,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
        )
    }

    pub(super) fn search_names(&self, locale: &Locale) -> Vec<String> {
        self.search.names(locale)
    }

    pub(super) fn series(&self) -> &SeriesCatalog {
        &self.series
    }

    pub(super) fn series_view(
        &self,
        locale: &Locale,
        name: &crate::config::SeriesName,
        root: &str,
    ) -> Option<SeriesView> {
        self.series.view(
            locale,
            name,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
            root,
        )
    }

    pub(super) fn series_index(&self, locale: &Locale, root: &str) -> Option<SeriesView> {
        self.series.index(
            locale,
            self.page_links.base_path(),
            self.page_links.canonical_urls(),
            root,
        )
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

    /// Resolves a resource helper's optional locale against the render locale
    /// and the finite configured locale set.
    pub(super) fn requested_locale(
        &self,
        state: &tera::State,
        requested: Option<&str>,
    ) -> tera::TeraResult<Locale> {
        let current = self.locale(state)?;
        self.page_links
            .resolve_locale_for(&current, requested)
            .map_err(|error| tera::Error::message(error.to_string()))
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
    ) -> tera::TeraResult<MarkdownContent<'a>> {
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
        let footnotes = page
            .footnotes()
            .filter(|footnotes| footnotes.content() == name);
        let (content, replacements, components) = content;
        Ok(MarkdownContent {
            source: content,
            replacements,
            toc,
            footnotes,
            has_components: components.is_some(),
            citations: page
                .citations()
                .filter(|citations| citations.content == name),
        })
    }

    pub(super) fn is_selected_source(
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
        let selected_toc = page
            .toc()
            .and_then(|toc| page.content_markdown(toc.content()))
            .is_some_and(|(content, _, _)| content == input);
        let selected_footnotes = page
            .footnotes()
            .and_then(|footnotes| page.content_markdown(footnotes.content()))
            .is_some_and(|(content, _, _)| content == input);
        let selected_component = page.component_markdown_names().any(|name| {
            page.content_markdown(name)
                .is_some_and(|(content, _, _)| content == input)
        });
        let selected_citations = page
            .citations()
            .and_then(|citation| page.content_markdown(&citation.content))
            .is_some_and(|(content, _, _)| content == input);
        Ok(selected_toc || selected_footnotes || selected_component || selected_citations)
    }

    pub(super) fn margin_footnote_labels(
        &self,
        state: &tera::State,
        plan: &crate::markdown::MarginFootnotePlan,
    ) -> tera::TeraResult<Vec<String>> {
        let locale = self.locale(state)?;
        self.margin_footnote_labels_for_locale(&locale, plan)
            .map_err(tera::Error::message)
    }

    pub(super) fn margin_footnote_labels_for_locale(
        &self,
        locale: &Locale,
        plan: &crate::markdown::MarginFootnotePlan,
    ) -> Result<Vec<String>, String> {
        (1..=plan.number_count())
            .map(|number| {
                let mut arguments = Arguments::new();
                arguments.set("number", number as i64);
                self.localization
                    .format(locale, plan.marker_message(), None, &arguments)
                    .map_err(|error| error.to_string())
            })
            .collect()
    }

    pub(super) fn begin_source_aware_renders(
        &self,
        toc: Option<&crate::markdown::TocPlan>,
        footnotes: Option<&crate::site::FootnotesView>,
        components: impl IntoIterator<Item = String>,
        citations: Option<&crate::site::CitationView>,
    ) -> Result<(), String> {
        let mut current = self
            .source_aware_renders
            .lock()
            .map_err(|_| "Heine source-aware Markdown render tracking is unavailable".to_owned())?;
        current.clear();
        if let Some(toc) = toc {
            current.push(SourceAwareRender {
                kind: SourceAwareRenderKind::TableOfContents,
                content: toc.content().to_owned(),
                calls: 0,
            });
        }
        if let Some(footnotes) = footnotes {
            current.push(SourceAwareRender {
                kind: SourceAwareRenderKind::MarginFootnotes,
                content: footnotes.content().to_owned(),
                calls: 0,
            });
        }
        if let Some(citations) = citations {
            current.push(SourceAwareRender {
                kind: SourceAwareRenderKind::Citations,
                content: citations.content.clone(),
                calls: 0,
            });
        }
        for content in components {
            if current.iter().all(|selected| selected.content != content) {
                current.push(SourceAwareRender {
                    kind: SourceAwareRenderKind::MarkdownComponents,
                    content,
                    calls: 0,
                });
            }
        }
        drop(current);
        Ok(())
    }

    pub(super) fn record_source_aware_render(&self, content: &str) -> tera::TeraResult<()> {
        let mut current = self.source_aware_renders.lock().map_err(|_| {
            tera::Error::message("Heine source-aware Markdown render tracking is unavailable")
        })?;
        for selected in current
            .iter_mut()
            .filter(|selected| selected.content == content)
        {
            selected.calls += 1;
        }
        drop(current);
        Ok(())
    }

    pub(super) fn finish_source_aware_renders(&self) -> Result<Vec<SourceAwareRender>, String> {
        self.source_aware_renders
            .lock()
            .map_err(|_| "Heine source-aware Markdown render tracking is unavailable".to_owned())
            .map(|mut current| std::mem::take(&mut *current))
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
