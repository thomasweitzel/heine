//! Source-aware parsing and policy requirements for page metadata.

use std::path::Path;

use miette::SourceSpan;

use crate::config::EditorialTimeZone;

use super::{
    Error,
    loader::invalid,
    page::{Metadata, PublicationTime, RawMetadata},
};

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawMetadataPolicy {
    #[serde(default)]
    required: Vec<toml::Spanned<String>>,
}

#[derive(Debug, Clone)]
pub(super) struct Requirement {
    pub(super) name: String,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Copy)]
pub(super) enum RequirementTarget {
    Directory,
    Page,
}

pub(super) fn parse_metadata(
    raw: RawMetadata,
    path: &Path,
    source: &str,
    time_zone: Option<&EditorialTimeZone>,
) -> Result<Metadata, Error> {
    let title_span = raw.title.as_ref().map(|value| value.span().into());
    let summary_span = raw.summary.as_ref().map(|value| value.span().into());
    let updated_span = raw.updated.as_ref().map(|value| value.span().into());
    let published = raw
        .published
        .map(|value| parse_publication_time("published", value, time_zone, path, source))
        .transpose()?;
    let updated = raw
        .updated
        .map(|value| parse_publication_time("updated", value, time_zone, path, source))
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
        title_span,
        description: raw.description.map(toml::Spanned::into_inner),
        summary: raw.summary.map(toml::Spanned::into_inner),
        summary_span,
        authors: raw.authors,
        published,
        updated,
        updated_span,
        extra: serde_json::to_value(extra).expect("toml values are serializable"),
    })
}

pub(super) fn requirements(
    raw: Option<RawMetadataPolicy>,
    path: &Path,
    source: &str,
    target: RequirementTarget,
) -> Result<Vec<Requirement>, Error> {
    raw.unwrap_or_default()
        .required
        .into_iter()
        .map(|value| {
            let span = value.span();
            let name = value.into_inner();
            if permitted_requirement(&name, target) {
                Ok(Requirement {
                    name,
                    span: span.into(),
                })
            } else {
                Err(invalid(path, source, span.into(), requirement_help(target)))
            }
        })
        .collect()
}

/// Locates a TOML key when serde supplies a span for its value or table rather
/// than the key itself. Strict schemas ensure a matching assignment cannot be
/// confused with an unrelated user-defined field outside this namespace.
pub(super) fn key_span(source: &str, fallback: SourceSpan, key: &str) -> SourceSpan {
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

fn parse_publication_time(
    field: &str,
    value: toml::Spanned<toml::Value>,
    time_zone: Option<&EditorialTimeZone>,
    path: &Path,
    source: &str,
) -> Result<PublicationTime, Error> {
    let span: SourceSpan = value.span().into();
    let toml::Value::Datetime(value) = value.into_inner() else {
        return Err(invalid(
            path,
            source,
            span,
            format!("field `{field}` must be a TOML local date-time without a UTC offset"),
        ));
    };
    if value.date.is_none() || value.time.is_none() || value.offset.is_some() {
        return Err(invalid(
            path,
            source,
            span,
            format!("field `{field}` must be a TOML local date-time without a UTC offset"),
        ));
    }
    let Some(time_zone) = time_zone else {
        return Err(invalid(
            path,
            source,
            span,
            format!(
                "field `{field}` requires site.time_zone; add an IANA time-zone name such as \"Europe/Berlin\" under [site]"
            ),
        ));
    };
    time_zone
        .resolve_local(field, &value)
        .map(PublicationTime::new)
        .map_err(|message| invalid(path, source, span, message))
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
                if !crate::source_path::is_simple_identifier(key) {
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

fn permitted_requirement(name: &str, target: RequirementTarget) -> bool {
    matches!(name, "title" | "description" | "summary")
        || (matches!(target, RequirementTarget::Page)
            && matches!(name, "authors" | "published" | "updated"))
        || valid_extra_requirement(name)
}

fn requirement_help(target: RequirementTarget) -> &'static str {
    match target {
        RequirementTarget::Directory => {
            "directory metadata requirement must name `title`, `description`, `summary`, or `extra.<name>`"
        }
        RequirementTarget::Page => {
            "page metadata requirement must name `title`, `description`, `summary`, `authors`, `published`, `updated`, or `extra.<name>`"
        }
    }
}

fn valid_extra_requirement(name: &str) -> bool {
    name.strip_prefix("extra.").is_some_and(|path| {
        !path.is_empty()
            && path
                .split('.')
                .all(crate::source_path::is_simple_identifier)
    })
}
