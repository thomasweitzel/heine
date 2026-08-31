//! Markdown is a content transformation rather than a template concern.
//!
//! Templates may invoke the `markdown` filter, while content loading uses the
//! same renderer to validate MathML input with its original source available.

use std::{collections::HashSet, ops::Range, path::Path, sync::Arc};

use miette::{Diagnostic, NamedSource, SourceSpan};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};
use pulldown_latex::{
    Parser as LatexParser, Storage,
    config::{DisplayMode, RenderConfig},
    push_mathml,
};
use serde::Serialize;
use thiserror::Error;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::{
    asset::AssetLinks,
    config::{MarkdownConfig, MarkdownExtension, MathMode},
    site::PageLinks,
};

mod highlighting;
use highlighting::Highlighter;

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum InitializationError {
    #[error("could not initialize syntax highlighting: {source}")]
    Giallo {
        #[source]
        source: giallo::Error,
    },
    #[error("invalid syntax-highlighting configuration: {message}")]
    #[diagnostic(code(heine::markdown::highlighting_config))]
    Config {
        message: String,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("invalid highlighting configuration here")]
        span: SourceSpan,
        #[help]
        help: String,
    },
}

/// Per-build Markdown renderer. The optional Giallo registry is loaded once
/// and then shared by page-content rendering and the template filter.
#[derive(Clone)]
pub(crate) struct Renderer {
    config: MarkdownConfig,
    highlighter: Option<Arc<Highlighter>>,
}

/// The one source-backed heading model shared by the template view and
/// anchor-aware Markdown rendering.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TocPlan {
    content: String,
    #[serde(rename = "headings")]
    view: Vec<TocHeading>,
    #[serde(skip)]
    anchors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TocHeading {
    level: u8,
    depth: u8,
    text: String,
    fragment: String,
}

impl TocPlan {
    pub(crate) fn content(&self) -> &str {
        &self.content
    }
    pub(crate) fn anchors(&self) -> &[String] {
        &self.anchors
    }
}

pub(crate) fn toc_plan(
    content: String,
    input: &str,
    config: &MarkdownConfig,
    min_level: u8,
    max_level: u8,
) -> TocPlan {
    let mut used = HashSet::new();
    let mut anchors = Vec::new();
    let mut view = Vec::new();
    let mut heading: Option<(u8, String)> = None;
    for event in Parser::new_ext(input, options(config)) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((level as u8, String::new()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, value)) = &mut heading {
                    value.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, value)) = &mut heading
                    && !value.ends_with(' ')
                {
                    value.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = heading.take() {
                    let fragment = unique_fragment(&text, &mut used);
                    anchors.push(fragment.clone());
                    if (min_level..=max_level).contains(&level) {
                        view.push(TocHeading {
                            level,
                            depth: level - min_level + 1,
                            text,
                            fragment,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    TocPlan {
        content,
        view,
        anchors,
    }
}

fn unique_fragment(text: &str, used: &mut HashSet<String>) -> String {
    let base = fragment_base(text);
    let mut candidate = base.clone();
    let mut suffix = 2;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

fn fragment_base(text: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    let mut has_base = false;
    let mut can_mark = false;
    for character in text.nfc().collect::<String>().to_lowercase().chars() {
        if is_combining_mark(character) {
            if can_mark {
                output.push(character);
            } else {
                separator = !output.is_empty();
            }
        } else if character.is_alphanumeric() {
            if separator && !output.is_empty() {
                output.push('-');
            }
            output.push(character);
            has_base = true;
            can_mark = true;
            separator = false;
        } else {
            separator = !output.is_empty();
            can_mark = false;
        }
    }
    if has_base { output } else { "section".into() }
}

impl Renderer {
    pub(crate) fn new(config: MarkdownConfig) -> Result<Self, Box<InitializationError>> {
        let highlighter = config.highlighting().map(|highlighting| {
            Highlighter::new(highlighting).map(Arc::new).map_err(|source| {
                let Some(source_code) = highlighting.stylesheet_source().cloned() else {
                    return InitializationError::Giallo { source };
                };
                InitializationError::Config {
                    message: source.to_string(),
                    source_code,
                    span: match &source {
                        giallo::Error::ThemeNotFound(name) => highlighting.theme_span(name),
                        _ => highlighting.stylesheet_span(),
                    },
                    help: "choose names from Giallo's bundled themes, or correct the highlighting configuration".into(),
                }
            }).map_err(Box::new)
        }).transpose()?;
        Ok(Self {
            config,
            highlighter,
        })
    }

    pub(crate) fn config(&self) -> &MarkdownConfig {
        &self.config
    }

    pub(crate) fn stylesheet(&self) -> Option<String> {
        self.highlighter
            .as_ref()
            .map(|highlighter| highlighter.stylesheet())
    }

    pub(crate) fn stylesheet_path(&self) -> Option<&crate::output::OutputPath> {
        self.config
            .highlighting()
            .map(crate::config::HighlightingConfig::stylesheet)
    }

    pub(crate) fn stylesheet_claim(&self) -> Option<crate::output::OutputClaim> {
        self.config
            .highlighting()
            .and_then(crate::config::HighlightingConfig::output_claim)
    }
}

#[derive(Debug, Error)]
pub(crate) enum RenderError {
    #[error("could not render {kind} TeX {tex:?}: {message}")]
    Math {
        kind: &'static str,
        tex: String,
        message: String,
        span: Range<usize>,
    },
    #[error("could not resolve Markdown asset {id:?}: {message}")]
    Asset {
        id: String,
        message: String,
        span: Range<usize>,
    },
    #[error("could not resolve Markdown page {id:?}: {message}")]
    Page {
        id: String,
        message: String,
        span: Range<usize>,
    },
    #[error("could not highlight fenced code as {language:?}: {message}")]
    Highlight {
        language: String,
        message: String,
        span: Range<usize>,
    },
}

#[derive(Debug, Diagnostic, Error)]
#[error("invalid Markdown {kind} in {path}")]
#[diagnostic(code(heine::markdown::invalid))]
pub(crate) struct SourceError {
    path: Box<Path>,
    kind: &'static str,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: &'static str,
    #[help]
    help: String,
}

/// Renders Markdown according to the validated site configuration.
#[cfg(test)]
pub(crate) fn render(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
) -> Result<String, RenderError> {
    render_with_links(input, renderer, assets, None)
}

pub(crate) fn render_with_links(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
) -> Result<String, RenderError> {
    render_with_links_anchors(input, renderer, assets, pages, None)
}

pub(crate) fn render_with_links_anchors(
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: Option<&[String]>,
) -> Result<String, RenderError> {
    let mut anchors = anchors.unwrap_or_default().iter();
    let events = Parser::new_ext(input, options(renderer.config()))
        .into_offset_iter()
        .map(|(event, span)| {
            transform_event(
                (event, span.clone()),
                renderer.config().math(),
                assets,
                pages,
                &mut anchors,
            )
            .map(|event| (event, span))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let events = highlight_code_blocks(events, renderer.highlighter.as_deref())?;
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter().map(|(event, _)| event));
    Ok(output)
}

/// Validates Markdown transformations while a content-file source is still
/// available. Rendering remains template-driven, preserving the established
/// `markdown` filter contract.
pub(crate) fn validate_source_with_links(
    path: &Path,
    input: &str,
    renderer: &Renderer,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
) -> Result<(), Box<SourceError>> {
    if !requires_source_validation(
        renderer.config().math(),
        assets.is_some() || pages.is_some(),
        renderer.highlighter.is_some(),
    ) {
        return Ok(());
    }
    render_with_links(input, renderer, assets, pages).map(|_| ()).map_err(|error| {
        let (kind, label, help, span) = match &error {
            RenderError::Math { span, .. } => (
                "math",
                "invalid TeX here",
                "correct the TeX expression or set markdown.math = \"off\"",
                span.clone(),
            ),
            RenderError::Asset { span, .. } => (
                "asset reference",
                "invalid asset reference here",
                "use an existing asset ID relative to content/, without a leading '/', or '.' or '..' path segments",
                span.clone(),
            ),
            RenderError::Page { span, .. } => (
                "page reference", "invalid page reference here",
                "use page:<id> for the current locale or page:<locale>:<id> for an explicitly configured locale", span.clone(),
            ),
            RenderError::Highlight { span, .. } => (
                "code fence",
                "invalid language here",
                "use a bundled Giallo language name, omit the fence language, or set markdown.highlighting.missing_language = \"plain\"",
                span.clone(),
            ),
        };
        Box::new(SourceError {
            path: path.into(),
            kind,
            message: error.to_string(),
            source_code: crate::source_diagnostic::named_source(path, input, "Markdown"),
            span: span.into(),
            label,
            help: help.into(),
        })
    })
}

/// Markdown only needs an early pass when it can report a problem against the
/// authored source. Ordinary and client-rendered TeX are rendered later by the
/// template; MathML, asset URLs, and highlighting need source-aware errors.
fn requires_source_validation(math: MathMode, has_assets: bool, has_highlighter: bool) -> bool {
    math == MathMode::Mathml || has_assets || has_highlighter
}

fn highlight_code_blocks(
    events: Vec<(Event<'static>, Range<usize>)>,
    highlighter: Option<&Highlighter>,
) -> Result<Vec<(Event<'static>, Range<usize>)>, RenderError> {
    let Some(highlighter) = highlighter else {
        return Ok(events);
    };
    let mut output = Vec::with_capacity(events.len());
    let mut events = events.into_iter();
    while let Some((event, span)) = events.next() {
        let Event::Start(Tag::CodeBlock(kind)) = event else {
            output.push((event, span));
            continue;
        };
        let language = match kind {
            CodeBlockKind::Fenced(ref info) => info
                .split_ascii_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned(),
            CodeBlockKind::Indented => String::new(),
        };
        let mut source = String::new();
        let mut block = vec![(Event::Start(Tag::CodeBlock(kind.clone())), span.clone())];
        for (event, event_span) in events.by_ref() {
            block.push((event.clone(), event_span));
            match event {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(text) | Event::Code(text) => source.push_str(&text),
                _ => {}
            }
        }
        if language.is_empty() {
            output.extend(block);
            continue;
        }
        let rendered =
            highlighter
                .render(&language, &source)
                .map_err(|error| RenderError::Highlight {
                    language,
                    message: error.to_string(),
                    span,
                })?;
        output.push((Event::Html(rendered.into()), block[0].1.clone()));
    }
    Ok(output)
}

fn options(config: &MarkdownConfig) -> Options {
    let mut options = Options::empty();
    for extension in config.extensions() {
        options.insert(match extension {
            MarkdownExtension::Tables => Options::ENABLE_TABLES,
            MarkdownExtension::Strikethrough => Options::ENABLE_STRIKETHROUGH,
            MarkdownExtension::TaskLists => Options::ENABLE_TASKLISTS,
            MarkdownExtension::Footnotes => Options::ENABLE_FOOTNOTES,
        });
    }
    if config.math() != MathMode::Off {
        options.insert(Options::ENABLE_MATH);
    }
    options
}

fn transform_event(
    (event, span): (Event<'_>, Range<usize>),
    math: MathMode,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
    anchors: &mut std::slice::Iter<'_, String>,
) -> Result<Event<'static>, RenderError> {
    match event {
        Event::Start(Tag::Heading {
            level,
            classes,
            attrs,
            ..
        }) => Ok(Event::Start(Tag::Heading {
            level,
            id: anchors.next().map(|value| value.clone().into()),
            classes: classes
                .into_iter()
                .map(|value| value.into_static())
                .collect(),
            attrs: attrs
                .into_iter()
                .map(|(name, value)| (name.into_static(), value.map(|value| value.into_static())))
                .collect(),
        })),
        Event::InlineMath(tex) => match math {
            MathMode::Mathml => render_math(&tex, DisplayMode::Inline, "inline", span)
                .map(|mathml| Event::InlineHtml(mathml.into())),
            MathMode::Tex => Ok(Event::Text(format!("${tex}$").into())),
            MathMode::Off => Ok(Event::Text(format!("${tex}$").into())),
        },
        Event::DisplayMath(tex) => match math {
            MathMode::Mathml => render_math(&tex, DisplayMode::Block, "display", span)
                .map(|mathml| Event::Html(mathml.into())),
            MathMode::Tex => Ok(Event::Text(format!("$${tex}$$").into())),
            MathMode::Off => Ok(Event::Text(format!("$${tex}$$").into())),
        },
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => resolve_link_destination(&dest_url, span, assets, pages).map(|url| {
            Event::Start(Tag::Link {
                link_type,
                dest_url: url.into(),
                title: title.into_static(),
                id: id.into_static(),
            })
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => resolve_image_destination(&dest_url, span, assets).map(|url| {
            Event::Start(Tag::Image {
                link_type,
                dest_url: url.into(),
                title: title.into_static(),
                id: id.into_static(),
            })
        }),
        event => Ok(event.into_static()),
    }
}

fn resolve_link_destination(
    destination: &str,
    span: Range<usize>,
    assets: Option<&AssetLinks>,
    pages: Option<&PageLinks>,
) -> Result<String, RenderError> {
    if let Some(destination) = destination.strip_prefix("page:") {
        let Some(pages) = pages else {
            return Ok(format!("page:{destination}"));
        };
        let (reference, suffix) = split_link_suffix(destination);
        let (locale, id) = reference
            .split_once(':')
            .map_or((None, reference), |(locale, id)| (Some(locale), id));
        return pages
            .resolve(id, locale)
            .map(|target| format!("{}{suffix}", target.url))
            .map_err(|error| RenderError::Page {
                id: destination.into(),
                message: error.to_string(),
                span,
            });
    }
    resolve_asset_destination(destination, span, assets)
}

fn resolve_image_destination(
    destination: &str,
    span: Range<usize>,
    assets: Option<&AssetLinks>,
) -> Result<String, RenderError> {
    if let Some(id) = destination.strip_prefix("page:") {
        return Err(RenderError::Page {
            id: id.into(),
            message: "a page reference cannot be used as an image source".into(),
            span,
        });
    }
    resolve_asset_destination(destination, span, assets)
}

fn resolve_asset_destination(
    destination: &str,
    span: Range<usize>,
    assets: Option<&AssetLinks>,
) -> Result<String, RenderError> {
    let Some(id) = destination.strip_prefix("asset:") else {
        return Ok(destination.into());
    };
    let Some(assets) = assets else {
        return Ok(destination.into());
    };
    assets
        .resolve(id)
        .map(|asset| asset.url().into())
        .map_err(|error| RenderError::Asset {
            id: id.into(),
            message: error.to_string(),
            span,
        })
}

fn split_link_suffix(destination: &str) -> (&str, &str) {
    destination
        .find(['?', '#'])
        .map(|index| destination.split_at(index))
        .unwrap_or((destination, ""))
}

fn render_math(
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
    let mut output = String::new();
    push_mathml(
        &mut output,
        events.into_iter().map(Ok::<_, std::io::Error>),
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
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commonmark_rendering_is_available_without_extensions() {
        let renderer = Renderer::new(MarkdownConfig::default()).unwrap();
        let rendered = render("# heading\n\n**strong**", &renderer, None).unwrap();

        assert_eq!(
            rendered,
            "<h1>heading</h1>\n<p><strong>strong</strong></p>\n"
        );
    }

    #[test]
    fn dollar_delimited_text_uses_commonmark_when_math_is_off() {
        let renderer = Renderer::new(MarkdownConfig::default()).unwrap();
        let rendered = render("$_emphasis_$", &renderer, None).unwrap();

        assert_eq!(rendered, "<p>$<em>emphasis</em>$</p>\n");
    }

    #[test]
    fn validates_sources_only_for_source_aware_transformations() {
        assert!(!requires_source_validation(MathMode::Off, false, false));
        assert!(!requires_source_validation(MathMode::Tex, false, false));
        assert!(requires_source_validation(MathMode::Mathml, false, false));
        assert!(requires_source_validation(MathMode::Off, true, false));
        assert!(requires_source_validation(MathMode::Off, false, true));
    }

    #[test]
    fn toc_fragments_preserve_unicode_and_make_collisions_unique() {
        let plan = toc_plan(
            "main".into(),
            "## Überblick\n\n## Überblick\n\n## Introduction 2\n\n## Introduction\n\n## Introduction\n",
            &MarkdownConfig::default(),
            2,
            3,
        );

        assert_eq!(
            plan.anchors(),
            [
                "überblick",
                "überblick-2",
                "introduction-2",
                "introduction",
                "introduction-3",
            ]
        );
    }

    #[test]
    fn toc_fragments_discard_standalone_marks_and_keep_attached_marks() {
        assert_eq!(fragment_base("\u{11c3a}"), "section");
        assert_eq!(fragment_base("a\u{11c3a}"), "a\u{11c3a}");
    }
}
