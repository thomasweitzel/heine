//! Filesystem discovery and source-aware loading of page definitions.

mod discovery;
mod policy;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_json::Value;
use thiserror::Error as ThisError;
use walkdir::WalkDir;

use crate::{
    config::{
        CollectionName, CollectionsConfig, EditorialTimeZone, SeriesConfig, SeriesName,
        TaxonomiesConfig, TaxonomyName,
    },
    content,
    locale::Locale,
    output::OutputPath,
    site::{
        Error,
        metadata::{Requirement, key_span, parse_metadata},
        page::{
            DirectoryView, LoadedPage, Metadata, NavigationWindow, OutputSuffix, Page, PageId,
            Pagination, PaginationSegment, RawMetadata, RawPage, RawRendererSource,
            SeriesMembership, SiblingWeight, Task, TaxonomyMembership, TaxonomyTermMembership,
            TranslationId,
        },
    },
    source_path,
};

pub(super) use discovery::{content_entries, relative_path, subdirectories};
pub(crate) use policy::DuplicateCollectionMembership;
pub(super) use policy::load_descriptors;
use policy::{
    DirectoryDescriptor, FootnotesPolicy, authors_for, collections_for, descriptor_ancestors,
    effective_footnotes, footnotes_for, parse_footnotes, parse_toc, template_for, toc_for,
};

/// Keeps Markdown-owned interface messages source-aware through locale
/// validation, rather than turning a missing translation into a render error.
#[derive(Debug, Diagnostic, ThisError)]
#[error("invalid {setting} configuration")]
#[diagnostic(code(heine::markdown::message_configuration))]
pub(crate) struct MarkdownMessageConfigurationDiagnostic {
    setting: &'static str,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("configured message")]
    span: SourceSpan,
    #[help]
    help: String,
    #[related]
    related: Vec<MarkdownMessageSource>,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("configured message")]
struct MarkdownMessageSource {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("message selected here")]
    span: SourceSpan,
}

impl MarkdownMessageConfigurationDiagnostic {
    pub(super) fn unused(
        source_code: NamedSource<String>,
        span: SourceSpan,
        setting: &'static str,
        help: String,
    ) -> Self {
        Self {
            setting,
            source_code,
            span,
            help,
            related: Vec::new(),
        }
    }

    pub(super) fn localization(
        source_code: NamedSource<String>,
        span: SourceSpan,
        setting: &'static str,
        locale: &Locale,
        message: String,
        help: String,
        message_source: Option<(PathBuf, String, SourceSpan)>,
    ) -> Self {
        Self {
            setting,
            source_code,
            span,
            help: format!("{help} for locale {locale}: {message}"),
            related: message_source
                .into_iter()
                .map(|(path, source, span)| MarkdownMessageSource {
                    source_code: crate::source_diagnostic::named_source_at_display_path(
                        &path, source, "Fluent",
                    ),
                    span,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("Series {series:?} is declared more than once")]
#[diagnostic(code(heine::series::duplicate_membership))]
pub(crate) struct DuplicateSeriesMembership {
    series: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("duplicate membership declared here")]
    span: SourceSpan,
    #[related]
    first: Vec<FirstSeriesMembership>,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("first Series membership is declared here")]
struct FirstSeriesMembership {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first membership")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("first declaration of this collection membership")]
struct FirstCollectionMembership {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first membership here")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("taxonomy term {term:?} is declared more than once for taxonomy {taxonomy:?}")]
#[diagnostic(code(heine::taxonomy::duplicate_term))]
pub(crate) struct DuplicateTaxonomyTerm {
    taxonomy: String,
    term: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("duplicate term here")]
    span: SourceSpan,
    #[related]
    first: Vec<FirstTaxonomyTerm>,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("first declaration of this taxonomy term")]
struct FirstTaxonomyTerm {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first term here")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("page sitemap setting cannot override an excluded directory")]
#[diagnostic(code(heine::sitemap::directory_override))]
pub(crate) struct SitemapDirectoryOverride {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this attempts to re-include the page")]
    span: SourceSpan,
    #[related]
    boundary: Vec<SitemapDirectoryBoundary>,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("this directory excludes every descendant page from the sitemap")]
struct SitemapDirectoryBoundary {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("recursive sitemap boundary")]
    span: SourceSpan,
}

/// The small source-level fact needed to decide whether a page belongs to the
/// published catalog before its richer page relationships are resolved.
pub(super) struct PageLoadContext<'a> {
    pub(super) project_content_root: &'a Path,
    pub(super) citation_inputs: &'a mut super::citation_inputs::InputCache,
    pub(super) content_root: &'a Path,
    pub(super) diagnostics: &'a crate::source_diagnostic::DiagnosticPaths,
    pub(super) locale: &'a Locale,
    pub(super) output_prefix: &'a Path,
    pub(super) subdirectories: &'a BTreeMap<PathBuf, Vec<String>>,
    pub(super) descriptors: &'a BTreeMap<PathBuf, DirectoryDescriptor>,
    pub(super) collections: &'a CollectionsConfig,
    pub(super) series: Option<&'a SeriesConfig>,
    pub(super) taxonomies: &'a TaxonomiesConfig,
    pub(super) time_zone: Option<&'a EditorialTimeZone>,
    pub(super) sitemap_enabled: bool,
    pub(super) markdown: &'a crate::config::MarkdownConfig,
}

pub(super) struct PageProbe {
    text: String,
    draft: bool,
    content_inputs: BTreeSet<PathBuf>,
    footnotes: DraftFootnotes,
}

#[derive(Clone, Copy)]
enum DraftFootnotes {
    Absent,
    Disabled,
    Present,
}

#[derive(serde::Deserialize)]
struct RawPageProbe {
    draft: Option<toml::Spanned<toml::Value>>,
    content: Option<toml::Spanned<toml::Value>>,
    footnotes: Option<toml::Spanned<toml::Value>>,
    #[serde(default)]
    render: BTreeMap<toml::Spanned<String>, RawRendererSource>,
}

impl PageProbe {
    pub(super) fn is_draft(&self) -> bool {
        self.draft
    }

    pub(super) fn content_inputs(&self) -> &BTreeSet<PathBuf> {
        &self.content_inputs
    }

    pub(super) fn into_text(self) -> String {
        self.text
    }
}

pub(super) fn probe_page(
    path: &Path,
    content_root: &Path,
    project_content_root: &Path,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<PageProbe, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let raw: RawPageProbe =
        toml::from_str(&text).map_err(|error| page_toml_error(path, &text, error, diagnostics))?;
    let draft = match raw.draft {
        None => false,
        Some(value) => {
            let span: SourceSpan = value.span().into();
            match value.into_inner() {
                toml::Value::Boolean(true) => true,
                toml::Value::Boolean(false) => {
                    return Err(invalid_with(
                        diagnostics,
                        path,
                        &text,
                        span,
                        "field `draft` accepts only true; omit it for a published page",
                    ));
                }
                _ => {
                    return Err(invalid_with(
                        diagnostics,
                        path,
                        &text,
                        span,
                        "field `draft` must be true; omit it for a published page",
                    ));
                }
            }
        }
    };
    let footnotes = match raw.footnotes.map(toml::Spanned::into_inner) {
        None => DraftFootnotes::Absent,
        Some(toml::Value::Boolean(false)) => DraftFootnotes::Disabled,
        Some(_) => DraftFootnotes::Present,
    };
    let mut content_inputs = BTreeSet::new();
    if draft && let Some(content) = raw.content {
        let span: SourceSpan = content.span().into();
        let toml::Value::Table(entries) = content.into_inner() else {
            return Err(invalid_with(
                diagnostics,
                path,
                &text,
                span,
                "field `content` must be a table of declared content files",
            ));
        };
        for (name, value) in entries {
            let Some(file) = value.as_str() else {
                return Err(invalid(
                    path,
                    &text,
                    span,
                    format!("declared content file {name:?} must be a string"),
                ));
            };
            let content_path = resolve_content_path(content_root, path, file)
                .map_err(|message| invalid_with(diagnostics, path, &text, span, message))?;
            content_inputs.insert(content_path);
        }
    }
    if draft {
        let renderer_sources = renderer_sources_for(
            raw.render,
            project_content_root,
            path,
            &diagnostics.display_path(path),
            &text,
        )?;
        for renderer_source in renderer_sources.values() {
            debug_assert!(renderer_source.entry().starts_with(renderer_source.root()));
            content_inputs.extend(renderer_source.files().iter().cloned());
        }
    }
    Ok(PageProbe {
        text,
        draft,
        content_inputs,
        footnotes,
    })
}

pub(super) fn draft_uses_margin_footnotes(
    probe: &PageProbe,
    relative: &Path,
    descriptors: &BTreeMap<PathBuf, DirectoryDescriptor>,
) -> bool {
    match probe.footnotes {
        DraftFootnotes::Disabled => false,
        DraftFootnotes::Present => true,
        DraftFootnotes::Absent => {
            let ancestors =
                descriptor_ancestors(relative.parent().unwrap_or(Path::new("")), descriptors);
            effective_footnotes(&ancestors)
                .and_then(|descriptor| descriptor.footnotes.as_ref())
                .is_some_and(|policy| matches!(policy, FootnotesPolicy::Enabled(_)))
        }
    }
}

pub(super) fn load_page(
    source_path: &Path,
    relative: &Path,
    text: String,
    draft: bool,
    context: PageLoadContext<'_>,
) -> Result<LoadedPage, Error> {
    let path = &context.diagnostics.display_path(source_path);
    let raw: RawPage = toml::from_str(&text)
        .map_err(|error| page_toml_error(path, &text, error, context.diagnostics))?;
    if let Some(date) = &raw.legacy_date {
        return Err(invalid(
            path,
            &text,
            date.span().into(),
            "field `date` is no longer supported; use `published = 2026-08-14T09:30:00` with site.time_zone instead",
        ));
    }
    let id = PageId::from_logical_path(
        source_path::LogicalPath::from_path(&relative.with_extension(""))
            .expect("discovered page IDs remain logical"),
    );
    let directory_path = relative.parent().unwrap_or(Path::new(""));
    let ancestors = descriptor_ancestors(directory_path, context.descriptors);
    let template = template_for(raw.template, &ancestors, path, &text)?;
    let page_toc = raw
        .toc
        .map(|value| parse_toc(value, path, &text, "toc"))
        .transpose()?;
    let toc_config = toc_for(page_toc.as_ref(), &ancestors, path, &text)?;
    let page_footnotes = raw
        .footnotes
        .map(|value| parse_footnotes(value, path, &text, "footnotes"))
        .transpose()?;
    let footnotes_config = footnotes_for(page_footnotes.as_ref(), &ancestors, path, &text)?;
    if let Some(toc) = &toc_config
        && template.is_none()
    {
        return Err(invalid(
            toc.path,
            toc.source,
            toc.settings.span,
            format!(
                "a table of contents for page {id:?} requires that page to have a template; configure one or suppress the inherited default with toc = false"
            ),
        ));
    }
    if let Some(footnotes) = &footnotes_config {
        if template.is_none() {
            return Err(invalid(
                footnotes.path,
                footnotes.source,
                footnotes.settings.span,
                format!(
                    "margin footnotes for page {id:?} require that page to have a template; configure one or suppress the inherited setting with footnotes = false"
                ),
            ));
        }
        if !context
            .markdown
            .extensions()
            .contains(&crate::config::MarkdownExtension::Footnotes)
        {
            return Err(invalid(
                footnotes.path,
                footnotes.source,
                footnotes.settings.span,
                "margin footnotes require markdown.extensions to include \"footnotes\"",
            ));
        }
    }
    let suffix_span = raw.suffix.span();
    let suffix: OutputSuffix = raw
        .suffix
        .into_inner()
        .try_into()
        .map_err(|message| invalid(path, &text, suffix_span.clone().into(), message))?;
    let taxonomies = taxonomies_for(raw.taxonomies, context.taxonomies, path, &text)?;
    let collections = collections_for(
        raw.collections,
        &ancestors,
        context.collections,
        path,
        &text,
    )?;
    let series = series_for(raw.series, context.series, path, &text)?;
    let authors = authors_for(raw.authors, &ancestors, path, &text)?;
    let pagination = pagination_for(
        raw.pagination,
        template.is_some(),
        context.collections,
        path,
        &text,
    )?;
    let weight_span = raw.weight.as_ref().map(|value| value.span().into());
    let weight = parse_weight(raw.weight.as_ref(), &id)
        .map_err(|message| invalid(path, &text, weight_span.unwrap_or((0, 0).into()), message))?;
    let mut latest = metadata_modified(source_path)?;
    for descriptor in &ancestors {
        latest = latest.max(descriptor.modified);
    }
    let mut content = BTreeMap::new();
    let mut content_inputs = BTreeSet::new();
    let mut markdown_sources = BTreeMap::new();
    let mut markdown_names = BTreeSet::new();
    let renderer_sources = renderer_sources_for(
        raw.render,
        context.project_content_root,
        source_path,
        path,
        &text,
    )?;
    for source in renderer_sources.values() {
        debug_assert!(source.entry().starts_with(source.root()));
        for file in source.files() {
            latest = latest.max(content_modified(file).map_err(|error| {
                invalid(
                    path,
                    &text,
                    (0, 0).into(),
                    format!(
                        "could not access declared renderer source {}: {error}",
                        file.display()
                    ),
                )
            })?);
            content_inputs.insert(file.clone());
        }
    }
    for (name, file) in raw.content {
        let file_span: SourceSpan = file.span().into();
        if !source_path::is_simple_identifier(&name) {
            return Err(invalid(
                path,
                &text,
                key_span(&text, file_span, &name),
                format!("content name {name:?} must be a simple template identifier"),
            ));
        }
        let file = file.into_inner();
        let content_path = resolve_content_path(context.content_root, source_path, &file)
            .map_err(|message| invalid(path, &text, file_span, message))?;
        latest = latest.max(content_modified(&content_path).map_err(|error| {
            invalid(
                path,
                &text,
                file_span,
                format!("could not access declared content file {file:?}: {error}"),
            )
        })?);
        content_inputs.insert(content_path.clone());
        let loaded =
            content::load(&content_path, context.diagnostics).map_err(|error| match error {
                content::Error::Read { source, .. } => invalid(
                    path,
                    &text,
                    file_span,
                    format!("could not read declared content file {file:?}: {source}"),
                ),
                error => Error::Content(error),
            })?;
        if let Some(source) = loaded.markdown_source {
            markdown_names.insert(name.clone());
            markdown_sources.insert(name.clone(), source);
        }
        content.insert(name, loaded.value);
    }
    let page_citations = raw
        .citations
        .map(|value| policy::parse_citations(value, path, &text, false))
        .transpose()?;
    let declarations =
        ancestors
            .iter()
            .filter_map(|descriptor| {
                descriptor.citations.as_ref().map(|policy| {
                    super::citation_inputs::policy::Declaration {
                        policy,
                        path: &descriptor.path,
                        source: &descriptor.text,
                    }
                })
            })
            .chain(page_citations.as_ref().map(|policy| {
                super::citation_inputs::policy::Declaration {
                    policy,
                    path,
                    source: &text,
                }
            }));
    let citation_setting =
        super::citation_inputs::policy::resolve(declarations).map_err(|error| {
            let (error_path, span) = match &error {
                super::citation_inputs::policy::Error::TrueSwitch { path, span }
                | super::citation_inputs::policy::Error::NothingToDisable { path, span } => {
                    (path.as_ref(), span.clone())
                }
            };
            let source = if error_path == path {
                &text
            } else {
                ancestors
                    .iter()
                    .find(|item| item.path == error_path)
                    .map_or(&text, |item| &item.text)
            };
            invalid(error_path, source, span.into(), error.to_string())
        })?;
    let citations = citation_setting
        .map(|setting| {
            let root = context
                .project_content_root
                .parent()
                .unwrap_or(context.project_content_root);
            context
                .citation_inputs
                .plan(
                    setting.settings,
                    &root.join("citations"),
                    &content,
                    &markdown_names,
                    (setting.path, setting.source),
                )
                .map_err(|error| {
                    if let super::citation_inputs::PlanError::Inputs(
                        super::citation_inputs::SettingsError::Bibliography(bib_error),
                    ) = &error
                        && let Some(report) = citation_input_diagnostic(
                            bib_error,
                            &root.join("citations"),
                            context.diagnostics,
                        )
                    {
                        return Error::CitationInput(Box::new(report));
                    }
                    let span = match &error {
                        super::citation_inputs::PlanError::Content { span, .. } => span.clone(),
                        super::citation_inputs::PlanError::Inputs(
                            super::citation_inputs::SettingsError::Input { span, .. }
                            | super::citation_inputs::SettingsError::Content { span }
                            | super::citation_inputs::SettingsError::Locale { span, .. }
                            | super::citation_inputs::SettingsError::Style { span, .. },
                        ) => span.clone(),
                        _ => 0..0,
                    };
                    let mut message = error.to_string();
                    let mut cause = std::error::Error::source(&error);
                    while let Some(source) = cause {
                        message.push_str(": ");
                        message.push_str(&source.to_string());
                        cause = source.source();
                    }
                    invalid(setting.path, setting.source, span.into(), message)
                })
        })
        .transpose()?;
    if let Some(plan) = &citations
        && template.is_none()
    {
        return Err(invalid(
            &plan.declaration_path,
            &plan.declaration_source,
            plan.declaration_span.clone().into(),
            "citations require a page template to render the source and bibliography",
        ));
    }
    let toc_input = if let Some(config) = toc_config {
        let settings = config.settings;
        if !markdown_names.contains(&settings.content) {
            return Err(invalid(
                config.path,
                config.source,
                settings.content_span,
                format!(
                    "toc.content {:?} must name a declared Markdown file on page {id:?}",
                    settings.content
                ),
            ));
        }
        let Some(markdown) = content.get(&settings.content).and_then(Value::as_str) else {
            return Err(invalid(
                config.path,
                config.source,
                settings.content_span,
                format!(
                    "toc.content {:?} must be one unsegmented Markdown document on page {id:?}",
                    settings.content
                ),
            ));
        };
        Some((settings, markdown))
    } else {
        None
    };
    let footnote_input = if let Some(config) = footnotes_config {
        let settings = config.settings;
        let Some(markdown) = markdown_sources.get(&settings.content) else {
            return Err(invalid(
                config.path,
                config.source,
                settings.content_span,
                format!(
                    "footnotes.content {:?} must name a declared unsegmented Markdown file on page {id:?}",
                    settings.content
                ),
            ));
        };
        let marker = context.markdown.margin_footnotes()
            .map(crate::config::MarginFootnotesConfig::marker_message)
            .ok_or_else(|| invalid(config.path, config.source, settings.span,
                "margin footnotes require markdown.margin_footnotes.marker_message in heine.toml"))?;
        Some((settings, markdown, marker))
    } else {
        None
    };

    let mut component_plans = BTreeMap::new();
    if let Some((settings, source, _)) = &footnote_input
        && source.source.contains(":::component[")
        && let Some(plan) = crate::markdown::ComponentPlan::parse_for_planning(&source.source)
            .map_err(|error| {
                Error::Markdown(Box::new(crate::markdown::source_error(
                    &source.display_path,
                    &source.source,
                    error,
                )))
            })?
    {
        component_plans.insert(settings.content.clone(), plan);
    }
    if let Some((settings, markdown)) = &toc_input
        && !component_plans.contains_key(&settings.content)
        && footnote_input
            .as_ref()
            .is_none_or(|(footnotes, _, _)| footnotes.content != settings.content)
        && markdown.contains(":::component[")
        && let Ok(Some(plan)) = crate::markdown::ComponentPlan::parse_for_planning(markdown)
    {
        // A table-of-contents-only component error retains its later diagnostic order.
        component_plans.insert(settings.content.clone(), plan);
    }

    let mut shared_footnotes = None;
    let toc = toc_input.map(|(settings, markdown)| {
        let input = component_plans
            .get(&settings.content)
            .map_or(markdown, |plan| plan.normalized());
        if let Some((_, source, marker)) = footnote_input
            .as_ref()
            .filter(|(footnotes, _, _)| footnotes.content == settings.content)
        {
            let (toc, notes) = crate::markdown::margin_footnotes::plan_with_toc(
                &source.display_path,
                settings.content.clone(),
                (input, &source.source),
                context.markdown,
                (*marker).to_owned(),
                settings.min_level,
                settings.max_level,
            );
            shared_footnotes = Some(notes);
            toc
        } else {
            crate::markdown::toc_plan(
                settings.content.clone(),
                input,
                context.markdown,
                settings.min_level,
                settings.max_level,
            )
        }
    });
    let footnotes = if let Some((settings, source, marker)) = footnote_input {
        let input = component_plans
            .get(&settings.content)
            .map_or(source.source.as_str(), |plan| plan.normalized());
        Some(
            match shared_footnotes {
                Some(result) => result,
                None => crate::markdown::margin_footnotes::plan(
                    &source.display_path,
                    settings.content.clone(),
                    (input, &source.source),
                    context.markdown,
                    marker.to_owned(),
                ),
            }
            .map_err(Error::MarginFootnotes)?,
        )
    } else {
        None
    };
    let metadata = parse_metadata(
        RawMetadata {
            title: raw.title,
            description: raw.description,
            summary: raw.summary,
            authors,
            published: raw.published,
            updated: raw.updated,
            extra: raw.extra,
        },
        path,
        &text,
        context.time_zone,
    )?;
    if let (Some(published), Some(updated)) = (&metadata.published, &metadata.updated)
        && updated < published
    {
        return Err(invalid(
            path,
            &text,
            metadata.updated_span.unwrap_or((0, 0).into()),
            "field `updated` must not be earlier than `published`",
        ));
    }
    let sitemap_include = sitemap_for(
        raw.sitemap,
        template.is_some(),
        context.sitemap_enabled,
        &ancestors,
        path,
        &text,
    )?;
    let requirements = ancestors.iter().flat_map(|descriptor| {
        descriptor
            .page_requirements
            .iter()
            .map(|requirement| (*descriptor, requirement))
    });
    validate_requirements("page", path, &text, &metadata, requirements)?;
    let translationid_span = raw.translationid.as_ref().map(|value| value.span().into());
    let translationid = raw
        .translationid
        .map(toml::Spanned::into_inner)
        .map(|value| {
            TranslationId::parse(value).map_err(|message| {
                invalid(
                    path,
                    &text,
                    translationid_span.unwrap_or((0, 0).into()),
                    message,
                )
            })
        })
        .transpose()?;
    let output_path = template
        .as_ref()
        .map(|_| {
            let derived_output = context
                .output_prefix
                .join(relative)
                .with_extension(suffix.as_str());
            OutputPath::try_new(derived_output.clone()).map_err(|message| {
                Error::InvalidPageOutput(Box::new(PageOutputPathDiagnostic {
                    path: path.into(),
                    output: derived_output,
                    message,
                }))
            })
        })
        .transpose()?;
    let (path_value, filename) = match &output_path {
        Some(output) => (
            Some(output.url_path()),
            output
                .as_path()
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
        ),
        None => (None, None),
    };
    let directory = relative
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|path| {
            source_path::LogicalPath::from_path(path)
                .expect("discovered page directories remain logical")
                .as_str()
                .to_owned()
        })
        .unwrap_or_default();
    let source_dir = source_path.parent().expect("page file has parent");
    let subdirs = context
        .subdirectories
        .get(source_dir)
        .cloned()
        .unwrap_or_default();
    let sitemap_lastmod = metadata.updated.or(metadata.published);
    let page_source =
        crate::source_diagnostic::named_source_at_display_path(path, text.clone(), "TOML");
    let page = Page {
        id: id.clone(),
        output_path: output_path.clone(),
        path: path_value,
        dir: directory.clone(),
        filename,
        subdirs,
        content,
        markdown_content: markdown_names,
        markdown_sources,
        renderer_sources,
        renderer_requests: BTreeMap::new(),
        renderer_replacements: BTreeMap::new(),
        component_plans,
        component_sources: BTreeMap::new(),
        renderer_resources: Vec::new(),
        markdown_facts: BTreeMap::new(),
        markdown_search_sections: None,
        toc,
        footnotes,
        citations,
        draft,
        metadata,
        translationid,
        translationid_span,
        source: page_source.clone(),
        directory: context
            .descriptors
            .get(directory_path)
            .map(|descriptor| DirectoryView::new(directory.clone(), &descriptor.metadata))
            .unwrap_or_else(|| DirectoryView::new(directory.clone(), &Metadata::default())),
        taxonomies,
        collections,
        series,
        pagination,
        weight,
        weight_span,
        translations: BTreeMap::new(),
    };
    let task = template.zip(output_path).map(
        |((template, template_span, template_source), output_path)| Task {
            locale: context.locale.clone(),
            template_span,
            output_span: suffix_span.into(),
            page_id: id,
            template,
            page_source: Arc::new(page_source),
            template_source: Arc::new(template_source),
            output_path,
            latest_input: latest,
            pagination: None,
            sitemap_include,
            sitemap_lastmod,
        },
    );
    Ok(LoadedPage {
        page,
        task,
        content_inputs,
    })
}

fn page_toml_error(
    path: &Path,
    text: &str,
    error: toml::de::Error,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Error {
    Error::PageToml(Box::new(PageTomlDiagnostic {
        path: diagnostics.display_path(path),
        message: error.message().to_owned(),
        source_code: diagnostics.named_source(path, text.to_owned(), "TOML"),
        span: error.span().unwrap_or(0..0).into(),
    }))
}

fn pagination_for(
    raw: Option<super::page::RawPagination>,
    rendered: bool,
    collections: &CollectionsConfig,
    path: &Path,
    source: &str,
) -> Result<Option<Pagination>, Error> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let collection_span: SourceSpan = raw.collection.span().into();
    let collection_value = raw.collection.into_inner();
    let collection = CollectionName::parse(collection_value.clone())
        .map_err(|message| invalid(path, source, collection_span, message))?;
    if !collections.contains(&collection) {
        return Err(invalid(
            path,
            source,
            collection_span,
            format!(
                "pagination collection {collection_value:?} is not configured; add a [collections.{collection_value}] table to heine.toml"
            ),
        ));
    }
    if !rendered {
        return Err(invalid(
            path,
            source,
            collection_span,
            "pagination requires this page to have a template; add `template = \"…\"` or remove [pagination]",
        ));
    }
    let per_page_span: SourceSpan = raw.per_page.span().into();
    let per_page = raw.per_page.into_inner();
    if per_page == 0 {
        return Err(invalid(
            path,
            source,
            per_page_span,
            "pagination.per_page must be a positive integer",
        ));
    }
    let has_authored_segment = !raw.segment.span().is_empty();
    let segment_span: SourceSpan = raw.segment.span().into();
    let segment = PaginationSegment::parse(raw.segment.into_inner())
        .map_err(|message| invalid(path, source, segment_span, message))?;
    let window_span: SourceSpan = raw.navigation.window.span().into();
    let navigation_window = NavigationWindow::parse(raw.navigation.window.into_inner())
        .map_err(|message| invalid(path, source, window_span, message))?;
    Ok(Some(Pagination {
        collection,
        per_page,
        segment,
        navigation_window,
        route_span: if has_authored_segment {
            segment_span
        } else {
            collection_span
        },
    }))
}

fn sitemap_for(
    raw: Option<toml::Spanned<bool>>,
    rendered: bool,
    sitemap_enabled: bool,
    ancestors: &[&DirectoryDescriptor],
    path: &Path,
    source: &str,
) -> Result<bool, Error> {
    let directory_exclusion = ancestors
        .iter()
        .rev()
        .find_map(|descriptor| descriptor.sitemap_exclusion.map(|span| (*descriptor, span)));
    let Some(raw) = raw else {
        return Ok(directory_exclusion.is_none());
    };
    let span: SourceSpan = raw.span().into();
    if !sitemap_enabled {
        return Err(invalid(
            path,
            source,
            span,
            "field `sitemap` on a page requires [sitemap] in heine.toml",
        ));
    }
    if !rendered {
        return Err(invalid(
            path,
            source,
            span,
            "field `sitemap` requires this page to have a template",
        ));
    }
    if raw.into_inner() {
        if let Some((descriptor, boundary_span)) = directory_exclusion {
            return Err(Error::SitemapDirectoryOverride(Box::new(
                SitemapDirectoryOverride {
                    source_code: Arc::new(crate::source_diagnostic::named_source_at_display_path(
                        path,
                        source.to_owned(),
                        "TOML",
                    )),
                    span,
                    boundary: vec![SitemapDirectoryBoundary {
                        source_code: Arc::new(descriptor.source.clone()),
                        span: boundary_span,
                    }],
                },
            )));
        }
        return Err(invalid(
            path,
            source,
            span,
            "field `sitemap` accepts only false; omit it to include this page",
        ));
    }
    Ok(false)
}

/// Parses page-local Series memberships. Unlike collections, Series never
/// inherit from a directory because each page owns its own position.
fn series_for(
    values: Vec<super::page::RawSeriesMembership>,
    config: Option<&SeriesConfig>,
    path: &Path,
    source: &str,
) -> Result<Vec<SeriesMembership>, Error> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let first_span: SourceSpan = values[0].id.span().into();
    let config = config.ok_or_else(|| {
        invalid(
            path,
            source,
            first_span,
            "field `series` requires a [series] table in heine.toml",
        )
    })?;
    let named_source = Arc::new(crate::source_diagnostic::named_source_at_display_path(
        path,
        source.to_owned(),
        "TOML",
    ));
    let mut memberships = Vec::with_capacity(values.len());
    for value in values {
        let span: SourceSpan = value.id.span().into();
        let raw = value.id.into_inner();
        let name = SeriesName::parse(raw.clone())
            .map_err(|message| invalid(path, source, span, message))?;
        if !config.contains(&name) {
            let available = config
                .definitions()
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let detail = if available.is_empty() {
                "no Series definitions are configured".to_owned()
            } else {
                format!("available Series: {}", available.join(", "))
            };
            return Err(invalid(
                path,
                source,
                span,
                format!("series {raw:?} is not configured; {detail}"),
            ));
        }
        if let Some(first) = memberships
            .iter()
            .find(|membership: &&SeriesMembership| membership.name == name)
        {
            return Err(Error::DuplicateSeriesMembership(Box::new(
                DuplicateSeriesMembership {
                    series: raw,
                    source_code: named_source,
                    span,
                    first: vec![FirstSeriesMembership {
                        source_code: first.source.clone(),
                        span: first.span,
                    }],
                },
            )));
        }
        let position_span: SourceSpan = value.position.span().into();
        let position = value.position.into_inner();
        let position = usize::try_from(position)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                invalid(
                    path,
                    source,
                    position_span,
                    "series position must be a positive integer",
                )
            })?;
        memberships.push(SeriesMembership {
            name,
            position,
            source: named_source.clone(),
            span,
            position_span,
        });
    }
    Ok(memberships)
}

fn taxonomies_for(
    values: BTreeMap<toml::Spanned<String>, Vec<toml::Spanned<String>>>,
    config: &TaxonomiesConfig,
    path: &Path,
    source: &str,
) -> Result<Vec<TaxonomyMembership>, Error> {
    let named_source = Arc::new(crate::source_diagnostic::named_source_at_display_path(
        path,
        source.to_owned(),
        "TOML",
    ));
    let mut memberships = Vec::with_capacity(values.len());
    for (name, values) in values {
        let name_span: SourceSpan = name.span().into();
        let raw_name = name.into_inner();
        let name = TaxonomyName::parse(raw_name.clone())
            .map_err(|message| invalid(path, source, name_span, message))?;
        if !config.contains(&name) {
            return Err(invalid(
                path,
                source,
                name_span,
                format!("taxonomy {raw_name:?} is not configured"),
            ));
        }
        if values.is_empty() {
            return Err(invalid(
                path,
                source,
                name_span,
                "taxonomy term lists must contain at least one term",
            ));
        }
        let mut terms = Vec::with_capacity(values.len());
        for value in values {
            let span: SourceSpan = value.span().into();
            let term = value.into_inner();
            if term.is_empty() {
                return Err(invalid(
                    path,
                    source,
                    span,
                    "taxonomy terms must not be empty",
                ));
            }
            if !crate::output::is_safe_output_component(&term) {
                return Err(invalid(
                    path,
                    source,
                    span,
                    "taxonomy terms must be portable output filename components",
                ));
            }
            if let Some(first) = terms
                .iter()
                .find(|existing: &&TaxonomyTermMembership| existing.term == term)
            {
                return Err(Error::DuplicateTaxonomyTerm(Box::new(
                    DuplicateTaxonomyTerm {
                        taxonomy: name.to_string(),
                        term,
                        source_code: named_source,
                        span,
                        first: vec![FirstTaxonomyTerm {
                            source_code: first.source.clone(),
                            span: first.span,
                        }],
                    },
                )));
            }
            terms.push(TaxonomyTermMembership {
                term,
                source: named_source.clone(),
                span,
            });
        }
        memberships.push(TaxonomyMembership { name, terms });
    }
    Ok(memberships)
}

fn validate_requirements<'a>(
    subject: &str,
    path: &Path,
    source: &str,
    metadata: &Metadata,
    requirements: impl Iterator<Item = (&'a DirectoryDescriptor, &'a Requirement)>,
) -> Result<(), Error> {
    for (descriptor, requirement) in requirements {
        if !metadata.field(&requirement.name) {
            return Err(Error::MissingMetadata(Box::new(MissingMetadata {
                subject: subject.into(),
                field: requirement.name.clone(),
                path: path.into(),
                source_code: crate::source_diagnostic::named_source_at_display_path(
                    path,
                    source.to_owned(),
                    "TOML",
                ),
                span: (0, 0).into(),
                related: vec![RequirementOrigin {
                    source_code: descriptor.source.clone(),
                    span: requirement.span,
                }],
            })));
        }
    }
    Ok(())
}

fn resolve_content_path(
    content_root: &Path,
    page_path: &Path,
    reference: &str,
) -> Result<PathBuf, String> {
    let page_dir = page_path
        .parent()
        .and_then(|path| path.strip_prefix(content_root).ok())
        .ok_or_else(|| "page is outside content root".to_owned())?;
    let mut relative = page_dir.to_path_buf();
    for component in Path::new(reference).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !relative.pop() {
                    return Err(format!(
                        "content file {reference:?} escapes the content directory"
                    ));
                }
            }
            Component::Normal(part) => relative.push(part),
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "content file {reference:?} must be relative to its page file"
                ));
            }
        }
    }
    Ok(content_root.join(relative))
}

fn renderer_sources_for(
    declarations: BTreeMap<toml::Spanned<String>, RawRendererSource>,
    project_content_root: &Path,
    page_path: &Path,
    diagnostic_path: &Path,
    source: &str,
) -> Result<BTreeMap<String, crate::renderer::Source>, Error> {
    let mut resolved = BTreeMap::new();
    for (name, declaration) in declarations {
        let name_span = name.span().into();
        let name = name.into_inner();
        if !source_path::is_simple_identifier(&name) {
            return Err(invalid(
                diagnostic_path,
                source,
                name_span,
                format!("renderer source name {name:?} must be a simple identifier"),
            ));
        }
        let RawRendererSource { file, root, entry } = declaration;
        let renderer_source = match (file, root, entry) {
            (Some(file), None, None) => {
                let span = file.span().into();
                let reference = file.into_inner();
                let path = resolve_renderer_path(project_content_root, page_path, &reference)
                    .map_err(|message| invalid(diagnostic_path, source, span, message))?;
                check_renderer_file(&path, diagnostic_path, source, span, &reference)?;
                crate::renderer::Source::file(path, span)
            }
            (None, Some(root), Some(entry)) => {
                let root_span = root.span().into();
                let root_reference = root.into_inner();
                let root = resolve_renderer_path(project_content_root, page_path, &root_reference)
                    .map_err(|message| invalid(diagnostic_path, source, root_span, message))?;
                let metadata = fs::symlink_metadata(&root).map_err(|error| {
                    invalid(
                        diagnostic_path,
                        source,
                        root_span,
                        format!("could not access renderer source bundle root {root_reference:?}: {error}"),
                    )
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(invalid(
                        diagnostic_path,
                        source,
                        root_span,
                        format!(
                            "renderer source bundle root {root_reference:?} must be a non-symbolic-link directory"
                        ),
                    ));
                }
                let entry_span = entry.span().into();
                let entry_reference = entry.into_inner();
                let entry_path = resolve_renderer_path_from(&root, &root, &entry_reference)
                    .map_err(|message| invalid(diagnostic_path, source, entry_span, message))?;
                check_renderer_file(
                    &entry_path,
                    diagnostic_path,
                    source,
                    entry_span,
                    &entry_reference,
                )?;
                let mut files = Vec::new();
                for discovered in WalkDir::new(&root).sort_by_file_name() {
                    let discovered = discovered.map_err(|error| {
                        invalid(
                            diagnostic_path,
                            source,
                            root_span,
                            format!(
                                "could not walk renderer source bundle {root_reference:?}: {error}"
                            ),
                        )
                    })?;
                    if discovered.file_type().is_symlink() {
                        return Err(invalid(
                            diagnostic_path,
                            source,
                            root_span,
                            format!(
                                "renderer source bundle {root_reference:?} contains symbolic link {}",
                                discovered.path().display()
                            ),
                        ));
                    }
                    if discovered.file_type().is_file() {
                        files.push(discovered.into_path());
                    }
                }
                crate::renderer::Source::bundle(root, entry_path, files, root_span)
            }
            _ => {
                return Err(invalid(
                    diagnostic_path,
                    source,
                    (0, 0).into(),
                    format!(
                        "renderer source {name:?} must declare either file = \"...\" or both root = \"...\" and entry = \"...\""
                    ),
                ));
            }
        };
        resolved.insert(name, renderer_source);
    }
    let mut owners = BTreeMap::new();
    for (name, renderer_source) in &resolved {
        for file in renderer_source.files() {
            if let Some(first) = owners.insert(file, name) {
                return Err(invalid(
                    diagnostic_path,
                    source,
                    renderer_source.declaration_span(),
                    format!(
                        "renderer source {name:?} overlaps renderer source {first:?} at {}",
                        file.display()
                    ),
                ));
            }
        }
    }
    Ok(resolved)
}

fn resolve_renderer_path(
    project_content_root: &Path,
    page_path: &Path,
    reference: &str,
) -> Result<PathBuf, String> {
    let page_dir = page_path
        .parent()
        .ok_or_else(|| "page has no parent directory".to_owned())?;
    resolve_renderer_path_from(project_content_root, page_dir, reference)
}

fn resolve_renderer_path_from(
    root: &Path,
    start: &Path,
    reference: &str,
) -> Result<PathBuf, String> {
    if Path::new(reference).is_absolute() {
        return Err(format!(
            "renderer source {reference:?} must be relative to its page file"
        ));
    }
    let mut path = start.to_path_buf();
    for component in Path::new(reference).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if path == root {
                    return Err(format!(
                        "renderer source {reference:?} escapes the content directory"
                    ));
                }
                path.pop();
            }
            Component::Normal(part) if part.to_str().is_some_and(|part| !part.contains('\\')) => {
                path.push(part)
            }
            Component::Normal(_) => {
                return Err(format!(
                    "renderer source {reference:?} must use UTF-8 path components without backslashes"
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "renderer source {reference:?} must be relative to its page file"
                ));
            }
        }
    }
    if path.starts_with(root) {
        Ok(path)
    } else {
        Err(format!(
            "renderer source {reference:?} escapes the content directory"
        ))
    }
}

fn check_renderer_file(
    path: &Path,
    diagnostic_path: &Path,
    source: &str,
    span: SourceSpan,
    reference: &str,
) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        invalid(
            diagnostic_path,
            source,
            span,
            format!("could not access renderer source {reference:?}: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid(
            diagnostic_path,
            source,
            span,
            format!("renderer source {reference:?} must be a regular non-symbolic-link file"),
        ));
    }
    Ok(())
}

fn metadata_modified(path: &Path) -> Result<SystemTime, Error> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| Error::Io {
            path: path.into(),
            source,
        })
}

fn content_modified(path: &Path) -> Result<SystemTime, std::io::Error> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "declared content is not a regular file",
        ));
    }
    metadata.modified()
}

pub(super) fn invalid(
    path: &Path,
    source: &str,
    span: SourceSpan,
    message: impl Into<String>,
) -> Error {
    Error::InvalidPage(Box::new(PageDiagnostic {
        path: path.into(),
        message: message.into(),
        source_code: crate::source_diagnostic::named_source_at_display_path(path, source, "TOML"),
        span,
    }))
}

fn invalid_with(
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    path: &Path,
    source: &str,
    span: SourceSpan,
    message: impl Into<String>,
) -> Error {
    Error::InvalidPage(Box::new(PageDiagnostic {
        path: diagnostics.display_path(path),
        message: message.into(),
        source_code: diagnostics.named_source(path, source, "TOML"),
        span,
    }))
}

fn parse_weight(
    value: Option<&toml::Spanned<toml::Value>>,
    id: &PageId,
) -> Result<SiblingWeight, String> {
    match value {
        None if !id.is_index() => Ok(SiblingWeight::Ordered(0)),
        None => Ok(SiblingWeight::Excluded),
        Some(value) => match value.get_ref() {
            toml::Value::Integer(value) => Ok(SiblingWeight::Ordered(*value)),
            toml::Value::Boolean(false) if id.is_index() => Err(
                "field `weight = false` is redundant for an index page; omit `weight`, or set a numeric weight to join sibling ordering"
                    .into(),
            ),
            toml::Value::Boolean(false) => Ok(SiblingWeight::Excluded),
            _ => Err("field `weight` must be an integer or false".into()),
        },
    }
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("invalid TOML in content definition {path}")]
#[diagnostic(code(heine::page::invalid_toml))]
pub(crate) struct PageTomlDiagnostic {
    path: PathBuf,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{message}")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("invalid bibliography {path}: {message}")]
#[diagnostic(code(heine::citation::bibliography))]
pub(crate) struct CitationInputDiagnostic {
    path: PathBuf,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("bibliography problem here")]
    span: SourceSpan,
    #[related]
    related: Vec<CitationInputRelated>,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("first declaration is here")]
struct CitationInputRelated {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("first declaration")]
    span: SourceSpan,
}

pub(super) fn citation_input_diagnostic(
    error: &super::citation_inputs::bibliography::Error,
    root: &Path,
    paths: &crate::source_diagnostic::DiagnosticPaths,
) -> Option<CitationInputDiagnostic> {
    use super::citation_inputs::bibliography::{Error as BibError, Location};
    let (location, first) = match error {
        BibError::Parse { path, source } => (
            Location {
                path: path.clone(),
                span: source.span.clone(),
            },
            None,
        ),
        BibError::Duplicate { first, second, .. }
        | BibError::AliasCollision { first, second, .. } => (second.clone(), Some(first)),
        BibError::InvalidUrl { location, .. }
        | BibError::Cycle { location }
        | BibError::TooDeep { location }
        | BibError::InvalidAliases { location }
        | BibError::ExpansionTooLarge { location }
        | BibError::TooMuchWork { location, .. } => (location.clone(), None),
    };
    let read_source = |location: &Location| {
        let text = super::citation_inputs::read(root, &location.path).ok()?;
        let path = paths.display_path(&root.join(&location.path));
        Some((
            path.clone(),
            crate::source_diagnostic::named_source_at_display_path(&path, text, "BibTeX"),
        ))
    };
    let (path, source_code) = read_source(&location)?;
    let related = first
        .and_then(|first| {
            read_source(first).map(|(_, source_code)| CitationInputRelated {
                source_code,
                span: first.span.clone().into(),
            })
        })
        .into_iter()
        .collect();
    Some(CitationInputDiagnostic {
        path,
        message: error.to_string(),
        source_code,
        span: location.span.into(),
        related,
    })
}

/// A semantic page-definition error tied to the field that caused it.
#[derive(Debug, Diagnostic, ThisError)]
#[error("invalid page {path}: {message}")]
#[diagnostic(code(heine::page::invalid))]
pub(crate) struct PageDiagnostic {
    path: PathBuf,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("this content setting is invalid")]
    span: SourceSpan,
}

/// A page filename-derived output error. The invalid value is in the source
/// path rather than the TOML body, so this deliberately reports that path
/// instead of attaching a fabricated source span to unrelated page text.
#[derive(Debug, Diagnostic, ThisError)]
#[error("page {path} derives invalid output path {output}: {message}")]
#[diagnostic(
    code(heine::page::invalid_output_path),
    help("rename the page or choose a portable output filename")
)]
pub(crate) struct PageOutputPathDiagnostic {
    path: PathBuf,
    output: PathBuf,
    message: String,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("{subject} {path} is missing required metadata field `{field}`")]
#[diagnostic(code(heine::metadata::missing))]
pub(crate) struct MissingMetadata {
    subject: String,
    field: String,
    path: PathBuf,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("add `{field}` here")]
    #[allow(dead_code)]
    span: SourceSpan,
    #[related]
    related: Vec<RequirementOrigin>,
}

#[derive(Debug, Diagnostic, ThisError)]
#[error("required metadata declared here")]
struct RequirementOrigin {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("required by this directory policy")]
    span: SourceSpan,
}
