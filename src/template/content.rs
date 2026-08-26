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
    tera.register_filter(
        "markdown",
        move |input: &str,
              kwargs: tera::Kwargs,
              state: &tera::State|
              -> tera::TeraResult<tera::Value> {
            arguments::only("markdown", &kwargs, &[])?;
            let locale = markdown_runtime.locale(state)?;
            let asset_links = markdown_runtime.assets().with_locale(locale.clone());
            let page_links = markdown_runtime.pages().with_locale(locale);
            crate::markdown::render_with_links(
                input,
                &markdown_renderer,
                Some(&asset_links),
                Some(&page_links),
            )
            .map(|output| tera::Value::safe_string(&output))
            .map_err(|error| tera::Error::message(error.to_string()))
        },
    );
}
