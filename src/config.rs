//! Site configuration is parsed once at the application boundary and then
//! passed through the build pipeline as validated domain data.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Deserialize;
use thiserror::Error;

mod base_path;

use crate::{
    locale::{Locale, TextDirection},
    output::{OutputClaim, OutputPath},
    project, source_path,
    template::TemplateName,
};
pub(crate) use base_path::{BasePath, valid_url_segment};

#[derive(Debug, Clone)]
pub(crate) struct SiteConfig {
    site: SiteSettings,
    build: BuildConfig,
    assets: AssetConfig,
    markdown: MarkdownConfig,
    i18n: I18nConfig,
    collections: CollectionsConfig,
    taxonomies: TaxonomiesConfig,
}

impl SiteConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, Error> {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(Error::Missing { path: path.into() });
            }
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
                span: span.clone().into(),
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

    pub(crate) fn i18n(&self) -> &I18nConfig {
        &self.i18n
    }

    pub(crate) fn collections(&self) -> &CollectionsConfig {
        &self.collections
    }

    pub(crate) fn taxonomies(&self) -> &TaxonomiesConfig {
        &self.taxonomies
    }
}

/// Validated, site-wide collection definitions. Membership remains authored in
/// page metadata and is resolved separately for every locale.
#[derive(Debug, Clone, Default)]
pub(crate) struct CollectionsConfig {
    definitions: BTreeMap<CollectionName, CollectionDefinition>,
}

impl CollectionsConfig {
    pub(crate) fn definitions(&self) -> &BTreeMap<CollectionName, CollectionDefinition> {
        &self.definitions
    }

    pub(crate) fn contains(&self, name: &CollectionName) -> bool {
        self.definitions.contains_key(name)
    }
}

/// A collection name that is safe to expose as a template-map key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CollectionName(String);

impl CollectionName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if crate::source_path::is_simple_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "collection name {value:?} must be a simple template identifier"
            ))
        }
    }
}

impl std::fmt::Display for CollectionName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A taxonomy name that is safe to use as an authored page-table key and a
/// template-map key. It intentionally remains distinct from a collection name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TaxonomyName(String);

impl TaxonomyName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if crate::source_path::is_simple_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "taxonomy name {value:?} must be a simple template identifier"
            ))
        }
    }
}

impl std::fmt::Display for TaxonomyName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Validated generated-output contracts for site taxonomies.
#[derive(Debug, Clone, Default)]
pub(crate) struct TaxonomiesConfig {
    definitions: BTreeMap<TaxonomyName, TaxonomyDefinition>,
}

impl TaxonomiesConfig {
    pub(crate) fn definitions(&self) -> &BTreeMap<TaxonomyName, TaxonomyDefinition> {
        &self.definitions
    }

    pub(crate) fn contains(&self, name: &TaxonomyName) -> bool {
        self.definitions.contains_key(name)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaxonomyDefinition {
    collection: CollectionName,
    path: OutputPath,
    index_template: TemplateName,
    term_template: TemplateName,
    per_page: usize,
    translations: BTreeMap<String, TermTranslationGroup>,
    source: Option<Arc<NamedSource<String>>>,
    collection_span: SourceSpan,
    path_span: SourceSpan,
    index_template_span: SourceSpan,
    term_template_span: SourceSpan,
    per_page_span: SourceSpan,
}

impl TaxonomyDefinition {
    pub(crate) fn collection(&self) -> &CollectionName {
        &self.collection
    }
    pub(crate) fn path(&self) -> &OutputPath {
        &self.path
    }
    pub(crate) fn index_template(&self) -> &TemplateName {
        &self.index_template
    }
    pub(crate) fn term_template(&self) -> &TemplateName {
        &self.term_template
    }
    pub(crate) fn per_page(&self) -> usize {
        self.per_page
    }
    pub(crate) fn translations(&self) -> &BTreeMap<String, TermTranslationGroup> {
        &self.translations
    }
    pub(crate) fn origin(
        &self,
        span: SourceSpan,
    ) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.source.as_ref().map(|source| (source.clone(), span))
    }
    pub(crate) fn collection_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.collection_span)
    }
    pub(crate) fn path_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.path_span)
    }
    pub(crate) fn index_template_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.index_template_span)
    }
    pub(crate) fn term_template_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.term_template_span)
    }
    pub(crate) fn per_page_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.per_page_span)
    }
}

/// One explicit, potentially partial set of equivalent literal taxonomy terms.
/// The group name is opaque; locale members are validated later against the
/// resolved locale-local taxonomy terms.
#[derive(Debug, Clone)]
pub(crate) struct TermTranslationGroup {
    terms: BTreeMap<Locale, TermTranslation>,
}

impl TermTranslationGroup {
    pub(crate) fn terms(&self) -> &BTreeMap<Locale, TermTranslation> {
        &self.terms
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TermTranslation {
    term: String,
    span: SourceSpan,
}

impl TermTranslation {
    pub(crate) fn term(&self) -> &str {
        &self.term
    }
    pub(crate) fn span(&self) -> SourceSpan {
        self.span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CollectionOrder {
    DateAsc,
    DateDesc,
    TitleAsc,
    TitleDesc,
    WeightAsc,
    WeightDesc,
    IdAsc,
    IdDesc,
}

#[derive(Debug, Clone)]
pub(crate) struct CollectionDefinition {
    order: CollectionOrder,
    order_span: SourceSpan,
    source: Option<Arc<NamedSource<String>>>,
}

impl CollectionDefinition {
    pub(crate) fn order(&self) -> CollectionOrder {
        self.order
    }

    pub(crate) fn order_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.source
            .as_ref()
            .map(|source| (source.clone(), self.order_span))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct I18nConfig {
    default_locale: Locale,
    locales: BTreeMap<Locale, LocaleSettings>,
}

#[derive(Debug, Clone)]
struct LocaleSettings {
    direction: TextDirection,
}

impl LocaleSettings {
    pub(crate) fn direction(&self) -> TextDirection {
        self.direction
    }
}

impl I18nConfig {
    pub(crate) fn default_locale(&self) -> &Locale {
        &self.default_locale
    }
    pub(crate) fn locales(&self) -> impl Iterator<Item = &Locale> {
        self.locales.keys()
    }

    pub(crate) fn direction(&self, locale: &Locale) -> TextDirection {
        self.locales[locale].direction()
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
    pub(crate) fn base_path(&self) -> &BasePath {
        &self.base_path
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
            output_dir: OutputDirectory(
                OutputPath::try_new(PathBuf::from("public"))
                    .expect("the default output directory is portable"),
            ),
        }
    }
}

impl BuildConfig {
    pub(crate) fn output_dir(&self) -> &Path {
        self.output_dir.0.as_path()
    }
}

/// A safe output directory relative to the site root.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputDirectory(OutputPath);

impl TryFrom<String> for OutputDirectory {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        OutputPath::try_new(PathBuf::from(value))
            .map(Self)
            .map_err(|message| format!("build.output_dir {message}"))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AssetConfig {
    integrity: Option<IntegrityAlgorithm>,
}

/// Markdown parsing and rendering choices. The defaults retain CommonMark
/// behavior; extensions and math handling must be selected explicitly.
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
    Tex,
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

#[derive(Debug, Deserialize)]
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
    i18n: RawI18nConfig,
    #[serde(default)]
    collections: BTreeMap<toml::Spanned<String>, RawCollectionDefinition>,
    #[serde(default)]
    taxonomies: BTreeMap<toml::Spanned<String>, RawTaxonomyDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCollectionDefinition {
    order: toml::Spanned<CollectionOrder>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTaxonomyDefinition {
    collection: toml::Spanned<String>,
    path: toml::Spanned<String>,
    index_template: toml::Spanned<String>,
    term_template: toml::Spanned<String>,
    per_page: toml::Spanned<usize>,
    #[serde(default)]
    translations:
        BTreeMap<toml::Spanned<String>, BTreeMap<toml::Spanned<String>, toml::Spanned<String>>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawI18nConfig {
    default_locale: toml::Spanned<String>,
    locales: BTreeMap<toml::Spanned<String>, RawLocaleSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLocaleSettings {
    direction: toml::Spanned<TextDirection>,
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
        let default_locale_span = raw.i18n.default_locale.span();
        let default_locale_value = raw.i18n.default_locale.into_inner();
        let default_locale: Locale =
            default_locale_value
                .parse()
                .map_err(|error| ConfigValueError {
                    span: default_locale_span.clone().into(),
                    message: format!("i18n.default_locale must be a valid BCP-47 locale: {error}"),
                })?;
        if default_locale.to_string() != default_locale_value {
            return Err(ConfigValueError {
                span: default_locale_span.into(),
                message: format!(
                    "i18n.default_locale must use canonical BCP-47 spelling {:?}",
                    default_locale
                ),
            });
        }
        let mut locales = BTreeMap::new();
        for (value, settings) in raw.i18n.locales {
            let value_span = value.span();
            let value = value.into_inner();
            let locale: Locale = value.parse().map_err(|error| ConfigValueError {
                span: value_span.clone().into(),
                message: format!(
                    "i18n.locales key {value:?} must be a valid BCP-47 locale: {error}"
                ),
            })?;
            if locale.to_string() != value {
                return Err(ConfigValueError {
                    span: value_span.clone().into(),
                    message: format!(
                        "i18n.locales key {value:?} must use canonical BCP-47 spelling {:?}",
                        locale.to_string()
                    ),
                });
            }
            locales.insert(
                locale,
                LocaleSettings {
                    direction: settings.direction.into_inner(),
                },
            );
        }
        if locales.is_empty() {
            return Err(ConfigValueError {
                span: default_locale_span.clone().into(),
                message: "i18n.locales must declare at least one locale table".into(),
            });
        }
        if !locales.contains_key(&default_locale) {
            return Err(ConfigValueError {
                span: default_locale_span.into(),
                message: "i18n.locales must include i18n.default_locale".into(),
            });
        }
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
        if project::starts_with_input_root(output_dir.0.as_path()) {
            return Err(ConfigValueError {
                span: output_dir_span.into(),
                message: "build.output_dir must not be content/, data/, deploy/, templates/, locales/, or LICENSES/, or a directory beneath an input directory".into(),
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
            let stylesheet = OutputPath::try_new(path).map_err(|message| ConfigValueError {
                span: span.clone().into(),
                message: format!("markdown.highlighting.stylesheet {message}"),
            })?;
            Ok(HighlightingConfig {
                stylesheet,
                light_theme_span: raw.theme.light.span().into(),
                dark_theme_span: raw.theme.dark.span().into(),
                light_theme: raw.theme.light.into_inner(),
                dark_theme: raw.theme.dark.into_inner(),
                missing_language: raw.missing_language,
                stylesheet_span: span.into(),
                stylesheet_source: None,
            })
        }).transpose()?;
        let mut collections = BTreeMap::new();
        for (name, definition) in raw.collections {
            let span = name.span();
            let name =
                CollectionName::parse(name.into_inner()).map_err(|message| ConfigValueError {
                    span: span.into(),
                    message,
                })?;
            let order_span = definition.order.span().into();
            collections.insert(
                name,
                CollectionDefinition {
                    order: definition.order.into_inner(),
                    order_span,
                    source: None,
                },
            );
        }
        let mut taxonomies = BTreeMap::new();
        for (name, definition) in raw.taxonomies {
            let name_span = name.span();
            let name =
                TaxonomyName::parse(name.into_inner()).map_err(|message| ConfigValueError {
                    span: name_span.into(),
                    message,
                })?;
            let collection_span: SourceSpan = definition.collection.span().into();
            let collection_value = definition.collection.into_inner();
            let collection =
                CollectionName::parse(collection_value.clone()).map_err(|message| {
                    ConfigValueError {
                        span: collection_span,
                        message,
                    }
                })?;
            if !collections.contains_key(&collection) {
                return Err(ConfigValueError {
                    span: collection_span,
                    message: format!(
                        "taxonomy {name} uses collection {collection_value:?}, which is not configured"
                    ),
                });
            }
            let path_span: SourceSpan = definition.path.span().into();
            let path_value = PathBuf::from(definition.path.into_inner());
            let path = OutputPath::try_new(path_value).map_err(|message| ConfigValueError {
                span: path_span,
                message: format!("taxonomy {name} path {message}"),
            })?;
            if crate::output::is_reserved_output_path(path.as_path()) {
                return Err(ConfigValueError {
                    span: path_span,
                    message: format!(
                        "taxonomy {name} path must not use Heine's reserved __heine/ namespace"
                    ),
                });
            }
            let index_template_span: SourceSpan = definition.index_template.span().into();
            let index_template = TemplateName::parse(definition.index_template.into_inner())
                .map_err(|message| ConfigValueError {
                    span: index_template_span,
                    message: format!("taxonomy {name} index_template {message}"),
                })?;
            let term_template_span: SourceSpan = definition.term_template.span().into();
            let term_template = TemplateName::parse(definition.term_template.into_inner())
                .map_err(|message| ConfigValueError {
                    span: term_template_span,
                    message: format!("taxonomy {name} term_template {message}"),
                })?;
            let per_page_span: SourceSpan = definition.per_page.span().into();
            let per_page = definition.per_page.into_inner();
            if per_page == 0 {
                return Err(ConfigValueError {
                    span: per_page_span,
                    message: format!("taxonomy {name} per_page must be a positive integer"),
                });
            }
            let mut translations = BTreeMap::new();
            for (group, members) in definition.translations {
                let group_span: SourceSpan = group.span().into();
                let group = group.into_inner();
                if group.trim().is_empty() {
                    return Err(ConfigValueError {
                        span: group_span,
                        message: format!(
                            "taxonomy {name} translation group names must not be empty"
                        ),
                    });
                }
                if members.len() < 2 {
                    return Err(ConfigValueError {
                        span: group_span,
                        message: format!(
                            "taxonomy {name} translation group {group:?} must declare at least two locales"
                        ),
                    });
                }
                let mut terms = BTreeMap::new();
                for (locale, term) in members {
                    let locale_span: SourceSpan = locale.span().into();
                    let locale_value = locale.into_inner();
                    let locale: Locale = locale_value.parse().map_err(|error| ConfigValueError {
                        span: locale_span,
                        message: format!("taxonomy {name} translation group {group:?} locale {locale_value:?} must be a valid BCP-47 locale: {error}"),
                    })?;
                    if locale.to_string() != locale_value {
                        return Err(ConfigValueError {
                            span: locale_span,
                            message: format!(
                                "taxonomy {name} translation group {group:?} locale {locale_value:?} must use canonical BCP-47 spelling {:?}",
                                locale.to_string()
                            ),
                        });
                    }
                    if !locales.contains_key(&locale) {
                        return Err(ConfigValueError {
                            span: locale_span,
                            message: format!(
                                "taxonomy {name} translation group {group:?} locale {locale:?} is not configured"
                            ),
                        });
                    }
                    let span: SourceSpan = term.span().into();
                    let term = term.into_inner();
                    if term.is_empty() {
                        return Err(ConfigValueError {
                            span,
                            message: format!(
                                "taxonomy {name} translation group {group:?} has an empty term for locale {locale}"
                            ),
                        });
                    }
                    if terms
                        .insert(locale.clone(), TermTranslation { term, span })
                        .is_some()
                    {
                        return Err(ConfigValueError {
                            span: locale_span,
                            message: format!(
                                "taxonomy {name} translation group {group:?} declares locale {locale} more than once"
                            ),
                        });
                    }
                }
                translations.insert(group, TermTranslationGroup { terms });
            }
            taxonomies.insert(
                name,
                TaxonomyDefinition {
                    collection,
                    path,
                    index_template,
                    term_template,
                    per_page,
                    translations,
                    source: None,
                    collection_span,
                    path_span,
                    index_template_span,
                    term_template_span,
                    per_page_span,
                },
            );
        }
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
            i18n: I18nConfig {
                default_locale,
                locales,
            },
            collections: CollectionsConfig {
                definitions: collections,
            },
            taxonomies: TaxonomiesConfig {
                definitions: taxonomies,
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
        let collection_source =
            Arc::new(crate::source_diagnostic::named_source(path, source, "TOML"));
        for definition in self.collections.definitions.values_mut() {
            definition.source = Some(collection_source.clone());
        }
        for definition in self.taxonomies.definitions.values_mut() {
            definition.source = Some(collection_source.clone());
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
    #[error("site configuration {path} does not exist")]
    #[diagnostic(help(
        "create heine.toml with [i18n], default_locale, and locale direction tables"
    ))]
    Missing { path: PathBuf },
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
        let raw: RawSiteConfig = toml::from_str(
            "[assets]\nintegrity = 'sha384'\n[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n",
        )
        .unwrap();
        assert_eq!(
            SiteConfig::try_from(raw).unwrap().assets().integrity(),
            Some(IntegrityAlgorithm::Sha384)
        );
    }

    #[test]
    fn base_path_must_be_an_absolute_directory_path() {
        let raw: RawSiteConfig = toml::from_str(
            "[site]\nbase_path = 'docs'\n[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n",
        )
        .unwrap();

        let error = SiteConfig::try_from(raw).expect_err("relative base path is invalid");

        assert!(error.to_string().contains("must start"));
    }

    #[test]
    fn locale_configuration_requires_canonical_identifiers() {
        let noncanonical: RawSiteConfig =
            toml::from_str("[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de-at]\ndirection = 'ltr'\n").unwrap();
        assert!(
            SiteConfig::try_from(noncanonical)
                .expect_err("noncanonical locale is invalid")
                .to_string()
                .contains("canonical BCP-47")
        );
    }
}
