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
mod render;
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

use miette::Diagnostic;
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
    output::{OutputClaim, OutputPlan},
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
    pub(crate) selected_kit: Option<&'a crate::kit::Selected>,
    pub(crate) kit_assets: Option<&'a crate::asset::KitAssets>,
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
    pub(crate) reading_time: &'a crate::config::ReadingTimeConfig,
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
    MissingTemplate(#[from] Box<render::MissingTemplate>),
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
    renderer_resources: Vec<page::RendererResource>,
}

impl Site {
    pub(crate) fn load(inputs: LoadInputs<'_>) -> Result<Self, Error> {
        let LoadInputs {
            content_root,
            locales_root,
            selected_kit,
            kit_assets,
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
            reading_time,
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
            selected_kit,
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
        let search_selections = search::Selections::resolve(&collections, &search_config);
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
                        content_inputs: &locales.content_inputs,
                        output_prefix: &prefixes[locale],
                        shared: (!locales.shared_entries.is_empty()).then_some(SharedAssets {
                            root: &locales.shared_root,
                            entries: &locales.shared_entries,
                        }),
                        kit: kit_assets,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut assets = AssetCatalog::discover_locales(&asset_sources)?;
        licenses
            .attach_to_assets(
                &mut assets,
                content_root,
                kit_assets,
                licensing,
                diagnostics,
            )
            .map_err(Box::new)?;
        let assets = std::sync::Arc::new(assets);
        {
            // Source analysis needs the same locale-aware links as rendering,
            // but this temporary target catalog is not part of the final Site.
            // Release its per-page map before constructing render-facing views.
            let validation_assets = assets.links(
                i18n.default_locale().clone(),
                BasePath::root(),
                None,
                BTreeMap::new(),
            );
            let validation_pages = PageCatalog::links_from_pages(
                &pages,
                i18n.default_locale().clone(),
                BasePath::root(),
                None,
                excluded_drafts.clone(),
            );
            for (locale, locale_pages) in &mut pages {
                let assets = validation_assets.with_locale(locale.clone());
                let page_links = validation_pages.with_locale(locale.clone());
                for page in locale_pages.values_mut() {
                    page.analyze_markdown(
                        markdown_renderer,
                        &assets,
                        &page_links,
                        reading_time.words_per_minute(locale),
                        search_selections.contains(locale, &page.id),
                        site_settings.base_path().as_str(),
                    )?;
                }
            }
        }
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
        let canonical_urls = site_settings.canonical_urls();
        let feeds = feed::Catalog::resolve(feed::Inputs {
            pages: &pages,
            collections: &collections,
            config: &feed_config,
            prefixes: &prefixes,
            urls: canonical_urls.as_ref(),
            localization: &localization,
        })
        .map_err(|error| Error::Feed(Box::new(error)))?;
        let search = search::Catalog::resolve(search::Inputs {
            pages: &mut pages,
            selections: &search_selections,
            config: &search_config,
            prefixes: &prefixes,
            base_path: site_settings.base_path(),
            taxonomies: &taxonomy_config,
        })
        .map_err(|error| Error::Search(Box::new(error)))?;
        let renderer_resources = group_renderer_resources(
            pages
                .values()
                .flat_map(|pages| pages.values())
                .flat_map(|page| page.renderer_resources.iter().cloned()),
        );
        let pagers = pagination::Catalog::from_tasks(&tasks, &taxonomy_tasks, series.tasks());
        let sitemap = match (sitemap_config, canonical_urls.as_ref()) {
            (Some(config), Some(urls)) => Some(
                sitemap::Sitemap::new(
                    config,
                    urls,
                    &pages,
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
                    task.page_source.clone(),
                    task.output_span,
                )
            })
            .collect::<Vec<_>>();
        output_claims.extend(taxonomy_tasks.iter().map(|task| {
            OutputClaim::authored(
                task.output_path.clone(),
                format!("taxonomy {}", task.taxonomy.name),
                task.output_source.clone(),
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
        output_claims.extend(renderer_resources.iter().map(|resource| {
            OutputClaim::renderer(
                resource.output_path.clone(),
                "renderer resource",
                resource
                    .origins
                    .iter()
                    .map(|origin| (origin.source.clone(), origin.span)),
            )
        }));
        let output_plan = OutputPlan::validate(&output_claims)?;
        let pages = PageCatalog::from_pages(pages, excluded_drafts, canonical_urls.as_ref());
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
            renderer_resources,
        })
    }

    pub(crate) fn assets(&self) -> &AssetCatalog {
        self.assets.as_ref()
    }
}

fn group_renderer_resources(
    resources: impl IntoIterator<Item = page::RendererResource>,
) -> Vec<page::RendererResource> {
    resources
        .into_iter()
        .fold(
            BTreeMap::<_, page::RendererResource>::new(),
            |mut grouped, resource| {
                if let Some(existing) = grouped.get_mut(&resource.output_path) {
                    existing.origins.extend(resource.origins);
                } else {
                    grouped.insert(resource.output_path.clone(), resource);
                }
                grouped
            },
        )
        .into_values()
        .collect()
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
                task.output_source.clone(),
                task.output_span,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use miette::NamedSource;

    use super::{group_renderer_resources, page};

    #[test]
    fn identical_renderer_resources_retain_every_markdown_origin() {
        let output_path = crate::output::OutputPath::try_new("heine-renderers/a.svg".into())
            .expect("valid renderer output path");
        let resource = |name| page::RendererResource {
            output_path: output_path.clone(),
            bytes: b"<svg/>".to_vec(),
            origins: vec![page::RendererResourceOrigin {
                source: Arc::new(NamedSource::new(name, ":::render[diagram]".to_owned())),
                span: (0, 1).into(),
            }],
        };

        let grouped = group_renderer_resources([resource("first.md"), resource("second.md")]);

        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].origins.len(), 2);
    }
}
