//! Built-in TeX validation and MathML rendering.

use std::{ops::Range, sync::OnceLock};

use pulldown_latex::{
    Parser as LatexParser, Storage,
    config::{DisplayMode, RenderConfig},
    event::{Content as LatexContent, Event as LatexEvent},
    push_mathml,
};

use super::{RenderError, push_html_attribute};

pub(super) fn render_math(
    tex: &str,
    display_mode: DisplayMode,
    kind: &'static str,
    span: Range<usize>,
) -> Result<String, RenderError> {
    let storage = Storage::new();
    // `push_mathml` deliberately renders parser failures as `<merror>` nodes.
    // Heine validates user-authored input at build time instead, so collect the
    // parser stream before handing successful events to the renderer.
    let events = LatexParser::new(tex, &storage)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| RenderError::Math {
            kind,
            tex: tex.into(),
            message: error.to_string(),
            span: span.clone(),
        })?;
    // Preserve the event type so operator spacing remains the writer's job.
    // The probe lets an upstream escaping fix take over without double escaping.
    let escaped_functions: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            LatexEvent::Content(LatexContent::Function(value))
                if value.contains(['&', '<', '>'])
                    && value.chars().count() > 1
                    && !mathml_writer_escapes_functions() =>
            {
                let mut escaped = String::with_capacity(value.len());
                push_html_attribute(value, &mut escaped);
                Some((index, escaped))
            }
            _ => None,
        })
        .collect();
    let mut escaped_functions = escaped_functions.iter().peekable();
    let events = events.into_iter().enumerate().map(|(index, event)| {
        if let Some((_, value)) = escaped_functions.next_if(|(i, _)| *i == index) {
            LatexEvent::Content(LatexContent::Function(value))
        } else {
            event
        }
    });
    let mut output = String::new();
    push_mathml(
        &mut output,
        events.map(Ok::<_, std::io::Error>),
        RenderConfig {
            display_mode,
            ..RenderConfig::default()
        },
    )
    .map_err(|error| RenderError::Math {
        kind,
        tex: tex.into(),
        message: error.to_string(),
        span,
    })?;
    // pulldown-latex writes these operator characters as raw text. Restrict
    // the repair to its single-character MathML tokens so already escaped
    // text and markup are left untouched. Negation adds a combining overlay.
    for (raw, escaped) in [
        ("><</", ">&lt;</"),
        ("><\u{338}</", ">&lt;\u{338}</"),
        (">&</", ">&amp;</"),
        (">&\u{338}</", ">&amp;\u{338}</"),
    ] {
        if output.contains(raw) {
            output = output.replace(raw, escaped);
        }
    }
    Ok(output)
}

fn mathml_writer_escapes_functions() -> bool {
    static ESCAPES: OnceLock<bool> = OnceLock::new();
    *ESCAPES.get_or_init(|| {
        let mut output = String::new();
        let event = LatexEvent::Content(LatexContent::Function("<&>"));
        push_mathml(
            &mut output,
            std::iter::once(Ok::<_, std::io::Error>(event)),
            RenderConfig::default(),
        )
        .is_ok()
            && output.contains("&lt;&amp;&gt;")
    })
}

pub(super) fn validate_math(
    tex: &str,
    kind: &'static str,
    span: Range<usize>,
) -> Result<(), RenderError> {
    let storage = Storage::new();
    LatexParser::new(tex, &storage)
        .try_for_each(|event| event.map(|_| ()))
        .map_err(|error| RenderError::Math {
            kind,
            tex: tex.into(),
            message: error.to_string(),
            span,
        })
}
