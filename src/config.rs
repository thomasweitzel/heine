//! Site configuration is parsed once at the application boundary and then
//! passed through the build pipeline as validated domain data.

use std::{
    collections::BTreeMap,
    fs, io,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::Arc,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Deserialize;
use thiserror::Error;

mod base_path;
mod licensing;
mod origin;
mod parse;
mod time_zone;

use crate::{
    author::AuthorName,
    kit,
    locale::{Locale, TextDirection},
    output::{OutputClaim, OutputPath},
    renderer,
    template::TemplateName,
};
pub(crate) use base_path::{BasePath, valid_url_segment};
pub(crate) use licensing::{
    LicensingConfig, LicensingCoverage, LicensingDeclaration, LicensingRecord, LicensingValue,
};
pub(crate) use origin::CanonicalUrls;
use origin::SiteOrigin;
use parse::{ConfigValueError, RawSiteConfig};
pub(crate) use time_zone::EditorialTimeZone;

#[derive(Debug, Clone)]
pub(crate) struct SiteConfig {
    site: SiteSettings,
    build: BuildConfig,
    assets: AssetConfig,
    licensing: LicensingConfig,
    markdown: MarkdownConfig,
    reading_time: ReadingTimeConfig,
    i18n: I18nConfig,
    collections: CollectionsConfig,
    taxonomies: TaxonomiesConfig,
    series: Option<SeriesConfig>,
    sitemap: Option<SitemapConfig>,
    feeds: FeedsConfig,
    search: SearchConfig,
    renderers: renderer::Profiles,
    kit: Option<kit::Selection>,
}

impl SiteConfig {
    pub(crate) fn load(
        path: &Path,
        diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    ) -> Result<Self, Error> {
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
                source_code: diagnostics.named_source(path, source.clone(), "TOML"),
                span: span.into(),
            }))
        })?;
        let mut config: Self = raw.try_into().map_err(|error: ConfigValueError| {
            let ConfigValueError { span, message } = error;
            Error::Invalid(Box::new(ConfigDiagnostic {
                message,
                source_code: diagnostics.named_source(path, source.clone(), "TOML"),
                span,
            }))
        })?;
        config.attach_kit_extra(&source).map_err(|error| {
            Error::Invalid(Box::new(ConfigDiagnostic {
                message: error.message,
                source_code: diagnostics.named_source(path, source.clone(), "TOML"),
                span: error.span,
            }))
        })?;
        config.attach_source(path, &source, diagnostics);
        Ok(config)
    }

    pub(crate) fn assets(&self) -> &AssetConfig {
        &self.assets
    }

    pub(crate) fn licensing(&self) -> &LicensingConfig {
        &self.licensing
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

    pub(crate) fn reading_time(&self) -> &ReadingTimeConfig {
        &self.reading_time
    }

    pub(crate) fn collections(&self) -> &CollectionsConfig {
        &self.collections
    }

    pub(crate) fn taxonomies(&self) -> &TaxonomiesConfig {
        &self.taxonomies
    }

    pub(crate) fn series(&self) -> Option<&SeriesConfig> {
        self.series.as_ref()
    }

    pub(crate) fn sitemap(&self) -> Option<&SitemapConfig> {
        self.sitemap.as_ref()
    }

    pub(crate) fn feeds(&self) -> &FeedsConfig {
        &self.feeds
    }

    pub(crate) fn search(&self) -> &SearchConfig {
        &self.search
    }

    pub(crate) fn renderers(&self) -> &renderer::Profiles {
        &self.renderers
    }

    pub(crate) fn kit(&self) -> Option<&kit::Selection> {
        self.kit.as_ref()
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

/// A Series identifier is both a template lookup key and a generated route
/// component. It deliberately remains distinct from collections and
/// taxonomies because its membership has independent authored positions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SeriesName(String);

impl SeriesName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if !value.is_empty()
            && !matches!(value.as_str(), "." | "..")
            && !value.contains('%')
            && valid_url_segment(&value)
            && crate::output::is_safe_output_component(&value)
        {
            Ok(Self(value))
        } else {
            Err(format!(
                "series ID {value:?} must be a literal portable URL-path component without percent escapes"
            ))
        }
    }
}

impl std::fmt::Display for SeriesName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Checked contracts for generated Series indexes and listings.
#[derive(Debug, Clone)]
pub(crate) struct SeriesConfig {
    path: OutputPath,
    index_template: TemplateName,
    listing_template: TemplateName,
    index_title_key: String,
    index_description_key: Option<String>,
    per_page: usize,
    sitemap_include: bool,
    definitions: BTreeMap<SeriesName, SeriesDefinition>,
    source: Option<Arc<NamedSource<String>>>,
    path_span: SourceSpan,
    index_template_span: SourceSpan,
    listing_template_span: SourceSpan,
    index_title_key_span: SourceSpan,
    index_description_key_span: Option<SourceSpan>,
    per_page_span: SourceSpan,
}

impl SeriesConfig {
    pub(crate) fn path(&self) -> &OutputPath {
        &self.path
    }
    pub(crate) fn index_template(&self) -> &TemplateName {
        &self.index_template
    }
    pub(crate) fn listing_template(&self) -> &TemplateName {
        &self.listing_template
    }
    pub(crate) fn index_title_key(&self) -> &str {
        &self.index_title_key
    }
    pub(crate) fn index_description_key(&self) -> Option<&str> {
        self.index_description_key.as_deref()
    }
    pub(crate) fn per_page(&self) -> usize {
        self.per_page
    }
    pub(crate) fn sitemap_include(&self) -> bool {
        self.sitemap_include
    }
    pub(crate) fn definitions(&self) -> &BTreeMap<SeriesName, SeriesDefinition> {
        &self.definitions
    }
    pub(crate) fn contains(&self, name: &SeriesName) -> bool {
        self.definitions.contains_key(name)
    }
    pub(crate) fn origin(
        &self,
        span: SourceSpan,
    ) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.source.as_ref().map(|source| (source.clone(), span))
    }
    pub(crate) fn path_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.path_span)
    }
    pub(crate) fn index_template_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.index_template_span)
    }
    pub(crate) fn listing_template_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.listing_template_span)
    }
    pub(crate) fn index_title_key_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.index_title_key_span)
    }
    pub(crate) fn index_description_key_origin(
        &self,
    ) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.index_description_key_span
            .and_then(|span| self.origin(span))
    }
    pub(crate) fn per_page_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.per_page_span)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SeriesDefinition {
    title_key: String,
    description_key: Option<String>,
    source: Option<Arc<NamedSource<String>>>,
    name_span: SourceSpan,
    title_key_span: SourceSpan,
    description_key_span: Option<SourceSpan>,
}

impl SeriesDefinition {
    pub(crate) fn title_key(&self) -> &str {
        &self.title_key
    }
    pub(crate) fn description_key(&self) -> Option<&str> {
        self.description_key.as_deref()
    }
    pub(crate) fn origin(
        &self,
        span: SourceSpan,
    ) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.source.as_ref().map(|source| (source.clone(), span))
    }
    pub(crate) fn name_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.name_span)
    }
    pub(crate) fn title_key_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.title_key_span)
    }
    pub(crate) fn description_key_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.description_key_span.and_then(|span| self.origin(span))
    }
}

/// A feed name is an authored identifier used only by feed configuration and
/// checked template lookup. It is distinct from collection names even though
/// both use the same portable spelling rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FeedName(String);

impl FeedName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if crate::source_path::is_simple_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "feed name {value:?} must be a simple template identifier"
            ))
        }
    }
}

impl std::fmt::Display for FeedName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A search-index name is an authored identifier used by configuration and
/// checked template lookup. It remains distinct from collection and feed names
/// although all three use the same portable spelling rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SearchName(String);

impl SearchName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if crate::source_path::is_simple_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "search index name {value:?} must be a simple template identifier"
            ))
        }
    }
}

impl std::fmt::Display for SearchName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Checked contracts for collection-derived, locale-local JSON search indexes.
#[derive(Debug, Clone, Default)]
pub(crate) struct SearchConfig {
    definitions: BTreeMap<SearchName, SearchDefinition>,
}

impl SearchConfig {
    pub(crate) fn definitions(&self) -> &BTreeMap<SearchName, SearchDefinition> {
        &self.definitions
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchDefinition {
    collections: Vec<CollectionName>,
    path: OutputPath,
    versioned: bool,
    source: Option<Arc<NamedSource<String>>>,
    path_span: SourceSpan,
}

impl SearchDefinition {
    pub(crate) fn collections(&self) -> &[CollectionName] {
        &self.collections
    }

    pub(crate) fn path(&self) -> &OutputPath {
        &self.path
    }

    pub(crate) fn versioned(&self) -> bool {
        self.versioned
    }

    pub(crate) fn origin(
        &self,
        span: SourceSpan,
    ) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.source.as_ref().map(|source| (source.clone(), span))
    }

    pub(crate) fn path_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.path_span)
    }
}

/// Checked Atom-feed contracts. The feed module turns them into locale-local
/// generated resources after collection membership has resolved.
#[derive(Debug, Clone, Default)]
pub(crate) struct FeedsConfig {
    definitions: BTreeMap<FeedName, FeedDefinition>,
}

impl FeedsConfig {
    pub(crate) fn definitions(&self) -> &BTreeMap<FeedName, FeedDefinition> {
        &self.definitions
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FeedDefinition {
    collection: CollectionName,
    path: OutputPath,
    title_key: String,
    subtitle_key: Option<String>,
    limit: usize,
    empty_updated: Option<jiff::Timestamp>,
    empty_authors: Option<Vec<AuthorName>>,
    source: Option<Arc<NamedSource<String>>>,
    declaration_span: SourceSpan,
    path_span: SourceSpan,
    title_key_span: SourceSpan,
    subtitle_key_span: Option<SourceSpan>,
    empty_authors_span: Option<SourceSpan>,
}

impl FeedDefinition {
    pub(crate) fn collection(&self) -> &CollectionName {
        &self.collection
    }
    pub(crate) fn path(&self) -> &OutputPath {
        &self.path
    }
    pub(crate) fn title_key(&self) -> &str {
        &self.title_key
    }
    pub(crate) fn subtitle_key(&self) -> Option<&str> {
        self.subtitle_key.as_deref()
    }
    pub(crate) fn limit(&self) -> usize {
        self.limit
    }
    pub(crate) fn empty_updated(&self) -> Option<jiff::Timestamp> {
        self.empty_updated
    }
    pub(crate) fn empty_authors(&self) -> Option<&[AuthorName]> {
        self.empty_authors.as_deref()
    }
    pub(crate) fn origin(
        &self,
        span: SourceSpan,
    ) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.source.as_ref().map(|source| (source.clone(), span))
    }
    pub(crate) fn declaration_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.declaration_span)
    }
    pub(crate) fn path_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.path_span)
    }
    pub(crate) fn title_key_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.origin(self.title_key_span)
    }
    pub(crate) fn subtitle_key_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.subtitle_key_span.and_then(|span| self.origin(span))
    }
    pub(crate) fn empty_authors_origin(&self) -> Option<(Arc<NamedSource<String>>, SourceSpan)> {
        self.empty_authors_span.and_then(|span| self.origin(span))
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
    sitemap_include: bool,
    counterparts: BTreeMap<String, TermCounterpartGroup>,
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
    pub(crate) fn sitemap_include(&self) -> bool {
        self.sitemap_include
    }
    pub(crate) fn counterparts(&self) -> &BTreeMap<String, TermCounterpartGroup> {
        &self.counterparts
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
/// Heine uses the group name only to connect locale members. It validates those
/// members later against the resolved locale-local taxonomy terms.
#[derive(Debug, Clone)]
pub(crate) struct TermCounterpartGroup {
    terms: BTreeMap<Locale, TermCounterpart>,
}

impl TermCounterpartGroup {
    pub(crate) fn terms(&self) -> &BTreeMap<Locale, TermCounterpart> {
        &self.terms
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TermCounterpart {
    term: String,
    span: SourceSpan,
}

impl TermCounterpart {
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
    PublishedAsc,
    PublishedDesc,
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

/// Site-owned estimate policy. It is deliberately separate from locale
/// identity, because a reading rate is not an intrinsic property of a locale.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReadingTimeConfig {
    words_per_minute: BTreeMap<Locale, NonZeroU64>,
}

impl ReadingTimeConfig {
    pub(crate) fn words_per_minute(&self, locale: &Locale) -> Option<NonZeroU64> {
        self.words_per_minute.get(locale).copied()
    }
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
    origin: Option<SiteOrigin>,
    time_zone: Option<EditorialTimeZone>,
}

impl Default for SiteSettings {
    fn default() -> Self {
        Self {
            base_path: BasePath::root(),
            origin: None,
            time_zone: None,
        }
    }
}

impl SiteSettings {
    pub(crate) fn base_path(&self) -> &BasePath {
        &self.base_path
    }
    pub(crate) fn canonical_urls(&self) -> Option<CanonicalUrls> {
        self.origin
            .clone()
            .map(|origin| CanonicalUrls::new(origin, self.base_path.clone()))
    }
    pub(crate) fn time_zone(&self) -> Option<&EditorialTimeZone> {
        self.time_zone.as_ref()
    }
}

/// Enables structural generation of canonical sitemap resources.
#[derive(Debug, Clone)]
pub(crate) struct SitemapConfig {
    hreflang: bool,
    include_pagers: bool,
    span: SourceSpan,
    source: Option<Arc<NamedSource<String>>>,
}

impl SitemapConfig {
    pub(crate) fn hreflang(&self) -> bool {
        self.hreflang
    }

    pub(crate) fn include_pagers(&self) -> bool {
        self.include_pagers
    }

    pub(crate) fn output_claim(&self, path: OutputPath) -> Option<OutputClaim> {
        self.source
            .as_ref()
            .map(|source| OutputClaim::authored(path, "sitemap", source.clone(), self.span))
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
    tables: Option<TablesConfig>,
    margin_footnotes: Option<MarginFootnotesConfig>,
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

    pub(crate) fn tables(&self) -> Option<&TablesConfig> {
        self.tables.as_ref()
    }

    pub(crate) fn margin_footnotes(&self) -> Option<&MarginFootnotesConfig> {
        self.margin_footnotes.as_ref()
    }
}

/// Global localization for the optional responsive Markdown-table renderer.
#[derive(Debug, Clone)]
pub(crate) struct TablesConfig {
    scroll_message: MarkdownMessageConfig,
}

impl TablesConfig {
    pub(crate) fn scroll_message(&self) -> &str {
        self.scroll_message.value()
    }

    pub(crate) fn scroll_message_origin(&self) -> Option<(&NamedSource<String>, SourceSpan)> {
        self.scroll_message.origin()
    }

    fn scroll_message_span(&self) -> SourceSpan {
        self.scroll_message.span
    }

    fn attach_source(&mut self, source: NamedSource<String>) {
        self.scroll_message.attach_source(source);
    }
}

/// Global localization required by every effective margin-footnote setting.
/// Page and directory policy decide whether the feature is used at all.
#[derive(Debug, Clone)]
pub(crate) struct MarginFootnotesConfig {
    marker_message: MarkdownMessageConfig,
}

impl MarginFootnotesConfig {
    pub(crate) fn marker_message(&self) -> &str {
        self.marker_message.value()
    }

    pub(crate) fn marker_message_origin(&self) -> Option<(&NamedSource<String>, SourceSpan)> {
        self.marker_message.origin()
    }

    fn attach_source(&mut self, source: NamedSource<String>) {
        self.marker_message.attach_source(source);
    }
}

/// A message identifier keeps its authored span until locale validation can
/// report both configuration and Fluent locations together.
#[derive(Debug, Clone)]
struct MarkdownMessageConfig {
    value: String,
    span: SourceSpan,
    source: Option<NamedSource<String>>,
}

impl MarkdownMessageConfig {
    fn parse(value: toml::Spanned<String>, field: &'static str) -> Result<Self, ConfigValueError> {
        let span = value.span();
        let value = value.into_inner();
        if value.trim().is_empty() {
            return Err(ConfigValueError {
                span: span.into(),
                message: format!("{field} must not be empty"),
            });
        }
        Ok(Self {
            value,
            span: span.into(),
            source: None,
        })
    }

    fn value(&self) -> &str {
        &self.value
    }

    fn origin(&self) -> Option<(&NamedSource<String>, SourceSpan)> {
        self.source.as_ref().map(|source| (source, self.span))
    }

    fn attach_source(&mut self, source: NamedSource<String>) {
        self.source = Some(source);
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
                Arc::new(source_code.clone()),
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

impl SiteConfig {
    fn attach_kit_extra(&mut self, source: &str) -> Result<(), ConfigValueError> {
        let Some(kit) = &mut self.kit else {
            return Ok(());
        };
        let extra = kit::parse_selection_extra(source).map_err(|error| ConfigValueError {
            span: error.span,
            message: error.message,
        })?;
        kit.set_extra(extra);
        Ok(())
    }

    fn attach_source(
        &mut self,
        path: &Path,
        source: &str,
        diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    ) {
        if let Some(margin_footnotes) = &mut self.markdown.margin_footnotes {
            margin_footnotes.attach_source(diagnostics.named_source(path, source, "TOML"));
        }
        if let Some(tables) = &mut self.markdown.tables {
            tables.attach_source(diagnostics.named_source(path, source, "TOML"));
        }
        if let Some(highlighting) = &mut self.markdown.highlighting {
            highlighting.stylesheet_source = Some(diagnostics.named_source(path, source, "TOML"));
        }
        let licensing_source = Arc::new(diagnostics.named_source(path, source, "TOML"));
        self.licensing.attach_source(licensing_source.clone());
        self.renderers.attach_source(licensing_source);
        let collection_source = Arc::new(diagnostics.named_source(path, source, "TOML"));
        for definition in self.collections.definitions.values_mut() {
            definition.source = Some(collection_source.clone());
        }
        for definition in self.taxonomies.definitions.values_mut() {
            definition.source = Some(collection_source.clone());
        }
        for definition in self.feeds.definitions.values_mut() {
            definition.source = Some(collection_source.clone());
        }
        for definition in self.search.definitions.values_mut() {
            definition.source = Some(collection_source.clone());
        }
        if let Some(series) = &mut self.series {
            series.source = Some(collection_source.clone());
            for definition in series.definitions.values_mut() {
                definition.source = Some(collection_source.clone());
            }
        }
        if let Some(sitemap) = &mut self.sitemap {
            sitemap.source = Some(collection_source);
        }
        if let Some(kit) = &mut self.kit {
            kit.attach_source(diagnostics.named_source(path, source, "TOML"));
        }
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

    #[test]
    fn renderer_profiles_accept_a_checked_stream_configuration() {
        let raw: RawSiteConfig = toml::from_str(
            r#"
[i18n]
default_locale = "en"
[i18n.locales.en]
direction = "ltr"

[renderers.katex]
protocol = "stream"
program = "katex"
timeout_seconds = 10
cache_identity = "katex-0.16.27"
inputs = ["body", "file"]
result = "html"
[renderers.katex.inline]
arguments = ["--format", "htmlAndMathml"]
"#,
        )
        .expect("renderer configuration parses as TOML");

        SiteConfig::try_from(raw).expect("renderer configuration is valid");
    }

    #[test]
    fn renderer_workspace_profiles_require_an_output_placeholder() {
        let raw: RawSiteConfig = toml::from_str(
            r#"
[i18n]
default_locale = "en"
[i18n.locales.en]
direction = "ltr"

[renderers.report]
protocol = "workspace"
program = "typst"
timeout_seconds = 30
cache_identity = "typst-0.15.0"
inputs = ["file"]
result = "link"
media_type = "application/pdf"
extension = "pdf"
[renderers.report.block]
arguments = ["compile", "{input_file}"]
"#,
        )
        .expect("renderer configuration parses as TOML");

        assert!(
            SiteConfig::try_from(raw)
                .expect_err("workspace output placeholder is required")
                .to_string()
                .contains("{output_file} exactly once")
        );
    }

    #[test]
    fn renderer_svg_profiles_require_a_default_for_two_materializations() {
        let raw: RawSiteConfig = toml::from_str(
            r#"
[i18n]
default_locale = "en"
[i18n.locales.en]
direction = "ltr"

[renderers.diagram]
protocol = "stream"
program = "plantuml"
timeout_seconds = 20
cache_identity = "plantuml-1.2025.7"
inputs = ["body"]
result = "svg"
materializations = ["embedded", "resource"]
[renderers.diagram.block]
arguments = ["--svg"]
"#,
        )
        .expect("renderer configuration parses as TOML");

        assert!(
            SiteConfig::try_from(raw)
                .expect_err("two SVG materializations need a default")
                .to_string()
                .contains("must declare default_output")
        );
    }
}
