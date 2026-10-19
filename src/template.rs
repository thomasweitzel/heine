//! Tera is contained in this module; the rest of the application uses only
//! render-facing serde values rather than template-engine types.

mod arguments;
mod content;
mod localization;
mod resources;
mod runtime;
mod sources;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    asset::AssetLinks,
    config::{BasePath, CanonicalUrls},
    locale::Locale,
    localization::Localization,
    site::{
        FeedCatalog, PageCatalog, PageId, PageLinks, PageView, SearchCatalog, SeriesCatalog,
        TaxonomyCatalog,
    },
    source_path::LogicalPath,
};
use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;

/// `walkdir::Error` renders the physical path it inspected. Template
/// discovery has already established a project-owned display path, so retain
/// the operating-system detail without reintroducing an absolute path through
/// the diagnostic source chain.
#[derive(Debug, Error)]
#[error("{message}")]
pub(super) struct WalkFailure {
    message: String,
}

impl WalkFailure {
    pub(super) fn new(message: String) -> Self {
        Self { message }
    }
}

/// The current page augmented with taxonomy links whose relative URLs depend
/// on the output file currently being rendered.
#[derive(Serialize)]
pub(crate) struct RenderedPage<'a> {
    #[serde(flatten)]
    pub(crate) page: &'a PageView,
    pub(crate) taxonomies: BTreeMap<String, Vec<crate::site::PageTermView>>,
    pub(crate) series: Vec<crate::site::SeriesMembershipView>,
}

/// A validated template identifier relative to `templates/`.
///
/// Template discovery and page declarations share this type, so every Tera
/// lookup names the same logical template namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TemplateName(LogicalPath);

impl TemplateName {
    pub(crate) fn parse(name: String) -> Result<Self, String> {
        LogicalPath::parse(&name).map(Self).ok_or_else(|| {
            "template names must be non-empty relative paths without '.', '..', or backslashes"
                .into()
        })
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Root-authored selectors never address kit originals. Kit references are
    /// valid only inside template source, where the loader checks visibility.
    fn is_kit_namespace(&self) -> bool {
        matches!(
            self.0.as_path().components().next(),
            Some(std::path::Component::Normal(component)) if component == "kit"
        )
    }
}

impl std::fmt::Display for TemplateName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(formatter)
    }
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not read template {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not walk template directory {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: WalkFailure,
    },
    #[error("template path {path} must use UTF-8 components and must not contain backslashes")]
    NonUtf8Path { path: PathBuf },
    #[error("template path {path} is a symbolic link")]
    TemplateSymlink { path: PathBuf },
    #[error("root template {path} is inside the reserved kit namespace")]
    ReservedKitNamespace { path: PathBuf },
    #[error("internal selected kit manifest source was not attached")]
    MissingKitManifestSource,
    #[error(transparent)]
    #[diagnostic(transparent)]
    NormalizationCollision(#[from] Box<crate::source_path::NormalizationCollision>),
    #[error("could not load templates from {path}")]
    Load {
        path: PathBuf,
        #[source]
        source: tera::Error,
    },
    #[error("could not render template {template}{rendering}")]
    Render {
        template: String,
        rendering: String,
        #[source]
        source: tera::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Detailed(#[from] Box<TemplateDiagnostic>),
}

/// Template-only data derived from the resolved site and asset catalog.
///
/// This keeps Tera's needs at the rendering boundary instead of letting them
/// shape the internal domain model.
pub(crate) struct TemplateResources {
    asset_links: AssetLinks,
    relationships: TemplateRelationships,
    localization: std::sync::Arc<Localization>,
    highlight_css: Option<String>,
    table_scroll_message: Option<String>,
    editorial_time_zone: Option<crate::config::EditorialTimeZone>,
}

/// Immutable site relationships needed by template built-ins. This has no
/// Tera values itself; it keeps the validated page catalog and focused indexes
/// together at the template boundary.
pub(crate) struct TemplateRelationships {
    page_links: PageLinks,
    page_catalog: PageCatalog,
    collections:
        std::sync::Arc<BTreeMap<Locale, BTreeMap<String, crate::site::collection::CollectionView>>>,
    siblings: std::sync::Arc<BTreeMap<Locale, crate::site::SiblingIndex>>,
    collection_neighbors: std::sync::Arc<crate::site::collection::CollectionNeighborIndex>,
    taxonomies: std::sync::Arc<TaxonomyCatalog>,
    pagers: crate::site::PaginationCatalog,
    feeds: std::sync::Arc<FeedCatalog>,
    search: std::sync::Arc<SearchCatalog>,
    series: std::sync::Arc<SeriesCatalog>,
}

/// Resolved indexes exposed through checked template resource functions.
///
/// Grouping them keeps relationship construction aligned with the rendering
/// boundary as generated-resource features are added.
pub(crate) struct TemplateIndexes {
    pub(crate) collections:
        std::sync::Arc<BTreeMap<Locale, BTreeMap<String, crate::site::collection::CollectionView>>>,
    pub(crate) siblings: std::sync::Arc<BTreeMap<Locale, crate::site::SiblingIndex>>,
    pub(crate) collection_neighbors:
        std::sync::Arc<crate::site::collection::CollectionNeighborIndex>,
    pub(crate) taxonomies: std::sync::Arc<TaxonomyCatalog>,
    pub(crate) pagers: crate::site::PaginationCatalog,
    pub(crate) feeds: std::sync::Arc<FeedCatalog>,
    pub(crate) search: std::sync::Arc<SearchCatalog>,
    pub(crate) series: std::sync::Arc<SeriesCatalog>,
}

/// Values deliberately exposed to one template render. This is the boundary
/// between the resolved site model and Tera's serialization requirements.
#[derive(Serialize)]
pub(crate) struct RenderContext<'a> {
    pub(crate) site: SiteView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) kit: Option<&'a KitView>,
    pub(crate) locale: LocaleView,
    pub(crate) page: Option<RenderedPage<'a>>,
    pub(crate) taxonomy: Option<crate::site::TaxonomyView>,
    pub(crate) series: Option<crate::site::SeriesView>,
    pub(crate) pagination: Option<crate::site::PaginationView<'a>>,
    pub(crate) data: &'a serde_json::Value,
    pub(crate) locale_data: &'a serde_json::Value,
    pub(crate) root: &'a str,
    #[serde(rename = "__heine_runtime")]
    pub(crate) runtime: RuntimeContext<'a>,
}

/// Kit-owned defaults after root configuration has supplied checked overrides.
/// The view is absent for ordinary sites so templates can make kit use explicit.
#[derive(Serialize)]
pub(crate) struct KitView {
    pub(crate) extra: serde_json::Value,
}

impl From<&crate::kit::Selected> for KitView {
    fn from(kit: &crate::kit::Selected) -> Self {
        Self {
            extra: kit.extra().clone(),
        }
    }
}

/// The checked site-wide URL fact exposed to templates. The raw origin stays
/// configuration-only because it is not the canonical site root below a
/// deployment subpath.
#[derive(Serialize)]
pub(crate) struct SiteView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
}

impl SiteView {
    pub(crate) fn from_canonical_urls(canonical_urls: Option<&CanonicalUrls>) -> Self {
        Self {
            url: canonical_urls.map(CanonicalUrls::site_url),
        }
    }
}

/// The locale identity deliberately exposed to templates. It does not leak
/// locale paths or bundles; direction remains explicit site configuration,
/// never an automatic BCP-47 guess.
#[derive(Serialize)]
pub(crate) struct LocaleView {
    pub(crate) code: String,
    pub(crate) direction: String,
}

/// Values required by Heine's built-ins but not part of the public template
/// model. Their reserved context key prevents ordinary template locals such as
/// `locale`, `page`, or `root` from changing how built-ins resolve resources.
#[derive(Serialize)]
pub(crate) struct RuntimeContext<'a> {
    pub(crate) locale: String,
    pub(crate) page_dir: &'a str,
    pub(crate) page_id: Option<&'a PageId>,
    pub(crate) output_path: &'a str,
    pub(crate) root: &'a str,
    pub(crate) pagination_page: Option<usize>,
}

impl TemplateResources {
    pub(crate) fn new(
        asset_links: AssetLinks,
        relationships: TemplateRelationships,
        localization: std::sync::Arc<Localization>,
        highlight_css: Option<String>,
        table_scroll_message: Option<String>,
        editorial_time_zone: Option<crate::config::EditorialTimeZone>,
    ) -> Self {
        Self {
            asset_links,
            relationships,
            localization,
            highlight_css,
            table_scroll_message,
            editorial_time_zone,
        }
    }
}

impl TemplateRelationships {
    pub(crate) fn new(
        page_catalog: PageCatalog,
        default_locale: Locale,
        base_path: BasePath,
        canonical_urls: Option<crate::config::CanonicalUrls>,
        indexes: TemplateIndexes,
    ) -> Self {
        Self {
            page_links: page_catalog.links(default_locale, base_path, canonical_urls),
            page_catalog,
            collections: indexes.collections,
            siblings: indexes.siblings,
            collection_neighbors: indexes.collection_neighbors,
            taxonomies: indexes.taxonomies,
            pagers: indexes.pagers,
            feeds: indexes.feeds,
            search: indexes.search,
            series: indexes.series,
        }
    }
}

/// A Tera failure tied back to a concrete source file owned by Heine.
#[derive(Debug, Diagnostic, Error)]
#[error("{stage} error in template {template:?}: {message}{rendering}")]
#[diagnostic(code(heine::template::invalid))]
pub(crate) struct TemplateDiagnostic {
    stage: &'static str,
    rendering: String,
    template: String,
    message: String,
    #[source_code]
    source_code: miette::NamedSource<String>,
    label: String,
    #[label("{label}")]
    span: miette::SourceSpan,
    #[help]
    help: Option<String>,
}

pub(crate) struct TemplateEngine {
    tera: tera::Tera,
    sources: BTreeMap<TemplateName, miette::NamedSource<String>>,
    runtime: runtime::TemplateRuntime,
    canonical_urls_configured: bool,
}

impl TemplateEngine {
    pub(crate) fn load(
        templates: &Path,
        selected_kit: Option<&crate::kit::Selected>,
        resources: TemplateResources,
        markdown_renderer: crate::markdown::Renderer,
        diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    ) -> Result<Self, Error> {
        let mut tera = tera::Tera::default();
        let canonical_urls_configured = resources.asset_links.canonical_urls().is_some();
        let runtime = runtime::TemplateRuntime::from(resources);
        resources::register(&mut tera, &runtime);
        content::register(&mut tera, &runtime, markdown_renderer);
        localization::register(&mut tera, &runtime);
        let catalog = sources::Catalog::load(templates, selected_kit, diagnostics)?;
        let template_directory = diagnostics.display_path(templates);
        let raw_templates = catalog.tera_sources();
        tera.add_raw_templates(raw_templates).map_err(|source| {
            Error::from_tera(
                "parse",
                &template_directory,
                catalog.sources(),
                source,
                None,
                false,
            )
        })?;
        catalog.validate_components(selected_kit)?;
        Ok(Self {
            tera,
            sources: catalog.into_sources(),
            runtime,
            canonical_urls_configured,
        })
    }

    pub(crate) fn render(
        &self,
        template: &TemplateName,
        context: RenderContext<'_>,
        rendering: &str,
    ) -> Result<String, Error> {
        self.runtime
            .begin_source_aware_renders(
                context.page.as_ref().and_then(|page| page.page.toc()),
                context.page.as_ref().and_then(|page| page.page.footnotes()),
            )
            .map_err(|message| {
                Error::source_aware_render_error(template, rendering, &self.sources, None, message)
            })?;
        let context =
            tera::Context::from_serialize(&context).expect("render context is serializable");
        let rendered = self
            .tera
            .render(template.as_str(), &context)
            .map_err(|source| {
                Error::from_tera(
                    "render",
                    Path::new(template.as_str()),
                    &self.sources,
                    source,
                    Some(rendering),
                    self.canonical_urls_configured,
                )
            });
        let source_aware_renders =
            self.runtime
                .finish_source_aware_renders()
                .map_err(|message| {
                    Error::source_aware_render_error(
                        template,
                        rendering,
                        &self.sources,
                        None,
                        message,
                    )
                })?;
        let rendered = rendered?;
        let unmatched = source_aware_renders
            .into_iter()
            .filter(|source| source.calls != 1)
            .collect::<Vec<_>>();
        if !unmatched.is_empty() {
            let kind = (unmatched.len() == 1).then_some(unmatched[0].kind);
            let message = if unmatched.len() == 1 {
                let source = &unmatched[0];
                format!("this page's selected {}", source.exact_render_message())
            } else {
                let details = unmatched
                    .iter()
                    .map(|source| format!("  - {}", source.exact_render_message()))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "this page's selected Markdown inputs were not rendered exactly once:\n{details}"
                )
            };
            return Err(Error::source_aware_render_error(
                template,
                rendering,
                &self.sources,
                kind,
                message,
            ));
        }
        Ok(rendered)
    }

    pub(crate) fn contains(&self, template: &TemplateName) -> bool {
        !template.is_kit_namespace() && self.tera.contains_template(template.as_str())
    }
}

impl Error {
    fn source_aware_render_error(
        template: &TemplateName,
        rendering: &str,
        sources: &BTreeMap<TemplateName, miette::NamedSource<String>>,
        kind: Option<runtime::SourceAwareRenderKind>,
        message: String,
    ) -> Self {
        let source_code = sources
            .get(template)
            .cloned()
            .unwrap_or_else(|| miette::NamedSource::new(template.to_string(), String::new()));
        Self::Detailed(Box::new(TemplateDiagnostic {
            stage: "render",
            rendering: format!(" (while rendering {rendering})"),
            template: template.to_string(),
            message,
            source_code,
            label: kind.map_or_else(
                || "selected Markdown source requirements apply to this template render".into(),
                |kind| {
                    format!(
                        "selected {} source applies to this template render",
                        kind.name()
                    )
                },
            ),
            span: (0, 0).into(),
            help: None,
        }))
    }

    fn from_tera(
        stage: &'static str,
        location: &Path,
        sources: &BTreeMap<TemplateName, miette::NamedSource<String>>,
        source: tera::Error,
        rendering: Option<&str>,
        canonical_urls_configured: bool,
    ) -> Self {
        if let tera::ErrorKind::MissingParent { current, parent } = source.kind()
            && let Some(diagnostic) = Self::template_reference_diagnostic(
                "template inheritance",
                current,
                parent,
                format!("parent template {parent:?} was not found"),
                sources,
                rendering,
            )
        {
            return diagnostic;
        }
        let report = match source.kind() {
            tera::ErrorKind::SyntaxError(report) | tera::ErrorKind::RenderingError(report) => {
                report
            }
            _ => {
                return if stage == "render" {
                    Self::Render {
                        template: location.display().to_string(),
                        rendering: rendering
                            .map(|rendering| format!(" while rendering {rendering}"))
                            .unwrap_or_default(),
                        source,
                    }
                } else {
                    Self::Load {
                        path: location.into(),
                        source,
                    }
                };
            }
        };
        let template = report.filename();
        let Some(source_code) = TemplateName::parse(template.into())
            .ok()
            .and_then(|template| sources.get(&template))
        else {
            return if stage == "render" {
                Self::Render {
                    template: location.display().to_string(),
                    rendering: rendering
                        .map(|rendering| format!(" while rendering {rendering}"))
                        .unwrap_or_default(),
                    source,
                }
            } else {
                Self::Load {
                    path: location.into(),
                    source,
                }
            };
        };
        let range = report.span().range.clone();
        let source_fragment = source_code.inner().get(range.clone()).unwrap_or_default();
        // Tera knows that this field is absent, but not that Heine makes an
        // origin a prerequisite for canonical URL output. Preserve Tera's
        // precise source span while reporting the relevant configuration fact.
        let missing_canonical_url = !canonical_urls_configured
            && (source_fragment == "absolute_url"
                || (source_fragment == "url"
                    && source_code
                        .inner()
                        .get(..range.start)
                        .is_some_and(|prefix| prefix.ends_with("site."))));
        let (message, label) = if missing_canonical_url {
            (
                "canonical URLs require site.origin; configure an HTTP(S) origin in [site]".into(),
                "site.origin is not configured".into(),
            )
        } else {
            (report.message().into(), "error here".into())
        };
        Self::Detailed(Box::new(TemplateDiagnostic {
            stage,
            rendering: rendering
                .map(|rendering| format!(" (while rendering {rendering})"))
                .unwrap_or_default(),
            template: template.into(),
            message,
            source_code: source_code.clone(),
            label,
            span: (range.start, range.len()).into(),
            help: None,
        }))
    }

    fn template_reference_diagnostic(
        stage: &'static str,
        template: &str,
        reference: &str,
        message: String,
        sources: &BTreeMap<TemplateName, miette::NamedSource<String>>,
        rendering: Option<&str>,
    ) -> Option<Self> {
        let source_code = sources.get(&TemplateName::parse(template.into()).ok()?)?;
        Some(Self::Detailed(Box::new(TemplateDiagnostic {
            stage,
            rendering: rendering
                .map(|rendering| format!(" (while rendering {rendering})"))
                .unwrap_or_default(),
            template: template.into(),
            message,
            source_code: source_code.clone(),
            label: "referenced parent template here".into(),
            span: parent_reference_span(source_code.inner(), reference),
            help: None,
        })))
    }
}

/// Locates the string literal in an `extends` directive without mistaking a
/// matching name in a comment or ordinary template text for the reference.
/// Tera's missing-parent error does not retain its own span, so this small
/// syntax-aware recovery preserves a useful authored location at that API
/// boundary.
fn parent_reference_span(source: &str, reference: &str) -> miette::SourceSpan {
    let mut remaining = source;
    let mut offset = 0;
    let mut directive_span = None;
    while let Some(open) = remaining.find("{%") {
        offset += open;
        let directive = &remaining[open + 2..];
        let directive_offset = offset + 2;
        let (directive, trimmed) = trim_template_whitespace(directive);
        let directive_offset = directive_offset + trimmed;
        let (directive, trim_marker) = match directive.strip_prefix('-') {
            Some(directive) => (directive, 1),
            None => (directive, 0),
        };
        let directive_offset = directive_offset + trim_marker;
        let (directive, trimmed) = trim_template_whitespace(directive);
        let directive_offset = directive_offset + trimmed;
        if let Some(after_extends) = directive.strip_prefix("extends")
            && after_extends
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            directive_span = Some(
                (
                    directive_offset,
                    directive.find("%}").unwrap_or(directive.len()),
                )
                    .into(),
            );
            let extends_length = directive.len() - after_extends.len();
            let (argument, trimmed) = trim_template_whitespace(after_extends);
            let argument_offset = directive_offset + extends_length + trimmed;
            if let Some((value, value_offset)) = quoted_template_string(argument)
                && value == reference
            {
                return (argument_offset + value_offset, value.len()).into();
            }
        }
        let consumed = open + 2;
        remaining = &remaining[consumed..];
        offset += 2;
    }
    directive_span.unwrap_or((0, 0).into())
}

fn trim_template_whitespace(value: &str) -> (&str, usize) {
    let trimmed = value.trim_start_matches(char::is_whitespace);
    (trimmed, value.len() - trimmed.len())
}

fn quoted_template_string(value: &str) -> Option<(&str, usize)> {
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let value = &value[quote.len_utf8()..];
    let end = value.find(quote)?;
    Some((&value[..end], quote.len_utf8()))
}

#[cfg(test)]
mod tests {
    use super::TemplateName;
    use tera::Context;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ComponentBodyVisibility {
        Visible,
        MayHide,
    }

    fn component_body_visibility(
        component: &tera::ComponentInfo,
    ) -> Result<ComponentBodyVisibility, &'static str> {
        match component
            .metadata()
            .get("heine.body_visibility")
            .and_then(tera::Value::as_str)
        {
            Some("visible") => Ok(ComponentBodyVisibility::Visible),
            Some("may-hide") => Ok(ComponentBodyVisibility::MayHide),
            Some(_) => Err("the component has an unsupported body visibility contract"),
            None => Err("the component does not declare a body visibility contract"),
        }
    }

    fn check_component_body_headings(
        visibility: ComponentBodyVisibility,
        has_headings: bool,
    ) -> Result<(), &'static str> {
        match (visibility, has_headings) {
            (ComponentBodyVisibility::MayHide, true) => {
                Err("this component may hide its body, which cannot contain Markdown headings")
            }
            _ => Ok(()),
        }
    }

    fn is_host_owned_component_value(component: &tera::ComponentInfo, name: &str) -> bool {
        name == "body"
            || component
                .args()
                .iter()
                .any(|argument| argument.is_implicit() && argument.name() == name)
    }

    fn remove_one_body_marker(
        rendered: &str,
        opening: &str,
        body: &str,
        closing: &str,
    ) -> Result<String, &'static str> {
        if rendered.matches(opening).count() != 1 || rendered.matches(closing).count() != 1 {
            return Err("the component did not preserve the body exactly once");
        }
        let opening_start = rendered
            .find(opening)
            .ok_or("the component did not preserve the body opening marker")?;
        let body_start = opening_start + opening.len();
        let closing_offset = rendered[body_start..]
            .find(closing)
            .ok_or("the component did not preserve the body closing marker")?;
        let closing_start = body_start + closing_offset;
        if &rendered[body_start..closing_start] != body {
            return Err("the component changed the rendered body");
        }
        Ok(format!(
            "{}{}{}",
            &rendered[..opening_start],
            &rendered[body_start..closing_start],
            &rendered[closing_start + closing.len()..]
        ))
    }

    #[test]
    fn template_names_are_logical_template_paths() {
        assert_eq!(
            TemplateName::parse("posts/item.tera".into())
                .expect("valid template name")
                .as_str(),
            "posts/item.tera"
        );
        assert!(TemplateName::parse("../item.tera".into()).is_err());
        assert!(TemplateName::parse("posts\\item.tera".into()).is_err());
    }

    #[test]
    fn tera_component_bodies_preserve_rendered_markdown_and_checked_context() {
        let source = "## Read this first\n\nA note with **authored Markdown**.";
        let renderer = crate::markdown::Renderer::new(crate::config::MarkdownConfig::default())
            .expect("default Markdown renderer is valid");
        let toc = crate::markdown::toc_plan(
            "main".into(),
            source,
            &crate::config::MarkdownConfig::default(),
            2,
            3,
        );
        let body = crate::markdown::render_with_links_anchors(
            source,
            &renderer,
            None,
            None,
            Some(toc.anchors()),
            None,
            &[],
        )
        .expect("ordinary Markdown body renders before component presentation");

        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "components.tera",
            r#"
{% component site.note(kind: string, @page: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}" data-page="{{ page }}">{{ body }}</aside>{% endcomponent %}
{% component site.disclosure(kind: string) {"heine.body_visibility": "may-hide"} %}<details class="note-{{ kind }}">{{ body }}</details>{% endcomponent %}
{% component site.omit(kind: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}"></aside>{% endcomponent %}
{% component site.duplicate(kind: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}">{{ body }}{{ body }}</aside>{% endcomponent %}
{% component site.filtered(kind: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}">{{ body | upper }}</aside>{% endcomponent %}
{% component site.rewritten(kind: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}">{{ body | replace(from="Markdown", to="replacement") }}</aside>{% endcomponent %}
{% component site.attribute(kind: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}" data-body="{{ body }}"></aside>{% endcomponent %}
{% component site.no_visibility(kind: string) {} %}<aside class="note-{{ kind }}">{{ body }}</aside>{% endcomponent %}
"#,
        )
        .expect("component definition is valid");

        let mut arguments = Context::new();
        arguments.insert("kind", "warning");
        let mut implicits = Context::new();
        implicits.insert("page", "guide/install");

        let opening = "<!-- heine-component-body-open -->";
        let closing = "<!-- heine-component-body-close -->";
        let marked_body = format!("{opening}{body}{closing}");
        let rendered = tera
            .render_component_with_implicits(
                "site.note",
                &arguments,
                &implicits,
                Some(&marked_body),
                true,
            )
            .expect("known component accepts its typed argument and rendered body");

        let rendered = remove_one_body_marker(&rendered, opening, &body, closing)
            .expect("the note component preserves its body exactly once");

        assert!(rendered.starts_with(r#"<aside class="note-warning" data-page="guide/install"><h2 id="read-this-first">Read this first</h2>"#), "{rendered}");
        assert!(
            rendered.contains("<strong>authored Markdown</strong>"),
            "{rendered}"
        );
        assert!(rendered.ends_with("</aside>"), "{rendered}");

        let note_definition = tera
            .get_component_definition("site.note")
            .expect("component definition is available");
        let note_visibility = component_body_visibility(&note_definition)
            .expect("the note visibility contract is readable");
        let disclosure_visibility = component_body_visibility(
            &tera
                .get_component_definition("site.disclosure")
                .expect("component definition is available"),
        )
        .expect("the disclosure visibility contract is readable");
        assert_eq!(note_visibility, ComponentBodyVisibility::Visible);
        assert_eq!(disclosure_visibility, ComponentBodyVisibility::MayHide);
        assert!(check_component_body_headings(note_visibility, true).is_ok());
        assert!(check_component_body_headings(disclosure_visibility, false).is_ok());
        assert!(check_component_body_headings(disclosure_visibility, true).is_err());
        assert!(
            component_body_visibility(
                &tera
                    .get_component_definition("site.no_visibility")
                    .expect("component definition is available"),
            )
            .is_err()
        );
        assert!(is_host_owned_component_value(&note_definition, "body"));
        assert!(!is_host_owned_component_value(&note_definition, "kind"));

        let note_with_page = tera
            .get_component_definition("site.note")
            .expect("component definition is available");
        assert!(is_host_owned_component_value(&note_with_page, "page"));

        let mut shadowing_arguments = arguments.clone();
        shadowing_arguments.insert("page", "shadowed");
        let shadowed = tera
            .render_component_with_implicits(
                "site.note",
                &shadowing_arguments,
                &implicits,
                Some(&marked_body),
                true,
            )
            .expect("Tera permits an explicit value to override an implicit one");
        assert!(
            shadowed.contains(r#"data-page="shadowed""#),
            "the host must reject this directive argument before calling Tera"
        );

        let mut escaped_arguments = Context::new();
        escaped_arguments.insert("kind", "<unsafe>");
        let escaped = tera
            .render_component_with_implicits(
                "site.note",
                &escaped_arguments,
                &implicits,
                Some(&marked_body),
                true,
            )
            .expect("the component renders with host-selected autoescaping");
        let escaped = remove_one_body_marker(&escaped, opening, &body, closing)
            .expect("autoescaping keeps the safe Markdown body intact");
        assert!(
            escaped.contains(r#"class="note-&lt;unsafe&gt;""#),
            "{escaped}"
        );
        assert!(
            escaped.contains(r#"<h2 id="read-this-first">"#),
            "{escaped}"
        );

        for component in [
            "site.omit",
            "site.duplicate",
            "site.filtered",
            "site.rewritten",
        ] {
            let output = tera
                .render_component_with_implicits(
                    component,
                    &arguments,
                    &implicits,
                    Some(&marked_body),
                    true,
                )
                .expect("the probe component itself renders");
            assert!(
                remove_one_body_marker(&output, opening, &body, closing).is_err(),
                "{component} must fail the body-preservation check"
            );
        }

        let attribute_output = tera
            .render_component_with_implicits(
                "site.attribute",
                &arguments,
                &implicits,
                Some(&marked_body),
                true,
            )
            .expect("the probe component itself renders");
        assert!(
            remove_one_body_marker(&attribute_output, opening, &body, closing).is_ok(),
            "a marker count alone does not prove that the body became element content"
        );

        arguments.insert("kind", &true);
        assert!(
            tera.render_component_with_implicits(
                "site.note",
                &arguments,
                &implicits,
                Some("<p>body</p>"),
                true,
            )
            .is_err()
        );
    }
}
