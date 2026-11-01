//! HTML production for ordinary declared Markdown.
//!
//! The parent module owns shared transformations because margin-footnote
//! rendering and source analysis use the same checked event semantics. This
//! module owns the ordinary event pipeline and its final HTML serialization.

use pulldown_cmark::{Event, Parser, Tag, TagEnd, html};

use crate::{asset::AssetLinks, site::PageLinks};

use super::{
    OrdinaryFootnotes, RenderError, Renderer, TransformLinks, apply_renderer_replacements,
    highlight_code_blocks, options, picture, task_labels, transform_event,
    unresolved_footnote_reference, wrap_tables,
};

pub(crate) struct ComponentRenderContext<'a> {
    pub(crate) renderer: &'a Renderer,
    pub(crate) assets: Option<&'a AssetLinks>,
    pub(crate) pages: Option<&'a PageLinks>,
    pub(crate) anchors: Option<&'a [String]>,
    pub(crate) table_scroll_label: Option<&'a str>,
    pub(crate) replacements: &'a [super::RendererReplacement],
    pub(crate) citations: &'a [super::citation::Replacement],
}

#[cfg(test)]
pub(crate) fn render(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
) -> Result<String, RenderError> {
    render_with_links(input, renderer, assets, None, None)
}

pub(crate) fn render_with_links(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    table_scroll_label: Option<&str>,
) -> Result<String, RenderError> {
    render_with_links_anchors(
        input,
        renderer,
        assets,
        pages,
        None,
        table_scroll_label,
        &[],
    )
}

pub(crate) fn render_with_links_anchors(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: Option<&[String]>,
    table_scroll_label: Option<&str>,
    replacements: &[super::RendererReplacement],
) -> Result<String, RenderError> {
    let events = transformed_events_with_links_anchors(
        input,
        renderer,
        assets,
        pages,
        anchors,
        table_scroll_label,
        replacements,
    )?;
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter().map(|(event, _)| event));
    Ok(output)
}

#[allow(
    clippy::too_many_arguments,
    reason = "render dependencies remain explicit at this boundary"
)]
pub(crate) fn render_with_citations(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: Option<&[String]>,
    table_scroll_label: Option<&str>,
    replacements: &[super::RendererReplacement],
    citations: &[super::citation::Replacement],
) -> Result<String, RenderError> {
    let events = transformed_events_with_citations(
        input,
        renderer,
        assets,
        pages,
        anchors,
        table_scroll_label,
        replacements,
        citations,
    )?;
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter().map(|(event, _)| event));
    Ok(output)
}

/// Renders one planned document while replacing each component body before
/// final HTML serialization. The caller owns component lookup and presentation;
/// this module keeps the body's Markdown events in the shared document stream.
pub(crate) fn render_with_components<F>(
    input: &str,
    context: ComponentRenderContext<'_>,
    components: &[super::component::ComponentCall],
    render_component: F,
) -> Result<String, super::component::ComponentRenderError>
where
    F: FnMut(
        &super::component::ComponentCall,
        &str,
    ) -> Result<String, super::component::ComponentRenderFailure>,
{
    let events = transformed_events_with_citations(
        input,
        context.renderer,
        context.assets,
        context.pages,
        context.anchors,
        context.table_scroll_label,
        context.replacements,
        context.citations,
    )
    .map_err(super::component::ComponentRenderError::Markdown)?;
    let events = replace_component_bodies(events, components, render_component)?;
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter().map(|(event, _)| event));
    Ok(output)
}

pub(super) fn replace_component_bodies<F>(
    events: Vec<(Event<'static>, std::ops::Range<usize>)>,
    components: &[super::component::ComponentCall],
    mut render_component: F,
) -> Result<Vec<(Event<'static>, std::ops::Range<usize>)>, super::component::ComponentRenderError>
where
    F: FnMut(
        &super::component::ComponentCall,
        &str,
    ) -> Result<String, super::component::ComponentRenderFailure>,
{
    let mut remaining = events;
    for component in components.iter().rev() {
        let mut before = Vec::new();
        let mut body = Vec::new();
        let mut after = Vec::new();
        let mut found = false;
        for (event, span) in remaining {
            let body_end = component.body_span.end.saturating_add(1);
            if span.start >= component.body_span.start && span.end <= body_end {
                if !after.is_empty() {
                    return Err(super::component::ComponentRenderError::Markdown(
                        component_error(
                            component,
                            "component body is not one contiguous Markdown range",
                        ),
                    ));
                }
                found = true;
                body.push((event, span));
            } else if span.end <= component.body_span.start || span.start >= body_end {
                if found {
                    after.push((event, span));
                } else {
                    before.push((event, span));
                }
            } else {
                return Err(super::component::ComponentRenderError::Markdown(
                    component_error(
                        component,
                        "a Markdown event crosses a component body boundary",
                    ),
                ));
            }
        }
        if body.is_empty() {
            return Err(super::component::ComponentRenderError::Markdown(
                component_error(component, "component body has no rendered Markdown content"),
            ));
        }
        let mut body_html = String::new();
        html::push_html(&mut body_html, body.into_iter().map(|(event, _)| event));
        let output = render_component(component, &body_html).map_err(|failure| {
            super::component::ComponentRenderError::Component {
                call: Box::new(component.clone()),
                failure: Box::new(failure),
            }
        })?;
        before.push((Event::Html(output.into()), component.body_span.clone()));
        before.extend(after);
        remaining = before;
    }
    Ok(remaining)
}

fn component_error(
    component: &super::component::ComponentCall,
    message: impl Into<String>,
) -> RenderError {
    RenderError::Component {
        message: message.into(),
        span: component.header_span.clone(),
    }
}

/// Produces the ordinary rendered event stream before final HTML serialization.
///
/// Keeping this boundary separate lets the renderer own every source-aware
/// transformation before HTML becomes an opaque string. The component
/// feasibility tests use it to determine whether a future structural Markdown
/// construct can replace one planned event range without reparsing Markdown.
pub(super) fn transformed_events_with_links_anchors(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: Option<&[String]>,
    table_scroll_label: Option<&str>,
    replacements: &[super::RendererReplacement],
) -> Result<Vec<(Event<'static>, std::ops::Range<usize>)>, RenderError> {
    transformed_events_with_citations(
        input,
        renderer,
        assets,
        pages,
        anchors,
        table_scroll_label,
        replacements,
        &[],
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "render dependencies remain explicit at this boundary"
)]
fn transformed_events_with_citations(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: Option<&[String]>,
    table_scroll_label: Option<&str>,
    replacements: &[super::RendererReplacement],
    citations: &[super::citation::Replacement],
) -> Result<Vec<(Event<'static>, std::ops::Range<usize>)>, RenderError> {
    if let Some((label, span)) = unresolved_footnote_reference(input, renderer.config()) {
        return Err(RenderError::Footnote { label, span });
    }
    let mut anchors = anchors.unwrap_or_default().iter();
    let task_lists_enabled = renderer
        .config()
        .extensions()
        .contains(&crate::config::MarkdownExtension::TaskLists);
    let task_labels = if task_lists_enabled {
        task_labels(input, renderer.config())
    } else {
        Vec::new()
    };
    let mut task_labels = task_labels.iter();
    let mut ordinary_footnotes = OrdinaryFootnotes::default();
    let links = TransformLinks {
        math: renderer.config().math(),
        assets,
        pages,
    };
    let events = Parser::new_ext(input, options(renderer.config()))
        .into_offset_iter()
        .collect();
    let events = picture::transform(input, events, assets)?;
    let events = apply_renderer_replacements(events, replacements);
    let events = super::citation::replace_events(events, citations)?;
    let mut footnote_definition_depth = 0usize;
    let mut transformed_events = Vec::with_capacity(events.len());
    for (event, span) in events {
        let transformed = transform_event(
            (event.clone(), span.clone()),
            links,
            &mut anchors,
            &mut task_labels,
            footnote_definition_depth == 0,
            Some(&mut ordinary_footnotes),
        )?;
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => footnote_definition_depth += 1,
            Event::End(TagEnd::FootnoteDefinition) => footnote_definition_depth -= 1,
            _ => {}
        }
        transformed_events.push((transformed, span));
    }
    let events = highlight_code_blocks(transformed_events, renderer.highlighter.as_deref())?;
    let events = match table_scroll_label {
        Some(label) => wrap_tables(events, label),
        None => events,
    };
    Ok(events)
}
