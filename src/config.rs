//! Site configuration is parsed once at the application boundary and then
//! passed through the build pipeline as validated domain data.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Deserialize;
use thiserror::Error;

mod base_path;

use crate::{
    output::{OutputClaim, OutputPath},
    source_path,
};
use base_path::BasePath;

#[derive(Debug, Clone, Default)]
pub(crate) struct SiteConfig {
    site: SiteSettings,
    build: BuildConfig,
    assets: AssetConfig,
    markdown: MarkdownConfig,
}

impl SiteConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, Error> {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(Error::Read {
                    path: path.into(),
                    source,
                });
            }
        };
        let raw: RawSiteConfig = toml::from_str(&source).map_err(|error| {
            let span = error.span().unwrap_or(0..0);
            Error::Invalid(Box::new(ConfigDiagnostic {
                message: error.message().to_owned(),
                source_code: crate::source_diagnostic::named_source(path, source.clone(), "TOML"),
                span: span.into(),
            }))
        })?;
        let mut config: Self = raw.try_into().map_err(|error: ConfigValueError| {
            let ConfigValueError { span, message } = error;
            Error::Invalid(Box::new(ConfigDiagnostic {
                message,
                source_code: crate::source_diagnostic::named_source(path, source.clone(), "TOML"),
                span,
            }))
        })?;
        config.attach_source(path, &source);
        Ok(config)
    }

    pub(crate) fn assets(&self) -> &AssetConfig {
        &self.assets
    }

    pub(crate) fn site(&self) -> &SiteSettings {
        &self.site
    }

    pub(crate) fn build(&self) -> &BuildConfig {
        &self.build
    }

    pub(crate) fn markdown(&self) -> &MarkdownConfig {
        &self.markdown
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SiteSettings {
    base_path: BasePath,
}

impl Default for SiteSettings {
    fn default() -> Self {
        Self {
            base_path: BasePath::root(),
        }
    }
}

impl SiteSettings {
    pub(crate) fn base_path(&self) -> &str {
        self.base_path.as_str()
    }
}

/// Configuration for where a build is written on the local filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildConfig {
    output_dir: OutputDirectory,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            output_dir: OutputDirectory(PathBuf::from("public")),
        }
    }
}

impl BuildConfig {
    pub(crate) fn output_dir(&self) -> &Path {
        &self.output_dir.0
    }
}

/// A safe output directory relative to the site root.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputDirectory(PathBuf);

impl TryFrom<String> for OutputDirectory {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let path = PathBuf::from(value);
        if path.as_os_str().is_empty()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::Prefix(_)
                        | std::path::Component::RootDir
                        | std::path::Component::CurDir
                        | std::path::Component::ParentDir
                )
            })
        {
            return Err(
                "build.output_dir must be a non-empty relative path without '.' or '..'".into(),
            );
        }
        Ok(Self(path))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AssetConfig {
    integrity: Option<IntegrityAlgorithm>,
}

/// Markdown parsing and rendering choices. The defaults retain CommonMark
/// behavior; extensions and MathML rendering must be selected explicitly.
#[derive(Debug, Clone, Default)]
pub(crate) struct MarkdownConfig {
    extensions: Vec<MarkdownExtension>,
    math: MathMode,
    highlighting: Option<HighlightingConfig>,
}

impl MarkdownConfig {
    pub(crate) fn extensions(&self) -> &[MarkdownExtension] {
        &self.extensions
    }

    pub(crate) fn math(&self) -> MathMode {
        self.math
    }

    pub(crate) fn highlighting(&self) -> Option<&HighlightingConfig> {
        self.highlighting.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HighlightingConfig {
    stylesheet: OutputPath,
    light_theme: String,
    dark_theme: String,
    missing_language: MissingLanguage,
    stylesheet_span: SourceSpan,
    light_theme_span: SourceSpan,
    dark_theme_span: SourceSpan,
    stylesheet_source: Option<NamedSource<String>>,
}

impl HighlightingConfig {
    pub(crate) fn stylesheet(&self) -> &OutputPath {
        &self.stylesheet
    }
    pub(crate) fn light_theme(&self) -> &str {
        &self.light_theme
    }
    pub(crate) fn dark_theme(&self) -> &str {
        &self.dark_theme
    }
    pub(crate) fn missing_language(&self) -> MissingLanguage {
        self.missing_language
    }

    pub(crate) fn stylesheet_span(&self) -> SourceSpan {
        self.stylesheet_span
    }

    pub(crate) fn stylesheet_source(&self) -> Option<&NamedSource<String>> {
        self.stylesheet_source.as_ref()
    }

    pub(crate) fn theme_span(&self, theme: &str) -> SourceSpan {
        if theme.eq_ignore_ascii_case(&self.light_theme) {
            self.light_theme_span
        } else {
            self.dark_theme_span
        }
    }

    pub(crate) fn output_claim(&self) -> Option<OutputClaim> {
        self.stylesheet_source.as_ref().map(|source_code| {
            OutputClaim::authored(
                self.stylesheet.clone(),
                "syntax-highlighting stylesheet",
                source_code.clone(),
                self.stylesheet_span,
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MissingLanguage {
    Error,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MarkdownExtension {
    Tables,
    Strikethrough,
    TaskLists,
    Footnotes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MathMode {
    #[default]
    Off,
    Mathml,
}

impl AssetConfig {
    pub(crate) fn integrity(&self) -> Option<IntegrityAlgorithm> {
        self.integrity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IntegrityAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSiteConfig {
    #[serde(default)]
    site: RawSiteSettings,
    #[serde(default)]
    build: RawBuildConfig,
    #[serde(default)]
    assets: RawAssetConfig,
    #[serde(default)]
    markdown: RawMarkdownConfig,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSiteSettings {
    #[serde(default = "default_base_path")]
    base_path: toml::Spanned<String>,
}

impl Default for RawSiteSettings {
    fn default() -> Self {
        Self {
            base_path: default_base_path(),
        }
    }
}

fn default_base_path() -> toml::Spanned<String> {
    toml::Spanned::new(0..0, "/".into())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuildConfig {
    #[serde(default = "default_output_dir")]
    output_dir: toml::Spanned<String>,
}

impl Default for RawBuildConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
        }
    }
}

fn default_output_dir() -> toml::Spanned<String> {
    toml::Spanned::new(0..0, "public".into())
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawAssetConfig {
    integrity: Option<IntegrityAlgorithm>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMarkdownConfig {
    #[serde(default = "default_extensions")]
    extensions: toml::Spanned<Vec<MarkdownExtension>>,
    #[serde(default)]
    math: MathMode,
    highlighting: Option<RawHighlightingConfig>,
}

impl Default for RawMarkdownConfig {
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
            math: MathMode::default(),
            highlighting: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHighlightingConfig {
    stylesheet: toml::Spanned<String>,
    theme: RawHighlightingTheme,
    #[serde(default = "default_missing_language")]
    missing_language: MissingLanguage,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHighlightingTheme {
    light: toml::Spanned<String>,
    dark: toml::Spanned<String>,
}

fn default_missing_language() -> MissingLanguage {
    MissingLanguage::Error
}

fn default_extensions() -> toml::Spanned<Vec<MarkdownExtension>> {
    toml::Spanned::new(0..0, vec![])
}

impl TryFrom<RawSiteConfig> for SiteConfig {
    type Error = ConfigValueError;

    fn try_from(raw: RawSiteConfig) -> Result<Self, Self::Error> {
        let base_path_span = raw.site.base_path.span();
        let base_path = raw
            .site
            .base_path
            .into_inner()
            .try_into()
            .map_err(|message| ConfigValueError {
                span: base_path_span.into(),
                message,
            })?;
        let output_dir_span = raw.build.output_dir.span();
        let output_dir: OutputDirectory =
            raw.build
                .output_dir
                .into_inner()
                .try_into()
                .map_err(|message| ConfigValueError {
                    span: output_dir_span.clone().into(),
                    message,
                })?;
        if matches!(output_dir.0.components().next(), Some(std::path::Component::Normal(name)) if matches!(name.to_str(), Some("content" | "data" | "templates")))
        {
            return Err(ConfigValueError {
                span: output_dir_span.into(),
                message: "build.output_dir must not be content/, data/, or templates/, or a directory beneath an input directory".into(),
            });
        }
        let extensions_span = raw.markdown.extensions.span();
        let mut extensions = raw.markdown.extensions.into_inner();
        extensions.sort_by_key(|extension| match extension {
            MarkdownExtension::Tables => 0,
            MarkdownExtension::Strikethrough => 1,
            MarkdownExtension::TaskLists => 2,
            MarkdownExtension::Footnotes => 3,
        });
        if extensions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ConfigValueError {
                span: extensions_span.into(),
                message: "markdown.extensions must not contain duplicate values".into(),
            });
        }
        let highlighting = raw.markdown.highlighting.map(|raw| {
            let span = raw.stylesheet.span();
            let path = PathBuf::from(raw.stylesheet.into_inner());
            if !source_path::is_logical_id(&path) {
                return Err(ConfigValueError {
                    span: span.into(),
                    message: "markdown.highlighting.stylesheet must be a non-empty relative output path without '.', '..', or backslashes".into(),
                });
            }
            if path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("css")) {
                return Err(ConfigValueError {
                    span: span.into(),
                    message: "markdown.highlighting.stylesheet must end in .css".into(),
                });
            }
            if crate::output::is_reserved_output_path(&path) {
                return Err(ConfigValueError {
                    span: span.into(),
                    message: "markdown.highlighting.stylesheet must not use Heine's reserved __heine/ namespace".into(),
                });
            }
            Ok(HighlightingConfig {
                stylesheet: OutputPath::new(path),
                light_theme_span: raw.theme.light.span().into(),
                dark_theme_span: raw.theme.dark.span().into(),
                light_theme: raw.theme.light.into_inner(),
                dark_theme: raw.theme.dark.into_inner(),
                missing_language: raw.missing_language,
                stylesheet_span: span.into(),
                stylesheet_source: None,
            })
        }).transpose()?;
        Ok(Self {
            site: SiteSettings { base_path },
            build: BuildConfig { output_dir },
            assets: AssetConfig {
                integrity: raw.assets.integrity,
            },
            markdown: MarkdownConfig {
                extensions,
                math: raw.markdown.math,
                highlighting,
            },
        })
    }
}

impl SiteConfig {
    fn attach_source(&mut self, path: &Path, source: &str) {
        if let Some(highlighting) = &mut self.markdown.highlighting {
            highlighting.stylesheet_source =
                Some(crate::source_diagnostic::named_source(path, source, "TOML"));
        }
    }
}

#[derive(Debug)]
struct ConfigValueError {
    span: SourceSpan,
    message: String,
}

impl std::fmt::Display for ConfigValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not read configuration file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Invalid(#[from] Box<ConfigDiagnostic>),
}

#[derive(Debug, Diagnostic, Error)]
#[error("invalid configuration")]
#[diagnostic(code(heine::config::invalid))]
pub(crate) struct ConfigDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{message}")]
    span: SourceSpan,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_algorithm_deserializes_from_config() {
        let raw: RawSiteConfig = toml::from_str("[assets]\nintegrity = 'sha384'\n").unwrap();
        assert_eq!(
            SiteConfig::try_from(raw).unwrap().assets().integrity(),
            Some(IntegrityAlgorithm::Sha384)
        );
    }

    #[test]
    fn base_path_must_be_an_absolute_directory_path() {
        let raw: RawSiteConfig = toml::from_str("[site]\nbase_path = 'docs'\n").unwrap();

        let error = SiteConfig::try_from(raw).expect_err("relative base path is invalid");

        assert!(error.to_string().contains("must start"));
    }
}
