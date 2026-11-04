//! A small static-site generator.  Input parsing, the resolved site model,
//! template-facing values, rendering, and output writing deliberately live at
//! separate boundaries so future transformations do not need filesystem access.

mod asset;
mod author;
mod config;
mod content;
mod data;
mod deploy;
mod kit;
mod licensing;
mod locale;
mod localization;
mod markdown;
mod output;
mod project;
mod renderer;
mod server;
mod site;
mod source_diagnostic;
mod source_path;
mod structured;
mod template;

use std::path::{Path, PathBuf};

use crate::{
    config::{BasePath, SiteConfig},
    output::{OutputTarget, OutputWriter},
    project::ProjectPaths,
    site::{BuildOptions as SiteBuildOptions, PublicationMode, Site},
};

pub use server::ServeOptions;
pub use site::BuildMode;

/// The immutable output identity produced by one successful build.
///
/// It is intentionally created from the configuration used for that build,
/// rather than by rereading configuration after output publication.
#[derive(Debug, Clone)]
pub(crate) struct BuildPublication {
    pub(crate) base_path: BasePath,
    pub(crate) output_root: PathBuf,
    pub(crate) kit_paths: Option<kit::Paths>,
}

/// Options that control one site build.
///
/// `clean_output` is an explicit one-time adoption step for an existing
/// non-empty output directory that was not created by Heine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildOptions {
    mode: BuildMode,
    clean_output: bool,
    publication: PublicationMode,
    allow_external_renderers: bool,
    refresh_renderers: bool,
}

impl BuildOptions {
    pub fn new(mode: BuildMode) -> Self {
        Self {
            mode,
            clean_output: false,
            publication: PublicationMode::Published,
            allow_external_renderers: false,
            refresh_renderers: false,
        }
    }

    pub fn clean_output(mut self) -> Self {
        self.clean_output = true;
        self
    }

    /// Includes draft pages in an explicit preview build.
    ///
    /// The command-line interface exposes this only as `heine serve --drafts`.
    pub fn preview_drafts(mut self) -> Self {
        self.publication = PublicationMode::PreviewDrafts;
        self
    }

    /// Permits selected renderer requests to start their configured program.
    pub fn allow_external_renderers(mut self) -> Self {
        self.allow_external_renderers = true;
        self
    }

    /// Bypasses reusable renderer results for this build.
    pub fn refresh_renderers(mut self) -> Self {
        self.refresh_renderers = true;
        self
    }
}

/// Finds the nearest ancestor containing `heine.toml`, `content/`, and
/// `templates/`.
pub fn find_site_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| ProjectPaths::new(path).is_site_root())
        .map(Path::to_path_buf)
}

/// Builds a site rooted at `root`, writing to its configured output directory.
pub fn build_at(root: &Path, mode: BuildMode) -> miette::Result<()> {
    build_at_with_options(root, BuildOptions::new(mode))
}

/// Builds a site using explicit build options.
pub fn build_at_with_options(root: &Path, build: BuildOptions) -> miette::Result<()> {
    build_at_with_publication(root, build).map(|_| ())
}

/// Builds a site and returns the metadata belonging to the output generation
/// that was successfully published. This is kept crate-private because normal
/// callers only need the build side effect; the development server needs the
/// matching metadata to publish a coherent HTTP generation.
pub(crate) fn build_at_with_publication(
    root: &Path,
    build: BuildOptions,
) -> miette::Result<BuildPublication> {
    build_at_with_renderer_runtime(root, build).map(|(publication, _)| publication)
}

/// Builds a site and creates the renderer runtime that belongs to its process
/// session. The development server keeps that runtime for later rebuilds.
pub(crate) fn build_at_with_renderer_runtime(
    root: &Path,
    build: BuildOptions,
) -> miette::Result<(BuildPublication, renderer::RendererRuntime)> {
    build_at_with_publication_in_runtime(root, build, None)
}

/// Builds with a caller-owned renderer workspace session when repeated builds
/// belong to one long-lived process, such as the development server.
pub(crate) fn build_at_with_publication_in_runtime(
    root: &Path,
    build: BuildOptions,
    runtime: Option<&renderer::RendererRuntime>,
) -> miette::Result<(BuildPublication, renderer::RendererRuntime)> {
    let paths = ProjectPaths::new(root);
    let diagnostics = source_diagnostic::DiagnosticPaths::new(paths.root());
    let config = SiteConfig::load(paths.config(), &diagnostics).map_err(miette::Report::new)?;
    let selected_kit = config
        .kit()
        .map(|selection| kit::Selected::resolve(paths.root(), selection, &diagnostics))
        .transpose()
        .map_err(miette::Report::new)?;
    let kit_assets = selected_kit
        .as_ref()
        .map(asset::KitAssets::load)
        .transpose()
        .map_err(miette::Report::new)?;
    let renderer_runtime = runtime
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| {
            renderer::RendererRuntime::new(
                config.renderers(),
                paths.root().join(".heine/cache/renderers/v1"),
            )
        })
        .map_err(miette::Report::msg)?;
    let markdown = markdown::Renderer::with_renderer_profiles(
        config.markdown().clone(),
        config.renderers().clone(),
        build.allow_external_renderers,
        build.refresh_renderers,
        renderer_runtime.clone(),
    )
    .map_err(|error| miette::Report::new(*error))?;
    let (data, locale_data) =
        data::SiteData::load_localized(paths.data(), config.i18n().locales(), &diagnostics)
            .map_err(miette::Report::new)?;
    let deployment = deploy::Catalog::load(paths.deploy()).map_err(miette::Report::new)?;
    let licenses = licensing::Catalog::load(paths.licenses(), selected_kit.as_ref())
        .map_err(miette::Report::new)?;
    let site = Site::load(site::LoadInputs {
        content_root: paths.content(),
        locales_root: paths.locales(),
        selected_kit: selected_kit.as_ref(),
        kit_assets: kit_assets.as_ref(),
        diagnostics: &diagnostics,
        data,
        locale_data,
        i18n: config.i18n().clone(),
        collections: config.collections().clone(),
        taxonomies: config.taxonomies().clone(),
        series: config.series(),
        feeds: config.feeds().clone(),
        search: config.search().clone(),
        publications: config.publications(),
        site: config.site(),
        sitemap: config.sitemap(),
        deployment: &deployment,
        licenses: &licenses,
        licensing: config.licensing(),
        markdown_renderer: &markdown,
        reading_time: config.reading_time(),
        publication: build.publication,
    })
    .map_err(miette::Report::new)?;
    let output_dir = config.build().output_dir().to_path_buf();
    let output = paths.root().join(&output_dir);
    let publication = BuildPublication {
        base_path: config.site().base_path().to_owned(),
        output_root: output.clone(),
        kit_paths: selected_kit.as_ref().map(|kit| kit.paths().clone()),
    };
    let options = SiteBuildOptions::new(build.mode, config);
    let rendered = site
        .render(
            paths.templates(),
            selected_kit.as_ref(),
            &output,
            &options,
            &markdown,
            &diagnostics,
        )
        .map_err(miette::Report::new)?;
    let copied_files: [&dyn output::CopiedFiles; 3] = [site.assets(), &deployment, &licenses];
    OutputWriter::new(OutputTarget::new(paths.root(), &output_dir))
        .write(&copied_files, rendered, build.mode, build.clean_output)
        .map_err(miette::Report::new)?;
    Ok((publication, renderer_runtime))
}

/// Builds and serves a site for local development.
///
/// The server always performs complete builds. It serves files from the
/// generated output directory and rebuilds after source changes.
pub fn serve_at(root: &Path, options: ServeOptions) -> miette::Result<()> {
    server::serve(root, options)
}
