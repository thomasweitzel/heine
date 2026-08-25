//! Filesystem discovery and source-aware loading of page definitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error as ThisError;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};
use walkdir::{DirEntry, WalkDir};

use crate::{
    content,
    locale::Locale,
    output::OutputPath,
    site::{
        Error,
        page::{
            DirectoryView, LoadedPage, Metadata, MetadataDate, OutputSuffix, Page, PageId,
            RawMetadata, RawPage, Task, TemplateName, UrlPath,
        },
    },
    source_path,
};

pub(super) const DIRECTORY_FILE: &str = "_directory.toml";

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDirectory {
    title: Option<toml::Spanned<String>>,
    description: Option<toml::Spanned<String>>,
    summary: Option<toml::Spanned<String>>,
    date: Option<toml::Spanned<toml::Value>>,
    extra: Option<toml::Spanned<toml::Value>>,
    #[serde(rename = "metadata")]
    metadata_policy: Option<RawMetadataPolicy>,
    pages: Option<RawPagePolicy>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMetadataPolicy {
    #[serde(default)]
    required: Vec<toml::Spanned<String>>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPagePolicy {
    template: Option<toml::Spanned<toml::Value>>,
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
    modified: SystemTime,
}

#[derive(Debug, Clone)]
struct Requirement {
    name: String,
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
            Ok(entries)
        })
}

pub(super) fn relative_path(root: &Path, path: &Path) -> Result<PathBuf, Error> {
    source_path::relative_to(root, path).ok_or_else(|| Error::OutsideContent {
        path: path.into(),
        root: root.into(),
    })
}

pub(super) fn load_page(
    content_root: &Path,
    locale: &Locale,
    output_prefix: &Path,
    path: &Path,
    relative: &Path,
    entries: &[DirEntry],
    descriptors: &BTreeMap<PathBuf, DirectoryDescriptor>,
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
    let id = PageId::from_source_path(&relative.with_extension(""));
    let directory_path = relative.parent().unwrap_or(Path::new(""));
    let ancestors = descriptor_ancestors(directory_path, descriptors);
    let template = template_for(raw.template, &ancestors, path, &text)?;
    let page_source = crate::source_diagnostic::named_source(path, text.clone(), "TOML");
    let suffix_span = raw.suffix.span();
    let suffix: OutputSuffix = raw
        .suffix
        .into_inner()
        .try_into()
        .map_err(|message| invalid(path, &text, suffix_span.clone().into(), message))?;
    let tags = raw.tags;
    let weight_span = raw
        .weight
        .as_ref()
        .map(|value| value.span().into())
        .unwrap_or((0, 0).into());
    let weight = parse_weight(raw.weight.as_ref(), &id)
        .map_err(|message| invalid(path, &text, weight_span, message))?;
    let mut latest = metadata_modified(path)?;
    for descriptor in &ancestors {
        latest = latest.max(descriptor.modified);
    }
    let mut content = BTreeMap::new();
    let mut content_inputs = BTreeSet::new();
    let mut markdown_sources = Vec::new();
    for (name, file) in raw.content {
        let file_span: SourceSpan = file.span().into();
        if !valid_identifier(&name) {
            return Err(invalid(
                path,
                &text,
                key_span(&text, file_span, &name),
                format!("content name {name:?} must be a simple template identifier"),
            ));
        }
        let file = file.into_inner();
        let content_path = resolve_content_path(content_root, path, &file)
            .map_err(|message| invalid(path, &text, file_span, message))?;
        latest = latest.max(content_modified(&content_path).map_err(|error| {
            invalid(
                path,
                &text,
                file_span,
                format!("could not access declared content file {file:?}: {error}"),
            )
        })?);
        content_inputs.insert(relative_path(content_root, &content_path)?);
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
            markdown_sources.push(source);
        }
        content.insert(name, loaded.value);
    }
    let metadata = parse_metadata(
        RawMetadata {
            title: raw.title,
            description: raw.description,
            summary: raw.summary,
            date: raw.date,
            extra: raw.extra,
        },
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
            if value.trim().is_empty() {
                Err(invalid(
                    path,
                    &text,
                    translationid_span.unwrap_or((0, 0).into()),
                    "field `translationid` must not be empty",
                ))
            } else {
                Ok(value)
            }
        })
        .transpose()?;
    let output_path = template
        .as_ref()
        .map(|_| OutputPath::new(output_prefix.join(relative).with_extension(suffix.as_str())));
    let (path_value, filename) = match &output_path {
        Some(output) => (
            Some(UrlPath::from_output(output)),
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
        .map(source_path::url)
        .unwrap_or_default();
    let source_dir = path.parent().expect("page file has parent");
    let subdirs = entries
        .iter()
        .filter(|entry| entry.file_type().is_dir() && entry.path().parent() == Some(source_dir))
        .map(|entry| {
            entry
                .file_name()
                .to_str()
                .expect("content discovery validates safe UTF-8 path components")
                .to_owned()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let page = Page {
        id: id.clone(),
        path: path_value,
        dir: directory.clone(),
        filename,
        subdirs,
        content,
        metadata,
        translationid,
        translationid_span,
        source: page_source.clone(),
        directory: descriptors
            .get(directory_path)
            .map(|descriptor| DirectoryView::new(directory.clone(), &descriptor.metadata))
            .unwrap_or_else(|| DirectoryView::new(directory.clone(), &Metadata::default())),
        tags,
        weight,
        siblings: vec![],
        lighter: None,
        heavier: None,
        translations: BTreeMap::new(),
    };
    let task = template.zip(output_path).map(
        |((template, template_span, template_source), output_path)| Task {
            locale: locale.clone(),
            template_span,
            output_span: suffix_span.into(),
            page_id: id,
            template,
            page_source,
            template_source,
            output_path,
            latest_input: latest,
        },
    );
    Ok(LoadedPage {
        page,
        task,
        content_inputs,
        markdown_sources,
    })
}

pub(super) fn load_descriptors(
    content_root: &Path,
    entries: &[DirEntry],
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
                date: raw.date,
                extra: raw.extra,
            },
            path,
            &text,
        )?;
        let metadata_requirements = requirements(raw.metadata_policy, path, &text)?;
        let page_policy = raw.pages.unwrap_or_default();
        let page_requirements = requirements(page_policy.metadata, path, &text)?;
        let template = match page_policy.template {
            Some(value) => Some(parse_template(value, path, &text)?),
            None => None,
        };
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

fn parse_template(
    value: toml::Spanned<toml::Value>,
    path: &Path,
    source: &str,
) -> Result<TemplateChoice, Error> {
    let span = value.span();
    match value.into_inner() {
        toml::Value::String(name) => {
            Ok(TemplateChoice::Named(TemplateName::new(name), span.into()))
        }
        toml::Value::Boolean(false) => Ok(TemplateChoice::Disabled),
        _ => Err(invalid(
            path,
            source,
            span.into(),
            "field `template` must be a template name or `false`",
        )),
    }
}

fn parse_metadata(raw: RawMetadata, path: &Path, source: &str) -> Result<Metadata, Error> {
    let date = raw
        .date
        .map(|value| parse_date(value, path, source))
        .transpose()?;
    let (extra, extra_span) = raw
        .extra
        .map(|value| {
            let span = value.span().into();
            (value.into_inner(), span)
        })
        .unwrap_or_else(|| (toml::Value::Table(Default::default()), (0, 0).into()));
    if !matches!(extra, toml::Value::Table(_)) {
        return Err(invalid(
            path,
            source,
            extra_span,
            "field `extra` must be a TOML table",
        ));
    }
    validate_extra_keys(&extra, path, source, extra_span)?;
    Ok(Metadata {
        title: raw.title.map(toml::Spanned::into_inner),
        description: raw.description.map(toml::Spanned::into_inner),
        summary: raw.summary.map(toml::Spanned::into_inner),
        date,
        extra: serde_json::to_value(extra).expect("toml values are serializable"),
    })
}

fn parse_date(
    value: toml::Spanned<toml::Value>,
    path: &Path,
    source: &str,
) -> Result<MetadataDate, Error> {
    let span = value.span().into();
    let toml::Value::Datetime(value) = value.into_inner() else {
        return Err(invalid(
            path,
            source,
            span,
            "field `date` must be a TOML date or an offset-aware date-time",
        ));
    };
    let Some(date) = value.date else {
        return Err(invalid(
            path,
            source,
            span,
            "field `date` must include a calendar date",
        ));
    };
    let month = Month::try_from(date.month).map_err(|_| {
        invalid(
            path,
            source,
            span,
            "field `date` contains an invalid calendar month",
        )
    })?;
    let date = Date::from_calendar_date(i32::from(date.year), month, date.day).map_err(|_| {
        invalid(
            path,
            source,
            span,
            "field `date` contains an invalid calendar date",
        )
    })?;
    match (value.time, value.offset) {
        (None, None) => Ok(MetadataDate::Date(date)),
        (Some(time), Some(offset)) => {
            let time = Time::from_hms_nano(
                time.hour,
                time.minute,
                time.second.unwrap_or(0),
                time.nanosecond.unwrap_or(0),
            )
            .map_err(|_| {
                invalid(
                    path,
                    source,
                    span,
                    "field `date` contains an invalid clock time",
                )
            })?;
            let offset = match offset {
                toml::value::Offset::Z => UtcOffset::UTC,
                toml::value::Offset::Custom { minutes } => {
                    UtcOffset::from_whole_seconds(i32::from(minutes) * 60).map_err(|_| {
                        invalid(
                            path,
                            source,
                            span,
                            "field `date` contains an invalid UTC offset",
                        )
                    })?
                }
            };
            Ok(MetadataDate::OffsetDateTime(
                PrimitiveDateTime::new(date, time).assume_offset(offset),
            ))
        }
        _ => Err(invalid(
            path,
            source,
            span,
            "field `date` must be a TOML date or an offset-aware date-time",
        )),
    }
}

fn validate_extra_keys(
    value: &toml::Value,
    path: &Path,
    source: &str,
    span: SourceSpan,
) -> Result<(), Error> {
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                if !valid_identifier(key) {
                    return Err(invalid(
                        path,
                        source,
                        key_span(source, span, key),
                        format!(
                            "field `extra` contains key {key:?}; extra keys must be simple template identifiers"
                        ),
                    ));
                }
                validate_extra_keys(value, path, source, span)?;
            }
        }
        toml::Value::Array(values) => {
            for value in values {
                validate_extra_keys(value, path, source, span)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Locates a TOML key when serde supplies a span for its value or table rather
/// than the key itself. Strict schemas ensure a matching assignment cannot be
/// confused with an unrelated user-defined field outside this namespace.
fn key_span(source: &str, fallback: SourceSpan, key: &str) -> SourceSpan {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
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
    fallback
}

fn requirements(
    raw: Option<RawMetadataPolicy>,
    path: &Path,
    source: &str,
) -> Result<Vec<Requirement>, Error> {
    raw.unwrap_or_default().required.into_iter().map(|value| {
        let span = value.span(); let name = value.into_inner();
        if matches!(name.as_str(), "title" | "description" | "summary" | "date") || valid_extra_requirement(&name) {
            Ok(Requirement { name, span: span.into() })
        } else { Err(invalid(path, source, span.into(), "metadata requirement must name `title`, `description`, `summary`, `date`, or `extra.<name>`")) }
    }).collect()
}

fn valid_extra_requirement(name: &str) -> bool {
    name.strip_prefix("extra.")
        .is_some_and(|path| !path.is_empty() && path.split('.').all(valid_identifier))
}

fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
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

fn invalid(path: &Path, source: &str, span: SourceSpan, message: impl Into<String>) -> Error {
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
