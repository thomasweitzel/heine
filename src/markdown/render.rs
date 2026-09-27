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
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter().map(|(event, _)| event));
    Ok(output)
}
