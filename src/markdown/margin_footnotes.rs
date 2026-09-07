//! Source-aware planning and local rendering for margin footnotes.
//!
//! The plan keeps footnote relationships in Markdown terms. Rendering uses the
//! same transformed event stream as ordinary Markdown, so it never needs to
//! inspect or rewrite generated HTML.

use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    path::Path,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use pulldown_cmark::{Event, Parser, Tag, TagEnd, html};
use thiserror::Error;

use super::{
    RenderError, Renderer, highlight_code_blocks, options, push_html_attribute, task_labels,
    transform_event,
};
use crate::{asset::AssetLinks, config::MarkdownConfig, site::PageLinks};

/// The loading-time facts required to render one selected Markdown input as
/// local note instances. It deliberately excludes rendered HTML.
#[derive(Debug, Clone)]
pub(crate) struct MarginFootnotePlan {
    content: String,
    marker_message: String,
    references: BTreeMap<usize, Reference>,
    blocks: BTreeMap<usize, Block>,
}

#[derive(Debug, Clone)]
struct Reference {
    definition: String,
    number: usize,
    instance: usize,
}

#[derive(Debug, Clone)]
struct Block {
    references: Vec<usize>,
}

impl MarginFootnotePlan {
    pub(crate) fn content(&self) -> &str {
        &self.content
    }

    pub(crate) fn marker_message(&self) -> &str {
        &self.marker_message
    }

    pub(crate) fn number_count(&self) -> usize {
        self.references
            .values()
            .map(|reference| reference.number)
            .max()
            .unwrap_or_default()
    }
}

#[derive(Debug, Error)]
enum PlanError {
    #[error("margin footnote reference {label:?} has no definition")]
    MissingDefinition { label: String, span: Range<usize> },
    #[error("margin footnote definition {label:?} is not referenced")]
    UnusedDefinition { label: String, span: Range<usize> },
    #[error("margin footnote definition {label:?} is declared more than once")]
    DuplicateDefinition {
        label: String,
        span: Range<usize>,
        first_span: Range<usize>,
    },
    #[error("margin footnote references need an ordinary paragraph or heading")]
    UnsupportedPlacement { span: Range<usize> },
    #[error("a margin footnote paragraph or heading cannot contain raw HTML")]
    RawHtml { span: Range<usize> },
}

/// A source diagnostic with distinct summary and label text, including both
/// definition locations where the authored relationship has two endpoints.
#[derive(Debug, Diagnostic, Error)]
#[diagnostic(code(heine::markdown::margin_footnotes))]
#[error("{message}")]
pub(crate) struct MarginFootnoteDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
    #[help]
    help: String,
    #[related]
    related: Vec<RelatedDefinition>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("first definition of this margin footnote")]
struct RelatedDefinition {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("first definition here")]
    span: SourceSpan,
}

/// Parses and checks the whole footnote graph while the Markdown source and
/// parser offsets are available.
pub(crate) fn plan(
    path: &Path,
    content: String,
    input: &str,
    config: &MarkdownConfig,
    marker_message: String,
) -> Result<MarginFootnotePlan, Box<MarginFootnoteDiagnostic>> {
    plan_inner(content, input, config, marker_message).map_err(|error| {
        Box::new(MarginFootnoteDiagnostic::from_plan_error(
            path, input, error,
        ))
    })
}

fn plan_inner(
    content: String,
    input: &str,
    config: &MarkdownConfig,
    marker_message: String,
) -> Result<MarginFootnotePlan, PlanError> {
    let mut definitions = BTreeMap::<String, Range<usize>>::new();
    let mut references = Vec::<(String, Range<usize>, usize)>::new();
    let mut blocks = BTreeMap::<usize, PendingBlock>::new();
    let mut active_block = None::<usize>;
    let mut prohibited_depth = 0usize;
    let mut definition_depth = 0usize;
    let mut unresolved = UnresolvedReferenceScanner::default();

    for (event, span) in Parser::new_ext(input, options(config)).into_offset_iter() {
        match event {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                let label = label.into_string();
                if let Some(first_span) = definitions.insert(label.clone(), span.clone()) {
                    return Err(PlanError::DuplicateDefinition {
                        label,
                        span,
                        first_span,
                    });
                }
                definition_depth += 1;
            }
            Event::End(TagEnd::FootnoteDefinition) => definition_depth -= 1,
            Event::Start(tag) if is_prohibited_start(&tag) => prohibited_depth += 1,
            Event::End(end) if is_prohibited_end(end) => prohibited_depth -= 1,
            Event::Start(Tag::Paragraph) => {
                active_block = Some(span.start);
                blocks.insert(span.start, PendingBlock::default());
                unresolved.clear();
            }
            Event::Start(Tag::Heading { .. }) => {
                active_block = Some(span.start);
                blocks.insert(span.start, PendingBlock::default());
                unresolved.clear();
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::Heading(_)) => {
                active_block = None;
                unresolved.clear();
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                if let Some(start) = active_block
                    && let Some(block) = blocks.get_mut(&start)
                {
                    block.has_html = true;
                }
            }
            Event::FootnoteReference(label) => {
                let Some(block_start) = active_block else {
                    return Err(PlanError::UnsupportedPlacement { span });
                };
                if definition_depth > 0 || prohibited_depth > 0 {
                    return Err(PlanError::UnsupportedPlacement { span });
                }
                let reference_index = references.len();
                references.push((label.into_string(), span.clone(), block_start));
                let Some(block) = blocks.get_mut(&block_start) else {
                    return Err(PlanError::UnsupportedPlacement { span });
                };
                block.references.push(reference_index);
            }
            // pulldown-cmark splits an unresolved `[^label]` into ordinary
            // text events rather than producing `FootnoteReference`. Keep a
            // tiny scanner across adjacent text events so it still receives a
            // missing-definition diagnostic at its authored location.
            Event::Text(_) => {
                for (label, range) in unresolved.push(&input[span.clone()], span.start) {
                    if definition_depth > 0 || prohibited_depth > 0 {
                        return Err(PlanError::UnsupportedPlacement { span: range });
                    }
                    let Some(block_start) = active_block else {
                        return Err(PlanError::UnsupportedPlacement { span: range });
                    };
                    let reference_index = references.len();
                    references.push((label, range, block_start));
                    let Some(block) = blocks.get_mut(&block_start) else {
                        return Err(PlanError::UnsupportedPlacement { span: span.clone() });
                    };
                    block.references.push(reference_index);
                }
            }
            _ => {}
        }
    }

    let used = references
        .iter()
        .map(|(label, _, _)| label.clone())
        .collect::<BTreeSet<_>>();
    if let Some((label, span)) = definitions.iter().find(|(label, _)| !used.contains(*label)) {
        return Err(PlanError::UnusedDefinition {
            label: label.clone(),
            span: span.clone(),
        });
    }
    if let Some((label, span, _)) = references
        .iter()
        .find(|(label, _, _)| !definitions.contains_key(label))
    {
        return Err(PlanError::MissingDefinition {
            label: label.clone(),
            span: span.clone(),
        });
    }
    if let Some((_, block)) = blocks
        .iter()
        .find(|(_, block)| block.has_html && !block.references.is_empty())
    {
        let (_, span, _) = &references[block.references[0]];
        return Err(PlanError::RawHtml { span: span.clone() });
    }

    let mut numbers = BTreeMap::<String, usize>::new();
    let mut resolved_references = BTreeMap::new();
    let mut resolved_blocks = BTreeMap::new();
    for (index, (definition, span, block_start)) in references.into_iter().enumerate() {
        let next = numbers.len() + 1;
        let number = *numbers.entry(definition.clone()).or_insert(next);
        resolved_references.insert(
            span.start,
            Reference {
                definition,
                number,
                instance: index + 1,
            },
        );
        resolved_blocks
            .entry(block_start)
            .or_insert_with(|| Block {
                references: Vec::new(),
            })
            .references
            .push(span.start);
    }

    Ok(MarginFootnotePlan {
        content,
        marker_message,
        references: resolved_references,
        blocks: resolved_blocks,
    })
}

#[derive(Default)]
struct UnresolvedReferenceScanner {
    state: UnresolvedReferenceState,
}

#[derive(Default)]
enum UnresolvedReferenceState {
    #[default]
    Outside,
    OpenBracket {
        start: usize,
    },
    Label {
        start: usize,
        label: String,
    },
}

impl UnresolvedReferenceScanner {
    fn clear(&mut self) {
        self.state = UnresolvedReferenceState::Outside;
    }

    fn push(&mut self, text: &str, offset: usize) -> Vec<(String, Range<usize>)> {
        let mut references = Vec::new();
        for (index, character) in text.char_indices() {
            let position = offset + index;
            match &mut self.state {
                UnresolvedReferenceState::Outside if character == '[' => {
                    self.state = UnresolvedReferenceState::OpenBracket { start: position };
                }
                UnresolvedReferenceState::Outside => {}
                UnresolvedReferenceState::OpenBracket { start } if character == '^' => {
                    self.state = UnresolvedReferenceState::Label {
                        start: *start,
                        label: String::new(),
                    };
                }
                UnresolvedReferenceState::OpenBracket { .. } if character == '[' => {}
                UnresolvedReferenceState::OpenBracket { .. } => {
                    self.state = UnresolvedReferenceState::Outside;
                }
                UnresolvedReferenceState::Label { start, label } if character == ']' => {
                    if !label.is_empty() {
                        references.push((label.clone(), *start..position + character.len_utf8()));
                    }
                    self.state = UnresolvedReferenceState::Outside;
                }
                UnresolvedReferenceState::Label { .. }
                    if character.is_whitespace() || character == '[' =>
                {
                    self.state = UnresolvedReferenceState::Outside;
                }
                UnresolvedReferenceState::Label { label, .. } => label.push(character),
            }
        }
        references
    }
}

#[derive(Default)]
struct PendingBlock {
    references: Vec<usize>,
    has_html: bool,
}

fn is_prohibited_start(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::List(_)
            | Tag::Item
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::Link { .. }
            | Tag::Image { .. }
    )
}

fn is_prohibited_end(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::Link
            | TagEnd::Image
    )
}

impl MarginFootnoteDiagnostic {
    fn from_plan_error(path: &Path, input: &str, error: PlanError) -> Self {
        let source_code = crate::source_diagnostic::named_source(path, input, "Markdown");
        match error {
            PlanError::MissingDefinition { label, span } => Self {
                message: format!("margin footnote reference {label:?} has no definition"),
                source_code,
                span: span.into(),
                label: "reference without a matching definition".into(),
                help: "add one [^label]: definition, or remove this reference".into(),
                related: Vec::new(),
            },
            PlanError::UnusedDefinition { label, span } => Self {
                message: format!("margin footnote definition {label:?} is not referenced"),
                source_code,
                span: span.into(),
                label: "definition without a matching reference".into(),
                help: "add a [^label] reference in an ordinary paragraph or heading, or remove this definition".into(),
                related: Vec::new(),
            },
            PlanError::DuplicateDefinition {
                label,
                span,
                first_span,
            } => Self {
                message: format!("margin footnote definition {label:?} is declared more than once"),
                source_code: source_code.clone(),
                span: span.into(),
                label: "duplicate definition".into(),
                help: "keep one definition for each footnote label".into(),
                related: vec![RelatedDefinition {
                    source_code,
                    span: first_span.into(),
                }],
            },
            PlanError::UnsupportedPlacement { span } => Self {
                message: "margin footnote references need an ordinary paragraph or heading".into(),
                source_code,
                span: span.into(),
                label: "this reference has no valid local prose wrapper".into(),
                help: "move it to an ordinary paragraph or Markdown heading; lists, quotes, tables, links, images, code, and footnotes are not supported yet".into(),
                related: Vec::new(),
            },
            PlanError::RawHtml { span } => Self {
                message: "a margin footnote paragraph or heading cannot contain raw HTML".into(),
                source_code,
                span: span.into(),
                label: "this reference shares its prose block with raw HTML".into(),
                help: "move the raw HTML or the footnote reference into separate prose blocks".into(),
                related: Vec::new(),
            },
        }
    }
}

/// Renders one selected Markdown input. Definitions are removed from ordinary
/// flow and their transformed event bodies are repeated beside each reference.
pub(crate) fn render(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: Option<&[String]>,
    plan: &MarginFootnotePlan,
    labels: &[String],
) -> Result<String, RenderError> {
    let mut anchors = anchors.unwrap_or_default().iter();
    let task_labels = task_labels(input, renderer.config());
    let mut task_labels = task_labels.iter();
    let mut in_definition = 0usize;
    let mut events = Vec::new();
    for (event, span) in Parser::new_ext(input, options(renderer.config())).into_offset_iter() {
        let anchors_enabled = in_definition == 0;
        let transformed = transform_event(
            (event.clone(), span.clone()),
            renderer.config().math(),
            assets,
            pages,
            &mut anchors,
            &mut task_labels,
            anchors_enabled,
        )?;
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => in_definition += 1,
            Event::End(TagEnd::FootnoteDefinition) => in_definition -= 1,
            _ => {}
        }
        events.push((transformed, span));
    }
    let events = highlight_code_blocks(events, renderer.highlighter.as_deref())?;
    render_events(events, plan, labels)
}

fn render_events(
    events: Vec<(Event<'static>, Range<usize>)>,
    plan: &MarginFootnotePlan,
    labels: &[String],
) -> Result<String, RenderError> {
    let definitions = definition_bodies(&events);
    let mut output = String::new();
    let mut active_block = None::<usize>;
    let mut definition_depth = 0usize;

    for (event, span) in events {
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => {
                definition_depth += 1;
                continue;
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                definition_depth -= 1;
                continue;
            }
            _ if definition_depth > 0 => continue,
            Event::Start(Tag::Paragraph) | Event::Start(Tag::Heading { .. })
                if plan.blocks.contains_key(&span.start) =>
            {
                output.push_str("<div class=\"heine-annotated-block\">");
                active_block = Some(span.start);
                push_event(&mut output, event);
            }
            Event::FootnoteReference(_) => {
                let reference = plan.references.get(&span.start).ok_or_else(|| {
                    RenderError::MarginFootnote {
                        message: "the resolved margin-footnote plan no longer matches its Markdown source".into(),
                        span: span.clone(),
                    }
                })?;
                push_marker(&mut output, reference);
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::Heading(_))
                if active_block.is_some() =>
            {
                push_event(&mut output, event);
                let Some(start) = active_block.take() else {
                    return Err(RenderError::MarginFootnote {
                        message: "an annotated prose block ended without a recorded start".into(),
                        span: span.clone(),
                    });
                };
                let Some(block) = plan.blocks.get(&start) else {
                    return Err(RenderError::MarginFootnote {
                        message: "the resolved margin-footnote block is unavailable".into(),
                        span: span.clone(),
                    });
                };
                for reference_start in &block.references {
                    let reference = &plan.references[reference_start];
                    let label = labels.get(reference.number - 1).ok_or_else(|| {
                        RenderError::MarginFootnote {
                            message: "the localized margin-footnote marker is unavailable".into(),
                            span: span.clone(),
                        }
                    })?;
                    let body = definitions.get(&reference.definition).ok_or_else(|| {
                        RenderError::MarginFootnote {
                            message: "the resolved margin-footnote definition is unavailable"
                                .into(),
                            span: span.clone(),
                        }
                    })?;
                    push_toggle(&mut output, reference, label);
                    push_note(&mut output, reference, body);
                }
                output.push_str("</div>");
            }
            event => push_event(&mut output, event),
        }
    }
    Ok(output)
}

fn definition_bodies(events: &[(Event<'static>, Range<usize>)]) -> BTreeMap<String, String> {
    let mut definitions = BTreeMap::new();
    let mut current = None::<(String, Vec<Event<'static>>)>;
    for (event, _) in events {
        match event {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                current = Some((label.to_string(), Vec::new()));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some((label, body)) = current.take() {
                    let mut rendered = String::new();
                    html::push_html(&mut rendered, body.into_iter());
                    definitions.insert(label, rendered);
                }
            }
            event => {
                if let Some((_, body)) = &mut current {
                    body.push(event.clone());
                }
            }
        }
    }
    definitions
}

fn push_event(output: &mut String, event: Event<'static>) {
    html::push_html(output, std::iter::once(event));
}

fn push_marker(output: &mut String, reference: &Reference) {
    let toggle_id = format!("heine-footnote-toggle-{}", reference.instance);
    output.push_str("<sup class=\"heine-footnote-marker\"><label for=\"");
    output.push_str(&toggle_id);
    output.push_str("\">");
    output.push_str(&reference.number.to_string());
    output.push_str("</label></sup>");
}

fn push_toggle(output: &mut String, reference: &Reference, label: &str) {
    let toggle_id = format!("heine-footnote-toggle-{}", reference.instance);
    let note_id = format!("heine-footnote-{}", reference.instance);
    output.push_str("<input class=\"heine-footnote-toggle\" id=\"");
    output.push_str(&toggle_id);
    output.push_str("\" type=\"checkbox\" aria-label=\"");
    push_html_attribute(label, output);
    output.push_str("\" aria-controls=\"");
    output.push_str(&note_id);
    output.push_str("\" aria-describedby=\"");
    output.push_str(&note_id);
    output.push_str("\">");
}

fn push_note(output: &mut String, reference: &Reference, body: &str) {
    let note_id = format!("heine-footnote-{}", reference.instance);
    output.push_str("<aside class=\"heine-footnote\" id=\"");
    output.push_str(&note_id);
    output.push_str(
        "\" role=\"doc-footnote\"><span class=\"heine-footnote-number\" aria-hidden=\"true\">",
    );
    output.push_str(&reference.number.to_string());
    output.push_str("</span>");
    output.push_str(body);
    output.push_str("</aside>");
}
