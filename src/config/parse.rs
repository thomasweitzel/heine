//! TOML-shaped configuration and conversion into the checked site model.
//!
//! This module is the boundary between deserialized input and the configuration
//! that the rest of the build can trust. The parent module owns that checked
//! model and attaches source text for later diagnostics.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    path::PathBuf,
};

use miette::SourceSpan;
use serde::Deserialize;

use crate::{
    author::AuthorName,
    kit,
    locale::{Locale, TextDirection},
    output::OutputPath,
    project, renderer, source_path,
    template::TemplateName,
};

use super::licensing::{self, RawLicensingConfig};
use super::{
    AssetConfig, BuildConfig, CollectionDefinition, CollectionName, CollectionOrder,
    CollectionsConfig, EditorialTimeZone, FeedDefinition, FeedName, FeedsConfig,
    HighlightingConfig, I18nConfig, IntegrityAlgorithm, LocaleSettings, MarginFootnotesConfig,
    MarkdownConfig, MarkdownExtension, MarkdownMessageConfig, MathMode, MissingLanguage,
    OutputDirectory, ReadingTimeConfig, SearchConfig, SearchDefinition, SearchName, SeriesConfig,
    SeriesDefinition, SeriesName, SiteConfig, SiteSettings, SitemapConfig, TablesConfig,
    TaxonomiesConfig, TaxonomyDefinition, TaxonomyName, TermCounterpart, TermCounterpartGroup,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSiteConfig {
    #[serde(default)]
    site: RawSiteSettings,
    #[serde(default)]
    build: RawBuildConfig,
    #[serde(default)]
    assets: RawAssetConfig,
    #[serde(default)]
    licensing: RawLicensingConfig,
    #[serde(default)]
    markdown: RawMarkdownConfig,
    reading_time: Option<toml::Spanned<RawReadingTimeConfig>>,
    i18n: RawI18nConfig,
    #[serde(default)]
    collections: BTreeMap<toml::Spanned<String>, RawCollectionDefinition>,
    #[serde(default)]
    taxonomies: BTreeMap<toml::Spanned<String>, RawTaxonomyDefinition>,
    series: Option<toml::Spanned<RawSeriesConfig>>,
    sitemap: Option<toml::Spanned<RawSitemapConfig>>,
    #[serde(default)]
    feeds: BTreeMap<toml::Spanned<String>, RawFeedDefinition>,
    #[serde(default)]
    search: BTreeMap<toml::Spanned<String>, RawSearchDefinition>,
    publications: Option<super::PublicationsConfig>,
    #[serde(default)]
    renderers: BTreeMap<toml::Spanned<String>, renderer::RawProfile>,
    kit: Option<RawKitSelection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKitSelection {
    name: toml::Spanned<String>,
    version: Option<toml::Spanned<String>>,
    #[serde(default, rename = "extra")]
    _extra: Option<toml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReadingTimeConfig {
    words_per_minute: BTreeMap<toml::Spanned<String>, toml::Spanned<u64>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSitemapConfig {
    hreflang: Option<toml::Spanned<bool>>,
    include_pagers: Option<toml::Spanned<bool>>,
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
    sitemap: Option<toml::Spanned<bool>>,
    #[serde(default)]
    counterparts:
        BTreeMap<toml::Spanned<String>, BTreeMap<toml::Spanned<String>, toml::Spanned<String>>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeriesConfig {
    path: toml::Spanned<String>,
    index_template: toml::Spanned<String>,
    listing_template: toml::Spanned<String>,
    index_title_key: toml::Spanned<String>,
    index_description_key: Option<toml::Spanned<String>>,
    per_page: toml::Spanned<usize>,
    sitemap: Option<toml::Spanned<bool>>,
    definitions: BTreeMap<toml::Spanned<String>, RawSeriesDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeriesDefinition {
    title_key: toml::Spanned<String>,
    description_key: Option<toml::Spanned<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFeedDefinition {
    collection: toml::Spanned<String>,
    path: toml::Spanned<String>,
    title_key: toml::Spanned<String>,
    subtitle_key: Option<toml::Spanned<String>>,
    #[serde(default = "default_feed_limit")]
    limit: toml::Spanned<usize>,
    empty_updated: Option<toml::Spanned<toml::Value>>,
    empty_authors: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSearchDefinition {
    collections: toml::Spanned<Vec<toml::Spanned<String>>>,
    path: toml::Spanned<String>,
    #[serde(default = "default_search_versioned")]
    versioned: toml::Spanned<bool>,
}

fn default_search_versioned() -> toml::Spanned<bool> {
    toml::Spanned::new(0..0, true)
}

fn default_feed_limit() -> toml::Spanned<usize> {
    toml::Spanned::new(0..0, 20)
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
    origin: Option<toml::Spanned<String>>,
    time_zone: Option<toml::Spanned<String>>,
}

impl Default for RawSiteSettings {
    fn default() -> Self {
        Self {
            base_path: default_base_path(),
            origin: None,
            time_zone: None,
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
    tables: Option<RawTablesConfig>,
    margin_footnotes: Option<RawMarginFootnotesConfig>,
}

impl Default for RawMarkdownConfig {
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
            math: MathMode::default(),
            highlighting: None,
            tables: None,
            margin_footnotes: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMarginFootnotesConfig {
    marker_message: toml::Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTablesConfig {
    scroll_message: toml::Spanned<String>,
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
        if let Some(publications) = &raw.publications {
            if publications.bibliography.get_ref().is_empty() {
                return Err(ConfigValueError {
                    span: publications.bibliography.span().into(),
                    message: "publications.bibliography must contain at least one file".into(),
                });
            }
            let mut seen = BTreeSet::new();
            for field in &publications.extra_fields {
                let name = field.get_ref();
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    || !seen.insert(name)
                {
                    return Err(ConfigValueError {
                        span: field.span().into(),
                        message:
                            "publication extra fields must be unique lowercase ASCII field names"
                                .into(),
                    });
                }
            }
        }
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
                    "i18n.default_locale must use canonical BCP-47 spelling {default_locale:?}"
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
                    span: value_span.into(),
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
                span: default_locale_span.into(),
                message: "i18n.locales must declare at least one locale table".into(),
            });
        }
        if !locales.contains_key(&default_locale) {
            return Err(ConfigValueError {
                span: default_locale_span.into(),
                message: "i18n.locales must include i18n.default_locale".into(),
            });
        }
        let reading_time = parse_reading_time(raw.reading_time, &locales)?;
        let licensing = licensing::parse(raw.licensing)?;
        let renderers =
            renderer::parse(raw.renderers, &licensing.record_names()).map_err(|error| {
                ConfigValueError {
                    span: error.span,
                    message: error.message,
                }
            })?;
        let kit = raw
            .kit
            .map(|raw| {
                let RawKitSelection {
                    name,
                    version,
                    _extra: _,
                } = raw;
                kit::Selection::parse(name, version)
            })
            .transpose()
            .map_err(|error| ConfigValueError {
                span: error.span,
                message: error.message,
            })?;
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
        let origin = raw
            .site
            .origin
            .map(|value| {
                let span = value.span();
                value
                    .into_inner()
                    .try_into()
                    .map_err(|message| ConfigValueError {
                        span: span.into(),
                        message,
                    })
            })
            .transpose()?;
        let time_zone = raw
            .site
            .time_zone
            .map(|value| {
                let span = value.span();
                value
                    .into_inner()
                    .try_into()
                    .map_err(|message| ConfigValueError {
                        span: span.into(),
                        message,
                    })
            })
            .transpose()?;
        let sitemap = raw.sitemap.map(|value| {
                let span: SourceSpan = value.span().into();
                if origin.is_none() {
                    return Err(ConfigValueError {
                        span,
                        message:
                            "[sitemap] requires site.origin so Heine can generate canonical URLs"
                                .into(),
                    });
                }
                let RawSitemapConfig {
                    hreflang,
                    include_pagers,
                } = value.into_inner();
                let hreflang = match hreflang {
                    None => false,
                    Some(value) => {
                        let value_span: SourceSpan = value.span().into();
                        if value.into_inner() {
                            true
                        } else {
                            return Err(ConfigValueError {
                                span: value_span,
                                message: "sitemap.hreflang accepts only true; omit it to disable sitemap alternates".into(),
                            });
                        }
                    }
                };
                let include_pagers = match include_pagers {
                    None => true,
                    Some(value) => {
                        let value_span: SourceSpan = value.span().into();
                        if value.into_inner() {
                            return Err(ConfigValueError {
                                span: value_span,
                                message: "sitemap.include_pagers accepts only false; omit it to include derived pager pages".into(),
                            });
                        }
                        false
                    }
                };
                Ok(SitemapConfig {
                    hreflang,
                    include_pagers,
                    span,
                    source: None,
                })
            })
            .transpose()?;
        if origin.is_none()
            && let Some((name, _)) = raw.feeds.first_key_value()
        {
            return Err(ConfigValueError {
                span: name.span().into(),
                message: format!(
                    "feed {:?} requires site.origin so Heine can generate canonical Atom IDs and links",
                    name.get_ref()
                ),
            });
        }
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
                message: "build.output_dir must not be content/, citations/, data/, deploy/, templates/, locales/, or LICENSES/, or a directory beneath an input directory".into(),
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
        let tables_enabled = extensions.contains(&MarkdownExtension::Tables);
        let tables = raw
            .markdown
            .tables
            .map(|raw| {
                MarkdownMessageConfig::parse(raw.scroll_message, "markdown.tables.scroll_message")
                    .map(|scroll_message| TablesConfig { scroll_message })
            })
            .transpose()?;
        if !tables_enabled && let Some(config) = &tables {
            return Err(ConfigValueError {
                span: config.scroll_message_span(),
                message: "markdown.tables requires markdown.extensions to include \"tables\""
                    .into(),
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
        let margin_footnotes = raw
            .markdown
            .margin_footnotes
            .map(|raw| {
                MarkdownMessageConfig::parse(
                    raw.marker_message,
                    "markdown.margin_footnotes.marker_message",
                )
                .map(|marker_message| MarginFootnotesConfig { marker_message })
            })
            .transpose()?;
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
        let mut feeds = BTreeMap::new();
        for (name, definition) in raw.feeds {
            let name_span = name.span();
            let declaration_span: SourceSpan = name_span.clone().into();
            let name = FeedName::parse(name.into_inner()).map_err(|message| ConfigValueError {
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
                        "feed {name} uses collection {collection_value:?}, which is not configured"
                    ),
                });
            }
            let path_span: SourceSpan = definition.path.span().into();
            let path_value = PathBuf::from(definition.path.into_inner());
            let path = OutputPath::try_new(path_value).map_err(|message| ConfigValueError {
                span: path_span,
                message: format!("feed {name} path {message}"),
            })?;
            if crate::output::is_reserved_output_path(path.as_path()) {
                return Err(ConfigValueError {
                    span: path_span,
                    message: format!(
                        "feed {name} path must not use Heine's reserved __heine/ namespace"
                    ),
                });
            }
            let title_key_span: SourceSpan = definition.title_key.span().into();
            let title_key = required_message_key(
                definition.title_key.into_inner(),
                "title_key",
                &name,
                title_key_span,
            )?;
            let (subtitle_key, subtitle_key_span) = definition
                .subtitle_key
                .map(|value| {
                    let span: SourceSpan = value.span().into();
                    required_message_key(value.into_inner(), "subtitle_key", &name, span)
                        .map(|key| (key, span))
                })
                .transpose()?
                .map_or((None, None), |(key, span)| (Some(key), Some(span)));
            let limit_span: SourceSpan = definition.limit.span().into();
            let limit = definition.limit.into_inner();
            if limit == 0 {
                return Err(ConfigValueError {
                    span: limit_span,
                    message: format!("feed {name} limit must be a positive integer"),
                });
            }
            let empty_updated = definition
                .empty_updated
                .map(|value| {
                    let span: SourceSpan = value.span().into();
                    parse_feed_empty_updated(value.into_inner(), time_zone.as_ref(), span, &name)
                })
                .transpose()?;
            let empty_authors_span = definition
                .empty_authors
                .as_ref()
                .map(|authors| authors.span().into());
            let empty_authors = definition
                .empty_authors
                .map(|authors| parse_feed_authors(authors, &name))
                .transpose()?;
            feeds.insert(
                name,
                FeedDefinition {
                    collection,
                    path,
                    title_key,
                    subtitle_key,
                    limit,
                    empty_updated,
                    empty_authors,
                    source: None,
                    declaration_span,
                    path_span,
                    title_key_span,
                    subtitle_key_span,
                    empty_authors_span,
                },
            );
        }
        let mut search = BTreeMap::new();
        for (name, definition) in raw.search {
            let name_span = name.span();
            let name =
                SearchName::parse(name.into_inner()).map_err(|message| ConfigValueError {
                    span: name_span.into(),
                    message,
                })?;
            let collections_span: SourceSpan = definition.collections.span().into();
            let collection_values = definition.collections.into_inner();
            if collection_values.is_empty() {
                return Err(ConfigValueError {
                    span: collections_span,
                    message: format!("search index {name} collections must not be empty"),
                });
            }
            let mut selected_collections = Vec::with_capacity(collection_values.len());
            let mut seen = BTreeSet::new();
            for collection in collection_values {
                let span: SourceSpan = collection.span().into();
                let value = collection.into_inner();
                let parsed = CollectionName::parse(value.clone())
                    .map_err(|message| ConfigValueError { span, message })?;
                if !collections.contains_key(&parsed) {
                    return Err(ConfigValueError {
                        span,
                        message: format!(
                            "search index {name} uses collection {value:?}, which is not configured"
                        ),
                    });
                }
                if !seen.insert(parsed.clone()) {
                    return Err(ConfigValueError {
                        span,
                        message: format!(
                            "search index {name} lists collection {value:?} more than once"
                        ),
                    });
                }
                selected_collections.push(parsed);
            }
            let path_span: SourceSpan = definition.path.span().into();
            let path_value = PathBuf::from(definition.path.into_inner());
            let path = OutputPath::try_new(path_value).map_err(|message| ConfigValueError {
                span: path_span,
                message: format!("search index {name} path {message}"),
            })?;
            if path
                .as_path()
                .extension()
                .is_none_or(|extension| extension != "json")
            {
                return Err(ConfigValueError {
                    span: path_span,
                    message: format!(
                        "search index {name} path must use the lowercase .json suffix"
                    ),
                });
            }
            if crate::output::is_reserved_output_path(path.as_path()) {
                return Err(ConfigValueError {
                    span: path_span,
                    message: format!(
                        "search index {name} path must not use Heine's reserved __heine/ namespace"
                    ),
                });
            }
            search.insert(
                name,
                SearchDefinition {
                    collections: selected_collections,
                    path,
                    versioned: definition.versioned.into_inner(),
                    source: None,
                    path_span,
                },
            );
        }
        let sitemap_enabled = sitemap.is_some();
        let series = raw
            .series
            .map(|raw| parse_series_config(raw.into_inner(), sitemap_enabled))
            .transpose()?;
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
            let sitemap_include = generated_sitemap_include(
                definition.sitemap,
                sitemap_enabled,
                &format!("taxonomy {name} sitemap"),
            )?;
            let mut counterparts = BTreeMap::new();
            for (group, members) in definition.counterparts {
                let group_span: SourceSpan = group.span().into();
                let group = group.into_inner();
                if group.trim().is_empty() {
                    return Err(ConfigValueError {
                        span: group_span,
                        message: format!(
                            "taxonomy {name} counterpart group names must not be empty"
                        ),
                    });
                }
                if members.len() < 2 {
                    return Err(ConfigValueError {
                        span: group_span,
                        message: format!(
                            "taxonomy {name} counterpart group {group:?} must declare at least two locales"
                        ),
                    });
                }
                let mut terms = BTreeMap::new();
                for (locale, term) in members {
                    let locale_span: SourceSpan = locale.span().into();
                    let locale_value = locale.into_inner();
                    let locale: Locale = locale_value.parse().map_err(|error| ConfigValueError {
                        span: locale_span,
                        message: format!("taxonomy {name} counterpart group {group:?} locale {locale_value:?} must be a valid BCP-47 locale: {error}"),
                    })?;
                    if locale.to_string() != locale_value {
                        return Err(ConfigValueError {
                            span: locale_span,
                            message: format!(
                                "taxonomy {name} counterpart group {group:?} locale {locale_value:?} must use canonical BCP-47 spelling {:?}",
                                locale.to_string()
                            ),
                        });
                    }
                    if !locales.contains_key(&locale) {
                        return Err(ConfigValueError {
                            span: locale_span,
                            message: format!(
                                "taxonomy {name} counterpart group {group:?} locale {locale:?} is not configured"
                            ),
                        });
                    }
                    let span: SourceSpan = term.span().into();
                    let term = term.into_inner();
                    if term.is_empty() {
                        return Err(ConfigValueError {
                            span,
                            message: format!(
                                "taxonomy {name} counterpart group {group:?} has an empty term for locale {locale}"
                            ),
                        });
                    }
                    if terms
                        .insert(locale.clone(), TermCounterpart { term, span })
                        .is_some()
                    {
                        return Err(ConfigValueError {
                            span: locale_span,
                            message: format!(
                                "taxonomy {name} counterpart group {group:?} declares locale {locale} more than once"
                            ),
                        });
                    }
                }
                counterparts.insert(group, TermCounterpartGroup { terms });
            }
            taxonomies.insert(
                name,
                TaxonomyDefinition {
                    collection,
                    path,
                    index_template,
                    term_template,
                    per_page,
                    sitemap_include,
                    counterparts,
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
            site: SiteSettings {
                base_path,
                origin,
                time_zone,
            },
            build: BuildConfig { output_dir },
            assets: AssetConfig {
                integrity: raw.assets.integrity,
            },
            licensing,
            markdown: MarkdownConfig {
                extensions,
                math: raw.markdown.math,
                highlighting,
                tables,
                margin_footnotes,
            },
            reading_time,
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
            series,
            sitemap,
            feeds: FeedsConfig { definitions: feeds },
            search: SearchConfig {
                definitions: search,
            },
            publications: raw.publications,
            renderers,
            kit,
        })
    }
}

fn parse_reading_time(
    raw: Option<toml::Spanned<RawReadingTimeConfig>>,
    locales: &BTreeMap<Locale, LocaleSettings>,
) -> Result<ReadingTimeConfig, ConfigValueError> {
    let Some(raw) = raw else {
        return Ok(ReadingTimeConfig::default());
    };
    let table_span: SourceSpan = raw.span().into();
    let mut rates = BTreeMap::new();
    for (locale, rate) in raw.into_inner().words_per_minute {
        let locale_span: SourceSpan = locale.span().into();
        let value = locale.into_inner();
        let parsed: Locale = value.parse().map_err(|error| ConfigValueError {
            span: locale_span,
            message: format!(
                "reading_time.words_per_minute key {value:?} must be a valid BCP-47 locale: {error}"
            ),
        })?;
        if parsed.to_string() != value {
            return Err(ConfigValueError {
                span: locale_span,
                message: format!(
                    "reading_time.words_per_minute key {value:?} must use canonical BCP-47 spelling {parsed:?}"
                ),
            });
        }
        if !locales.contains_key(&parsed) {
            return Err(ConfigValueError {
                span: locale_span,
                message: format!(
                    "reading_time.words_per_minute names locale {value:?}, which is not configured under i18n.locales"
                ),
            });
        }
        let rate_span: SourceSpan = rate.span().into();
        let rate = NonZeroU64::new(rate.into_inner()).ok_or_else(|| ConfigValueError {
            span: rate_span,
            message: "reading_time.words_per_minute values must be positive integers".into(),
        })?;
        if rates.insert(parsed.clone(), rate).is_some() {
            return Err(ConfigValueError {
                span: locale_span,
                message: format!(
                    "reading_time.words_per_minute declares locale {parsed:?} more than once"
                ),
            });
        }
    }
    if let Some(locale) = locales.keys().find(|locale| !rates.contains_key(*locale)) {
        return Err(ConfigValueError {
            span: table_span,
            message: format!(
                "reading_time.words_per_minute must define a positive rate for configured locale {locale}"
            ),
        });
    }
    Ok(ReadingTimeConfig {
        words_per_minute: rates,
    })
}

fn parse_series_config(
    raw: RawSeriesConfig,
    sitemap_enabled: bool,
) -> Result<SeriesConfig, ConfigValueError> {
    let sitemap_include =
        generated_sitemap_include(raw.sitemap, sitemap_enabled, "series sitemap")?;
    let path_span: SourceSpan = raw.path.span().into();
    let path = OutputPath::try_new(PathBuf::from(raw.path.into_inner())).map_err(|message| {
        ConfigValueError {
            span: path_span,
            message: format!("series path {message}"),
        }
    })?;
    if crate::output::is_reserved_output_path(path.as_path()) {
        return Err(ConfigValueError {
            span: path_span,
            message: "series path must not use Heine's reserved __heine/ namespace".into(),
        });
    }
    let index_template_span: SourceSpan = raw.index_template.span().into();
    let index_template =
        TemplateName::parse(raw.index_template.into_inner()).map_err(|message| {
            ConfigValueError {
                span: index_template_span,
                message: format!("series index_template {message}"),
            }
        })?;
    let listing_template_span: SourceSpan = raw.listing_template.span().into();
    let listing_template =
        TemplateName::parse(raw.listing_template.into_inner()).map_err(|message| {
            ConfigValueError {
                span: listing_template_span,
                message: format!("series listing_template {message}"),
            }
        })?;
    let index_title_key_span: SourceSpan = raw.index_title_key.span().into();
    let index_title_key = nonempty_series_key(
        raw.index_title_key.into_inner(),
        "index_title_key",
        index_title_key_span,
    )?;
    let (index_description_key, index_description_key_span) = raw
        .index_description_key
        .map(|value| {
            let span: SourceSpan = value.span().into();
            nonempty_series_key(value.into_inner(), "index_description_key", span)
                .map(|key| (key, span))
        })
        .transpose()?
        .map_or((None, None), |(key, span)| (Some(key), Some(span)));
    let per_page_span: SourceSpan = raw.per_page.span().into();
    let per_page = raw.per_page.into_inner();
    if per_page == 0 {
        return Err(ConfigValueError {
            span: per_page_span,
            message: "series per_page must be a positive integer".into(),
        });
    }
    let mut definitions = BTreeMap::new();
    for (name, definition) in raw.definitions {
        let name_span: SourceSpan = name.span().into();
        let name = SeriesName::parse(name.into_inner()).map_err(|message| ConfigValueError {
            span: name_span,
            message,
        })?;
        let title_key_span: SourceSpan = definition.title_key.span().into();
        let title_key = nonempty_series_key(
            definition.title_key.into_inner(),
            "title_key",
            title_key_span,
        )?;
        let (description_key, description_key_span) = definition
            .description_key
            .map(|value| {
                let span: SourceSpan = value.span().into();
                nonempty_series_key(value.into_inner(), "description_key", span)
                    .map(|key| (key, span))
            })
            .transpose()?
            .map_or((None, None), |(key, span)| (Some(key), Some(span)));
        definitions.insert(
            name,
            SeriesDefinition {
                title_key,
                description_key,
                source: None,
                name_span,
                title_key_span,
                description_key_span,
            },
        );
    }
    Ok(SeriesConfig {
        path,
        index_template,
        listing_template,
        index_title_key,
        index_description_key,
        per_page,
        sitemap_include,
        definitions,
        source: None,
        path_span,
        index_template_span,
        listing_template_span,
        index_title_key_span,
        index_description_key_span,
        per_page_span,
    })
}

fn generated_sitemap_include(
    value: Option<toml::Spanned<bool>>,
    sitemap_enabled: bool,
    field: &str,
) -> Result<bool, ConfigValueError> {
    let Some(value) = value else {
        return Ok(true);
    };
    let span: SourceSpan = value.span().into();
    if !sitemap_enabled {
        return Err(ConfigValueError {
            span,
            message: format!("{field} requires [sitemap] in heine.toml"),
        });
    }
    if value.into_inner() {
        return Err(ConfigValueError {
            span,
            message: format!(
                "{field} accepts only false; omit it to include its generated listing family"
            ),
        });
    }
    Ok(false)
}

fn nonempty_series_key(
    value: String,
    field: &str,
    span: SourceSpan,
) -> Result<String, ConfigValueError> {
    if value.trim().is_empty() {
        Err(ConfigValueError {
            span,
            message: format!("series {field} must name a non-empty Fluent message"),
        })
    } else {
        Ok(value)
    }
}

#[derive(Debug)]
pub(super) struct ConfigValueError {
    pub(super) span: SourceSpan,
    pub(super) message: String,
}

impl std::fmt::Display for ConfigValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

fn required_message_key(
    value: String,
    field: &str,
    feed: &FeedName,
    span: SourceSpan,
) -> Result<String, ConfigValueError> {
    if value.trim().is_empty() {
        Err(ConfigValueError {
            span,
            message: format!("feed {feed} {field} must name a non-empty Fluent message"),
        })
    } else {
        Ok(value)
    }
}

fn parse_feed_empty_updated(
    value: toml::Value,
    time_zone: Option<&EditorialTimeZone>,
    span: SourceSpan,
    feed: &FeedName,
) -> Result<jiff::Timestamp, ConfigValueError> {
    let toml::Value::Datetime(value) = value else {
        return Err(ConfigValueError {
            span,
            message: format!(
                "feed {feed} empty_updated must be a TOML local date-time without a UTC offset"
            ),
        });
    };
    if value.date.is_none() || value.time.is_none() || value.offset.is_some() {
        return Err(ConfigValueError {
            span,
            message: format!(
                "feed {feed} empty_updated must be a TOML local date-time without a UTC offset"
            ),
        });
    }
    let Some(time_zone) = time_zone else {
        return Err(ConfigValueError {
            span,
            message: format!(
                "feed {feed} empty_updated requires site.time_zone; add an IANA time-zone name under [site]"
            ),
        });
    };
    time_zone
        .resolve_local("empty_updated", &value)
        .map_err(|message| ConfigValueError { span, message })
}

fn parse_feed_authors(
    values: toml::Spanned<Vec<toml::Spanned<String>>>,
    feed: &FeedName,
) -> Result<Vec<AuthorName>, ConfigValueError> {
    let span = values.span().into();
    let values = values.into_inner();
    if values.is_empty() {
        return Err(ConfigValueError {
            span,
            message: format!("feed {feed} empty_authors must contain at least one name"),
        });
    }
    let mut authors = Vec::with_capacity(values.len());
    for value in values {
        let span: SourceSpan = value.span().into();
        authors.push(
            AuthorName::parse(value.into_inner())
                .map_err(|message| ConfigValueError { span, message })?,
        );
    }
    Ok(authors)
}
