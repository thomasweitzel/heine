//! Parsed Fluent-resource lookup and static function-call validation.
//!
//! Resource loading calls this module before it creates a runtime bundle. The
//! resolver also reuses its parsed-pattern helpers, so loading and rendering
//! agree on what each Fluent message exposes.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};

use fluent_syntax::{
    ast::{CallArguments, Entry, Expression, InlineExpression, Pattern, PatternElement, Resource},
    parser,
};
use miette::SourceSpan;

use super::{Error, FluentDiagnostic, FluentEntry, FluentFunction, number};

pub(super) fn message_entries(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(Vec<FluentEntry>, Arc<Resource<String>>), Error> {
    let resource = parser::parse(source).map_err(|(_, errors)| {
        let error = errors.first();
        Error::Parse(Box::new(FluentDiagnostic {
            path: diagnostics.display_path(path),
            message: error.map_or_else(
                || "Fluent parser rejected this resource".to_owned(),
                ToString::to_string,
            ),
            source_code: diagnostics.named_source(path, source, "Fluent"),
            span: error.map_or((0, 0).into(), |error| error.pos.clone().into()),
        }))
    })?;
    validate_fluent_function_calls(&resource.body, path, source, diagnostics)?;
    let parsed = Arc::new(parser::parse(source.to_owned()).map_err(|(_, errors)| {
        let error = errors.first();
        Error::Parse(Box::new(FluentDiagnostic {
            path: diagnostics.display_path(path),
            message: error.map_or_else(
                || "Fluent parser rejected this resource".to_owned(),
                ToString::to_string,
            ),
            source_code: diagnostics.named_source(path, source, "Fluent"),
            span: error.map_or((0, 0).into(), |error| error.pos.clone().into()),
        }))
    })?);
    let mut offset = 0;
    let entries = resource
        .body
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Message(message) => {
                Some((message.id.name.to_string(), message.id.name.to_string()))
            }
            Entry::Term(term) => Some((format!("-{}", term.id.name), term.id.name.to_string())),
            _ => None,
        })
        .map(|(id, bundle_id)| {
            let (span, next_offset) = message_span(source, offset, &id).ok_or_else(|| {
                Error::Parse(Box::new(FluentDiagnostic {
                    path: diagnostics.display_path(path),
                    message: format!("could not locate parsed Fluent message {id:?}"),
                    source_code: diagnostics.named_source(path, source, "Fluent"),
                    span: (0, 0).into(),
                }))
            })?;
            offset = next_offset;
            Ok(FluentEntry {
                id,
                bundle_id,
                span,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok((entries, parsed))
}

/// Heine deliberately accepts only the decimal `NUMBER()` options that its
/// shared formatter implements. Fluent otherwise ignores unsupported named
/// options, which would turn a misspelling into silently different output.
fn validate_fluent_function_calls(
    entries: &[Entry<&str>],
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    let mut calls = FluentCallCursor::new(source);
    for entry in entries {
        match entry {
            Entry::Message(message) => {
                validate_fluent_function_pattern(
                    message.value.as_ref(),
                    path,
                    source,
                    diagnostics,
                    &mut calls,
                    None,
                )?;
                for attribute in &message.attributes {
                    validate_fluent_function_pattern(
                        Some(&attribute.value),
                        path,
                        source,
                        diagnostics,
                        &mut calls,
                        None,
                    )?;
                }
            }
            Entry::Term(term) => {
                validate_fluent_function_pattern(
                    Some(&term.value),
                    path,
                    source,
                    diagnostics,
                    &mut calls,
                    None,
                )?;
                for attribute in &term.attributes {
                    validate_fluent_function_pattern(
                        Some(&attribute.value),
                        path,
                        source,
                        diagnostics,
                        &mut calls,
                        None,
                    )?;
                }
            }
            Entry::Junk { .. }
            | Entry::Comment(_)
            | Entry::GroupComment(_)
            | Entry::ResourceComment(_) => {}
        }
    }
    Ok(())
}

pub(super) fn fluent_pattern<'a>(
    entries: &'a [Entry<String>],
    id: &str,
    attribute: Option<&str>,
) -> Option<&'a Pattern<String>> {
    entries.iter().find_map(|entry| match entry {
        Entry::Message(message) if message.id.name == id => match attribute {
            Some(attribute) => message
                .attributes
                .iter()
                .find(|candidate| candidate.id.name == attribute)
                .map(|attribute| &attribute.value),
            None => message.value.as_ref(),
        },
        Entry::Term(term) if id.strip_prefix('-') == Some(term.id.name.as_ref()) => match attribute
        {
            Some(attribute) => term
                .attributes
                .iter()
                .find(|candidate| candidate.id.name == attribute)
                .map(|attribute| &attribute.value),
            None => Some(&term.value),
        },
        _ => None,
    })
}

/// Lists the concrete patterns that a message or term exposes. A missing
/// message value is distinct from an attribute and must not be treated as a
/// callable formatting target.
pub(super) fn fluent_pattern_targets(entries: &[Entry<String>], id: &str) -> Vec<Option<String>> {
    let entry = entries.iter().find(|entry| match entry {
        Entry::Message(message) => message.id.name == id,
        Entry::Term(term) => id.strip_prefix('-') == Some(term.id.name.as_ref()),
        Entry::Junk { .. }
        | Entry::Comment(_)
        | Entry::GroupComment(_)
        | Entry::ResourceComment(_) => false,
    });
    let Some(entry) = entry else {
        return Vec::new();
    };
    let (value, attributes) = match entry {
        Entry::Message(message) => (
            message.value.is_some(),
            message
                .attributes
                .iter()
                .map(|attribute| attribute.id.name.to_string())
                .collect::<Vec<_>>(),
        ),
        Entry::Term(term) => (
            true,
            term.attributes
                .iter()
                .map(|attribute| attribute.id.name.to_string())
                .collect(),
        ),
        Entry::Junk { .. }
        | Entry::Comment(_)
        | Entry::GroupComment(_)
        | Entry::ResourceComment(_) => return Vec::new(),
    };
    value
        .then_some(None)
        .into_iter()
        .chain(attributes.into_iter().map(Some))
        .collect()
}

pub(super) fn resolve_binding(
    bindings: &BTreeMap<String, Option<String>>,
    variable: &str,
) -> Option<String> {
    bindings
        .get(variable)
        .cloned()
        .unwrap_or_else(|| Some(variable.to_owned()))
}

fn validate_fluent_function_pattern(
    pattern: Option<&Pattern<&str>>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    let Some(pattern) = pattern else {
        return Ok(());
    };
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            validate_fluent_function_expression(
                expression,
                path,
                source,
                diagnostics,
                calls,
                selector,
            )?;
        }
    }
    Ok(())
}

fn validate_fluent_function_expression(
    expression: &Expression<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    match expression {
        Expression::Inline(expression) => {
            validate_fluent_function_inline(expression, path, source, diagnostics, calls, selector)
        }
        Expression::Select { selector, variants } => {
            validate_fluent_function_inline(selector, path, source, diagnostics, calls, None)?;
            let nested_selector = match selector {
                InlineExpression::VariableReference { id } => Some(id.name),
                _ => None,
            };
            for variant in variants {
                validate_fluent_function_pattern(
                    Some(&variant.value),
                    path,
                    source,
                    diagnostics,
                    calls,
                    nested_selector,
                )?;
            }
            Ok(())
        }
    }
}

pub(super) fn number_arguments_use_default_precision<S: AsRef<str>>(
    arguments: &CallArguments<S>,
) -> bool {
    arguments.named.iter().all(|argument| match argument.name.name.as_ref() {
        "minimumFractionDigits" => matches!(&argument.value, InlineExpression::NumberLiteral { value } if value.as_ref() == "0"),
        "maximumFractionDigits" => matches!(&argument.value, InlineExpression::NumberLiteral { value } if value.as_ref() == "3"),
        _ => true,
    })
}

fn validate_fluent_function_inline(
    expression: &InlineExpression<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    match expression {
        InlineExpression::FunctionReference { id, arguments } => {
            let span = calls.next(id.name);
            match FluentFunction::parse(id.name) {
                Some(FluentFunction::Number) => {
                    validate_number_arguments(arguments, path, source, diagnostics, calls, span)?;
                    let selected = matches!(
                        arguments.positional.as_slice(),
                        [InlineExpression::VariableReference { id }] if Some(id.name) == selector
                    );
                    if selected && !number_arguments_use_default_precision(arguments) {
                        return Err(invalid_number_call(
                            path,
                            source,
                            diagnostics,
                            "NUMBER() for a plural selector must use the default fraction-digit policy",
                            span,
                        ));
                    }
                    Ok(())
                }
                Some(FluentFunction::DateTime) => {
                    validate_datetime_arguments(arguments, path, source, diagnostics, calls, span)
                }
                None => Err(unknown_fluent_function(
                    path,
                    source,
                    diagnostics,
                    id.name,
                    span,
                )),
            }
        }
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => validate_fluent_function_arguments(
            arguments,
            path,
            source,
            diagnostics,
            calls,
            selector,
        ),
        InlineExpression::Placeable { expression } => validate_fluent_function_expression(
            expression,
            path,
            source,
            diagnostics,
            calls,
            selector,
        ),
        _ => Ok(()),
    }
}

fn validate_fluent_function_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    for argument in &arguments.positional {
        validate_fluent_function_inline(argument, path, source, diagnostics, calls, selector)?;
    }
    for argument in &arguments.named {
        validate_fluent_function_inline(
            &argument.value,
            path,
            source,
            diagnostics,
            calls,
            selector,
        )?;
    }
    Ok(())
}

fn validate_number_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    span: SourceSpan,
) -> Result<(), Error> {
    let invalid = || invalid_number_call(path, source, diagnostics, "invalid NUMBER() call", span);
    if arguments.positional.len() != 1 {
        return Err(invalid());
    }
    let mut minimum = 0;
    let mut maximum = None;
    for argument in &arguments.named {
        match argument.name.name {
            "useGrouping" => match &argument.value {
                InlineExpression::StringLiteral { value } if matches!(*value, "true" | "false") => {
                }
                _ => {
                    return Err(invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "useGrouping must be the string literal \"true\" or \"false\"",
                        span,
                    ));
                }
            },
            "minimumFractionDigits" => {
                minimum = number_option(&argument.value).ok_or_else(|| {
                    invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "minimumFractionDigits must be a supported non-negative integer",
                        span,
                    )
                })?;
            }
            "maximumFractionDigits" => {
                maximum = Some(number_option(&argument.value).ok_or_else(|| {
                    invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "maximumFractionDigits must be a supported non-negative integer",
                        span,
                    )
                })?);
            }
            _ => {
                return Err(invalid_number_call(
                    path,
                    source,
                    diagnostics,
                    "NUMBER() supports only useGrouping, minimumFractionDigits, and maximumFractionDigits",
                    span,
                ));
            }
        }
    }
    number::Options::with_optional_maximum(true, minimum, maximum).map_err(|error| {
        let message = match error {
            number::Error::ImplicitMaximumTooSmall {
                minimum,
                default_maximum,
            } => format!(
                "minimumFractionDigits={minimum} requires maximumFractionDigits of at least {minimum}; the omitted maximum defaults to {default_maximum}"
            ),
            _ => error.to_string(),
        };
        invalid_number_call(path, source, diagnostics, &message, span)
    })?;
    validate_fluent_function_arguments(arguments, path, source, diagnostics, calls, None)
}

/// `DATETIME()` deliberately exposes only date and time field-set lengths.
/// Fluent otherwise accepts arbitrary named options and silently ignores them.
fn validate_datetime_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    span: SourceSpan,
) -> Result<(), Error> {
    let invalid = |message| invalid_datetime_call(path, source, diagnostics, message, span);
    if arguments.positional.len() != 1 {
        return Err(invalid("DATETIME() requires exactly one value argument"));
    }
    if !matches!(
        arguments.positional.as_slice(),
        [InlineExpression::VariableReference { .. }]
    ) {
        return Err(invalid("DATETIME() requires a variable value argument"));
    }
    let mut styles = BTreeSet::new();
    for argument in &arguments.named {
        let name = argument.name.name;
        if !matches!(name, "dateStyle" | "timeStyle") {
            return Err(invalid(
                "DATETIME() supports only dateStyle and timeStyle options",
            ));
        }
        if !styles.insert(name) {
            return Err(invalid("DATETIME() cannot repeat a style option"));
        }
        match &argument.value {
            InlineExpression::StringLiteral { value }
                if matches!(*value, "short" | "medium" | "long") => {}
            _ => {
                return Err(invalid(
                    "DATETIME() styles must be the string literal \"short\", \"medium\", or \"long\"",
                ));
            }
        }
    }
    if styles.is_empty() {
        return Err(invalid("DATETIME() requires dateStyle or timeStyle"));
    }
    validate_fluent_function_arguments(arguments, path, source, diagnostics, calls, None)
}

fn invalid_number_call(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    message: &str,
    span: SourceSpan,
) -> Error {
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: message.into(),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span,
    }))
}

fn invalid_datetime_call(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    message: &str,
    span: SourceSpan,
) -> Error {
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: message.into(),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span,
    }))
}

/// Fluent-bundle registers one function for Heine. Checking every function
/// reference while loading a resource keeps a typo in a translation from
/// becoming a later, unrelated template-rendering error.
fn unknown_fluent_function(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    function: &str,
    span: SourceSpan,
) -> Error {
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: format!(
            "unknown Fluent function {function:?}; Heine currently provides NUMBER() and DATETIME()"
        ),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span,
    }))
}

/// Fluent's AST intentionally does not retain source spans. This narrow lexer
/// recovers function-name spans without mistaking ordinary resource text or a
/// string literal for a call. Validation visits expressions in source order,
/// so the cursor connects each validated call to its own source span.
struct FluentCallCursor {
    calls: Vec<(String, SourceSpan)>,
    next: usize,
}

impl FluentCallCursor {
    fn new(source: &str) -> Self {
        let mut calls = Vec::new();
        let mut offset = 0;
        let mut placeables = 0usize;
        let mut quoted = false;
        let mut line_start = true;

        while offset < source.len() {
            let Some(character) = source[offset..].chars().next() else {
                break;
            };
            if quoted {
                if character == '\\' {
                    offset += 1;
                    if let Some(next) = source[offset..].chars().next() {
                        offset += next.len_utf8();
                    }
                    continue;
                }
                if character == '"' {
                    quoted = false;
                }
                offset += character.len_utf8();
                continue;
            }

            if line_start && character.is_ascii_whitespace() && character != '\n' {
                offset += character.len_utf8();
                continue;
            }
            if line_start && character == '#' {
                let line_end = source[offset..]
                    .find('\n')
                    .map_or(source.len(), |length| offset + length);
                offset = line_end;
                continue;
            }

            line_start = character == '\n';
            match character {
                '"' if placeables > 0 => quoted = true,
                '{' => placeables += 1,
                '}' => placeables = placeables.saturating_sub(1),
                _ if placeables > 0 && fluent_identifier_character(character) => {
                    let start = offset;
                    offset += character.len_utf8();
                    while source[offset..]
                        .chars()
                        .next()
                        .is_some_and(fluent_identifier_character)
                    {
                        offset += source[offset..].chars().next().map_or(0, char::len_utf8);
                    }
                    let name = &source[start..offset];
                    let mut next = offset;
                    while source[next..]
                        .chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_whitespace())
                    {
                        next += source[next..].chars().next().map_or(0, char::len_utf8);
                    }
                    if source[next..].starts_with('(') {
                        calls.push((name.to_owned(), (start, name.len()).into()));
                    }
                    continue;
                }
                _ => {}
            }
            offset += character.len_utf8();
        }

        Self { calls, next: 0 }
    }

    fn next(&mut self, name: &str) -> SourceSpan {
        let Some((position, (_, span))) = self
            .calls
            .iter()
            .enumerate()
            .skip(self.next)
            .find(|(_, (candidate, _))| candidate == name)
        else {
            return (0, name.len()).into();
        };
        self.next = position + 1;
        *span
    }
}

fn fluent_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn number_option(value: &InlineExpression<&str>) -> Option<u16> {
    let InlineExpression::NumberLiteral { value } = value else {
        return None;
    };
    value
        .parse::<u16>()
        .ok()
        .filter(|value| *value <= number::MAX_FRACTION_DIGITS)
}

/// Finds each parsed top-level entry after its predecessor. `split_inclusive`
/// preserves CRLF byte offsets, which is essential for miette source spans.
fn message_span(source: &str, offset: usize, id: &str) -> Option<(SourceSpan, usize)> {
    let mut line_offset = offset;
    for line in source[offset..].split_inclusive('\n') {
        let leading = line.len() - line.trim_start().len();
        let candidate = &line[leading..];
        if leading == 0
            && candidate
                .strip_prefix(id)
                .is_some_and(|tail| tail.starts_with('=') || tail.starts_with(char::is_whitespace))
        {
            return Some((
                (line_offset + leading, id.len()).into(),
                line_offset + line.len(),
            ));
        }
        line_offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::message_span;

    #[test]
    fn message_spans_only_match_top_level_fluent_declarations() {
        let source =
            "first =\n    duplicate = text inside a multiline value\nduplicate = actual message\n";

        let (span, _) = message_span(source, 0, "duplicate").expect("message declaration");

        assert_eq!(span.offset(), source.find("duplicate = actual").unwrap());
        assert_eq!(span.len(), "duplicate".len());
    }
}
