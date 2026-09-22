//! `_directory.toml` parsing and inherited page policy.
//!
//! A descriptor defines policy for its directory subtree. This module owns
//! parsing that policy and resolving its nearest-ancestor defaults; page
//! loading consumes the resulting values without reinterpreting descriptors.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error as ThisError;
use walkdir::DirEntry;

use crate::{
    author::AuthorName,
    config::{CollectionName, CollectionsConfig},
    site::{
        Error,
        metadata::{
            RawMetadataPolicy, Requirement, RequirementTarget, parse_metadata, requirements,
        },
        page::{CollectionMembership, Metadata, RawMetadata, ResolvedAuthors},
    },
    source_path,
    template::TemplateName,
};

use super::{
    FirstCollectionMembership, PageTomlDiagnostic, invalid, metadata_modified, relative_path,
    validate_requirements,
};

pub(super) fn parse_toc(
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
                    table_key_span(source, span, table_name, "content"),
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
                                table_key_span(source, span, table_name, "min_level"),
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
                                table_key_span(source, span, table_name, "max_level"),
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
                    table_key_span(source, span, table_name, &field),
                    format!("unknown toc field {field:?}"),
                ));
            }
            if !(1..=6).contains(&min_level) {
                return Err(invalid(
                    path,
                    source,
                    table_key_span(source, span, table_name, "min_level"),
                    "toc.min_level must be an integer from 1 through 6",
                ));
            }
            if !(1..=6).contains(&max_level) {
                return Err(invalid(
                    path,
                    source,
                    table_key_span(source, span, table_name, "max_level"),
                    "toc.max_level must be an integer from 1 through 6",
                ));
            }
            if min_level > max_level {
                return Err(invalid(
                    path,
                    source,
                    table_key_span(source, span, table_name, "min_level"),
                    "toc.min_level must not exceed toc.max_level",
                ));
            }
            Ok(TocPolicy::Enabled(TocSettings {
                content,
                content_span: table_key_span(source, span, table_name, "content"),
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

pub(super) fn parse_footnotes(
    raw: toml::Spanned<toml::Value>,
    path: &Path,
    source: &str,
    table_name: &str,
) -> Result<FootnotesPolicy, Error> {
    let span: SourceSpan = raw.span().into();
    match raw.into_inner() {
        toml::Value::Boolean(false) => Ok(FootnotesPolicy::Disabled(span)),
        toml::Value::Boolean(true) => Err(invalid(
            path,
            source,
            span,
            "field `footnotes` accepts only false; use [footnotes] to select a Markdown source",
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
                        "footnotes.content must be a string declared content name",
                    )
                })?;
            if !source_path::is_simple_identifier(&content) {
                return Err(invalid(
                    path,
                    source,
                    table_key_span(source, span, table_name, "content"),
                    "footnotes.content must be a simple declared content name",
                ));
            }
            if let Some((field, _)) = table.into_iter().next() {
                return Err(invalid(
                    path,
                    source,
                    table_key_span(source, span, table_name, &field),
                    format!("unknown footnotes field {field:?}"),
                ));
            }
            Ok(FootnotesPolicy::Enabled(FootnotesSettings {
                content,
                content_span: table_key_span(source, span, table_name, "content"),
                span,
            }))
        }
        _ => Err(invalid(
            path,
            source,
            span,
            "field `footnotes` must be false or a [footnotes] table",
        )),
    }
}

/// Locates a policy key inside its named TOML table, rather than allowing an
/// identically named key in arbitrary site metadata to claim the diagnostic
/// label. Serde's span for a TOML table names only its header, so it cannot
/// delimit the table body itself.
fn table_key_span(source: &str, table_span: SourceSpan, table_name: &str, key: &str) -> SourceSpan {
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

pub(super) fn toc_for<'a>(
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

pub(super) fn effective_footnotes<'a>(
    ancestors: &'a [&'a DirectoryDescriptor],
) -> Option<&'a DirectoryDescriptor> {
    ancestors
        .iter()
        .rev()
        .find(|descriptor| descriptor.footnotes.is_some())
        .copied()
}

pub(super) fn footnotes_for<'a>(
    raw: Option<&'a FootnotesPolicy>,
    ancestors: &'a [&'a DirectoryDescriptor],
    path: &'a Path,
    source: &'a str,
) -> Result<Option<FootnotesSettingsRef<'a>>, Error> {
    match raw {
        Some(FootnotesPolicy::Disabled(span)) => match effective_footnotes(ancestors) {
            Some(DirectoryDescriptor {
                footnotes: Some(FootnotesPolicy::Enabled(_)),
                ..
            }) => Ok(None),
            _ => Err(invalid(
                path,
                source,
                *span,
                "footnotes = false requires an otherwise effective inherited margin-footnote setting",
            )),
        },
        Some(FootnotesPolicy::Enabled(settings)) => Ok(Some(FootnotesSettingsRef {
            settings,
            path,
            source,
        })),
        None => match effective_footnotes(ancestors) {
            Some(descriptor) => match descriptor.footnotes.as_ref() {
                Some(FootnotesPolicy::Enabled(settings)) => Ok(Some(FootnotesSettingsRef {
                    settings,
                    path: &descriptor.path,
                    source: &descriptor.text,
                })),
                Some(FootnotesPolicy::Disabled(_)) | None => Ok(None),
            },
            None => Ok(None),
        },
    }
}

pub(super) const DIRECTORY_FILE: &str = "_directory.toml";

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDirectory {
    draft: Option<toml::Spanned<toml::Value>>,
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
    footnotes: Option<toml::Spanned<toml::Value>>,
    metadata: Option<RawMetadataPolicy>,
}

#[derive(Debug, Clone)]
pub(in crate::site) struct DirectoryDescriptor {
    pub(super) relative: PathBuf,
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) source: NamedSource<String>,
    pub(super) metadata: Metadata,
    pub(super) metadata_requirements: Vec<Requirement>,
    pub(super) page_requirements: Vec<Requirement>,
    pub(super) template: Option<TemplateChoice>,
    pub(super) collections: Option<Vec<CollectionMembership>>,
    pub(super) authors: Option<ResolvedAuthors>,
    pub(super) sitemap_exclusion: Option<SourceSpan>,
    pub(super) toc: Option<TocPolicy>,
    pub(super) footnotes: Option<FootnotesPolicy>,
    pub(super) modified: SystemTime,
}

#[derive(Debug, Clone)]
pub(in crate::site) enum TocPolicy {
    Disabled(SourceSpan),
    Enabled(TocSettings),
}

#[derive(Debug, Clone)]
pub(in crate::site) struct TocSettings {
    pub(super) content: String,
    pub(super) content_span: SourceSpan,
    pub(super) min_level: u8,
    pub(super) max_level: u8,
    pub(super) span: SourceSpan,
}

pub(in crate::site) struct TocSettingsRef<'a> {
    pub(super) settings: &'a TocSettings,
    pub(super) path: &'a Path,
    pub(super) source: &'a str,
}

#[derive(Debug, Clone)]
pub(in crate::site) enum FootnotesPolicy {
    Disabled(SourceSpan),
    Enabled(FootnotesSettings),
}

#[derive(Debug, Clone)]
pub(in crate::site) struct FootnotesSettings {
    pub(super) content: String,
    pub(super) content_span: SourceSpan,
    pub(super) span: SourceSpan,
}

pub(in crate::site) struct FootnotesSettingsRef<'a> {
    pub(super) settings: &'a FootnotesSettings,
    pub(super) path: &'a Path,
    pub(super) source: &'a str,
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

#[derive(Debug, Clone)]
pub(super) enum TemplateChoice {
    Named(TemplateName, SourceSpan),
    Disabled,
}

pub(in crate::site) fn load_descriptors(
    content_root: &Path,
    entries: &[DirEntry],
    collection_config: &CollectionsConfig,
    sitemap_enabled: bool,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<BTreeMap<PathBuf, DirectoryDescriptor>, Error> {
    let mut descriptors = BTreeMap::new();
    for entry in entries {
        if !entry.file_type().is_file() || entry.file_name() != DIRECTORY_FILE {
            continue;
        }
        let source_path = entry.path();
        let path = &diagnostics.display_path(source_path);
        let text = fs::read_to_string(source_path).map_err(|source| Error::Io {
            path: source_path.into(),
            source,
        })?;
        let raw: RawDirectory = toml::from_str(&text).map_err(|error| {
            Error::PageToml(Box::new(PageTomlDiagnostic {
                path: diagnostics.display_path(path),
                message: error.message().to_owned(),
                source_code: diagnostics.named_source(path, text.clone(), "TOML"),
                span: error.span().unwrap_or(0..0).into(),
            }))
        })?;
        if let Some(draft) = raw.draft {
            return Err(invalid(
                path,
                &text,
                draft.span().into(),
                "field `draft` belongs only in a .page file; directory descriptors do not inherit publication state",
            ));
        }
        let relative_file = relative_path(content_root, source_path)?;
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
        let footnotes = page_policy
            .footnotes
            .map(|value| parse_footnotes(value, path, &text, "pages.footnotes"))
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
                let author_source = Arc::new(diagnostics.named_source(path, text.clone(), "TOML"));
                parse_authors(authors, path, &text).map(|authors| authors.directory(author_source))
            })
            .transpose()?;
        descriptors.insert(
            relative.clone(),
            DirectoryDescriptor {
                relative,
                path: source_path.into(),
                text: text.clone(),
                source: diagnostics.named_source(path, text, "TOML"),
                metadata,
                metadata_requirements,
                page_requirements,
                template,
                collections,
                authors,
                sitemap_exclusion,
                toc,
                footnotes,
                modified: metadata_modified(source_path)?,
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
        if let Some(FootnotesPolicy::Disabled(span)) = descriptor.footnotes {
            let ancestors = descriptor_ancestors(&descriptor.relative, &descriptors);
            let inherited = ancestors
                .iter()
                .rev()
                .find(|ancestor| {
                    ancestor.relative != descriptor.relative && ancestor.footnotes.is_some()
                })
                .copied();
            if !matches!(
                inherited.and_then(|ancestor| ancestor.footnotes.as_ref()),
                Some(FootnotesPolicy::Enabled(_))
            ) {
                return Err(invalid(
                    &descriptor.path,
                    &descriptor.text,
                    span,
                    "pages.footnotes = false requires an otherwise effective inherited margin-footnote setting",
                ));
            }
        }
    }
    Ok(descriptors)
}

pub(super) fn descriptor_ancestors<'a>(
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

pub(super) fn template_for(
    page: Option<toml::Spanned<toml::Value>>,
    ancestors: &[&DirectoryDescriptor],
    path: &Path,
    source: &str,
) -> Result<Option<(TemplateName, SourceSpan, NamedSource<String>)>, Error> {
    let choice = match page {
        Some(value) => Some((
            parse_template(value, path, source)?,
            crate::source_diagnostic::named_source_at_display_path(path, source.to_owned(), "TOML"),
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
pub(super) fn collections_for(
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
pub(super) fn authors_for(
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
    let named_source = Arc::new(crate::source_diagnostic::named_source_at_display_path(
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
                    source_code: named_source,
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

pub(super) fn parse_template(
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
