//! Resolved site construction and rendering orchestration.
//!
//! Page parsing/loading, validated page state, and deterministic indexes live
//! in dedicated child modules. `Site` coordinates those stages without owning
//! template-engine or output filesystem implementation details.

mod index;
mod loader;
mod page;

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use crate::{
    asset::AssetCatalog,
    config::SiteConfig,
    content,
    data::SiteData,
    markdown::{self, Renderer as MarkdownRenderer},
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
    template::{RenderContext, TemplateEngine, TemplateResources},
};

use page::{Page, Task};
pub(crate) use page::{PageId, PageView};

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
    pages: BTreeMap<PageId, Page>,
    tasks: Vec<Task>,
    tags: BTreeMap<String, Vec<PageId>>,
    assets: AssetCatalog,
    data: SiteData,
    output_plan: OutputPlan,
}

impl Site {
    pub(crate) fn load(
        content_root: &Path,
        data: SiteData,
        markdown_renderer: &MarkdownRenderer,
    ) -> Result<Self, Error> {
        if !content_root.is_dir() {
            return Err(Error::MissingContent(content_root.into()));
        }
        let entries = loader::content_entries(content_root)?;
        let descriptors = loader::load_descriptors(content_root, &entries)?;
        let mut pages = BTreeMap::new();
        let mut tasks = Vec::new();
        let mut content_inputs = BTreeSet::new();
        let mut markdown_sources = Vec::new();
        for entry in &entries {
            if !entry.file_type().is_file() {
                continue;
            }
            let source = entry.path();
            let relative = loader::relative_path(content_root, source)?;
            if source
                .extension()
                .is_some_and(|extension| extension == "page")
            {
                let loaded =
                    loader::load_page(content_root, source, &relative, &entries, &descriptors)?;
                if let Some(task) = loaded.task {
                    tasks.push(task);
                }
                content_inputs.extend(loaded.content_inputs);
                markdown_sources.extend(loaded.markdown_sources);
                pages.insert(loaded.page.id.clone(), loaded.page);
            }
        }
        tasks.sort_by(|left, right| left.page_id.cmp(&right.page_id));
        if let Some(latest_data_input) = data.latest_input() {
            for task in &mut tasks {
                task.latest_input = task.latest_input.max(latest_data_input);
            }
        }
        index::fix_siblings(&mut pages);
        let tags = index::tags_for(&pages);
        let assets = AssetCatalog::discover(content_root, &entries, &content_inputs)?;
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
        let asset_links = assets.links("/".into(), BTreeMap::new());
        for source in &markdown_sources {
            markdown::validate_source(
                &source.path,
                &source.source,
                markdown_renderer,
                Some(&asset_links),
            )?;
        }
        Ok(Self {
            pages,
            tasks,
            tags,
            assets,
            data,
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
            self.assets.links(base_path.clone(), integrities),
            base_path.clone(),
            self.pages
                .iter()
                .filter_map(|(id, page)| {
                    page.path
                        .as_ref()
                        .map(|path| (id.to_string(), PathBuf::from(path.as_str())))
                })
                .collect(),
            markdown_renderer
                .stylesheet_path()
                .map(|path| format!("{base_path}{}", crate::source_path::url(path.as_path()))),
        );
        let engine = TemplateEngine::load(templates, resources, markdown_renderer.clone())?;
        self.validate_templates(&engine)?;
        let views = self.render_views();
        let mut rendered = Vec::new();
        for task in &self.tasks {
            if !needs_render(task, existing_output, options.mode())? {
                continue;
            }
            let page = views
                .get(&task.page_id)
                .expect("tasks refer to loaded pages");
            let root = root_for(task.output_path.as_path());
            let contents = engine.render(
                task.template.as_str(),
                RenderContext {
                    page,
                    pages: &views,
                    tags: &self.tags,
                    assets: self.assets.paths(),
                    data: self.data.value(),
                    root: &root,
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
        &self.assets
    }

    fn render_views(&self) -> BTreeMap<PageId, PageView> {
        self.pages
            .iter()
            .map(|(id, page)| (id.clone(), page.view()))
            .collect()
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
