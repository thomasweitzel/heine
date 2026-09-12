//! Resolved site construction and rendering orchestration.
//!
//! Page parsing/loading, validated page state, and deterministic indexes live
//! in dedicated child modules. `Site` coordinates those stages without owning
//! template-engine or output filesystem implementation details.

mod collation;
pub(crate) mod collection;
mod feed;
mod index;
mod links;
mod loader;
mod locales;
mod metadata;
mod page;
mod pagination;
mod search;
mod series;
mod sitemap;
mod taxonomy;

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use crate::{
    asset::{AssetCatalog, AssetSources, SharedAssets},
    config::{
        BasePath, CollectionsConfig, FeedsConfig, I18nConfig, LicensingConfig, SearchConfig,
        SeriesConfig, SiteConfig, SiteSettings, SitemapConfig, TaxonomiesConfig,
    },
    content,
    data::SiteData,
    deploy::Catalog as DeploymentCatalog,
    licensing::Catalog as LicenseCatalog,
    locale::Locale,
    localization::Localization,
    markdown::{self, Renderer as MarkdownRenderer},
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
    template::{
        LocaleView, RenderContext, RuntimeContext, TemplateEngine, TemplateIndexes,
        TemplateRelationships, TemplateResources,
    },
};

pub(crate) use feed::Catalog as FeedCatalog;
pub(crate) use feed::View as FeedView;
pub(crate) use index::{Neighbors, SiblingIndex};
pub(crate) use links::{PageCatalog, PageLinkTarget, PageLinks, PageResolveError};
use page::Task;
pub(crate) use page::{FootnotesView, PageId, PageView, Pager};
pub(crate) use pagination::{Catalog as PaginationCatalog, View as PaginationView};
pub(crate) use search::Catalog as SearchCatalog;
pub(crate) use search::View as SearchView;
pub(crate) use series::{
    Catalog as SeriesCatalog, MembershipView as SeriesMembershipView, View as SeriesView,
};
pub(crate) use taxonomy::View as TaxonomyView;
pub(crate) use taxonomy::{
    Catalog as TaxonomyCatalog, IndexView as TaxonomyIndexView, PageTermView,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Full,
    Quick,
}

/// Selects the authored pages that participate in one resolved site.
///
/// This is distinct from build mode: a quick build may still describe the
/// published site, while the development server can explicitly preview drafts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublicationMode {
    Published,
    PreviewDrafts,
}

impl PublicationMode {
    pub(crate) fn includes_drafts(self) -> bool {
        matches!(self, Self::PreviewDrafts)
    }
}

/// Validated options that travel with one build invocation.
#[derive(Debug, Clone)]
pub(crate) struct BuildOptions {
    mode: BuildMode,
    config: SiteConfig,
}

/// Inputs consumed while resolving one site. Grouping them keeps the site
/// boundary stable as independently-owned source domains grow.
pub(crate) struct LoadInputs<'a> {
    pub(crate) content_root: &'a Path,
    pub(crate) locales_root: &'a Path,
    pub(crate) diagnostics: &'a crate::source_diagnostic::DiagnosticPaths,
    pub(crate) data: SiteData,
    pub(crate) locale_data: BTreeMap<Locale, SiteData>,
    pub(crate) i18n: I18nConfig,
    pub(crate) collections: CollectionsConfig,
    pub(crate) taxonomies: TaxonomiesConfig,
    pub(crate) series: Option<&'a SeriesConfig>,
    pub(crate) feeds: FeedsConfig,
    pub(crate) search: SearchConfig,
    pub(crate) site: &'a SiteSettings,
    pub(crate) sitemap: Option<&'a SitemapConfig>,
    pub(crate) deployment: &'a DeploymentCatalog,
    pub(crate) licenses: &'a LicenseCatalog,
    pub(crate) licensing: &'a LicensingConfig,
    pub(crate) markdown_renderer: &'a MarkdownRenderer,
    pub(crate) publication: PublicationMode,
}

impl BuildOptions {
    pub(crate) fn new(mode: BuildMode, config: SiteConfig) -> Self {
        Self { mode, config }
    }

    pub(crate) fn mode(&self) -> BuildMode {
        self.mode
    }

    pub(crate) fn config(&self) -> &SiteConfig {
        &self.config
    }
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("content directory {0} does not exist or is not a directory")]
    MissingContent(PathBuf),
    #[error("configured locale {locale} has no content directory at {path}")]
    MissingLocaleContent { locale: Locale, path: PathBuf },
    #[error("content path {path} is not a directory")]
    ContentNotDirectory { path: PathBuf },
    #[error("content root entry {path} is neither `assets` nor a configured locale directory")]
    UnexpectedContentRoot { path: PathBuf },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Licensing(#[from] Box<crate::licensing::Error>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateTranslation(#[from] Box<index::DuplicateTranslation>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Collection(#[from] collection::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Taxonomy(#[from] Box<taxonomy::Error>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Series(#[from] series::Error),
    #[error("could not inspect {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not walk content directory {0}")]
    Walk(#[from] walkdir::Error),
    #[error("content path {path} must use UTF-8 components and must not contain backslashes")]
    NonUtf8Path { path: PathBuf },
    #[error("content path {path} is a symbolic link")]
    ContentSymlink { path: PathBuf },
    #[error("content path {path} is outside content root {root}")]
    OutsideContent { path: PathBuf, root: PathBuf },
    #[error(transparent)]
    #[diagnostic(transparent)]
    NormalizationCollision(#[from] Box<crate::source_path::NormalizationCollision>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    PageToml(#[from] Box<loader::PageTomlDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidPage(#[from] Box<loader::PageDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    SitemapDirectoryOverride(#[from] Box<loader::SitemapDirectoryOverride>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidPageOutput(#[from] Box<loader::PageOutputPathDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingMetadata(#[from] Box<loader::MissingMetadata>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateCollectionMembership(#[from] Box<loader::DuplicateCollectionMembership>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateSeriesMembership(#[from] Box<loader::DuplicateSeriesMembership>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateTaxonomyTerm(#[from] Box<loader::DuplicateTaxonomyTerm>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Content(#[from] content::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Markdown(#[from] Box<markdown::SourceError>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MarginFootnotes(#[from] Box<markdown::MarginFootnoteDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MarkdownMessageConfiguration(#[from] Box<loader::MarkdownMessageConfigurationDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Asset(#[from] crate::asset::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Template(#[from] crate::template::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Localization(#[from] crate::localization::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingTemplate(#[from] Box<MissingTemplate>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] crate::output::Error),
    #[error(transparent)]
    Sitemap(#[from] sitemap::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Feed(Box<feed::Error>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Search(Box<search::Error>),
}

/// A semantic error tied to the precise source range that referenced a
/// template. It is boxed in `Error` to keep the common error path compact.
#[derive(Debug, Diagnostic, Error)]
#[error("template {template:?} was not found")]
#[diagnostic(code(heine::template::not_found))]
pub(crate) struct MissingTemplate {
    template: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("template referenced here")]
    span: SourceSpan,
    #[help]
    help: String,
}

/// Fully validated site data. This is deliberately independent of Tera.
#[derive(Debug)]
pub(crate) struct Site {
    pages: PageCatalog,
    siblings: std::sync::Arc<BTreeMap<Locale, index::SiblingIndex>>,
    collection_neighbors: std::sync::Arc<collection::CollectionNeighborIndex>,
    tasks: Vec<Task>,
    taxonomy_tasks: Vec<taxonomy::Task>,
    taxonomies: std::sync::Arc<taxonomy::Catalog>,
    series: std::sync::Arc<series::Catalog>,
    collections: std::sync::Arc<BTreeMap<Locale, BTreeMap<String, collection::CollectionView>>>,
    pagers: pagination::Catalog,
    assets: std::sync::Arc<AssetCatalog>,
    data: SiteData,
    locale_data: BTreeMap<Locale, SiteData>,
    i18n: I18nConfig,
    localization: std::sync::Arc<Localization>,
    output_plan: OutputPlan,
    sitemap: Option<sitemap::Sitemap>,
    feeds: std::sync::Arc<feed::Catalog>,
    search: std::sync::Arc<search::Catalog>,
}

impl Site {
    pub(crate) fn load(inputs: LoadInputs<'_>) -> Result<Self, Error> {
        let LoadInputs {
            content_root,
            locales_root,
            diagnostics,
            data,
            locale_data,
            i18n,
            collections: collection_config,
            taxonomies: taxonomy_config,
            series: series_config,
            feeds: feed_config,
            search: search_config,
            site: site_settings,
            sitemap: sitemap_config,
            deployment,
            licenses,
            licensing,
            markdown_renderer,
            publication,
        } = inputs;
        match fs::symlink_metadata(content_root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::ContentSymlink {
                    path: content_root.into(),
                });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Error::MissingContent(content_root.into()));
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(Error::MissingContent(content_root.into()));
            }
            Err(source) => {
                return Err(Error::Io {
                    path: content_root.into(),
                    source,
                });
            }
        }
        let localization = std::sync::Arc::new(Localization::load(
            locales_root,
            i18n.locales(),
            i18n.default_locale().clone(),
            diagnostics,
        )?);
        let locales = locales::load(locales::Inputs {
            content_root,
            diagnostics,
            i18n: &i18n,
            collections: &collection_config,
            series: series_config,
            taxonomies: &taxonomy_config,
            time_zone: site_settings.time_zone(),
            sitemap_enabled: sitemap_config.is_some(),
            markdown: markdown_renderer.config(),
            localization: &localization,
            publication,
        })?;
        let mut pages = locales.pages;
        let excluded_drafts = std::sync::Arc::new(locales.excluded_drafts);
        let siblings = locales.siblings;
        let mut tasks = locales.tasks;
        if let Some(latest_data_input) = data.latest_input() {
            for task in &mut tasks {
                task.latest_input = task.latest_input.max(latest_data_input);
            }
        }
        for task in &mut tasks {
            if let Some(latest_input) = locale_data[&task.locale].latest_input() {
                task.latest_input = task.latest_input.max(latest_input);
            }
            if let Some(latest_input) = localization.latest_input(&task.locale) {
                task.latest_input = task.latest_input.max(latest_input);
            }
        }
        let collections = collection::resolve(&pages, &collection_config)?;
        let collection_neighbors = std::sync::Arc::new(collection::resolve_neighbors(&collections));
        pagination::expand(&pages, &collections, &mut tasks);
        index::resolve_translations(&mut pages).map_err(Error::DuplicateTranslation)?;
        let sources = i18n
            .locales()
            .map(|locale| (locale.clone(), content_root.join(locale.to_string())))
            .collect::<Vec<_>>();
        let prefixes = i18n
            .locales()
            .map(|locale| {
                (
                    locale.clone(),
                    if locale == i18n.default_locale() {
                        PathBuf::new()
                    } else {
                        PathBuf::from(locale.to_string())
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let taxonomies =
            taxonomy::resolve(&pages, &collections, &taxonomy_config, prefixes.clone())
                .map_err(|error| Error::Taxonomy(Box::new(error)))?;
        let mut series = series::resolve(&pages, series_config, &prefixes, &localization)
            .map_err(Error::Series)?;
        let asset_sources = sources
            .iter()
            .map(|(locale, root)| {
                (
                    locale.clone(),
                    AssetSources {
                        root,
                        entries: &locales.entries[locale],
                        content_inputs: &locales.content_inputs[locale],
                        output_prefix: &prefixes[locale],
                        shared: (!locales.shared_entries.is_empty()).then_some(SharedAssets {
                            root: &locales.shared_root,
                            entries: &locales.shared_entries,
                        }),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut assets = AssetCatalog::discover_locales(&asset_sources)?;
        licenses
            .attach_to_assets(&mut assets, content_root, licensing, diagnostics)
            .map_err(Box::new)?;
        let assets = std::sync::Arc::new(assets);
        if let Some(latest_asset_input) = assets.latest_input() {
            for task in &mut tasks {
                task.latest_input = task.latest_input.max(latest_asset_input);
            }
        }
        let latest_inputs = tasks
            .iter()
            .map(|task| {
                (
                    (task.locale.clone(), task.page_id.clone()),
                    task.latest_input,
                )
            })
            .collect();
        let shared_latest = i18n
            .locales()
            .map(|locale| {
                let mut latest = data.latest_input().unwrap_or(SystemTime::UNIX_EPOCH);
                if let Some(input) = locale_data[locale].latest_input() {
                    latest = latest.max(input);
                }
                if let Some(input) = localization.latest_input(locale) {
                    latest = latest.max(input);
                }
                if let Some(input) = assets.latest_input() {
                    latest = latest.max(input);
                }
                (locale.clone(), latest)
            })
            .collect();
        let taxonomy_tasks = taxonomies.tasks(&latest_inputs, &shared_latest);
        let taxonomies = std::sync::Arc::new(taxonomies);
        series.update_latest_inputs(&latest_inputs, &shared_latest);
        let series = std::sync::Arc::new(series);
        let feeds = feed::Catalog::resolve(feed::Inputs {
            pages: &pages,
            collections: &collections,
            config: &feed_config,
            prefixes: &prefixes,
            origin: site_settings.origin(),
            base_path: site_settings.base_path(),
            localization: &localization,
        })
        .map_err(|error| Error::Feed(Box::new(error)))?;
        let search = search::Catalog::resolve(search::Inputs {
            pages: &pages,
            collections: &collections,
            config: &search_config,
            prefixes: &prefixes,
            base_path: site_settings.base_path(),
            markdown: markdown_renderer.config(),
            taxonomies: &taxonomy_config,
        })
        .map_err(|error| Error::Search(Box::new(error)))?;
        let pagers = pagination::Catalog::from_tasks(&tasks, &taxonomy_tasks, series.tasks());
        let sitemap = match (sitemap_config, site_settings.origin()) {
            (Some(config), Some(origin)) => Some(
                sitemap::Sitemap::new(
                    config,
                    origin,
                    site_settings.base_path(),
                    &tasks,
                    &taxonomy_tasks,
                    series.tasks(),
                )
                .map_err(Error::Sitemap)?,
            ),
            _ => None,
        };
        let mut output_claims = tasks
            .iter()
            .map(|task| {
                OutputClaim::authored(
                    task.output_path.clone(),
                    task.pagination.as_ref().map_or_else(
                        || format!("page {}", task.page_id),
                        |pager| {
                            format!(
                                "pagination page {} of listing {}",
                                pager.page_number, task.page_id
                            )
                        },
                    ),
                    task.page_source.as_ref().clone(),
                    task.output_span,
                )
            })
            .collect::<Vec<_>>();
        output_claims.extend(taxonomy_tasks.iter().map(|task| {
            OutputClaim::authored(
                task.output_path.clone(),
                format!("taxonomy {}", task.taxonomy.name),
                task.output_source.as_ref().clone(),
                task.output_span,
            )
        }));
        output_claims.extend(self_series_claims(&series));
        output_claims.extend(
            assets
                .paths()
                .iter()
                .map(|asset| OutputClaim::file(asset.clone(), assets.source_path(asset).into())),
        );
        output_claims.extend(
            deployment
                .paths()
                .iter()
                .map(|path| OutputClaim::file(path.clone(), deployment.source_path(path).into())),
        );
        output_claims.extend(
            licenses
                .paths()
                .iter()
                .map(|path| OutputClaim::file(path.clone(), licenses.source_path(path).into())),
        );
        output_claims.extend(markdown_renderer.stylesheet_claim());
        if let Some((_, claims)) = &sitemap {
            output_claims.extend(claims.iter().cloned());
        }
        output_claims.extend(feeds.output_claims());
        output_claims.extend(search.output_claims());
        let output_plan = OutputPlan::validate(&output_claims)?;
        let asset_links = assets.links(
            i18n.default_locale().clone(),
            BasePath::root(),
            BTreeMap::new(),
        );
        let page_links = PageCatalog::links_from_pages(
            &pages,
            i18n.default_locale().clone(),
            BasePath::root(),
            excluded_drafts.clone(),
        );
        for (locale, locale_pages) in &pages {
            let asset_links = asset_links.with_locale(locale.clone());
            let page_links = page_links.with_locale(locale.clone());
            for page in locale_pages.values() {
                for source in page.markdown_sources.values() {
                    markdown::validate_source_with_links(
                        &source.display_path,
                        &source.source,
                        markdown_renderer,
                        Some(&asset_links),
                        Some(&page_links),
                    )?;
                }
            }
        }
        let pages = PageCatalog::from_pages(pages, excluded_drafts);
        let collections = std::sync::Arc::new(
            collections
                .into_iter()
                .map(|(locale, collections)| {
                    (
                        locale,
                        collections
                            .into_iter()
                            .map(|(name, collection)| (name.to_string(), collection.view()))
                            .collect(),
                    )
                })
                .collect(),
        );
        Ok(Self {
            pages,
            siblings: std::sync::Arc::new(siblings),
            collection_neighbors,
            tasks,
            taxonomy_tasks,
            taxonomies,
            series,
            collections,
            pagers,
            assets,
            data,
            locale_data,
            i18n,
            localization,
            output_plan,
            sitemap: sitemap.map(|(sitemap, _)| sitemap),
            feeds: std::sync::Arc::new(feeds),
            search: std::sync::Arc::new(search),
        })
    }

    pub(crate) fn render(
        &self,
        templates: &Path,
        existing_output: &Path,
        options: &BuildOptions,
        markdown_renderer: &MarkdownRenderer,
        diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    ) -> Result<Vec<GeneratedFile>, Error> {
        let base_path = options.config().site().base_path().to_owned();
        let integrities = self
            .assets
            .integrity_values(options.config().assets().integrity())?;
        let resources = TemplateResources::new(
            self.assets.links(
                self.i18n.default_locale().clone(),
                base_path.clone(),
                integrities,
            ),
            TemplateRelationships::new(
                self.pages.clone(),
                self.i18n.default_locale().clone(),
                base_path.clone(),
                TemplateIndexes {
                    collections: self.collections.clone(),
                    siblings: self.siblings.clone(),
                    collection_neighbors: self.collection_neighbors.clone(),
                    taxonomies: self.taxonomies.clone(),
                    pagers: self.pagers.clone(),
                    feeds: self.feeds.clone(),
                    search: self.search.clone(),
                    series: self.series.clone(),
                },
            ),
            self.localization.clone(),
            markdown_renderer
                .stylesheet_path()
                .map(|path| base_path.join(&path.url_path())),
            options
                .config()
                .markdown()
                .tables()
                .map(|tables| tables.scroll_message().to_owned()),
        );
        let engine =
            TemplateEngine::load(templates, resources, markdown_renderer.clone(), diagnostics)?;
        self.validate_templates(&engine)?;
        let mut rendered = Vec::new();
        for task in &self.tasks {
            if !needs_render(task, existing_output, options.mode())? {
                continue;
            }
            let page = self
                .pages
                .view(&task.locale, &task.page_id)
                .expect("tasks refer to loaded pages");
            let page_dir = page.directory();
            let root = root_for(task.output_path.as_path());
            let render_page = crate::template::RenderedPage {
                page,
                taxonomies: self.taxonomies.page_terms(
                    &task.locale,
                    &task.page_id,
                    &base_path,
                    &root,
                ),
                series: self.series.page_memberships(
                    &task.locale,
                    &task.page_id,
                    &self.pages,
                    &base_path,
                    &root,
                ),
            };
            let pagination = task
                .pagination
                .as_ref()
                .map(|pager| pager.view(&self.pages, &task.locale, &base_path, &root));
            let rendering = task.pagination.as_ref().map_or_else(
                || {
                    format!(
                        "page {} ({}) to {}",
                        task.page_id,
                        task.locale,
                        task.output_path.as_path().display()
                    )
                },
                |pager| {
                    format!(
                        "pagination page {} of listing {} ({}) to {}",
                        pager.page_number,
                        task.page_id,
                        task.locale,
                        task.output_path.as_path().display()
                    )
                },
            );
            let contents = engine.render(
                &task.template,
                RenderContext {
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page: Some(render_page),
                    taxonomy: None,
                    series: None,
                    pagination,
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir,
                        page_id: Some(&task.page_id),
                        output_path: task
                            .output_path
                            .as_path()
                            .to_str()
                            .expect("validated output paths are UTF-8"),
                        root: &root,
                        pagination_page: task.pagination.as_ref().map(|pager| pager.page_number),
                    },
                },
                &rendering,
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("page task output was validated in the output plan");
            rendered.push(GeneratedFile::text(path, contents));
        }
        for task in &self.taxonomy_tasks {
            if !needs_render_generated(
                task.latest_input,
                &task.output_path,
                existing_output,
                options.mode(),
            )? {
                continue;
            }
            let root = root_for(task.output_path.as_path());
            let pagination = task
                .pagination
                .as_ref()
                .map(|pager| pager.view(&self.pages, &task.locale, &base_path, &root));
            let contents = engine.render(
                &task.template,
                RenderContext {
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page: None,
                    taxonomy: Some(task.taxonomy_view(&base_path, &root)),
                    series: None,
                    pagination,
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir: task
                            .output_path
                            .as_path()
                            .parent()
                            .and_then(|path| path.to_str())
                            .unwrap_or(""),
                        page_id: None,
                        output_path: task
                            .output_path
                            .as_path()
                            .to_str()
                            .expect("validated output paths are UTF-8"),
                        root: &root,
                        pagination_page: task.pagination.as_ref().map(|pager| pager.page_number),
                    },
                },
                &format!(
                    "taxonomy {} ({}) to {}",
                    task.taxonomy.name,
                    task.locale,
                    task.output_path.as_path().display()
                ),
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("taxonomy task output was validated in the output plan");
            rendered.push(GeneratedFile::text(path, contents));
        }
        for task in self.series.tasks() {
            if !needs_render_generated(
                task.latest_input,
                &task.output_path,
                existing_output,
                options.mode(),
            )? {
                continue;
            }
            let root = root_for(task.output_path.as_path());
            let series = match &task.series_id {
                Some(id) => self.series.view(&task.locale, id, &base_path, &root),
                None => self.series.index(&task.locale, &base_path, &root),
            }
            .expect("resolved Series task keeps its corresponding view");
            let contents = engine.render(
                &task.template,
                RenderContext {
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page: None,
                    taxonomy: None,
                    series: Some(series),
                    pagination: task
                        .pagination
                        .as_ref()
                        .map(|pager| pager.view(&self.pages, &task.locale, &base_path, &root)),
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir: task
                            .output_path
                            .as_path()
                            .parent()
                            .and_then(|path| path.to_str())
                            .unwrap_or(""),
                        page_id: None,
                        output_path: task
                            .output_path
                            .as_path()
                            .to_str()
                            .expect("validated output paths are UTF-8"),
                        root: &root,
                        pagination_page: task.pagination.as_ref().map(|pager| pager.page_number),
                    },
                },
                &format!(
                    "Series ({}) to {}",
                    task.locale,
                    task.output_path.as_path().display()
                ),
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("Series task output was validated in the output plan");
            rendered.push(GeneratedFile::text(path, contents));
        }
        if let (Some(stylesheet), Some(path)) = (
            markdown_renderer.stylesheet(),
            markdown_renderer.stylesheet_path(),
        ) {
            let path = self
                .output_plan
                .planned_path(path)
                .expect("highlighting stylesheet was validated in the output plan");
            rendered.push(GeneratedFile::text(path, stylesheet));
        }
        if options.mode() == BuildMode::Full
            && let Some(sitemap) = &self.sitemap
        {
            rendered.extend(sitemap.generated(&self.output_plan));
        }
        if options.mode() == BuildMode::Full {
            rendered.extend(self.feeds.generated(&self.output_plan));
            rendered.extend(self.search.generated(&self.output_plan));
        }
        Ok(rendered)
    }

    pub(crate) fn assets(&self) -> &AssetCatalog {
        self.assets.as_ref()
    }

    fn validate_templates(&self, engine: &TemplateEngine) -> Result<(), Error> {
        for task in &self.tasks {
            validate_template_reference(
                engine,
                &task.template,
                task.template_source.as_ref().clone(),
                task.template_span,
            )?;
        }
        for reference in self.taxonomies.template_references() {
            validate_template_reference(
                engine,
                &reference.template,
                reference.source.as_ref().clone(),
                reference.span,
            )?;
        }
        for reference in self.series.template_references() {
            validate_template_reference(
                engine,
                &reference.template,
                reference.source.as_ref().clone(),
                reference.span,
            )?;
        }
        Ok(())
    }
}

fn self_series_claims(series: &series::Catalog) -> Vec<OutputClaim> {
    series
        .tasks()
        .iter()
        .map(|task| {
            OutputClaim::authored(
                task.output_path.clone(),
                task.series_id
                    .as_ref()
                    .map_or_else(|| "Series index".into(), |id| format!("Series {id}")),
                task.output_source.as_ref().clone(),
                task.output_span,
            )
        })
        .collect()
}

fn validate_template_reference(
    engine: &TemplateEngine,
    template: &crate::template::TemplateName,
    source_code: NamedSource<String>,
    span: SourceSpan,
) -> Result<(), Error> {
    if engine.contains(template) {
        return Ok(());
    }
    Err(Box::new(MissingTemplate {
        template: template.to_string(),
        source_code,
        span,
        help: format!(
            "add {:?} under templates/, or correct this reference",
            template
        ),
    })
    .into())
}

fn root_for(output: &Path) -> String {
    let levels = output
        .parent()
        .map(|parent| parent.components().count())
        .unwrap_or(0);
    if levels == 0 {
        "./".into()
    } else {
        "../".repeat(levels)
    }
}

fn needs_render(task: &Task, output_root: &Path, mode: BuildMode) -> Result<bool, Error> {
    needs_render_generated(task.latest_input, &task.output_path, output_root, mode)
}

fn needs_render_generated(
    latest_input: SystemTime,
    output_path: &OutputPath,
    output_root: &Path,
    mode: BuildMode,
) -> Result<bool, Error> {
    if mode == BuildMode::Full {
        return Ok(true);
    }
    let target = output_root.join(output_path.as_path());
    if !target.exists() {
        return Ok(true);
    }
    let modified = std::fs::metadata(&target)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| Error::Io {
            path: target,
            source,
        })?;
    Ok(modified <= latest_input)
}
