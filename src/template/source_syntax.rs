//! Conservative inspection and rewriting of executable Tera source.
//!
//! Tera does not expose the small source-level facts that Kit ownership needs.
//! This module deliberately recognizes only those facts while excluding
//! comments and raw blocks, which are examples rather than executable code.

use miette::SourceSpan;

#[derive(Debug)]
pub(super) struct TemplateReference<'a> {
    pub(super) name: &'a str,
    pub(super) span: SourceSpan,
}

#[derive(Debug)]
pub(super) struct ComponentReference<'a> {
    pub(super) name: &'a str,
    pub(super) span: SourceSpan,
}

#[derive(Debug)]
pub(super) struct ComponentCall {
    pub(super) name: String,
    pub(super) span: SourceSpan,
}

pub(super) fn template_references(source: &str) -> Vec<TemplateReference<'_>> {
    let mut references = Vec::new();
    visit_template_directives(source, |content, offset| {
        let Some(name) = directive_name(content) else {
            return;
        };
        if !matches!(name, "extends" | "include") {
            return;
        }
        let after_name = &content[name.len()..];
        let (argument, argument_offset) = trim_directive(after_name);
        if let Some((reference, value_offset)) = quoted_template_string(argument) {
            references.push(TemplateReference {
                name: reference,
                span: (
                    offset + name.len() + argument_offset + value_offset,
                    reference.len(),
                )
                    .into(),
            });
        }
    });
    references
}

pub(super) fn component_definitions(source: &str) -> Vec<ComponentReference<'_>> {
    let mut definitions = Vec::new();
    visit_template_directives(source, |content, offset| {
        if directive_name(content) != Some("component") {
            return;
        }
        let after = &content["component".len()..];
        let (after, after_offset) = trim_directive(after);
        let name_len = after
            .find(|character: char| character == '(' || character.is_whitespace())
            .unwrap_or(after.len());
        let name = &after[..name_len];
        if !name.is_empty() {
            definitions.push(ComponentReference {
                name,
                span: (offset + "component".len() + after_offset, name.len()).into(),
            });
        }
    });
    definitions
}

pub(super) fn component_calls(source: &str, prefix: &str) -> Vec<ComponentCall> {
    let mut calls = Vec::new();
    visit_template_expressions(source, |expression, offset| {
        let mut cursor = 0;
        let mut quote = None;
        while cursor < expression.len() {
            let remaining = &expression[cursor..];
            let Some(character) = remaining.chars().next() else {
                break;
            };
            if let Some(delimiter) = quote {
                cursor += character.len_utf8();
                if character == '\\' {
                    if let Some(escaped) = expression[cursor..].chars().next() {
                        cursor += escaped.len_utf8();
                    }
                } else if character == delimiter {
                    quote = None;
                }
                continue;
            }
            if matches!(character, '\'' | '"') {
                quote = Some(character);
                cursor += character.len_utf8();
                continue;
            }
            if character == '<' {
                let after = &expression[cursor + character.len_utf8()..];
                let name_len = after
                    .find(|character: char| {
                        character.is_whitespace() || matches!(character, '(' | '/' | '>')
                    })
                    .unwrap_or(after.len());
                let name = &after[..name_len];
                if name.starts_with(prefix) {
                    calls.push(ComponentCall {
                        name: name.to_owned(),
                        span: (offset + cursor + character.len_utf8(), name.len()).into(),
                    });
                }
                cursor += character.len_utf8() + name_len.max(1);
                continue;
            }
            cursor += character.len_utf8();
        }
    });
    calls
}

/// `asset` and `__a__` have equal byte length, so replacing the internal
/// capability name preserves Tera's diagnostics against authored source.
pub(super) fn rewrite_kit_asset_calls(source: &str) -> Option<String> {
    let mut replacements = Vec::new();
    visit_template_expressions(source, |expression, offset| {
        if let Some(rewritten) = rewrite_function_calls(expression, "asset", "__a__") {
            replacements.push((offset, expression.len(), rewritten));
        }
    });
    if replacements.is_empty() {
        return None;
    }

    let mut rewritten = String::with_capacity(source.len());
    let mut copied_until = 0;
    for (offset, length, replacement) in replacements {
        rewritten.push_str(&source[copied_until..offset]);
        rewritten.push_str(&replacement);
        copied_until = offset + length;
    }
    rewritten.push_str(&source[copied_until..]);
    Some(rewritten)
}

pub(super) fn template_function_call(source: &str, name: &str) -> Option<SourceSpan> {
    let mut result = None;
    visit_template_expressions(source, |expression, offset| {
        if result.is_none() {
            result = function_call_offset(expression, name)
                .map(|call_offset| (offset + call_offset, name.len()).into());
        }
    });
    result
}

fn visit_template_directives<'source>(
    source: &'source str,
    mut visit: impl FnMut(&'source str, usize),
) {
    visit_template_expressions(source, |expression, offset| {
        let (directive, directive_offset) = trim_directive(expression);
        if !matches!(directive_name(directive), Some("raw" | "endraw")) {
            visit(directive, offset + directive_offset);
        }
    });
}

fn visit_template_expressions<'source>(
    source: &'source str,
    mut visit: impl FnMut(&'source str, usize),
) {
    let mut cursor = 0;
    let mut raw_depth = 0;
    while cursor < source.len() {
        let remaining = &source[cursor..];
        let next = [
            remaining.find("{%"),
            remaining.find("{{"),
            remaining.find("{#"),
        ]
        .into_iter()
        .flatten()
        .min();
        let Some(next) = next else {
            break;
        };
        cursor += next;
        let remaining = &source[cursor..];
        if remaining.starts_with("{#") {
            let Some(end) = remaining.find("#}") else {
                break;
            };
            cursor += end + 2;
            continue;
        }
        let (open, close) = if remaining.starts_with("{%") {
            ("{%", "%}")
        } else {
            ("{{", "}}")
        };
        let Some(end) = remaining.find(close) else {
            break;
        };
        let expression = &remaining[open.len()..end];
        let directive = trim_directive(expression).0;
        let name = directive_name(directive);
        if raw_depth == 0 {
            visit(expression, cursor + open.len());
        }
        if open == "{%" {
            match name {
                Some("raw") if raw_depth == 0 => raw_depth += 1,
                Some("endraw") if raw_depth > 0 => raw_depth -= 1,
                _ => {}
            }
        }
        cursor += end + close.len();
    }
}

fn rewrite_function_calls(expression: &str, from: &str, to: &str) -> Option<String> {
    debug_assert_eq!(from.len(), to.len());
    function_call_offset(expression, from)?;
    let mut output = String::with_capacity(expression.len());
    let mut cursor = 0;
    let mut quote = None;
    while cursor < expression.len() {
        let remaining = &expression[cursor..];
        let character = remaining.chars().next()?;
        if let Some(delimiter) = quote {
            output.push(character);
            cursor += character.len_utf8();
            if character == '\\' {
                if let Some(escaped) = expression[cursor..].chars().next() {
                    output.push(escaped);
                    cursor += escaped.len_utf8();
                }
            } else if character == delimiter {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            output.push(character);
            cursor += character.len_utf8();
            continue;
        }
        if remaining.starts_with(from) && function_call_offset(remaining, from) == Some(0) {
            output.push_str(to);
            cursor += from.len();
            continue;
        }
        output.push(character);
        cursor += character.len_utf8();
    }
    Some(output)
}

fn function_call_offset(expression: &str, name: &str) -> Option<usize> {
    let mut cursor = 0;
    let mut quote = None;
    while cursor < expression.len() {
        let remaining = &expression[cursor..];
        let character = remaining.chars().next()?;
        if let Some(delimiter) = quote {
            cursor += character.len_utf8();
            if character == '\\' {
                cursor += expression[cursor..].chars().next()?.len_utf8();
            } else if character == delimiter {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            cursor += character.len_utf8();
            continue;
        }
        if remaining.starts_with(name)
            && !expression[..cursor]
                .chars()
                .next_back()
                .is_some_and(is_identifier_character)
        {
            let after_name = cursor + name.len();
            let after_whitespace = expression[after_name..]
                .find(|character: char| !character.is_whitespace())
                .map(|offset| after_name + offset)?;
            if expression[after_whitespace..].starts_with('(')
                && !expression[after_name..]
                    .chars()
                    .next()
                    .is_some_and(is_identifier_character)
            {
                return Some(cursor);
            }
        }
        cursor += character.len_utf8();
    }
    None
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn trim_directive(value: &str) -> (&str, usize) {
    let value = value.trim_start_matches(char::is_whitespace);
    let marker = value.strip_prefix('-').unwrap_or(value);
    let marker_offset = value.len() - marker.len();
    let trimmed = marker.trim_start_matches(char::is_whitespace);
    (trimmed, marker_offset + marker.len() - trimmed.len())
}

fn directive_name(value: &str) -> Option<&str> {
    let end = value
        .find(|character: char| character.is_whitespace() || character == '-')
        .unwrap_or(value.len());
    (!value[..end].is_empty()).then_some(&value[..end])
}

fn quoted_template_string(value: &str) -> Option<(&str, usize)> {
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"' | '`') {
        return None;
    }
    let value = &value[quote.len_utf8()..];
    let end = value.find(quote)?;
    Some((&value[..end], quote.len_utf8()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_literal_template_references_but_ignores_comments_and_raw_content() {
        let source = r#"
{# {% extends "kit/paper/comment.tera" %} #}
{% extends "kit/paper/base.tera" %}
{% include "kit/paper/partial.tera" %}
{% include `kit/paper/raw.tera` %}
{% raw %}{% include "kit/paper/example.tera" %}{% endraw %}
"#;
        let references = template_references(source)
            .into_iter()
            .map(|reference| reference.name)
            .collect::<Vec<_>>();
        assert_eq!(
            references,
            [
                "kit/paper/base.tera",
                "kit/paper/partial.tera",
                "kit/paper/raw.tera"
            ]
        );
    }

    #[test]
    fn rewrites_only_executable_kit_asset_calls_without_moving_spans() {
        let source = r#"
{# {{ asset(id='kit/paper/assets/comment.svg') }} #}
{% raw %}{{ asset(id='kit/paper/assets/example.svg') }}{% endraw %}
{{ asset (id='kit/paper/assets/mark.svg') }}
{{ "asset(id='literal')" }}
"#;
        let rewritten =
            rewrite_kit_asset_calls(source).expect("executable asset call is rewritten");
        assert_eq!(rewritten.len(), source.len());
        assert!(rewritten.contains("{{ __a__ (id='kit/paper/assets/mark.svg') }}"));
        assert!(rewritten.contains("asset(id='kit/paper/assets/comment.svg')"));
        assert!(rewritten.contains("asset(id='kit/paper/assets/example.svg')"));
        assert!(rewritten.contains("\"asset(id='literal')\""));
    }

    #[test]
    fn finds_reserved_internal_asset_calls_only_in_executable_expressions() {
        let source = "{# {{ __a__(id='comment') }} #}\n{{ __a__(id='actual') }}";
        let span = template_function_call(source, "__a__").expect("call is found");
        assert_eq!(span.offset(), source.find("__a__(id='actual')").unwrap());
    }

    #[test]
    fn finds_component_calls_only_outside_quoted_template_strings() {
        let source = r#"
{# {{ <paper.private /> }} #}
{% raw %}{{ <paper.private /> }}{% endraw %}
{{ "<paper.private />" }}
{{ <paper.card label="Public" /> }}
"#;

        let calls = component_calls(source, "paper.")
            .into_iter()
            .map(|call| call.name)
            .collect::<Vec<_>>();
        assert_eq!(calls, ["paper.card"]);
    }
}
