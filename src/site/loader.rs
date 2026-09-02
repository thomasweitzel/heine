//! Filesystem discovery and source-aware loading of page definitions.

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
use walkdir::{DirEntry, WalkDir};

use crate::{
    author::AuthorName,
    config::{
        CollectionName, CollectionsConfig, EditorialTimeZone, SeriesConfig, SeriesName,
        TaxonomiesConfig, TaxonomyName,
    },
    content,
    locale::Locale,
    output::OutputPath,
    site::{
        Error,
        metadata::{
            RawMetadataPolicy, Requirement, RequirementTarget, key_span, parse_metadata,
            requirements,
        },
        page::{
            CollectionMembership, DirectoryView, LoadedPage, Metadata, OutputSuffix, Page, PageId,
            Pagination, PaginationSegment, RawMetadata, RawPage, ResolvedAuthors, SeriesMembership,
            Task, TaxonomyMembership, TaxonomyTermMembership, TranslationId,
        },
    },
    source_path,
    template::TemplateName,
};

fn parse_toc(
    raw: toml::Spanned<toml::Value>,
    path: &Path,
    source: &str,
    table_name: &str,
) -> Result<TocPolicy, Error> {
    let span: SourceSpan = raw.span().into();
    match raw.into_inner() {
        toml::Value::Boolean(false) => Ok(TocPolicy::Disabled(span)),
        toml::Value::Boolean(true) => Err(invalid(
            path,
            source,
            span,
            "field `toc` accepts only false; use [toc] to configure a table of contents",
        )),
        toml::Value::Table(mut table) => {
            let content = table
                .remove("content")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    invalid(
                        path,
                        source,
                        span,
                        "toc.content must be a string declared content name",
                    )
                })?;
            if !source_path::is_simple_identifier(&content) {
                return Err(invalid(
                    path,
                    source,
                    toc_key_span(source, span, table_name, "content"),
                    "toc.content must be a simple declared content name",
                ));
            }
            let min_level = table
                .remove("min_level")
                .map(|value| {
                    value
                        .as_integer()
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or_else(|| {
                            invalid(
                                path,
                                source,
                                toc_key_span(source, span, table_name, "min_level"),
                                "toc.min_level must be an integer from 1 through 6",
                            )
                        })
                })
                .transpose()?
                .unwrap_or(1);
            let max_level = table
                .remove("max_level")
                .map(|value| {
                    value
                        .as_integer()
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or_else(|| {
                            invalid(
                                path,
                                source,
                                toc_key_span(source, span, table_name, "max_level"),
                                "toc.max_level must be an integer from 1 through 6",
                            )
                        })
                })
                .transpose()?
                .unwrap_or(6);
            if let Some((field, _)) = table.into_iter().next() {
                return Err(invalid(
                    path,
                    source,
                    toc_key_span(source, span, table_name, &field),
                    format!("unknown toc field {field:?}"),
                ));
            }
            if !(1..=6).contains(&min_level) {
                return Err(invalid(
                    path,
                    source,
                    toc_key_span(source, span, table_name, "min_level"),
                    "toc.min_level must be an integer from 1 through 6",
                ));
            }
            if !(1..=6).contains(&max_level) {
                return Err(invalid(
                    path,
                    source,
                    toc_key_span(source, span, table_name, "max_level"),
                    "toc.max_level must be an integer from 1 through 6",
                ));
            }
            if min_level > max_level {
                return Err(invalid(
                    path,
                    source,
                    toc_key_span(source, span, table_name, "min_level"),
                    "toc.min_level must not exceed toc.max_level",
                ));
            }
            Ok(TocPolicy::Enabled(TocSettings {
                content,
                content_span: toc_key_span(source, span, table_name, "content"),
                min_level,
                max_level,
                span,
            }))
        }
        _ => Err(invalid(
            path,
            source,
            span,
            "field `toc` must be false or a [toc] table",
        )),
    }
}

/// Locates a ToC key inside its named TOML table, rather than allowing an
/// identically named key in arbitrary site metadata to claim the diagnostic
/// label. Serde's span for a TOML table names only its header, so it cannot
/// delimit the table body itself.
fn toc_key_span(source: &str, table_span: SourceSpan, table_name: &str, key: &str) -> SourceSpan {
    let table_header = format!("[{table_name}]");
    let mut offset = 0;
    let mut in_table = false;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim();
        if !in_table {
            if trimmed == table_header {
                in_table = true;
            }
            offset += line.len();
            continue;
        }
        if trimmed.starts_with('[') {
            break;
        }
        let leading = line.len() - line.trim_start().len();
        let candidate = &line[leading..];
        for spelling in [key.to_owned(), format!("{key:?}"), format!("'{key}'")] {
            if candidate
                .strip_prefix(&spelling)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
            {
                return (offset + leading, spelling.len()).into();
            }
        }
        offset += line.len();
    }
    table_span
}

fn effective_toc<'a>(ancestors: &'a [&'a DirectoryDescriptor]) -> Option<&'a DirectoryDescriptor> {
    ancestors
        .iter()
        .rev()
        .find(|descriptor| descriptor.toc.is_some())
        .copied()
}

fn toc_for<'a>(
    raw: Option<&'a TocPolicy>,
    ancestors: &'a [&'a DirectoryDescriptor],
    path: &'a Path,
    source: &'a str,
) -> Result<Option<TocSettingsRef<'a>>, Error> {
    match raw {
        Some(TocPolicy::Disabled(span)) => match effective_toc(ancestors) {
            Some(DirectoryDescriptor {
                toc: Some(TocPolicy::Enabled(_)),
                ..
            }) => Ok(None),
            _ => Err(invalid(
                path,
                source,
                *span,
                "toc = false requires an otherwise effective inherited table of contents",
            )),
        },
        Some(TocPolicy::Enabled(settings)) => Ok(Some(TocSettingsRef {
            settings,
            path,
            source,
        })),
        None => match effective_toc(ancestors) {
            Some(descriptor) => match descriptor.toc.as_ref() {
                Some(TocPolicy::Enabled(settings)) => Ok(Some(TocSettingsRef {
                    settings,
                    path: &descriptor.path,
                    source: &descriptor.text,
                })),
                Some(TocPolicy::Disabled(_)) | None => Ok(None),
            },
            None => Ok(None),
        },
    }
}

pub(super) const DIRECTORY_FILE: &str = "_directory.toml";

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDirectory {
    title: Option<toml::Spanned<String>>,
    description: Option<toml::Spanned<String>>,
    summary: Option<toml::Spanned<String>>,
    extra: Option<toml::Spanned<toml::Value>>,
    #[serde(rename = "metadata")]
    metadata_policy: Option<RawMetadataPolicy>,
    pages: Option<RawPagePolicy>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPagePolicy {
    template: Option<toml::Spanned<toml::Value>>,
    collections: Option<Vec<toml::Spanned<String>>>,
    authors: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
    sitemap: Option<toml::Spanned<bool>>,
    toc: Option<toml::Spanned<toml::Value>>,
    metadata: Option<RawMetadataPolicy>,
}

#[derive(Debug, Clone)]
pub(super) struct DirectoryDescriptor {
    relative: PathBuf,
    path: PathBuf,
    text: String,
    source: NamedSource<String>,
    metadata: Metadata,
    metadata_requirements: Vec<Requirement>,
    page_requirements: Vec<Requirement>,
    template: Option<TemplateChoice>,
    collections: Option<Vec<CollectionMembership>>,
    authors: Option<ResolvedAuthors>,
    sitemap_exclusion: Option<SourceSpan>,
    toc: Option<TocPolicy>,
    modified: SystemTime,
}

#[derive(Debug, Clone)]
enum TocPolicy {
    Disabled(SourceSpan),
    Enabled(TocSettings),
}

#[derive(Debug, Clone)]
struct TocSettings {
    content: String,
    content_span: SourceSpan,
    min_level: u8,
    max_level: u8,
    span: SourceSpan,
}

struct TocSettingsRef<'a> {
    settings: &'a TocSettings,
    path: &'a Path,
    source: &'a str,
}

/// Shared locale-loading inputs for one page. Keeping this as one domain
/// context makes page loading independent of the caller's traversal details.
pub(super) struct PageLoadContext<'a> {
    pub(super) content_root: &'a Path,
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

#[derive(Debug, Diagnostic, ThisError)]
#[error("collection {collection:?} is declared more than once")]
#[diagnostic(code(heine::collection::duplicate_membership))]
pub(crate) struct DuplicateCollectionMembership {
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("duplicate membership here")]
    span: SourceSpan,
    #[related]
    first: Vec<FirstCollectionMembership>,
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

#[derive(Debug, Clone)]
enum TemplateChoice {
    Named(TemplateName, SourceSpan),
    Disabled,
}

pub(super) fn content_entries(root: &Path) -> Result<Vec<DirEntry>, Error> {
    WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Walk)
        .and_then(|entries| {
            for entry in &entries {
                if entry.file_type().is_symlink() {
                    return Err(Error::ContentSymlink {
                        path: entry.path().into(),
                    });
                }
                let relative = relative_path(root, entry.path())?;
                if !source_path::has_safe_components(&relative) {
                    return Err(Error::NonUtf8Path {
                        path: entry.path().into(),
                    });
                }
            }
            if let Some(collision) = source_path::normalization_collision(
                entries
                    .iter()
                    .filter(|entry| entry.path() != root)
                    .map(|entry| {
                        relative_path(root, entry.path())
                            .map(|relative| (relative, entry.path().into()))
                    })
                    .collect::<Result<Vec<_>, Error>>()?,
            ) {
                return Err(Error::NormalizationCollision(Box::new(collision)));
            }
            Ok(entries)
        })
}

/// Builds the child-directory view once for a locale tree instead of scanning
/// every discovered entry again for every page in that tree.
pub(super) fn subdirectories(entries: &[DirEntry]) -> BTreeMap<PathBuf, Vec<String>> {
    let mut directories: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for entry in entries.iter().filter(|entry| entry.file_type().is_dir()) {
        let Some(parent) = entry.path().parent() else {
            continue;
        };
        let name = entry
            .file_name()
            .to_str()
            .expect("content discovery validates safe UTF-8 path components")
            .to_owned();
        directories
            .entry(parent.to_path_buf())
            .or_default()
            .push(name);
    }
    for names in directories.values_mut() {
        names.sort();
    }
    directories
}

pub(super) fn relative_path(root: &Path, path: &Path) -> Result<PathBuf, Error> {
    source_path::relative_to(root, path).ok_or_else(|| Error::OutsideContent {
        path: path.into(),
        root: root.into(),
    })
}

pub(super) fn load_page(
    path: &Path,
    relative: &Path,
    context: PageLoadContext<'_>,
) -> Result<LoadedPage, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let raw: RawPage = toml::from_str(&text).map_err(|error| {
        Error::PageToml(Box::new(PageTomlDiagnostic {
            path: path.into(),
            message: error.message().to_owned(),
            source_code: crate::source_diagnostic::named_source(path, text.clone(), "TOML"),
            span: error.span().unwrap_or(0..0).into(),
        }))
    })?;
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
    let mut latest = metadata_modified(path)?;
    for descriptor in &ancestors {
        latest = latest.max(descriptor.modified);
    }
    let mut content = BTreeMap::new();
    let mut content_inputs = BTreeSet::new();
    let mut markdown_sources = Vec::new();
    let mut markdown_names = BTreeSet::new();
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
        let content_path = resolve_content_path(context.content_root, path, &file)
            .map_err(|message| invalid(path, &text, file_span, message))?;
        latest = latest.max(content_modified(&content_path).map_err(|error| {
            invalid(
                path,
                &text,
                file_span,
                format!("could not access declared content file {file:?}: {error}"),
            )
        })?);
        content_inputs.insert(relative_path(context.content_root, &content_path)?);
        let loaded = content::load(&content_path).map_err(|error| match error {
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
            markdown_sources.push(source);
        }
        content.insert(name, loaded.value);
    }
    let toc = if let Some(config) = toc_config {
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
        Some(crate::markdown::toc_plan(
            settings.content.clone(),
            markdown,
            context.markdown,
            settings.min_level,
            settings.max_level,
        ))
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
    let source_dir = path.parent().expect("page file has parent");
    let subdirs = context
        .subdirectories
        .get(source_dir)
        .cloned()
        .unwrap_or_default();
    let sitemap_lastmod = metadata.updated.or(metadata.published);
    let page_source = crate::source_diagnostic::named_source(path, text.clone(), "TOML");
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
        toc,
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
    let navigation_window = raw.navigation.window.into_inner();
    if !(1..=5).contains(&navigation_window) {
        return Err(invalid(
            path,
            source,
            window_span,
            "pagination.navigation.window must be an integer from 1 through 5",
        ));
    }
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
                    source_code: Arc::new(crate::source_diagnostic::named_source(
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

pub(super) fn load_descriptors(
    content_root: &Path,
    entries: &[DirEntry],
    collection_config: &CollectionsConfig,
    sitemap_enabled: bool,
) -> Result<BTreeMap<PathBuf, DirectoryDescriptor>, Error> {
    let mut descriptors = BTreeMap::new();
    for entry in entries {
        if !entry.file_type().is_file() || entry.file_name() != DIRECTORY_FILE {
            continue;
        }
        let path = entry.path();
        let text = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?;
        let raw: RawDirectory = toml::from_str(&text).map_err(|error| {
            Error::PageToml(Box::new(PageTomlDiagnostic {
                path: path.into(),
                message: error.message().to_owned(),
                source_code: crate::source_diagnostic::named_source(path, text.clone(), "TOML"),
                span: error.span().unwrap_or(0..0).into(),
            }))
        })?;
        let relative_file = relative_path(content_root, path)?;
        let relative = relative_file
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let metadata = parse_metadata(
            RawMetadata {
                title: raw.title,
                description: raw.description,
                summary: raw.summary,
                authors: None,
                published: None,
                updated: None,
                extra: raw.extra,
            },
            path,
            &text,
            None,
        )?;
        let metadata_requirements = requirements(
            raw.metadata_policy,
            path,
            &text,
            RequirementTarget::Directory,
        )?;
        let page_policy = raw.pages.unwrap_or_default();
        let sitemap_exclusion = page_policy
            .sitemap
            .map(|value| {
                let span: SourceSpan = value.span().into();
                if !sitemap_enabled {
                    return Err(invalid(
                        path,
                        &text,
                        span,
                        "field `pages.sitemap` requires [sitemap] in heine.toml",
                    ));
                }
                if value.into_inner() {
                    return Err(invalid(
                        path,
                        &text,
                        span,
                        "field `pages.sitemap` accepts only false; omit it to keep ordinary sitemap membership",
                    ));
                }
                Ok(span)
            })
            .transpose()?;
        let toc = page_policy
            .toc
            .map(|value| parse_toc(value, path, &text, "pages.toc"))
            .transpose()?;
        let page_requirements =
            requirements(page_policy.metadata, path, &text, RequirementTarget::Page)?;
        let template = match page_policy.template {
            Some(value) => Some(parse_template(value, path, &text)?),
            None => None,
        };
        let collections = page_policy
            .collections
            .map(|collections| parse_collections(collections, collection_config, path, &text))
            .transpose()?;
        let authors = page_policy
            .authors
            .map(|authors| {
                let author_source = Arc::new(crate::source_diagnostic::named_source(
                    path,
                    text.clone(),
                    "TOML",
                ));
                parse_authors(authors, path, &text).map(|authors| authors.directory(author_source))
            })
            .transpose()?;
        descriptors.insert(
            relative.clone(),
            DirectoryDescriptor {
                relative,
                path: path.into(),
                text: text.clone(),
                source: crate::source_diagnostic::named_source(path, text, "TOML"),
                metadata,
                metadata_requirements,
                page_requirements,
                template,
                collections,
                authors,
                sitemap_exclusion,
                toc,
                modified: metadata_modified(path)?,
            },
        );
    }
    for descriptor in descriptors.values() {
        let ancestors = descriptor_ancestors(&descriptor.relative, &descriptors);
        let requirements = ancestors.iter().flat_map(|ancestor| {
            ancestor
                .metadata_requirements
                .iter()
                .map(|requirement| (*ancestor, requirement))
        });
        validate_requirements(
            "directory",
            &descriptor.path,
            &descriptor.text,
            &descriptor.metadata,
            requirements,
        )?;
        if let Some(TocPolicy::Disabled(span)) = descriptor.toc {
            let ancestors = descriptor_ancestors(&descriptor.relative, &descriptors);
            let inherited = ancestors
                .iter()
                .rev()
                .find(|ancestor| ancestor.relative != descriptor.relative && ancestor.toc.is_some())
                .copied();
            if !matches!(
                inherited.and_then(|ancestor| ancestor.toc.as_ref()),
                Some(TocPolicy::Enabled(_))
            ) {
                return Err(invalid(
                    &descriptor.path,
                    &descriptor.text,
                    span,
                    "pages.toc = false requires an otherwise effective inherited table of contents",
                ));
            }
        }
    }
    Ok(descriptors)
}

fn descriptor_ancestors<'a>(
    directory: &Path,
    descriptors: &'a BTreeMap<PathBuf, DirectoryDescriptor>,
) -> Vec<&'a DirectoryDescriptor> {
    let mut ancestors = Vec::new();
    let mut current = Some(directory);
    while let Some(path) = current {
        if let Some(descriptor) = descriptors.get(path) {
            ancestors.push(descriptor);
        }
        current = path.parent().filter(|parent| *parent != path);
    }
    ancestors.reverse();
    ancestors
}

fn template_for(
    page: Option<toml::Spanned<toml::Value>>,
    ancestors: &[&DirectoryDescriptor],
    path: &Path,
    source: &str,
) -> Result<Option<(TemplateName, SourceSpan, NamedSource<String>)>, Error> {
    let choice = match page {
        Some(value) => Some((
            parse_template(value, path, source)?,
            crate::source_diagnostic::named_source(path, source.to_owned(), "TOML"),
        )),
        None => ancestors.iter().rev().find_map(|descriptor| {
            descriptor
                .template
                .clone()
                .map(|choice| (choice, descriptor.source.clone()))
        }),
    };
    match choice {
        None | Some((TemplateChoice::Disabled, _)) => Ok(None),
        Some((TemplateChoice::Named(name, span), source)) => Ok(Some((name, span, source))),
    }
}

/// Resolves the page-level collection policy using the same nearest-descriptor
/// override rule as the existing template policy. An explicit empty array is a
/// deliberate opt-out from an inherited membership default.
fn collections_for(
    page: Option<Vec<toml::Spanned<String>>>,
    ancestors: &[&DirectoryDescriptor],
    collection_config: &CollectionsConfig,
    path: &Path,
    source: &str,
) -> Result<Vec<CollectionMembership>, Error> {
    match page {
        Some(collections) => parse_collections(collections, collection_config, path, source),
        None => ancestors
            .iter()
            .rev()
            .find_map(|descriptor| descriptor.collections.clone())
            .map(Ok)
            .unwrap_or_else(|| Ok(Vec::new())),
    }
}

/// Resolves a page's authors with the same nearest-descriptor replacement
/// rule used for other page defaults. An omitted value remains absent; an
/// explicit empty list is rejected because it cannot describe authorship.
fn authors_for(
    page: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
    ancestors: &[&DirectoryDescriptor],
    path: &Path,
    source: &str,
) -> Result<Option<ResolvedAuthors>, Error> {
    match page {
        Some(authors) => parse_authors(authors, path, source).map(Some),
        None => Ok(ancestors
            .iter()
            .rev()
            .find_map(|descriptor| descriptor.authors.clone())),
    }
}

fn parse_authors(
    values: toml::Spanned<Vec<toml::Spanned<String>>>,
    path: &Path,
    source: &str,
) -> Result<ResolvedAuthors, Error> {
    let span = values.span().into();
    let values = values.into_inner();
    if values.is_empty() {
        return Err(invalid(
            path,
            source,
            span,
            "field `authors` must contain at least one non-empty name",
        ));
    }
    let mut authors = Vec::with_capacity(values.len());
    for value in values {
        let span: SourceSpan = value.span().into();
        let name = value.into_inner();
        authors
            .push(AuthorName::parse(name).map_err(|message| invalid(path, source, span, message))?);
    }
    Ok(ResolvedAuthors::page(authors, span))
}

fn parse_collections(
    values: Vec<toml::Spanned<String>>,
    collection_config: &CollectionsConfig,
    path: &Path,
    source: &str,
) -> Result<Vec<CollectionMembership>, Error> {
    let named_source = Arc::new(crate::source_diagnostic::named_source(
        path,
        source.to_owned(),
        "TOML",
    ));
    let mut collections = Vec::with_capacity(values.len());
    for value in values {
        let span: SourceSpan = value.span().into();
        let raw = value.into_inner();
        let name = CollectionName::parse(raw.clone())
            .map_err(|message| invalid(path, source, span, message))?;
        if !collection_config.contains(&name) {
            let available = collection_config
                .definitions()
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let message = match available.as_slice() {
                [] => format!(
                    "collection {raw:?} is not configured; add a [collections.{raw}] table to heine.toml"
                ),
                _ => format!(
                    "collection {raw:?} is not configured; available collections: {}",
                    available.join(", ")
                ),
            };
            return Err(invalid(path, source, span, message));
        }
        if let Some(first) = collections
            .iter()
            .find(|membership: &&CollectionMembership| membership.name == name)
        {
            return Err(Error::DuplicateCollectionMembership(Box::new(
                DuplicateCollectionMembership {
                    collection: raw,
                    source_code: named_source.clone(),
                    span,
                    first: vec![FirstCollectionMembership {
                        source_code: first.source.clone(),
                        span: first.span,
                    }],
                },
            )));
        }
        collections.push(CollectionMembership {
            name,
            source: named_source.clone(),
            span,
        });
    }
    Ok(collections)
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
    let named_source = Arc::new(crate::source_diagnostic::named_source(
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
                    source_code: named_source.clone(),
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
    let named_source = Arc::new(crate::source_diagnostic::named_source(
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
                        source_code: named_source.clone(),
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

fn parse_template(
    value: toml::Spanned<toml::Value>,
    path: &Path,
    source: &str,
) -> Result<TemplateChoice, Error> {
    let span: SourceSpan = value.span().into();
    match value.into_inner() {
        toml::Value::String(name) => TemplateName::parse(name)
            .map(|name| TemplateChoice::Named(name, span))
            .map_err(|message| invalid(path, source, span, message)),
        toml::Value::Boolean(false) => Ok(TemplateChoice::Disabled),
        _ => Err(invalid(
            path,
            source,
            span,
            "field `template` must be a template name or `false`",
        )),
    }
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
                source_code: crate::source_diagnostic::named_source(
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
        source_code: crate::source_diagnostic::named_source(path, source, "TOML"),
        span,
    }))
}

fn parse_weight(
    value: Option<&toml::Spanned<toml::Value>>,
    id: &PageId,
) -> Result<Option<i64>, String> {
    match value {
        None if !id.is_index() => Ok(Some(0)),
        None => Ok(None),
        Some(value) if matches!(value.get_ref(), toml::Value::Integer(_)) => {
            let toml::Value::Integer(value) = value.get_ref() else {
                unreachable!("the match establishes an integer value");
            };
            Ok(Some(*value))
        }
        Some(value) if matches!(value.get_ref(), toml::Value::String(value) if value == "None") => {
            Ok(None)
        }
        _ => Err("field `weight` must be an integer or the string \"None\"".into()),
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

/// A semantic page-definition error tied to the field that caused it.
#[derive(Debug, Diagnostic, ThisError)]
#[error("invalid page {path}: {message}")]
#[diagnostic(code(heine::page::invalid))]
pub(crate) struct PageDiagnostic {
    path: PathBuf,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{message}")]
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
