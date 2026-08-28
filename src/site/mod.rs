//! Resolved site construction and rendering orchestration.
//!
//! Page parsing/loading, validated page state, and deterministic indexes live
//! in dedicated child modules. `Site` coordinates those stages without owning
//! template-engine or output filesystem implementation details.

mod collation;
pub(crate) mod collection;
mod index;
mod loader;
mod locales;
mod page;
mod pagination;
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
        BasePath, CollectionsConfig, I18nConfig, SiteConfig, SiteSettings, SitemapConfig,
        TaxonomiesConfig,
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
        LocaleView, RenderContext, RuntimeContext, TemplateEngine, TemplateRelationships,
        TemplateResources,
    },
};

pub(crate) use index::SiblingIndex;
use page::Task;
pub(crate) use page::{PageId, PageView, Pager};
pub(crate) use pagination::{Catalog as PaginationCatalog, View as PaginationView};
pub(crate) use taxonomy::View as TaxonomyView;
pub(crate) use taxonomy::{
    Catalog as TaxonomyCatalog, IndexView as TaxonomyIndexView, PageTermView,
};

/// The complete, locale-aware catalog of rendered pages.
///
/// Keeping template views and their output targets together makes it
/// impossible for the template boundary to observe two independently-built
/// page indexes. The catalog is constructed only after page relationships
/// have been resolved and is immutable for the lifetime of a build.
#[derive(Debug, Clone)]
pub(crate) struct PageCatalog {
    views: std::sync::Arc<BTreeMap<Locale, BTreeMap<PageId, PageView>>>,
    targets: std::sync::Arc<BTreeMap<Locale, BTreeMap<PageId, OutputPath>>>,
}

impl PageCatalog {
    fn from_pages(pages: BTreeMap<Locale, BTreeMap<PageId, page::Page>>) -> Self {
        let mut views = BTreeMap::new();
        let mut targets = BTreeMap::new();
        for (locale, pages) in pages {
            let mut locale_views = BTreeMap::new();
            let mut locale_targets = BTreeMap::new();
            for (id, page) in pages {
                if let Some(output) = page.output_path.clone() {
                    locale_targets.insert(id.clone(), output);
                }
                locale_views.insert(id, page.into_view());
            }
            views.insert(locale.clone(), locale_views);
            targets.insert(locale, locale_targets);
        }
        Self {
            views: std::sync::Arc::new(views),
            targets: std::sync::Arc::new(targets),
        }
    }

    pub(crate) fn links(&self, current_locale: Locale, base_path: BasePath) -> PageLinks {
        PageLinks::from_targets(current_locale, base_path, self.targets.clone())
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
    targets: std::sync::Arc<BTreeMap<Locale, BTreeMap<PageId, OutputPath>>>,
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
}

impl PageLinks {
    pub(crate) fn from_targets(
        current_locale: Locale,
        base_path: BasePath,
        targets: std::sync::Arc<BTreeMap<Locale, BTreeMap<PageId, OutputPath>>>,
    ) -> Self {
        Self {
            current_locale,
            base_path,
            targets,
        }
    }
    pub(crate) fn with_locale(&self, current_locale: Locale) -> Self {
        Self {
            current_locale,
            base_path: self.base_path.clone(),
            targets: self.targets.clone(),
        }
    }
    pub(crate) fn base_path(&self) -> &BasePath {
        &self.base_path
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
            .ok_or_else(|| PageResolveError::NotFound {
                locale: locale.to_string(),
                id: requested.to_string(),
            })?;
        Ok(PageLinkTarget {
            id: requested,
            locale,
            output_path: path.clone(),
            url: self.base_path.join(&path.url_path()),
        })
    }

    /// Resolves an optional template locale against the finite configured
    /// locale set. Resource helpers use this shared validation before looking
    /// up their own locale-local values.
    pub(crate) fn resolve_locale(&self, locale: Option<&str>) -> Result<Locale, PageResolveError> {
        let locale = match locale {
            Some(locale) => locale
                .parse()
                .map_err(|_| PageResolveError::InvalidLocale {
                    locale: locale.into(),
                })?,
            None => self.current_locale.clone(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Full,
    Quick,
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
    pub(crate) data: SiteData,
    pub(crate) locale_data: BTreeMap<Locale, SiteData>,
    pub(crate) i18n: I18nConfig,
    pub(crate) collections: CollectionsConfig,
    pub(crate) taxonomies: TaxonomiesConfig,
    pub(crate) site: &'a SiteSettings,
    pub(crate) sitemap: Option<&'a SitemapConfig>,
    pub(crate) deployment: &'a DeploymentCatalog,
    pub(crate) licenses: &'a LicenseCatalog,
    pub(crate) markdown_renderer: &'a MarkdownRenderer,
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
    DuplicateTaxonomyTerm(#[from] Box<loader::DuplicateTaxonomyTerm>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Content(#[from] content::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Markdown(#[from] Box<markdown::SourceError>),
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
    tasks: Vec<Task>,
    taxonomy_tasks: Vec<taxonomy::Task>,
    taxonomies: std::sync::Arc<taxonomy::Catalog>,
    collections: std::sync::Arc<BTreeMap<Locale, BTreeMap<String, collection::CollectionView>>>,
    pagers: pagination::Catalog,
    assets: std::sync::Arc<AssetCatalog>,
    data: SiteData,
    locale_data: BTreeMap<Locale, SiteData>,
    i18n: I18nConfig,
    localization: std::sync::Arc<Localization>,
    output_plan: OutputPlan,
    sitemap: Option<sitemap::Sitemap>,
}

impl Site {
    pub(crate) fn load(inputs: LoadInputs<'_>) -> Result<Self, Error> {
        let LoadInputs {
            content_root,
            locales_root,
            data,
            locale_data,
            i18n,
            collections: collection_config,
            taxonomies: taxonomy_config,
            site: site_settings,
            sitemap: sitemap_config,
            deployment,
            licenses,
            markdown_renderer,
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
        )?);
        let locales = locales::load(
            content_root,
            &i18n,
            &collection_config,
            &taxonomy_config,
            site_settings.time_zone(),
            sitemap_config.is_some(),
        )?;
        let mut pages = locales.pages;
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
            .attach_to_assets(&mut assets, content_root)
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
        let pagers = pagination::Catalog::from_tasks(&tasks, &taxonomy_tasks);
        let sitemap = match (sitemap_config, site_settings.origin()) {
            (Some(config), Some(origin)) => Some(
                sitemap::Sitemap::new(
                    config,
                    origin,
                    site_settings.base_path(),
                    &tasks,
                    &taxonomy_tasks,
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
        let output_plan = OutputPlan::validate(&output_claims)?;
        let asset_links = assets.links(
            i18n.default_locale().clone(),
            BasePath::root(),
            BTreeMap::new(),
        );
        let pages = PageCatalog::from_pages(pages);
        let page_links = pages.links(i18n.default_locale().clone(), BasePath::root());
        for (locale, source) in &locales.markdown_sources {
            let asset_links = asset_links.with_locale(locale.clone());
            let page_links = page_links.with_locale(locale.clone());
            markdown::validate_source_with_links(
                &source.path,
                &source.source,
                markdown_renderer,
                Some(&asset_links),
                Some(&page_links),
            )?;
        }
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
            tasks,
            taxonomy_tasks,
            taxonomies,
            collections,
            pagers,
            assets,
            data,
            locale_data,
            i18n,
            localization,
            output_plan,
            sitemap: sitemap.map(|(sitemap, _)| sitemap),
        })
    }

    pub(crate) fn render(
        &self,
        templates: &Path,
        existing_output: &Path,
        options: &BuildOptions,
        markdown_renderer: &MarkdownRenderer,
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
                self.collections.clone(),
                self.siblings.clone(),
                self.taxonomies.clone(),
                self.pagers.clone(),
            ),
            self.localization.clone(),
            markdown_renderer
                .stylesheet_path()
                .map(|path| base_path.join(&path.url_path())),
        );
        let engine = TemplateEngine::load(templates, resources, markdown_renderer.clone())?;
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
        Ok(())
    }
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
