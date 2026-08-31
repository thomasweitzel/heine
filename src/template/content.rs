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

    let markdown_runtime = runtime.clone();
    let markdown_filter_renderer = markdown_renderer.clone();
    tera.register_filter(
        "markdown",
        move |input: &str,
              kwargs: tera::Kwargs,
              state: &tera::State|
              -> tera::TeraResult<tera::Value> {
            arguments::only("markdown", &kwargs, &[])?;
            if markdown_runtime.is_selected_toc_source(state, input)? {
                return Err(tera::Error::message(
                    "this page's selected table-of-contents source must use markdown(content=\"…\") so its headings receive matching fragment IDs",
                ));
            }
            let locale = markdown_runtime.locale(state)?;
            let asset_links = markdown_runtime.assets().with_locale(locale.clone());
            let page_links = markdown_runtime.pages().with_locale(locale);
            crate::markdown::render_with_links(
                input,
                &markdown_filter_renderer,
                Some(&asset_links),
                Some(&page_links),
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
            let (content, toc) = markdown_content_runtime.markdown_content(state, name)?;
            crate::markdown::render_with_links_anchors(
                content,
                &markdown_renderer,
                Some(&asset_links),
                Some(&page_links),
                toc.map(|toc| toc.anchors()),
            )
            .map(|output| tera::Value::safe_string(&output))
            .map_err(|error| tera::Error::message(error.to_string()))
        },
    );
}
