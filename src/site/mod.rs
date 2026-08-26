//! Resolved site construction and rendering orchestration.
//!
//! Page parsing/loading, validated page state, and deterministic indexes live
//! in dedicated child modules. `Site` coordinates those stages without owning
//! template-engine or output filesystem implementation details.

pub(crate) mod collection;
mod index;
mod loader;
mod locales;
mod page;

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use crate::{
    asset::{AssetCatalog, AssetSources, SharedAssets},
    config::{CollectionsConfig, I18nConfig, SiteConfig},
    content,
    data::SiteData,
    locale::Locale,
    localization::Localization,
    markdown::{self, Renderer as MarkdownRenderer},
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
    template::{
        LocaleView, RenderContext, RuntimeContext, TemplateEngine, TemplateRelationships,
        TemplateResources,
    },
};

pub(crate) use index::{SiblingIndex, TagIndex, TagView};
use page::Task;
pub(crate) use page::{PageId, PageView};

/// The complete, locale-aware catalog of rendered pages.
///
/// Keeping template views and their output targets together makes it
/// impossible for the template boundary to observe two independently-built
/// page indexes. The catalog is constructed only after page relationships
/// have been resolved and is immutable for the lifetime of a build.
#[derive(Debug, Clone)]
pub(crate) struct PageCatalog {
    views: std::sync::Arc<BTreeMap<Locale, BTreeMap<PageId, PageView>>>,
    targets: std::sync::Arc<BTreeMap<Locale, BTreeMap<String, PathBuf>>>,
}

impl PageCatalog {
    fn from_pages(pages: BTreeMap<Locale, BTreeMap<PageId, page::Page>>) -> Self {
        let views = pages
            .into_iter()
            .map(|(locale, pages)| {
                (
                    locale,
                    pages
                        .into_iter()
                        .map(|(id, page)| (id, page.into_view()))
                        .collect::<BTreeMap<_, _>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let targets = views
            .iter()
            .map(|(locale, pages)| {
                (
                    locale.clone(),
                    pages
                        .iter()
                        .filter_map(|(id, page)| {
                            page.output_path()
                                .map(|path| (id.to_string(), PathBuf::from(path)))
                        })
                        .collect(),
                )
            })
            .collect();
        Self {
            views: std::sync::Arc::new(views),
            targets: std::sync::Arc::new(targets),
        }
    }

    pub(crate) fn links(&self, current_locale: Locale, base_path: String) -> PageLinks {
        PageLinks::from_targets(current_locale, base_path, self.targets.clone())
    }

    pub(crate) fn view(&self, locale: &Locale, id: &PageId) -> Option<&PageView> {
        self.views.get(locale)?.get(id)
    }
}

/// Locale-aware rendered-page lookup shared by Tera and Markdown. Page IDs
/// remain locale-local; callers opt into another locale explicitly.
#[derive(Clone)]
pub(crate) struct PageLinks {
    current_locale: Locale,
    base_path: String,
    targets: std::sync::Arc<BTreeMap<Locale, BTreeMap<String, PathBuf>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PageLinkTarget {
    pub(crate) locale: Locale,
    pub(crate) output_path: PathBuf,
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
        base_path: String,
        targets: std::sync::Arc<BTreeMap<Locale, BTreeMap<String, PathBuf>>>,
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
    pub(crate) fn base_path(&self) -> &str {
        &self.base_path
    }
    pub(crate) fn resolve(
        &self,
        id: &str,
        locale: Option<&str>,
    ) -> Result<PageLinkTarget, PageResolveError> {
        let requested = PathBuf::from(id);
        if !crate::source_path::is_logical_id(&requested) {
            return Err(PageResolveError::InvalidId);
        }
        let locale = match locale {
            Some(locale) => locale
                .parse()
                .map_err(|_| PageResolveError::InvalidLocale {
                    locale: locale.into(),
                })?,
            None => self.current_locale.clone(),
        };
        if !self.targets.contains_key(&locale) {
            return Err(PageResolveError::InvalidLocale {
                locale: locale.to_string(),
            });
        }
        let path = self
            .targets
            .get(&locale)
            .and_then(|pages| pages.get(id))
            .ok_or_else(|| PageResolveError::NotFound {
                locale: locale.to_string(),
                id: id.into(),
            })?;
        Ok(PageLinkTarget {
            locale,
            output_path: path.clone(),
            url: format!("{}{}", self.base_path, crate::source_path::url(path)),
        })
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
    DuplicateTranslation(#[from] Box<index::DuplicateTranslation>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Collection(#[from] collection::Error),
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
    PageToml(#[from] Box<loader::PageTomlDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidPage(#[from] Box<loader::PageDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingMetadata(#[from] Box<loader::MissingMetadata>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateCollectionMembership(#[from] Box<loader::DuplicateCollectionMembership>),
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
    tags: std::sync::Arc<BTreeMap<Locale, TagIndex>>,
    collections: std::sync::Arc<BTreeMap<Locale, BTreeMap<String, collection::CollectionView>>>,
    assets: std::sync::Arc<AssetCatalog>,
    data: SiteData,
    locale_data: BTreeMap<Locale, SiteData>,
    i18n: I18nConfig,
    localization: std::sync::Arc<Localization>,
    output_plan: OutputPlan,
}

impl Site {
    pub(crate) fn load(
        content_root: &Path,
        locales_root: &Path,
        data: SiteData,
        locale_data: BTreeMap<Locale, SiteData>,
        i18n: I18nConfig,
        collection_config: CollectionsConfig,
        markdown_renderer: &MarkdownRenderer,
    ) -> Result<Self, Error> {
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
        let locales = locales::load(content_root, &i18n, &collection_config)?;
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
        let tags = std::sync::Arc::new(
            pages
                .iter()
                .map(|(locale, pages)| (locale.clone(), index::tags_for(pages)))
                .collect(),
        );
        let collections = collection::resolve(&pages, &collection_config)?;
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
        let assets = std::sync::Arc::new(AssetCatalog::discover_locales(&asset_sources)?);
        if let Some(latest_asset_input) = assets.latest_input() {
            for task in &mut tasks {
                task.latest_input = task.latest_input.max(latest_asset_input);
            }
        }
        let mut output_claims = tasks
            .iter()
            .map(|task| {
                OutputClaim::authored(
                    task.output_path.clone(),
                    format!("page {}", task.page_id),
                    task.page_source.clone(),
                    task.output_span,
                )
            })
            .collect::<Vec<_>>();
        output_claims.extend(assets.paths().iter().map(|asset| {
            OutputClaim::file(
                OutputPath::new(PathBuf::from(asset)),
                assets.source_path(asset),
            )
        }));
        output_claims.extend(markdown_renderer.stylesheet_claim());
        let output_plan = OutputPlan::validate(&output_claims)?;
        let asset_links = assets.links(i18n.default_locale().clone(), "/".into(), BTreeMap::new());
        let pages = PageCatalog::from_pages(pages);
        let page_links = pages.links(i18n.default_locale().clone(), "/".into());
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
            tags,
            collections,
            assets,
            data,
            locale_data,
            i18n,
            localization,
            output_plan,
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
                self.tags.clone(),
            ),
            self.localization.clone(),
            markdown_renderer
                .stylesheet_path()
                .map(|path| format!("{base_path}{}", crate::source_path::url(path.as_path()))),
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
            let contents = engine.render(
                task.template.as_str(),
                RenderContext {
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page,
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir,
                        page_id: &task.page_id,
                        root: &root,
                    },
                },
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("page task output was validated in the output plan");
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
        Ok(rendered)
    }

    pub(crate) fn assets(&self) -> &AssetCatalog {
        self.assets.as_ref()
    }

    fn validate_templates(&self, engine: &TemplateEngine) -> Result<(), Error> {
        for task in &self.tasks {
            if !engine.contains(task.template.as_str()) {
                return Err(Box::new(MissingTemplate {
                    template: task.template.to_string(),
                    source_code: task.template_source.clone(),
                    span: task.template_span,
                    help: format!(
                        "add {:?} under templates/, or correct this reference",
                        task.template
                    ),
                })
                .into());
            }
        }
        Ok(())
    }
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
    if mode == BuildMode::Full {
        return Ok(true);
    }
    let target = output_root.join(task.output_path.as_path());
    if !target.exists() {
        return Ok(true);
    }
    let modified = std::fs::metadata(&target)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| Error::Io {
            path: target,
            source,
        })?;
    Ok(modified <= task.latest_input)
}
