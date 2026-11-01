//! Template built-ins that render authored content.

use super::{arguments, runtime::TemplateRuntime};

pub(super) fn register(
    tera: &mut tera::Tera,
    runtime: &TemplateRuntime,
    markdown_renderer: crate::markdown::Renderer,
) {
    let highlighting_runtime = runtime.clone();
    tera.register_function(
        "highlight_css",
        move |kwargs: tera::Kwargs, _: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("highlight_css", &kwargs, &[])?;
            highlighting_runtime.highlight_css().map(Into::into).ok_or_else(|| {
                tera::Error::message(
                    "syntax highlighting is not enabled; configure [markdown.highlighting] in heine.toml",
                )
            })
        },
    );

    let bibliography_runtime = runtime.clone();
    tera.register_function(
        "bibliography",
        move |kwargs: tera::Kwargs, _: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("bibliography", &kwargs, &[])?;
            bibliography_runtime
                .bibliography()
                .map(|html| tera::Value::safe_string(&html))
        },
    );

    let markdown_runtime = runtime.clone();
    let markdown_filter_renderer = markdown_renderer.clone();
    tera.register_filter(
        "markdown",
        move |input: &str,
              kwargs: tera::Kwargs,
              state: &tera::State|
              -> tera::TeraResult<tera::Value> {
            arguments::only("markdown", &kwargs, &[])?;
            if markdown_runtime.is_selected_source(state, input)? {
                return Err(tera::Error::message(
                    "this page's selected source-aware Markdown content must use markdown(content=\"…\") so Heine can apply its checked rendering plan",
                ));
            }
            let locale = markdown_runtime.locale(state)?;
            let asset_links = markdown_runtime.assets().with_locale(locale.clone());
            let page_links = markdown_runtime.pages().with_locale(locale);
            let table_scroll_label = markdown_runtime.table_scroll_label(state)?;
            crate::markdown::render_with_links(
                input,
                &markdown_filter_renderer,
                Some(&asset_links),
                Some(&page_links),
                table_scroll_label.as_deref(),
            )
            .map(|output| tera::Value::safe_string(&output))
            .map_err(|error| tera::Error::message(error.to_string()))
        },
    );

    let markdown_content_runtime = runtime.clone();
    tera.register_function(
        "markdown",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("markdown", &kwargs, &["content"])?;
            let name = kwargs.must_get::<&str>("content")?;
            let locale = markdown_content_runtime.locale(state)?;
            let asset_links = markdown_content_runtime
                .assets()
                .with_locale(locale.clone());
            let page_links = markdown_content_runtime.pages().with_locale(locale);
            let content = markdown_content_runtime.markdown_content(state, name)?;
            if let Some(prepared) = markdown_content_runtime.prepared_markdown(name)? {
                if content.toc.is_some()
                    || content.footnotes.is_some()
                    || content.has_components
                    || content.citations.is_some()
                {
                    markdown_content_runtime.record_source_aware_render(name)?;
                }
                return Ok(tera::Value::safe_string(&prepared));
            }
            let table_scroll_label = markdown_content_runtime.table_scroll_label(state)?;
            if content.toc.is_some()
                || content.footnotes.is_some()
                || content.has_components
                || content.citations.is_some()
            {
                markdown_content_runtime.record_source_aware_render(name)?;
            }
            let rendered = if let Some(footnotes) = content.footnotes {
                let labels =
                    markdown_content_runtime.margin_footnote_labels(state, footnotes.plan())?;
                crate::markdown::margin_footnotes::render(
                    content.source,
                    crate::markdown::margin_footnotes::RenderContext {
                        renderer: &markdown_renderer,
                        assets: Some(&asset_links),
                        pages: Some(&page_links),
                        anchors: content.toc.map(crate::markdown::TocPlan::anchors),
                        table_scroll_label: table_scroll_label.as_deref(),
                        replacements: content.replacements,
                        citations: content.citations.map_or(&[], |value| &value.replacements),
                    },
                    footnotes.plan(),
                    &labels,
                )
            } else {
                crate::markdown::render::render_with_citations(
                    content.source,
                    &markdown_renderer,
                    Some(&asset_links),
                    Some(&page_links),
                    content.toc.map(crate::markdown::TocPlan::anchors),
                    table_scroll_label.as_deref(),
                    content.replacements,
                    content.citations.map_or(&[], |value| &value.replacements),
                )
            };
            rendered
                .map(|output| tera::Value::safe_string(&output))
                .map_err(|error| tera::Error::message(error.to_string()))
        },
    );
}
