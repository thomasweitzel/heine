//! Checked parsing and merging of a Kit’s root-owned `[kit.extra]` settings.

use std::collections::BTreeMap;

use miette::{NamedSource, SourceSpan};

use super::{ExtraDiagnostic, ResolveError, Selection, ValueError};
#[derive(Debug, Clone, Default)]
pub(crate) struct ExtraSchema {
    values: BTreeMap<String, ExtraValue>,
}

impl ExtraSchema {
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
}

/// One schema value retains its authored source span so later overlay errors
/// can identify the manifest fact that established the required shape.
#[derive(Debug, Clone)]
pub(crate) struct ExtraValue {
    kind: ExtraValueKind,
    span: SourceSpan,
}

/// TOML's complete value domain, preserved rather than narrowed to the JSON
/// subset because a kit schema describes authored TOML before template values
/// are constructed.
#[derive(Debug, Clone)]
pub(crate) enum ExtraValueKind {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Datetime(String),
    Array(Vec<ExtraValue>),
    Table(BTreeMap<String, ExtraValue>),
}

impl ExtraValueKind {
    fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::Boolean(_) => "boolean",
            Self::Datetime(_) => "datetime",
            Self::Array(_) => "array",
            Self::Table(_) => "table",
        }
    }
}

/// Parses root-owned settings after ordinary configuration parsing has
/// established that a kit selection exists. `toml_edit` retains the spans for
/// nested values that serde's generic TOML values do not expose.
pub(super) fn parse_selection(source: &str) -> Result<ExtraSchema, ValueError> {
    let document = source
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| ValueError {
            span: (0, 0).into(),
            message: format!("could not inspect kit extra settings: {error}"),
        })?;
    let Some(kit) = document.get("kit") else {
        return Ok(ExtraSchema::default());
    };
    let kit = kit.as_table().ok_or_else(|| ValueError {
        span: item_span(kit),
        message: "kit must be a TOML table".into(),
    })?;
    let Some(extra) = kit.get("extra") else {
        return Ok(ExtraSchema::default());
    };
    let table = extra.as_table().ok_or_else(|| ValueError {
        span: item_span(extra),
        message: "kit.extra must be a TOML table".into(),
    })?;
    Ok(ExtraSchema {
        values: parse_extra_table(table, ExtraArrayPolicy::AllowEmpty)?,
    })
}

pub(super) fn merge(
    defaults: &ExtraSchema,
    overrides: &ExtraSchema,
    selection: &Selection,
) -> Result<serde_json::Value, ResolveError> {
    let source_code = selection
        .source
        .clone()
        .ok_or(ResolveError::MissingSelectionSource)?;
    let values = merge_extra_table(
        &defaults.values,
        &overrides.values,
        &source_code,
        "kit.extra",
    )?;
    Ok(extra_table_to_json(&values))
}

fn merge_extra_table(
    defaults: &BTreeMap<String, ExtraValue>,
    overrides: &BTreeMap<String, ExtraValue>,
    source_code: &NamedSource<String>,
    path: &str,
) -> Result<BTreeMap<String, ExtraValue>, ResolveError> {
    let mut values = defaults.clone();
    for (key, override_value) in overrides {
        let setting = format!("{path}.{key}");
        let Some(default) = defaults.get(key) else {
            return Err(extra_error(
                source_code,
                override_value.span,
                format!("{setting} is not declared by the selected kit"),
                "remove this setting, or add it to the selected kit's [extra] schema",
            ));
        };
        values.insert(
            key.clone(),
            merge_extra_value(default, override_value, &setting, source_code)?,
        );
    }
    Ok(values)
}

fn merge_extra_value(
    default: &ExtraValue,
    override_value: &ExtraValue,
    path: &str,
    source_code: &NamedSource<String>,
) -> Result<ExtraValue, ResolveError> {
    match (&default.kind, &override_value.kind) {
        (ExtraValueKind::Table(defaults), ExtraValueKind::Table(overrides)) => Ok(ExtraValue {
            kind: ExtraValueKind::Table(merge_extra_table(defaults, overrides, source_code, path)?),
            span: override_value.span,
        }),
        (ExtraValueKind::Array(defaults), ExtraValueKind::Array(overrides)) => {
            let expected = &defaults[0].kind;
            if let Some(value) = overrides
                .iter()
                .find(|value| value.kind.type_name() != expected.type_name())
            {
                return Err(extra_error(
                    source_code,
                    value.span,
                    format!(
                        "{path} array entries must be {}, matching the kit schema",
                        expected.type_name()
                    ),
                    "replace the entry with the schema's TOML value type",
                ));
            }
            Ok(override_value.clone())
        }
        _ if default.kind.type_name() == override_value.kind.type_name() => {
            Ok(override_value.clone())
        }
        _ => Err(extra_error(
            source_code,
            override_value.span,
            format!(
                "{path} must remain a {}, matching the kit schema",
                default.kind.type_name()
            ),
            "use the TOML value type declared by the selected kit",
        )),
    }
}

fn extra_error(
    source_code: &NamedSource<String>,
    span: SourceSpan,
    message: String,
    help: &str,
) -> ResolveError {
    ResolveError::Extra(Box::new(ExtraDiagnostic {
        message,
        source_code: source_code.clone(),
        span,
        help: help.into(),
    }))
}

fn extra_table_to_json(values: &BTreeMap<String, ExtraValue>) -> serde_json::Value {
    serde_json::Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), extra_value_to_json(value)))
            .collect(),
    )
}

fn extra_value_to_json(value: &ExtraValue) -> serde_json::Value {
    match &value.kind {
        ExtraValueKind::String(value) | ExtraValueKind::Datetime(value) => value.clone().into(),
        ExtraValueKind::Integer(value) => (*value).into(),
        ExtraValueKind::Float(value) => match serde_json::Number::from_f64(*value) {
            Some(number) => serde_json::Value::Number(number),
            // Parsing rejects non-finite values. Retaining the text here avoids
            // silently changing a value if that invariant is ever weakened.
            None => value.to_string().into(),
        },
        ExtraValueKind::Boolean(value) => (*value).into(),
        ExtraValueKind::Array(values) => values.iter().map(extra_value_to_json).collect(),
        ExtraValueKind::Table(values) => extra_table_to_json(values),
    }
}
pub(super) fn parse_schema(source: &str) -> Result<ExtraSchema, ValueError> {
    let document = source
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| ValueError {
            span: (0, 0).into(),
            message: format!("could not inspect kit extra schema: {error}"),
        })?;
    let Some(extra) = document.get("extra") else {
        return Ok(ExtraSchema::default());
    };
    let table = extra.as_table().ok_or_else(|| ValueError {
        span: item_span(extra),
        message: "kit extra must be a TOML table".into(),
    })?;
    Ok(ExtraSchema {
        values: parse_extra_table(table, ExtraArrayPolicy::RequireElements)?,
    })
}

#[derive(Clone, Copy)]
enum ExtraArrayPolicy {
    RequireElements,
    AllowEmpty,
}

fn parse_extra_table(
    table: &toml_edit::Table,
    array_policy: ExtraArrayPolicy,
) -> Result<BTreeMap<String, ExtraValue>, ValueError> {
    table
        .iter()
        .map(|(key, value)| Ok((key.to_owned(), parse_extra_item(value, array_policy)?)))
        .collect()
}

fn parse_extra_item(
    item: &toml_edit::Item,
    array_policy: ExtraArrayPolicy,
) -> Result<ExtraValue, ValueError> {
    match item {
        toml_edit::Item::Value(value) => parse_extra_value(value, array_policy),
        toml_edit::Item::Table(table) => Ok(ExtraValue {
            kind: ExtraValueKind::Table(parse_extra_table(table, array_policy)?),
            span: item_span(item),
        }),
        toml_edit::Item::ArrayOfTables(tables) => {
            let values = tables
                .iter()
                .map(|table| {
                    Ok(ExtraValue {
                        kind: ExtraValueKind::Table(parse_extra_table(table, array_policy)?),
                        span: table.span().unwrap_or(0..0).into(),
                    })
                })
                .collect::<Result<Vec<_>, ValueError>>()?;
            parse_extra_array(values, item_span(item), array_policy)
        }
        toml_edit::Item::None => Err(ValueError {
            span: item_span(item),
            message: "kit extra must contain a TOML value".into(),
        }),
    }
}

fn parse_extra_value(
    value: &toml_edit::Value,
    array_policy: ExtraArrayPolicy,
) -> Result<ExtraValue, ValueError> {
    let span = value.span().unwrap_or(0..0).into();
    let kind = match value {
        toml_edit::Value::String(value) => ExtraValueKind::String(value.value().clone()),
        toml_edit::Value::Integer(value) => ExtraValueKind::Integer(*value.value()),
        toml_edit::Value::Float(value) => {
            let value = *value.value();
            if !value.is_finite() {
                return Err(ValueError {
                    span,
                    message: "kit extra floating-point values must be finite".into(),
                });
            }
            ExtraValueKind::Float(value)
        }
        toml_edit::Value::Boolean(value) => ExtraValueKind::Boolean(*value.value()),
        toml_edit::Value::Datetime(value) => ExtraValueKind::Datetime(value.value().to_string()),
        toml_edit::Value::Array(values) => {
            let values = values
                .iter()
                .map(|value| parse_extra_value(value, array_policy))
                .collect::<Result<Vec<_>, _>>()?;
            return parse_extra_array(values, span, array_policy);
        }
        toml_edit::Value::InlineTable(table) => ExtraValueKind::Table(
            table
                .iter()
                .map(|(key, value)| Ok((key.to_owned(), parse_extra_value(value, array_policy)?)))
                .collect::<Result<_, ValueError>>()?,
        ),
    };
    Ok(ExtraValue { kind, span })
}

fn parse_extra_array(
    values: Vec<ExtraValue>,
    span: SourceSpan,
    array_policy: ExtraArrayPolicy,
) -> Result<ExtraValue, ValueError> {
    if values.is_empty() && matches!(array_policy, ExtraArrayPolicy::RequireElements) {
        return Err(ValueError {
            span,
            message: "kit extra schema arrays must not be empty".into(),
        });
    }
    if let Some(first) = values.first().map(|value| value.kind.type_name())
        && let Some(value) = values.iter().find(|value| value.kind.type_name() != first)
    {
        return Err(ValueError {
            span: value.span,
            message: "kit extra schema arrays must contain one TOML value type".into(),
        });
    }
    Ok(ExtraValue {
        kind: ExtraValueKind::Array(values),
        span,
    })
}

fn item_span(item: &toml_edit::Item) -> SourceSpan {
    item.span().unwrap_or(0..0).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_or_mixed_schema_arrays() {
        for (source, expected) in [
            ("[extra]\nitems=[]\n", "must not be empty"),
            ("[extra]\nitems=[1, 'two']\n", "one TOML value type"),
        ] {
            let error = parse_schema(source).expect_err("invalid extra schema");
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }
}
