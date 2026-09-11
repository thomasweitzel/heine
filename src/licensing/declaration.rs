//! Normalization shared by adjacent sidecars and configured licensing groups.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use miette::SourceSpan;

use crate::{
    asset::{AssetLicensing, LicenseText},
    config::{LicensingDeclaration, LicensingValue},
    output::OutputPath,
};

use super::LICENSE_DIRECTORY;

/// A declaration value retains its authored location until SPDX validation has
/// identified the precise part that needs a diagnostic.
#[derive(Debug)]
pub(super) struct TaggedValue {
    pub(super) value: String,
    pub(super) span: SourceSpan,
    pub(super) span_is_exact: bool,
}

#[derive(Debug)]
pub(super) struct DeclarationFields {
    pub(super) copyrights: Vec<TaggedValue>,
    pub(super) expressions: Vec<TaggedValue>,
    pub(super) comments: Vec<TaggedValue>,
}

#[derive(Debug)]
pub(super) enum DeclarationProblem {
    InvalidExpression {
        span: SourceSpan,
        message: String,
    },
    UnsupportedReference {
        span: SourceSpan,
        message: &'static str,
        help: &'static str,
    },
    MissingLicenseText {
        span: SourceSpan,
        id: String,
    },
}

pub(super) fn from_config(declaration: &LicensingDeclaration) -> DeclarationFields {
    DeclarationFields {
        copyrights: values(declaration.copyrights()),
        expressions: values(declaration.expressions()),
        comments: values(declaration.comments()),
    }
}

pub(super) fn resolve(
    texts: &BTreeSet<String>,
    fields: DeclarationFields,
) -> Result<AssetLicensing, DeclarationProblem> {
    let mut license_ids = BTreeMap::new();
    for expression in &fields.expressions {
        let parsed = spdx::Expression::parse(&expression.value).map_err(|error| {
            DeclarationProblem::InvalidExpression {
                span: expression_span(expression, error.span.clone()),
                message: error.to_string(),
            }
        })?;
        for requirement in parsed.requirements() {
            let span = expression_requirement_span(expression, requirement.span.clone());
            let license = match &requirement.req.license {
                spdx::LicenseItem::Spdx { .. } => requirement.req.license.to_string(),
                spdx::LicenseItem::Other(reference) if reference.doc_ref.is_none() => {
                    requirement.req.license.to_string()
                }
                spdx::LicenseItem::Other(_) => {
                    return Err(DeclarationProblem::UnsupportedReference {
                        span,
                        message: "external DocumentRef license references are not supported by Heine's local LICENSES/ model",
                        help: "use a local LicenseRef, or provide a recognized SPDX license identifier",
                    });
                }
            };
            license_ids.insert(license, span);
            if let Some(addition) = &requirement.req.addition {
                let addition = match addition {
                    spdx::AdditionItem::Spdx(_) => addition.to_string(),
                    spdx::AdditionItem::Other(_) => {
                        return Err(DeclarationProblem::UnsupportedReference {
                            span,
                            message: "AdditionRef exceptions are not supported by Heine's local LICENSES/ model",
                            help: "use a recognized SPDX exception identifier",
                        });
                    }
                };
                license_ids.insert(addition, span);
            }
        }
    }
    for (id, span) in &license_ids {
        if !texts.contains(id) {
            return Err(DeclarationProblem::MissingLicenseText {
                span: *span,
                id: id.clone(),
            });
        }
    }
    Ok(AssetLicensing {
        copyrights: fields
            .copyrights
            .into_iter()
            .map(|value| value.value)
            .collect(),
        expressions: fields
            .expressions
            .into_iter()
            .map(|value| value.value)
            .collect(),
        comments: fields
            .comments
            .into_iter()
            .map(|value| value.value)
            .collect(),
        license_texts: license_ids
            .into_keys()
            .map(|id| LicenseText {
                output: OutputPath::try_new(Path::new(LICENSE_DIRECTORY).join(format!("{id}.txt")))
                    .expect("validated SPDX identifiers create portable license output paths"),
                id,
            })
            .collect(),
    })
}

fn values(values: &[LicensingValue]) -> Vec<TaggedValue> {
    values
        .iter()
        .map(|value| TaggedValue {
            value: value.value().into(),
            span: value.span(),
            span_is_exact: true,
        })
        .collect()
}

fn expression_span(expression: &TaggedValue, span: std::ops::Range<usize>) -> SourceSpan {
    if !expression.span_is_exact {
        return expression.span;
    }
    let offset = expression.span.offset();
    (
        offset + span.start,
        span.end.saturating_sub(span.start).max(1),
    )
        .into()
}

fn expression_requirement_span(expression: &TaggedValue, span: std::ops::Range<u32>) -> SourceSpan {
    if !expression.span_is_exact {
        return expression.span;
    }
    (
        expression.span.offset()
            + usize::try_from(span.start).expect("SPDX spans fit into the local architecture"),
        usize::try_from(span.end - span.start).expect("SPDX spans fit into the local architecture"),
    )
        .into()
}
