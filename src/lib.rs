//! A small static-site generator.  Input parsing, the resolved site model,
//! template-facing values, rendering, and output writing deliberately live at
//! separate boundaries so future transformations do not need filesystem access.

mod asset;
mod config;
mod content;
mod data;
mod locale;
mod localization;
mod markdown;
mod output;
mod project;
mod server;
mod site;
mod source_diagnostic;
mod source_path;
mod structured;
mod template;

use std::path::{Path, PathBuf};

use crate::{
    config::SiteConfig,
    output::{OutputTarget, OutputWriter},
    project::ProjectPaths,
    site::{BuildOptions as SiteBuildOptions, Site},
};

pub use server::ServeOptions;
pub use site::BuildMode;

/// The immutable output identity produced by one successful build.
///
/// It is intentionally created from the configuration used for that build,
/// rather than by rereading configuration after output publication.
#[derive(Debug, Clone)]
pub(crate) struct BuildPublication {
    pub(crate) base_path: String,
    pub(crate) output_root: PathBuf,
}

/// Options that control one site build.
///
/// `clean_output` is an explicit one-time adoption step for an existing
/// non-empty output directory that was not created by Heine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildOptions {
    mode: BuildMode,
    clean_output: bool,
}

impl BuildOptions {
    pub fn new(mode: BuildMode) -> Self {
        Self {
            mode,
            clean_output: false,
        }
    }

    pub fn clean_output(mut self) -> Self {
        self.clean_output = true;
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
    let paths = ProjectPaths::new(root);
    let config = SiteConfig::load(paths.config()).map_err(miette::Report::new)?;
    let markdown = markdown::Renderer::new(config.markdown().clone())
        .map_err(|error| miette::Report::new(*error))?;
    let (data, locale_data) = data::SiteData::load_localized(paths.data(), config.i18n().locales())
        .map_err(miette::Report::new)?;
    let site = Site::load(
        paths.content(),
        paths.locales(),
        data,
        locale_data,
        config.i18n().clone(),
        &markdown,
    )
    .map_err(miette::Report::new)?;
    let output_dir = config.build().output_dir().to_path_buf();
    let output = paths.root().join(&output_dir);
    let publication = BuildPublication {
        base_path: config.site().base_path().to_owned(),
        output_root: output.clone(),
    };
    let options = SiteBuildOptions::new(build.mode, config);
    let rendered = site
        .render(paths.templates(), &output, &options, &markdown)
        .map_err(miette::Report::new)?;
    OutputWriter::new(OutputTarget::new(paths.root(), &output_dir))
        .write(site.assets(), rendered, build.mode, build.clean_output)
        .map_err(miette::Report::new)?;
    Ok(publication)
}

/// Builds and serves a site for local development.
///
/// The server always performs complete builds. It serves files from the
/// generated output directory and rebuilds after source changes.
pub fn serve_at(root: &Path, options: ServeOptions) -> miette::Result<()> {
    server::serve(root, options)
}
