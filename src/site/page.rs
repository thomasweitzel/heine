//! Raw and validated page representations, plus the template-facing page view.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    num::NonZeroU64,
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};

use jiff::Timestamp;
use miette::{NamedSource, SourceSpan};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::{
    asset::{AssetLicensing, AssetLinks},
    author::AuthorName,
    config::{CollectionName, SeriesName, TaxonomyName},
    content::MarkdownSource,
    markdown::{MarginFootnotePlan, Renderer as MarkdownRenderer, SourceAnalysisInputs, TocPlan},
    output::{OutputPath, UrlPath},
    site::PageLinks,
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

/// An explicit, nonblank identity relating corresponding pages across locales.
/// Heine uses its value only for that relationship.
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
    pub(super) navigation_window: NavigationWindow,
    pub(super) paths: Arc<Vec<OutputPath>>,
}

/// A page resolved from its serialized form, retaining the source inputs used
/// to classify the remainder of `content/` as publishable assets.
pub(super) struct LoadedPage {
    pub(super) page: Page,
    pub(super) task: Option<Task>,
    pub(super) content_inputs: BTreeSet<PathBuf>,
}

/// The TOML shape written by site authors. This is intentionally separate
/// from `Page`, whose fields represent only validated application state.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPage {
    // `PageProbe` validates draft syntax before full page loading. Keeping the
    // field here preserves this strict schema without repeating that work.
    #[serde(rename = "draft")]
    pub(super) _draft: Option<toml::Spanned<bool>>,
    pub(super) template: Option<toml::Spanned<toml::Value>>,
    pub(super) title: Option<toml::Spanned<String>>,
    pub(super) description: Option<toml::Spanned<String>>,
    pub(super) summary: Option<toml::Spanned<String>>,
    pub(super) authors: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
    pub(super) translationid: Option<toml::Spanned<String>>,
    #[serde(rename = "date")]
    pub(super) legacy_date: Option<toml::Spanned<toml::Value>>,
    pub(super) published: Option<toml::Spanned<toml::Value>>,
    pub(super) updated: Option<toml::Spanned<toml::Value>>,
    pub(super) sitemap: Option<toml::Spanned<bool>>,
    pub(super) toc: Option<toml::Spanned<toml::Value>>,
    pub(super) footnotes: Option<toml::Spanned<toml::Value>>,
    pub(super) citations: Option<toml::Spanned<toml::Value>>,
    pub(super) extra: Option<toml::Spanned<toml::Value>>,
    #[serde(default)]
    pub(super) content: BTreeMap<String, toml::Spanned<String>>,
    #[serde(default)]
    pub(super) render: BTreeMap<toml::Spanned<String>, RawRendererSource>,
    #[serde(default)]
    pub(super) taxonomies: BTreeMap<toml::Spanned<String>, Vec<toml::Spanned<String>>>,
    pub(super) collections: Option<Vec<toml::Spanned<String>>>,
    #[serde(default)]
    pub(super) series: Vec<RawSeriesMembership>,
    pub(super) weight: Option<toml::Spanned<toml::Value>>,
    pub(super) pagination: Option<RawPagination>,
    #[serde(default = "default_suffix")]
    pub(super) suffix: toml::Spanned<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRendererSource {
    pub(super) file: Option<toml::Spanned<String>>,
    pub(super) root: Option<toml::Spanned<String>>,
    pub(super) entry: Option<toml::Spanned<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSeriesMembership {
    pub(super) id: toml::Spanned<String>,
    pub(super) position: toml::Spanned<i64>,
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
    pub(super) authors: Option<ResolvedAuthors>,
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
    pub(super) fn timestamp(&self) -> Timestamp {
        self.0
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Metadata {
    pub(super) title: Option<String>,
    pub(super) title_span: Option<SourceSpan>,
    pub(super) description: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) summary_span: Option<SourceSpan>,
    pub(super) authors: Option<ResolvedAuthors>,
    pub(super) published: Option<PublicationTime>,
    pub(super) updated: Option<PublicationTime>,
    pub(super) updated_span: Option<SourceSpan>,
    pub(super) extra: Value,
}

/// Page authors with the authored location that supplied the resolved list.
/// A directory default retains its own source; a page list uses the page
/// source that a later diagnostic identifies.
#[derive(Debug, Clone)]
pub(super) struct ResolvedAuthors {
    names: Vec<AuthorName>,
    span: SourceSpan,
    source: Option<Arc<NamedSource<String>>>,
}

impl ResolvedAuthors {
    pub(super) fn page(names: Vec<AuthorName>, span: SourceSpan) -> Self {
        Self {
            names,
            span,
            source: None,
        }
    }

    pub(super) fn directory(mut self, source: Arc<NamedSource<String>>) -> Self {
        self.source = Some(source);
        self
    }

    pub(super) fn names(&self) -> &[AuthorName] {
        &self.names
    }

    pub(super) fn origin(
        &self,
        page_source: &NamedSource<String>,
    ) -> (Arc<NamedSource<String>>, SourceSpan) {
        (
            self.source
                .clone()
                .unwrap_or_else(|| Arc::new(page_source.clone())),
            self.span,
        )
    }
}

impl Metadata {
    pub(super) fn field(&self, name: &str) -> bool {
        match name {
            "title" => self.title.is_some(),
            "description" => self.description.is_some(),
            "summary" => self.summary.is_some(),
            "authors" => self.authors.is_some(),
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
    pub(super) markdown_content: BTreeSet<String>,
    /// Original declared Markdown inputs are retained for source-derived
    /// generated resources such as search, not for template rendering.
    pub(super) markdown_sources: BTreeMap<String, MarkdownSource>,
    pub(super) renderer_sources: BTreeMap<String, crate::renderer::Source>,
    pub(super) renderer_requests: BTreeMap<String, Vec<crate::markdown::ResolvedRequest>>,
    pub(super) renderer_replacements: BTreeMap<String, Vec<crate::markdown::RendererReplacement>>,
    pub(super) component_plans: BTreeMap<String, crate::markdown::ComponentPlan>,
    pub(super) component_sources: BTreeMap<String, MarkdownSource>,
    pub(super) renderer_resources: Vec<RendererResource>,
    pub(super) markdown_facts: BTreeMap<String, crate::markdown::MarkdownFacts>,
    /// Search owns these transient sections while it constructs its generated
    /// documents. Pages outside every configured search index never allocate
    /// or retain them.
    pub(super) markdown_search_sections:
        Option<BTreeMap<String, Vec<crate::markdown::SearchTextSection>>>,
    pub(super) toc: Option<TocPlan>,
    pub(super) footnotes: Option<MarginFootnotePlan>,
    pub(super) citations: Option<super::citation_inputs::PagePlan>,
    pub(super) draft: bool,
    pub(super) metadata: Metadata,
    pub(super) translationid: Option<TranslationId>,
    pub(super) translationid_span: Option<SourceSpan>,
    pub(super) source: NamedSource<String>,
    pub(super) directory: DirectoryView,
    pub(super) taxonomies: Vec<TaxonomyMembership>,
    pub(super) collections: Vec<CollectionMembership>,
    pub(super) series: Vec<SeriesMembership>,
    pub(super) pagination: Option<Pagination>,
    pub(super) weight: SiblingWeight,
    pub(super) weight_span: Option<SourceSpan>,
    pub(super) translations: BTreeMap<String, PageLink>,
}

#[derive(Debug, Clone)]
pub(super) struct RendererResource {
    pub(super) output_path: OutputPath,
    pub(super) bytes: Vec<u8>,
    pub(super) media_type: String,
    pub(super) licensing: Option<Arc<AssetLicensing>>,
    pub(super) origins: Vec<RendererResourceOrigin>,
}

#[derive(Debug, Clone)]
pub(super) struct RendererResourceOrigin {
    pub(super) description: String,
    pub(super) source: Arc<NamedSource<String>>,
    pub(super) span: SourceSpan,
    pub(super) licensing_record: Option<crate::markdown::LicensingRecordSelection>,
    pub(super) page_output_path: OutputPath,
    pub(super) locale: crate::locale::Locale,
    pub(super) input: String,
    pub(super) kind: &'static str,
    pub(super) text: String,
    pub(super) position: usize,
}

/// Inputs that remain stable while one page analyzes its declared Markdown.
/// Grouping them keeps the page boundary readable as renderer-resource
/// contracts gain additional checked inputs.
pub(super) struct MarkdownAnalysisInputs<'a> {
    pub(super) renderer: &'a MarkdownRenderer,
    pub(super) diagnostics: &'a crate::source_diagnostic::DiagnosticPaths,
    pub(super) assets: &'a AssetLinks,
    pub(super) pages: &'a PageLinks,
    pub(super) words_per_minute: Option<NonZeroU64>,
    pub(super) collect_search_sections: bool,
    pub(super) resource_base_path: &'a str,
    pub(super) licensing_coverage: crate::config::LicensingCoverage,
    pub(super) locale: &'a crate::locale::Locale,
}

struct RendererResourceOriginContext<'a> {
    source: &'a Arc<NamedSource<String>>,
    page_output_path: &'a OutputPath,
    locale: &'a crate::locale::Locale,
    input: &'a str,
    request: &'a crate::markdown::ResolvedRequest,
}

impl RendererResourceOriginContext<'_> {
    fn origin(
        &self,
        description: String,
        kind: &'static str,
        text: String,
    ) -> RendererResourceOrigin {
        RendererResourceOrigin {
            description,
            source: self.source.clone(),
            span: self.request.span.clone().into(),
            licensing_record: self.request.licensing_record.clone(),
            page_output_path: self.page_output_path.clone(),
            locale: self.locale.clone(),
            input: self.input.to_owned(),
            kind,
            text,
            position: self.request.span.start,
        }
    }
}

/// The resolved relationship between a page and directory sibling ordering.
///
/// TOML has no null value, so loading maps the public integer-or-`false`
/// syntax into this finite domain before relationship resolution begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SiblingWeight {
    Ordered(i64),
    Excluded,
}

impl SiblingWeight {
    pub(super) fn ordered(self) -> Option<i64> {
        match self {
            Self::Ordered(value) => Some(value),
            Self::Excluded => None,
        }
    }
}

/// Validated pagination declared by one authored listing page. Generated
/// pager tasks are resolved later, after collection membership is stable.
#[derive(Debug, Clone)]
pub(super) struct Pagination {
    pub(super) collection: CollectionName,
    pub(super) per_page: usize,
    pub(super) segment: PaginationSegment,
    pub(super) navigation_window: NavigationWindow,
    /// The authored value responsible for generated pager routes. A default
    /// segment has no source range, so it falls back to the collection value
    /// that declares the pagination relationship.
    pub(super) route_span: SourceSpan,
}

/// A compact pager retains fixed first and last anchors plus a bounded number
/// of neighbors around its current page. Keeping the bound here prevents
/// generated-resource features from drifting into different navigation rules.
#[derive(Debug, Clone, Copy)]
pub(super) struct NavigationWindow(usize);

impl NavigationWindow {
    pub(super) const DEFAULT: Self = Self(1);

    pub(super) fn parse(value: usize) -> Result<Self, &'static str> {
        if (1..=3).contains(&value) {
            Ok(Self(value))
        } else {
            Err("pagination.navigation.window must be an integer from 1 through 3")
        }
    }

    pub(super) fn neighbors(self) -> usize {
        self.0
    }
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

/// One page-local position in a checked Series relationship.
#[derive(Debug, Clone)]
pub(super) struct SeriesMembership {
    pub(super) name: SeriesName,
    pub(super) position: usize,
    pub(super) source: Arc<NamedSource<String>>,
    pub(super) span: SourceSpan,
    pub(super) position_span: SourceSpan,
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
    #[serde(skip)]
    pub(super) output_path: OutputPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) absolute_url: Option<String>,
}

impl Page {
    /// Derives template facts for every Markdown input and, only when search
    /// selected this page, the source sections needed for its JSON document.
    /// Collection resolution precedes this work because membership determines
    /// which pages a search index actually publishes.
    pub(super) fn analyze_markdown(
        &mut self,
        inputs: MarkdownAnalysisInputs<'_>,
    ) -> Result<(), Box<crate::markdown::SourceError>> {
        let MarkdownAnalysisInputs {
            renderer,
            diagnostics,
            assets,
            pages,
            words_per_minute,
            collect_search_sections,
            resource_base_path,
            licensing_coverage,
            locale,
        } = inputs;
        let mut facts = BTreeMap::new();
        let mut renderer_requests = BTreeMap::new();
        let mut renderer_replacements = BTreeMap::new();
        let mut component_plans = BTreeMap::new();
        let mut component_sources = BTreeMap::new();
        let mut rendered_content = BTreeMap::new();
        let mut search_sections = collect_search_sections.then(BTreeMap::new);
        let mut renderer_resources = Vec::new();
        for (name, source) in &self.markdown_sources {
            let mut component_plan = if let Some(plan) = self.component_plans.remove(name) {
                crate::markdown::ComponentPlan::check_placement(&source.source).map_err(
                    |error| {
                        Box::new(crate::markdown::source_error(
                            &source.display_path,
                            &source.source,
                            error,
                        ))
                    },
                )?;
                Some(plan)
            } else {
                crate::markdown::ComponentPlan::parse(&source.source).map_err(|error| {
                    Box::new(crate::markdown::source_error(
                        &source.display_path,
                        &source.source,
                        error,
                    ))
                })?
            };
            let input = component_plan.as_ref().map_or(
                source.source.as_str(),
                crate::markdown::ComponentPlan::normalized,
            );
            let fragments = self
                .toc
                .as_ref()
                .filter(|toc| toc.content() == name)
                .map(TocPlan::anchors);
            let mut citation_collector = self
                .citations
                .as_ref()
                .filter(|plan| plan.content == *name)
                .map(|_| crate::markdown::citation::Collector::with_headings(fragments.is_some()));
            let analysis = crate::markdown::analyze_source_with_links(SourceAnalysisInputs {
                display_path: &source.display_path,
                input,
                renderer,
                assets,
                pages,
                words_per_minute,
                fragments,
                collect_search_sections,
                renderer_sources: &self.renderer_sources,
                citations: citation_collector.as_mut(),
            })?;
            if let Some(collector) = citation_collector {
                if let Some(problem) = collector.problems.into_iter().next() {
                    return Err(Box::new(crate::markdown::source_error(
                        &source.display_path,
                        &source.source,
                        crate::markdown::RenderError::Citation {
                            message: problem.message.to_owned(),
                            span: problem.span,
                        },
                    )));
                }
                if let Some(plan) = self.citations.as_mut() {
                    for group in &collector.groups {
                        for span in &group.keys {
                            let key = &input[span.clone()];
                            if !plan.contains_key(key) {
                                return Err(Box::new(crate::markdown::source_error(
                                    &source.display_path,
                                    &source.source,
                                    crate::markdown::RenderError::Citation {
                                        message: format!("unknown citation key {key:?}"),
                                        span: span.clone(),
                                    },
                                )));
                            }
                        }
                    }
                    plan.groups = collector.groups;
                    plan.format(input, &locale.to_string()).map_err(|error| {
                        if let Some(span) = error.citation_span() {
                            Box::new(crate::markdown::source_error(
                                &source.display_path,
                                &source.source,
                                crate::markdown::RenderError::Citation {
                                    message: error.to_string(),
                                    span,
                                },
                            ))
                        } else if let Some((path, input, span)) = error
                            .bibliography_span()
                            .and_then(|span| plan.bibliography_source(span))
                        {
                            Box::new(crate::markdown::citation_bibliography_error(
                                &diagnostics.display_path(&path),
                                input,
                                span,
                                error.to_string(),
                            ))
                        } else {
                            Box::new(crate::markdown::citation_configuration_error(
                                &plan.declaration_path,
                                &plan.declaration_source,
                                plan.style_span.clone(),
                                error.to_string(),
                            ))
                        }
                    })?;
                    if let Some(toc) = self.toc.as_mut().filter(|toc| toc.content() == name) {
                        toc.update_heading_texts(&plan.heading_texts(&collector.headings));
                    }
                }
            }
            if !analysis.renderer_requests.is_empty() && !renderer.allows_external_renderers() {
                return Err(Box::new(crate::markdown::source_error(
                    &source.display_path,
                    &source.source,
                    crate::markdown::RenderError::RendererPermission {
                        span: analysis.renderer_requests[0].span.clone(),
                    },
                )));
            }
            if self.output_path.is_none()
                && let Some(request) = analysis
                    .renderer_requests
                    .iter()
                    .find(|request| request.output == crate::renderer::Output::Resource)
            {
                return Err(Box::new(crate::markdown::source_error(
                    &source.display_path,
                    &source.source,
                    crate::markdown::RenderError::RendererResourcePage {
                        span: request.span.clone(),
                    },
                )));
            }
            if licensing_coverage == crate::config::LicensingCoverage::Required
                && let Some(request) = analysis.renderer_requests.iter().find(|request| {
                    request.output == crate::renderer::Output::Resource
                        && request.licensing_record.is_none()
                })
            {
                return Err(Box::new(crate::markdown::source_error(
                    &source.display_path,
                    &source.source,
                    crate::markdown::RenderError::RendererLicensing {
                        span: request.span.clone(),
                    },
                )));
            }
            let mut transformed = input.to_owned();
            let mut replacements = Vec::new();
            let mut rebases = Vec::new();
            let needs_rebase = self
                .citations
                .as_ref()
                .is_some_and(|plan| plan.content == *name)
                || self
                    .footnotes
                    .as_ref()
                    .is_some_and(|plan| plan.content() == name)
                || component_plan.is_some();
            let renderer_source = analysis
                .renderer_requests
                .iter()
                .any(|request| request.output == crate::renderer::Output::Resource)
                .then(|| {
                    Arc::new(NamedSource::new(
                        source.display_path.display().to_string(),
                        source.source.clone(),
                    ))
                });
            for request in analysis.renderer_requests.iter().rev() {
                let profile = renderer
                    .renderer_profiles()
                    .get(&request.profile)
                    .expect("resolved renderer profile");
                let execute = |appearance| -> Result<_, crate::renderer::RendererError> {
                    if profile.uses_workspace() {
                        let input = match &request.input {
                            crate::markdown::ResolvedInput::Body => {
                                crate::renderer::WorkspaceInput::Body(
                                    input[request.body_span.clone()].as_bytes(),
                                )
                            }
                            crate::markdown::ResolvedInput::Source(name) => {
                                crate::renderer::WorkspaceInput::Source(
                                    &self.renderer_sources[name],
                                )
                            }
                        };
                        crate::renderer::run_workspace(crate::renderer::WorkspaceRequest {
                            runtime: renderer.renderer_runtime(),
                            refresh: renderer.refresh_renderers(),
                            profile_name: &request.profile,
                            profile,
                            placement: request.placement,
                            appearance,
                            output: request.output,
                            input,
                        })
                    } else {
                        let input = match &request.input {
                            crate::markdown::ResolvedInput::Body => {
                                input[request.body_span.clone()].as_bytes().to_vec()
                            }
                            crate::markdown::ResolvedInput::Source(name) => {
                                let declared = &self.renderer_sources[name];
                                if declared.input() != crate::renderer::NamedInput::File {
                                    return Err(
                                        "stream renderers cannot consume source bundles".into()
                                    );
                                }
                                fs::read(declared.entry()).map_err(|error| {
                                    format!("could not read renderer source {name:?}: {error}")
                                })?
                            }
                        };
                        crate::renderer::run_stream(crate::renderer::StreamRequest {
                            runtime: renderer.renderer_runtime(),
                            refresh: renderer.refresh_renderers(),
                            profile_name: &request.profile,
                            profile,
                            placement: request.placement,
                            appearance,
                            output: request.output,
                            input: &input,
                        })
                    }
                };
                let process_error = |error| {
                    Box::new(crate::markdown::source_error(
                        &source.display_path,
                        &source.source,
                        crate::markdown::renderer_process_error(
                            error,
                            profile.uses_workspace()
                                && matches!(request.input, crate::markdown::ResolvedInput::Body),
                            request.span.clone(),
                        ),
                    ))
                };
                let resource_origin =
                    (request.output == crate::renderer::Output::Resource).then(|| {
                        RendererResourceOriginContext {
                            source: renderer_source
                                .as_ref()
                                .expect("resource renderer requests retain their source"),
                            page_output_path: self
                                .output_path
                                .as_ref()
                                .expect("renderer requests belong to rendered pages"),
                            locale,
                            input: name,
                            request,
                        }
                    });
                let replacement = if profile.result().is_picture() {
                    let light =
                        execute(Some(crate::renderer::Appearance::Light)).map_err(|error| {
                            process_error(error.for_appearance(crate::renderer::Appearance::Light))
                        })?;
                    let dark =
                        execute(Some(crate::renderer::Appearance::Dark)).map_err(|error| {
                            process_error(error.for_appearance(crate::renderer::Appearance::Dark))
                        })?;
                    let (light_path, light_bytes) = resource_output(light);
                    let (dark_path, dark_bytes) = resource_output(dark);
                    let dimensions = crate::markdown::picture::dimensions_for_picture(
                        request.width,
                        request.height,
                        crate::renderer::image_dimensions(&light_bytes),
                        crate::renderer::image_dimensions(&dark_bytes),
                    )
                    .map_err(|message| process_error(message.into()))?;
                    let light_url =
                        format!("{}{}", resource_base_path, light_path.url_path().as_str());
                    let dark_url =
                        format!("{}{}", resource_base_path, dark_path.url_path().as_str());
                    let replacement = crate::markdown::picture::generated_html(
                        &light_url,
                        &dark_url,
                        request.alt.as_deref().unwrap_or(""),
                        request.caption.as_deref(),
                        request.class.as_ref(),
                        dimensions,
                    );
                    for (appearance, output_path, bytes) in [
                        (crate::renderer::Appearance::Light, light_path, light_bytes),
                        (crate::renderer::Appearance::Dark, dark_path, dark_bytes),
                    ] {
                        renderer_resources.push(RendererResource {
                            output_path,
                            bytes,
                            media_type: profile
                                .result()
                                .resource_media_type()
                                .expect("picture profiles always publish a resource")
                                .to_owned(),
                            licensing: None,
                            origins: vec![
                                resource_origin
                                    .as_ref()
                                    .expect("picture profiles always publish a resource")
                                    .origin(
                                        format!("{} picture renderer resource", appearance.name()),
                                        "image",
                                        request.alt.clone().unwrap_or_default(),
                                    ),
                            ],
                        });
                    }
                    replacement
                } else {
                    match execute(None).map_err(process_error)? {
                        crate::renderer::RendererOutput::Embedded(replacement) => {
                            if let Some(class) = &request.class {
                                crate::renderer::add_embedded_svg_class(
                                    &replacement,
                                    class.as_str(),
                                )
                                .map_err(crate::renderer::RendererError::from)
                                .map_err(process_error)?
                            } else {
                                replacement
                            }
                        }
                        crate::renderer::RendererOutput::Resource { output_path, bytes } => {
                            let url = format!(
                                "{}{}",
                                resource_base_path,
                                output_path.url_path().as_str()
                            );
                            let replacement = resource_replacement(
                                profile.result(),
                                &url,
                                request.alt.as_deref(),
                                request.label.as_deref(),
                            );
                            renderer_resources.push(RendererResource {
                                output_path,
                                bytes,
                                media_type: profile
                                    .result()
                                    .resource_media_type()
                                    .expect("resource output has a configured media type")
                                    .to_owned(),
                                licensing: None,
                                origins: vec![
                                    resource_origin
                                        .as_ref()
                                        .expect("resource output retains renderer provenance")
                                        .origin(
                                            "renderer resource".to_owned(),
                                            if profile.result().needs_label(request.output) {
                                                "link"
                                            } else {
                                                "image"
                                            },
                                            request
                                                .label
                                                .clone()
                                                .or_else(|| request.alt.clone())
                                                .unwrap_or_default(),
                                        ),
                                ],
                            });
                            replacement
                        }
                    }
                };
                let replacement = crate::markdown::RendererReplacement::new(
                    &request.span,
                    request.placement,
                    replacement,
                );
                let marker = replacement.markdown_marker();
                if needs_rebase {
                    rebases.push((request.span.clone(), marker.len()));
                }
                transformed.replace_range(request.span.clone(), &marker);
                replacements.push(replacement);
            }
            rendered_content.insert(name.clone(), transformed);
            if !rebases.is_empty() {
                let offsets = crate::markdown::rebase::Rebase::new(&rebases).map_err(|span| {
                    Box::new(crate::markdown::source_error(
                        &source.display_path,
                        &source.source,
                        crate::markdown::RenderError::RendererOffsets { span },
                    ))
                })?;
                if let Some(plan) = self.citations.as_mut().filter(|plan| plan.content == *name) {
                    plan.rebase(&offsets).map_err(|span| {
                        Box::new(crate::markdown::source_error(
                            &source.display_path,
                            &source.source,
                            crate::markdown::RenderError::Citation {
                                message: "citation overlaps a renderer directive".into(),
                                span,
                            },
                        ))
                    })?;
                }
                if let Some(plan) = self
                    .footnotes
                    .as_mut()
                    .filter(|plan| plan.content() == name)
                {
                    plan.rebase(&offsets).map_err(|span| {
                        Box::new(crate::markdown::source_error(
                            &source.display_path,
                            &source.source,
                            crate::markdown::RenderError::MarginFootnote {
                                message: "margin footnote overlaps a renderer directive".into(),
                                span,
                            },
                        ))
                    })?;
                }
                if let Some(plan) = &mut component_plan {
                    plan.rebase(&offsets).map_err(|span| {
                        Box::new(crate::markdown::source_error(
                            &source.display_path,
                            &source.source,
                            crate::markdown::RenderError::Component {
                                message: "renderer directive crosses a component boundary".into(),
                                span,
                            },
                        ))
                    })?;
                }
            }
            if let Some(plan) = component_plan {
                component_plans.insert(name.clone(), plan);
                component_sources.insert(name.clone(), source.clone());
            }
            facts.insert(name.clone(), analysis.facts);
            renderer_requests.insert(name.clone(), analysis.renderer_requests);
            renderer_replacements.insert(name.clone(), replacements);
            if let Some(search_sections) = &mut search_sections {
                search_sections.insert(name.clone(), analysis.search_sections);
            }
        }
        self.markdown_facts = facts;
        self.renderer_requests = renderer_requests;
        self.renderer_replacements = renderer_replacements;
        self.component_plans = component_plans;
        self.component_sources = component_sources;
        self.renderer_resources = renderer_resources;
        for (name, content) in rendered_content {
            self.content.insert(name, Value::String(content));
        }
        self.markdown_search_sections = search_sections;
        Ok(())
    }

    /// Drops loading-only state after all site relationships are resolved.
    /// The resulting value is the sole page representation retained while
    /// templates render.
    pub(super) fn into_view(
        self,
        canonical_urls: Option<&crate::config::CanonicalUrls>,
    ) -> PageView {
        let Metadata {
            title,
            description,
            summary,
            authors,
            published,
            updated,
            extra,
            ..
        } = self.metadata;
        let absolute_url = self
            .output_path
            .as_ref()
            .and_then(|path| canonical_urls.map(|urls| urls.output_url(path)));
        let mut translations = self.translations;
        for link in translations.values_mut() {
            link.absolute_url = canonical_urls.map(|urls| urls.output_url(&link.output_path));
        }
        PageView {
            id: self.id,
            absolute_url,
            path: self.path.map(|path| path.as_str().into()),
            dir: self.dir,
            filename: self.filename,
            subdirs: self.subdirs,
            content: self.content,
            markdown_content: self.markdown_content,
            renderer_replacements: self.renderer_replacements,
            component_plans: self.component_plans,
            component_sources: self.component_sources,
            markdown: self.markdown_facts,
            toc: self.toc,
            footnotes: self.footnotes.map(FootnotesView::from),
            citations: self
                .citations
                .and_then(super::citation_inputs::PagePlan::into_view),
            draft: self.draft,
            title,
            description,
            summary,
            authors: authors.map(|authors| authors.names().to_vec()),
            published: published.as_ref().map(PublicationTime::render),
            updated: updated.as_ref().map(PublicationTime::render),
            extra,
            translations,
            directory: self.directory,
            weight: self.weight.ordered(),
        }
    }
}

fn resource_replacement(
    result: &crate::renderer::ResultCapabilities,
    url: &str,
    alt: Option<&str>,
    label: Option<&str>,
) -> String {
    let url = escape_html(url);
    match result {
        crate::renderer::ResultCapabilities::Svg { .. }
        | crate::renderer::ResultCapabilities::Image { .. } => {
            format!(
                "<img src=\"{url}\" alt=\"{}\">",
                escape_html(alt.unwrap_or(""))
            )
        }
        crate::renderer::ResultCapabilities::Link { .. } => {
            format!("<a href=\"{url}\">{}</a>", escape_html(label.unwrap_or("")))
        }
        crate::renderer::ResultCapabilities::Picture { .. } => {
            unreachable!("picture resources are replaced as a light and dark pair")
        }
        crate::renderer::ResultCapabilities::Html | crate::renderer::ResultCapabilities::Mathml => {
            unreachable!("embedded-only profile cannot produce a resource")
        }
    }
}

fn resource_output(output: crate::renderer::RendererOutput) -> (OutputPath, Vec<u8>) {
    match output {
        crate::renderer::RendererOutput::Resource { output_path, bytes } => (output_path, bytes),
        crate::renderer::RendererOutput::Embedded(_) => {
            unreachable!("picture profiles produce resources")
        }
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Deliberate, serializable template contract derived from a validated page.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PageView {
    id: PageId,
    #[serde(skip_serializing_if = "Option::is_none")]
    absolute_url: Option<String>,
    path: Option<String>,
    dir: String,
    filename: Option<String>,
    subdirs: Vec<String>,
    content: BTreeMap<String, Value>,
    #[serde(skip)]
    markdown_content: BTreeSet<String>,
    #[serde(skip)]
    renderer_replacements: BTreeMap<String, Vec<crate::markdown::RendererReplacement>>,
    #[serde(skip)]
    component_plans: BTreeMap<String, crate::markdown::ComponentPlan>,
    #[serde(skip)]
    component_sources: BTreeMap<String, MarkdownSource>,
    markdown: BTreeMap<String, crate::markdown::MarkdownFacts>,
    toc: Option<TocPlan>,
    footnotes: Option<FootnotesView>,
    #[serde(skip)]
    citations: Option<super::citation_inputs::CitationView>,
    draft: bool,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    authors: Option<Vec<AuthorName>>,
    published: Option<String>,
    updated: Option<String>,
    extra: Value,
    translations: BTreeMap<String, PageLink>,
    directory: DirectoryView,
    weight: Option<i64>,
}

impl PageView {
    pub(crate) fn renderer_markers(&self) -> impl Iterator<Item = &str> {
        self.renderer_replacements
            .values()
            .flatten()
            .map(|replacement| replacement.marker())
    }

    pub(crate) fn component_markdown_names(&self) -> impl Iterator<Item = &str> {
        self.component_plans.keys().map(String::as_str)
    }

    pub(crate) fn component_source(&self, name: &str) -> Option<&MarkdownSource> {
        self.component_sources.get(name)
    }

    pub(crate) fn is_draft(&self) -> bool {
        self.draft
    }

    pub(crate) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(crate) fn directory(&self) -> &str {
        &self.dir
    }

    pub(crate) fn toc(&self) -> Option<&TocPlan> {
        self.toc.as_ref()
    }

    pub(crate) fn citations(&self) -> Option<&super::citation_inputs::CitationView> {
        self.citations.as_ref()
    }

    pub(crate) fn footnotes(&self) -> Option<&FootnotesView> {
        self.footnotes.as_ref()
    }

    pub(crate) fn content_markdown(
        &self,
        name: &str,
    ) -> Option<(
        &str,
        &[crate::markdown::RendererReplacement],
        Option<&crate::markdown::ComponentPlan>,
    )> {
        self.markdown_content.contains(name).then(|| {
            let content = self.content.get(name)?.as_str()?;
            let replacements = self
                .renderer_replacements
                .get(name)
                .map(Vec::as_slice)
                .unwrap_or_default();
            Some((content, replacements, self.component_plans.get(name)))
        })?
    }
}

/// The small render-facing view for one source selected for margin footnotes.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FootnotesView {
    content: String,
    #[serde(skip)]
    plan: MarginFootnotePlan,
}

impl From<MarginFootnotePlan> for FootnotesView {
    fn from(plan: MarginFootnotePlan) -> Self {
        Self {
            content: plan.content().to_owned(),
            plan,
        }
    }
}

impl FootnotesView {
    pub(crate) fn content(&self) -> &str {
        &self.content
    }

    pub(crate) fn plan(&self) -> &MarginFootnotePlan {
        &self.plan
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
