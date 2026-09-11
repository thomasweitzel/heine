//! Source-aware copied-asset licensing configuration.
//!
//! Group membership cannot be checked while TOML is parsed: it names published
//! output paths, which exist only after copied assets have been discovered.
//! This module therefore preserves the validated authored facts and spans for
//! the licensing resolver.

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use miette::{NamedSource, SourceSpan};
use serde::{Deserialize, de};

use crate::output::OutputPath;

use super::ConfigValueError;

/// The site-wide policy for declarations attached to copied assets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LicensingCoverage {
    #[default]
    Optional,
    Required,
}

/// Configuration that remains source-aware until it can be resolved against
/// copied assets and local license texts.
#[derive(Debug, Clone, Default)]
pub(crate) struct LicensingConfig {
    coverage: LicensingCoverage,
    groups: Vec<LicensingGroup>,
}

impl LicensingConfig {
    pub(crate) fn coverage(&self) -> LicensingCoverage {
        self.coverage
    }

    pub(crate) fn groups(&self) -> &[LicensingGroup] {
        &self.groups
    }

    pub(crate) fn attach_source(&mut self, source: Arc<NamedSource<String>>) {
        for group in &mut self.groups {
            group.source = Some(source.clone());
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LicensingGroup {
    assets: Vec<LicensingGroupAsset>,
    declaration: LicensingDeclaration,
    source: Option<Arc<NamedSource<String>>>,
}

impl LicensingGroup {
    pub(crate) fn assets(&self) -> &[LicensingGroupAsset] {
        &self.assets
    }

    pub(crate) fn declaration(&self) -> &LicensingDeclaration {
        &self.declaration
    }

    pub(crate) fn source(&self) -> Option<&Arc<NamedSource<String>>> {
        self.source.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LicensingGroupAsset {
    output: OutputPath,
    span: SourceSpan,
}

impl LicensingGroupAsset {
    pub(crate) fn output(&self) -> &OutputPath {
        &self.output
    }

    pub(crate) fn span(&self) -> SourceSpan {
        self.span
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LicensingDeclaration {
    copyrights: Vec<LicensingValue>,
    expressions: Vec<LicensingValue>,
    comments: Vec<LicensingValue>,
}

impl LicensingDeclaration {
    pub(crate) fn copyrights(&self) -> &[LicensingValue] {
        &self.copyrights
    }

    pub(crate) fn expressions(&self) -> &[LicensingValue] {
        &self.expressions
    }

    pub(crate) fn comments(&self) -> &[LicensingValue] {
        &self.comments
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LicensingValue {
    value: String,
    span: SourceSpan,
}

impl LicensingValue {
    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    pub(crate) fn span(&self) -> SourceSpan {
        self.span
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RawLicensingConfig {
    required: Option<toml::Spanned<bool>>,
    #[serde(default)]
    groups: Vec<toml::Spanned<RawLicensingGroup>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLicensingGroup {
    assets: toml::Spanned<Vec<toml::Spanned<String>>>,
    #[serde(rename = "SPDX-FileCopyrightText")]
    copyrights: Option<toml::Spanned<RawLicensingValues>>,
    #[serde(rename = "SPDX-License-Identifier")]
    expressions: Option<toml::Spanned<RawLicensingValues>>,
    #[serde(rename = "SPDX-FileComment")]
    comments: Option<toml::Spanned<RawLicensingValues>>,
}

#[derive(Debug)]
enum RawLicensingValues {
    One(String),
    Many(Vec<toml::Spanned<String>>),
}

// `toml::Spanned` records a span only when it is the value currently being
// deserialized. A custom visitor preserves each array element's location so a
// later SPDX error can label the authored string rather than its whole array.
impl<'de> Deserialize<'de> for RawLicensingValues {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Values;

        impl<'de> de::Visitor<'de> for Values {
            type Value = RawLicensingValues;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a string or an array of strings")
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RawLicensingValues::One(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RawLicensingValues::One(value.into()))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RawLicensingValues::One(value.into()))
            }

            fn visit_seq<A>(self, mut values: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut parsed = Vec::new();
                while let Some(value) = values.next_element::<toml::Spanned<String>>()? {
                    parsed.push(value);
                }
                Ok(RawLicensingValues::Many(parsed))
            }
        }

        deserializer.deserialize_any(Values)
    }
}

pub(super) fn parse(raw: RawLicensingConfig) -> Result<LicensingConfig, ConfigValueError> {
    let coverage = match raw.required {
        Some(value) if *value.get_ref() => LicensingCoverage::Required,
        Some(value) => {
            return Err(ConfigValueError {
                span: value.span().into(),
                message: "licensing.required = false is redundant; omit required to allow undeclared copied assets".into(),
            });
        }
        None => LicensingCoverage::Optional,
    };
    let mut groups = Vec::with_capacity(raw.groups.len());
    for group in raw.groups {
        let group_span: SourceSpan = group.span().into();
        let group = group.into_inner();
        let assets_span: SourceSpan = group.assets.span().into();
        let asset_values = group.assets.into_inner();
        if asset_values.is_empty() {
            return Err(ConfigValueError {
                span: assets_span,
                message: "licensing group assets must not be empty".into(),
            });
        }
        let mut assets = Vec::with_capacity(asset_values.len());
        let mut seen = BTreeSet::new();
        for value in asset_values {
            let span: SourceSpan = value.span().into();
            let spelling = value.into_inner();
            let output = OutputPath::try_new(PathBuf::from(&spelling)).map_err(|message| {
                ConfigValueError {
                    span,
                    message: format!("licensing group asset path {message}"),
                }
            })?;
            if crate::output::is_reserved_output_path(output.as_path()) {
                return Err(ConfigValueError {
                    span,
                    message: "licensing group asset path must not use Heine's reserved __heine/ namespace".into(),
                });
            }
            if !seen.insert(output.clone()) {
                return Err(ConfigValueError {
                    span,
                    message: format!("licensing group names asset {spelling:?} more than once"),
                });
            }
            assets.push(LicensingGroupAsset { output, span });
        }
        let copyrights = required_values(group.copyrights, group_span, "SPDX-FileCopyrightText")?;
        let expressions =
            required_values(group.expressions, group_span, "SPDX-License-Identifier")?;
        let comments = optional_values(group.comments, "SPDX-FileComment")?;
        groups.push(LicensingGroup {
            assets,
            declaration: LicensingDeclaration {
                copyrights,
                expressions,
                comments,
            },
            source: None,
        });
    }
    Ok(LicensingConfig { coverage, groups })
}

fn required_values(
    values: Option<toml::Spanned<RawLicensingValues>>,
    group_span: SourceSpan,
    field: &str,
) -> Result<Vec<LicensingValue>, ConfigValueError> {
    let Some(values) = values else {
        return Err(ConfigValueError {
            span: group_span,
            message: format!("licensing group requires at least one {field}"),
        });
    };
    non_empty_values(values, field)
}

fn optional_values(
    values: Option<toml::Spanned<RawLicensingValues>>,
    field: &str,
) -> Result<Vec<LicensingValue>, ConfigValueError> {
    values.map_or_else(|| Ok(Vec::new()), |values| non_empty_values(values, field))
}

fn non_empty_values(
    values: toml::Spanned<RawLicensingValues>,
    field: &str,
) -> Result<Vec<LicensingValue>, ConfigValueError> {
    let values_span: SourceSpan = values.span().into();
    let values = match values.into_inner() {
        RawLicensingValues::One(value) => vec![(value, values_span)],
        RawLicensingValues::Many(values) => values
            .into_iter()
            .map(|value| {
                let span = value.span().into();
                (value.into_inner(), span)
            })
            .collect(),
    };
    if values.is_empty() {
        return Err(ConfigValueError {
            span: values_span,
            message: format!("licensing group {field} must not be an empty array"),
        });
    }
    values
        .into_iter()
        .map(|(value, span)| {
            if value.trim().is_empty() {
                return Err(ConfigValueError {
                    span,
                    message: format!("licensing group {field} values must not be empty"),
                });
            }
            Ok(LicensingValue { value, span })
        })
        .collect()
}
