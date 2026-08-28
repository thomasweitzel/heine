//! Raw and validated page representations, plus the template-facing page view.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};

use jiff::Timestamp;
use miette::{NamedSource, SourceSpan};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::{
    config::{CollectionName, TaxonomyName},
    content::MarkdownSource,
    output::{OutputPath, UrlPath},
    source_path::LogicalPath,
    template::TemplateName,
};

/// A stable logical page identifier, always relative to one locale tree without `.page`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PageId(LogicalPath);

impl PageId {
    pub(crate) fn from_logical_path(path: LogicalPath) -> Self {
        Self(path)
    }

    /// Parses a page ID supplied at a template boundary. Source discovery
    /// constructs page IDs directly from validated [`LogicalPath`] values.
    pub(crate) fn parse(value: &str) -> Option<Self> {
        LogicalPath::parse(value).map(Self)
    }

    pub(super) fn is_index(&self) -> bool {
        self.0.as_str() == "index" || self.0.as_str().ends_with("/index")
    }
}

/// An explicit, nonblank identity relating corresponding pages across
/// locales. Its value is intentionally otherwise opaque to Heine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct TranslationId(String);

impl TranslationId {
    pub(super) fn parse(value: String) -> Result<Self, String> {
        if value.trim().is_empty() {
            Err("field `translationid` must not be empty".into())
        } else {
            Ok(Self(value))
        }
    }
}

impl std::fmt::Display for TranslationId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::fmt::Display for PageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.as_str().fmt(f)
    }
}

impl Serialize for PageId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

#[derive(Debug, Clone)]
pub(super) struct Task {
    pub(super) locale: crate::locale::Locale,
    pub(super) page_id: PageId,
    pub(super) template: TemplateName,
    pub(super) page_source: Arc<NamedSource<String>>,
    pub(super) template_source: Arc<NamedSource<String>>,
    pub(super) template_span: SourceSpan,
    pub(super) output_span: SourceSpan,
    pub(super) output_path: OutputPath,
    pub(super) latest_input: SystemTime,
    pub(super) pagination: Option<Arc<Pager>>,
    pub(super) sitemap_include: bool,
    pub(super) sitemap_lastmod: Option<PublicationTime>,
}

/// One fully resolved generated view of an authored paginated listing.
///
/// It deliberately carries page IDs rather than page views: `PageCatalog`
/// remains the sole owner of render-facing page facts.
#[derive(Debug, Clone)]
pub(crate) struct Pager {
    pub(super) collection: String,
    pub(super) page_ids: Vec<PageId>,
    pub(super) page_number: usize,
    pub(super) page_count: usize,
    pub(super) item_count: usize,
    pub(super) per_page: usize,
    pub(super) navigation_window: usize,
    pub(super) paths: Arc<Vec<OutputPath>>,
}

/// A page resolved from its serialized form, retaining the source inputs used
/// to classify the remainder of `content/` as publishable assets.
pub(super) struct LoadedPage {
    pub(super) page: Page,
    pub(super) task: Option<Task>,
    pub(super) content_inputs: BTreeSet<PathBuf>,
    pub(super) markdown_sources: Vec<MarkdownSource>,
}

/// The TOML shape written by site authors. This is intentionally separate
/// from `Page`, whose fields represent only validated application state.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPage {
    pub(super) template: Option<toml::Spanned<toml::Value>>,
    pub(super) title: Option<toml::Spanned<String>>,
    pub(super) description: Option<toml::Spanned<String>>,
    pub(super) summary: Option<toml::Spanned<String>>,
    pub(super) translationid: Option<toml::Spanned<String>>,
    #[serde(rename = "date")]
    pub(super) legacy_date: Option<toml::Spanned<toml::Value>>,
    pub(super) published: Option<toml::Spanned<toml::Value>>,
    pub(super) updated: Option<toml::Spanned<toml::Value>>,
    pub(super) sitemap: Option<toml::Spanned<bool>>,
    pub(super) extra: Option<toml::Spanned<toml::Value>>,
    #[serde(default)]
    pub(super) content: BTreeMap<String, toml::Spanned<String>>,
    #[serde(default)]
    pub(super) taxonomies: BTreeMap<toml::Spanned<String>, Vec<toml::Spanned<String>>>,
    pub(super) collections: Option<Vec<toml::Spanned<String>>>,
    pub(super) weight: Option<toml::Spanned<toml::Value>>,
    pub(super) pagination: Option<RawPagination>,
    #[serde(default = "default_suffix")]
    pub(super) suffix: toml::Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPagination {
    pub(super) collection: toml::Spanned<String>,
    pub(super) per_page: toml::Spanned<usize>,
    #[serde(default = "default_pagination_segment")]
    pub(super) segment: toml::Spanned<String>,
    #[serde(default)]
    pub(super) navigation: RawPaginationNavigation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPaginationNavigation {
    #[serde(default = "default_navigation_window")]
    pub(super) window: toml::Spanned<usize>,
}

impl Default for RawPaginationNavigation {
    fn default() -> Self {
        Self {
            window: default_navigation_window(),
        }
    }
}

fn default_pagination_segment() -> toml::Spanned<String> {
    toml::Spanned::new(0..0, "page".into())
}

fn default_navigation_window() -> toml::Spanned<usize> {
    toml::Spanned::new(0..0, 1)
}

/// The common authored metadata shape for pages and directory descriptors.
#[derive(Debug, Default)]
pub(super) struct RawMetadata {
    pub(super) title: Option<toml::Spanned<String>>,
    pub(super) description: Option<toml::Spanned<String>>,
    pub(super) summary: Option<toml::Spanned<String>>,
    pub(super) published: Option<toml::Spanned<toml::Value>>,
    pub(super) updated: Option<toml::Spanned<toml::Value>>,
    pub(super) extra: Option<toml::Spanned<toml::Value>>,
}

/// A resolved instant written as a local civil time in the configured editorial
/// time zone. The local-time ambiguity check happens exactly once at loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PublicationTime(Timestamp);

impl PublicationTime {
    pub(super) fn new(value: Timestamp) -> Self {
        Self(value)
    }
    pub(super) fn render(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Metadata {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) published: Option<PublicationTime>,
    pub(super) updated: Option<PublicationTime>,
    pub(super) updated_span: Option<SourceSpan>,
    pub(super) extra: Value,
}

impl Metadata {
    pub(super) fn field(&self, name: &str) -> bool {
        match name {
            "title" => self.title.is_some(),
            "description" => self.description.is_some(),
            "summary" => self.summary.is_some(),
            "published" => self.published.is_some(),
            "updated" => self.updated.is_some(),
            _ => name
                .strip_prefix("extra.")
                .and_then(|path| {
                    path.split('.')
                        .try_fold(&self.extra, |value, part| value.get(part))
                })
                .is_some(),
        }
    }
}

fn default_suffix() -> toml::Spanned<String> {
    toml::Spanned::new(0..0, ".html".into())
}

/// A single filename extension used to derive a rendered page path.
///
/// It deliberately cannot carry path syntax: rendered output locations are
/// derived only from a page's validated source-relative path.
#[derive(Debug, Clone)]
pub(super) struct OutputSuffix(String);

impl OutputSuffix {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OutputSuffix {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let extension = value.trim_start_matches('.');
        if extension.is_empty()
            || extension.contains(['/', '\\'])
            || extension.contains('\0')
            || extension == "."
            || extension == ".."
        {
            return Err(
                "field `suffix` must be a non-empty filename extension without path separators"
                    .into(),
            );
        }
        Ok(Self(extension.into()))
    }
}

/// Validated page state. It deliberately has no template-engine traits.
#[derive(Debug, Clone)]
pub(super) struct Page {
    pub(super) id: PageId,
    pub(super) output_path: Option<OutputPath>,
    pub(super) path: Option<UrlPath>,
    pub(super) dir: String,
    pub(super) filename: Option<String>,
    pub(super) subdirs: Vec<String>,
    pub(super) content: BTreeMap<String, Value>,
    pub(super) metadata: Metadata,
    pub(super) translationid: Option<TranslationId>,
    pub(super) translationid_span: Option<SourceSpan>,
    pub(super) source: NamedSource<String>,
    pub(super) directory: DirectoryView,
    pub(super) taxonomies: Vec<TaxonomyMembership>,
    pub(super) collections: Vec<CollectionMembership>,
    pub(super) pagination: Option<Pagination>,
    pub(super) weight: Option<i64>,
    pub(super) weight_span: Option<SourceSpan>,
    pub(super) lighter: Option<PageId>,
    pub(super) heavier: Option<PageId>,
    pub(super) translations: BTreeMap<String, PageLink>,
}

/// Validated pagination declared by one authored listing page. Generated
/// pager tasks are resolved later, after collection membership is stable.
#[derive(Debug, Clone)]
pub(super) struct Pagination {
    pub(super) collection: CollectionName,
    pub(super) per_page: usize,
    pub(super) segment: PaginationSegment,
    pub(super) navigation_window: usize,
    /// The authored value responsible for generated pager routes. A default
    /// segment has no source range, so it falls back to the collection value
    /// that declares the pagination relationship.
    pub(super) route_span: SourceSpan,
}

/// A generated pager route component that is simultaneously a canonical URL
/// component and a portable output-directory name.
#[derive(Debug, Clone)]
pub(super) struct PaginationSegment(String);

impl PaginationSegment {
    pub(super) fn parse(value: String) -> Result<Self, String> {
        if value.is_empty()
            || matches!(value.as_str(), "." | "..")
            || value.contains('%')
            || !crate::config::valid_url_segment(&value)
            || !crate::output::is_safe_output_component(&value)
        {
            return Err(
                "pagination.segment must use a literal portable URL-path component: ASCII letters or digits, or -._~!$&'()*+,;=@; it cannot be '.', '..', end in '.', or contain percent escapes".into(),
            );
        }
        Ok(Self(value))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

/// An authored collection membership, retaining its declaration for
/// relationship diagnostics after page loading has finished.
#[derive(Debug, Clone)]
pub(super) struct CollectionMembership {
    pub(super) name: CollectionName,
    pub(super) source: Arc<NamedSource<String>>,
    pub(super) span: SourceSpan,
}

/// One literal term assignment, retaining its source range for taxonomy
/// relationship diagnostics.
#[derive(Debug, Clone)]
pub(super) struct TaxonomyMembership {
    pub(super) name: TaxonomyName,
    pub(super) terms: Vec<TaxonomyTermMembership>,
}

#[derive(Debug, Clone)]
pub(super) struct TaxonomyTermMembership {
    pub(super) term: String,
    pub(super) source: Arc<NamedSource<String>>,
    pub(super) span: SourceSpan,
}

/// A resolved counterpart deliberately exposed through `page.translations`.
#[derive(Debug, Clone, Serialize)]
pub(super) struct PageLink {
    pub(super) id: PageId,
    pub(super) path: String,
}

impl Page {
    /// Drops loading-only state after all site relationships are resolved.
    /// The resulting value is the sole page representation retained while
    /// templates render.
    pub(super) fn into_view(self) -> PageView {
        let Metadata {
            title,
            description,
            summary,
            published,
            updated,
            extra,
            ..
        } = self.metadata;
        PageView {
            id: self.id,
            path: self.path.map(|path| path.as_str().into()),
            dir: self.dir,
            filename: self.filename,
            subdirs: self.subdirs,
            content: self.content,
            title,
            description,
            summary,
            published: published.as_ref().map(PublicationTime::render),
            updated: updated.as_ref().map(PublicationTime::render),
            extra,
            translations: self.translations,
            directory: self.directory,
            weight: self.weight,
            lighter: self.lighter,
            heavier: self.heavier,
        }
    }
}

/// Deliberate, serializable template contract derived from a validated page.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PageView {
    id: PageId,
    path: Option<String>,
    dir: String,
    filename: Option<String>,
    subdirs: Vec<String>,
    content: BTreeMap<String, Value>,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    published: Option<String>,
    updated: Option<String>,
    extra: Value,
    translations: BTreeMap<String, PageLink>,
    directory: DirectoryView,
    weight: Option<i64>,
    lighter: Option<PageId>,
    heavier: Option<PageId>,
}

impl PageView {
    pub(crate) fn directory(&self) -> &str {
        &self.dir
    }
}

/// The immediate containing directory's deliberately small template view.
#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct DirectoryView {
    id: String,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    extra: Value,
}

impl DirectoryView {
    pub(super) fn new(id: String, metadata: &Metadata) -> Self {
        Self {
            id,
            title: metadata.title.clone(),
            description: metadata.description.clone(),
            summary: metadata.summary.clone(),
            extra: metadata.extra.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_index_page_names_receive_index_ordering() {
        assert!(PageId::parse("index").unwrap().is_index());
        assert!(PageId::parse("blog/index").unwrap().is_index());
        assert!(!PageId::parse("thinking").unwrap().is_index());
        assert!(!PageId::parse("myindex").unwrap().is_index());
    }

    #[test]
    fn page_names_accept_only_logical_paths() {
        assert_eq!(PageId::parse("blog/post").unwrap().to_string(), "blog/post");
        assert!(PageId::parse("../post").is_none());
    }

    #[test]
    fn pagination_segments_intersect_url_and_output_component_rules() {
        assert!(PaginationSegment::parse("older-posts".into()).is_ok());
        assert!(PaginationSegment::parse("older%20posts".into()).is_err());
        assert!(PaginationSegment::parse("older:posts".into()).is_err());
        assert!(PaginationSegment::parse("older.".into()).is_err());
    }
}
