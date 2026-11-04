//! Markdown is a content transformation rather than a template concern.
//!
//! Templates may invoke the `markdown` filter, while content loading uses the
//! same renderer to validate MathML input with its original source available.

use std::{
    collections::{BTreeMap, HashSet},
    ops::Range,
    path::Path,
    sync::Arc,
};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use miette::{Diagnostic, NamedSource, SourceSpan};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use pulldown_latex::{
    Parser as LatexParser, Storage,
    config::{DisplayMode, RenderConfig},
    push_mathml,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::{
    asset::AssetLinks,
    config::{MarkdownConfig, MarkdownExtension, MathMode},
    renderer::Profiles as RendererProfiles,
    site::PageLinks,
};

#[cfg(test)]
use std::path::PathBuf;

mod analysis;
pub(crate) mod citation;
mod class_list;
pub(crate) mod component;
mod directive;
mod highlighting;
pub(crate) mod margin_footnotes;
pub(crate) mod picture;
pub(crate) mod rebase;
pub(crate) mod render;
mod renderer_request;
mod search_text;
pub(crate) use analysis::MarkdownFacts;
#[cfg(test)]
pub(crate) use analysis::observe_citations;
pub(crate) use class_list::HtmlClassList;
pub(crate) use component::ComponentPlan;
use highlighting::Highlighter;
pub(crate) use margin_footnotes::{MarginFootnoteDiagnostic, MarginFootnotePlan};
#[cfg(test)]
pub(crate) use render::render;
pub(crate) use render::render_with_links;
#[cfg(test)]
pub(crate) use render::render_with_links_anchors;
pub(crate) use renderer_request::{
    LicensingRecordOrigin, LicensingRecordSelection, ResolvedInput, ResolvedRequest,
};
pub(crate) use search_text::SearchTextSection;
use search_text::raw_html_text;

/// One checked renderer replacement held outside the Markdown source until
/// Markdown has determined the authored document structure.
#[derive(Debug, Clone)]
pub(crate) struct RendererReplacement {
    marker: String,
    placement: crate::renderer::Placement,
    html: String,
}

impl RendererReplacement {
    pub(crate) fn new(
        span: &Range<usize>,
        placement: crate::renderer::Placement,
        html: String,
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(span.start.to_le_bytes());
        digest.update(span.end.to_le_bytes());
        digest.update(html.as_bytes());
        Self {
            marker: format!(
                "⟦heine-renderer-{}⟧",
                STANDARD_NO_PAD.encode(digest.finalize())
            ),
            placement,
            html,
        }
    }

    pub(crate) fn marker(&self) -> &str {
        &self.marker
    }

    /// Keeps block replacements isolated while Markdown determines structure.
    pub(crate) fn markdown_marker(&self) -> String {
        match self.placement {
            crate::renderer::Placement::Inline => self.marker.clone(),
            crate::renderer::Placement::Block => format!("\n\n{}\n\n", self.marker),
        }
    }
}

/// Inserts checked renderer HTML after Markdown has determined paragraphs and
/// other authored structure. A standalone block replacement removes only its
/// synthetic paragraph; an inline replacement stays within its paragraph.
pub(super) fn apply_renderer_replacements(
    events: Vec<(Event<'static>, Range<usize>)>,
    replacements: &[RendererReplacement],
) -> Vec<(Event<'static>, Range<usize>)> {
    if replacements.is_empty() {
        return events;
    }
    let mut restored = Vec::with_capacity(events.len());
    let mut index = 0;
    while index < events.len() {
        if let [
            (Event::Start(Tag::Paragraph), _),
            (Event::Text(marker), marker_span),
            (Event::End(TagEnd::Paragraph), _),
            ..,
        ] = &events[index..]
            && let Some(replacement) = replacements.iter().find(|replacement| {
                replacement.placement == crate::renderer::Placement::Block
                    && replacement.marker == marker.as_ref()
            })
        {
            restored.push((
                Event::Html(replacement.html.clone().into()),
                marker_span.clone(),
            ));
            index += 3;
            continue;
        }
        let (event, span) = &events[index];
        match event {
            Event::Text(text) => {
                let mut remaining = text.as_ref();
                while let Some((offset, replacement)) = replacements
                    .iter()
                    .filter_map(|replacement| {
                        remaining
                            .find(replacement.marker())
                            .map(|offset| (offset, replacement))
                    })
                    .min_by_key(|(offset, _)| *offset)
                {
                    if offset > 0 {
                        restored.push((
                            Event::Text(remaining[..offset].to_owned().into()),
                            span.clone(),
                        ));
                    }
                    restored.push((
                        Event::InlineHtml(replacement.html.clone().into()),
                        span.clone(),
                    ));
                    remaining = &remaining[offset + replacement.marker().len()..];
                }
                if !remaining.is_empty() {
                    restored.push((Event::Text(remaining.to_owned().into()), span.clone()));
                }
            }
            event => restored.push((event.clone(), span.clone())),
        }
        index += 1;
    }
    restored
}

// This prefix cannot be produced by the heading-fragment algorithm, which
// keeps generated heading and footnote identifiers in separate namespaces.
pub(super) const GENERATED_FOOTNOTE_ID_PREFIX: &str = "__heine-footnote";

/// Finds a footnote-like source sequence which pulldown-cmark leaves as text
/// because no definition exists. The parser normally represents resolved
/// references as `Event::FootnoteReference`, so this fills the one gap where
/// an authored relationship would otherwise silently become visible prose.
#[derive(Default)]
pub(crate) struct UnresolvedFootnoteReferenceScanner {
    state: UnresolvedFootnoteReferenceState,
    escaped: bool,
}

#[derive(Default)]
enum UnresolvedFootnoteReferenceState {
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

impl UnresolvedFootnoteReferenceScanner {
    pub(crate) fn clear(&mut self) {
        self.state = UnresolvedFootnoteReferenceState::Outside;
        self.escaped = false;
    }

    pub(crate) fn escape_next(&mut self) {
        self.escaped = true;
    }

    pub(crate) fn push(&mut self, text: &str, offset: usize) -> Vec<(String, Range<usize>)> {
        let mut references = Vec::new();
        for (index, character) in text.char_indices() {
            let position = offset + index;
            if self.escaped {
                self.escaped = false;
                continue;
            }
            if character == '\\' {
                self.state = UnresolvedFootnoteReferenceState::Outside;
                self.escaped = true;
                continue;
            }
            match &mut self.state {
                UnresolvedFootnoteReferenceState::Outside if character == '[' => {
                    self.state = UnresolvedFootnoteReferenceState::OpenBracket { start: position };
                }
                UnresolvedFootnoteReferenceState::Outside => {}
                UnresolvedFootnoteReferenceState::OpenBracket { start } if character == '^' => {
                    self.state = UnresolvedFootnoteReferenceState::Label {
                        start: *start,
                        label: String::new(),
                    };
                }
                UnresolvedFootnoteReferenceState::OpenBracket { .. } if character == '[' => {}
                UnresolvedFootnoteReferenceState::OpenBracket { .. } => {
                    self.state = UnresolvedFootnoteReferenceState::Outside;
                }
                UnresolvedFootnoteReferenceState::Label { start, label } if character == ']' => {
                    if !label.is_empty() {
                        references.push((label.clone(), *start..position + character.len_utf8()));
                    }
                    self.state = UnresolvedFootnoteReferenceState::Outside;
                }
                UnresolvedFootnoteReferenceState::Label { .. }
                    if character.is_whitespace() || character == '[' =>
                {
                    self.state = UnresolvedFootnoteReferenceState::Outside;
                }
                UnresolvedFootnoteReferenceState::Label { label, .. } => label.push(character),
            }
        }
        references
    }
}

/// Advances the shared unresolved-footnote scanner for one parsed event.
///
/// Callers retain their own block context because rendering and source
/// analysis consume different event streams. The reference grammar itself
/// must remain identical in both paths.
pub(super) fn scan_unresolved_footnote_event(
    scanner: &mut UnresolvedFootnoteReferenceScanner,
    event: &Event<'_>,
    span: &Range<usize>,
    input: &str,
    in_code_block: bool,
) -> Option<(String, Range<usize>)> {
    match event {
        Event::Start(Tag::CodeBlock(_)) | Event::End(TagEnd::CodeBlock) => scanner.clear(),
        Event::Text(_) if !in_code_block => {
            if is_escaped_markdown_character(input, span.start) {
                scanner.escape_next();
            }
            return scanner
                .push(&input[span.clone()], span.start)
                .into_iter()
                .next();
        }
        _ => scanner.clear(),
    }
    None
}

pub(crate) fn is_escaped_markdown_character(input: &str, offset: usize) -> bool {
    input[..offset]
        .chars()
        .rev()
        .take_while(|character| *character == '\\')
        .count()
        % 2
        == 1
}

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
    renderer_profiles: RendererProfiles,
    allow_external_renderers: bool,
    refresh_renderers: bool,
    renderer_runtime: crate::renderer::RendererRuntime,
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

    pub(crate) fn update_heading_texts(&mut self, texts: &[String]) {
        let mut selected = self.view.iter_mut().peekable();
        for (anchor, text) in self.anchors.iter().zip(texts) {
            if selected
                .peek()
                .is_some_and(|heading| heading.fragment == *anchor)
                && let Some(heading) = selected.next()
            {
                heading.text.clone_from(text);
            }
        }
    }
}

pub(crate) fn toc_plan(
    content: String,
    input: &str,
    config: &MarkdownConfig,
    min_level: u8,
    max_level: u8,
) -> TocPlan {
    let mut collector = TocCollector::new(min_level, max_level);
    for event in Parser::new_ext(input, options(config)) {
        collector.push(&event);
    }
    collector.finish(content)
}

struct TocCollector {
    used: HashSet<String>,
    anchors: Vec<String>,
    view: Vec<TocHeading>,
    heading: Option<(u8, String)>,
    footnote_definition_depth: usize,
    min_level: u8,
    max_level: u8,
}

impl TocCollector {
    fn new(min_level: u8, max_level: u8) -> Self {
        Self {
            used: HashSet::new(),
            anchors: Vec::new(),
            view: Vec::new(),
            heading: None,
            footnote_definition_depth: 0,
            min_level,
            max_level,
        }
    }

    fn push(&mut self, event: &Event<'_>) {
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => self.footnote_definition_depth += 1,
            Event::End(TagEnd::FootnoteDefinition) => self.footnote_definition_depth -= 1,
            _ if self.footnote_definition_depth > 0 => {}
            Event::Start(Tag::Heading { level, .. }) => {
                self.heading = Some((*level as u8, String::new()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, value)) = &mut self.heading {
                    value.push_str(text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, value)) = &mut self.heading
                    && !value.ends_with(' ')
                {
                    value.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = self.heading.take() {
                    let fragment = unique_fragment(&text, &mut self.used);
                    self.anchors.push(fragment.clone());
                    if (self.min_level..=self.max_level).contains(&level) {
                        self.view.push(TocHeading {
                            level,
                            depth: level - self.min_level + 1,
                            text,
                            fragment,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn finish(self, content: String) -> TocPlan {
        TocPlan {
            content,
            view: self.view,
            anchors: self.anchors,
        }
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
    #[cfg(test)]
    pub(crate) fn new(config: MarkdownConfig) -> Result<Self, Box<InitializationError>> {
        let renderer_profiles = RendererProfiles::default();
        let renderer_runtime = crate::renderer::RendererRuntime::new(
            &renderer_profiles,
            PathBuf::from(".heine/cache/renderers/v1"),
        )
        .expect("a stream-only renderer configuration has no workspace runtime");
        Self::with_renderer_profiles(config, renderer_profiles, false, false, renderer_runtime)
    }

    pub(crate) fn with_renderer_profiles(
        config: MarkdownConfig,
        renderer_profiles: RendererProfiles,
        allow_external_renderers: bool,
        refresh_renderers: bool,
        renderer_runtime: crate::renderer::RendererRuntime,
    ) -> Result<Self, Box<InitializationError>> {
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
            renderer_profiles,
            allow_external_renderers,
            refresh_renderers,
            renderer_runtime,
        })
    }

    pub(crate) fn config(&self) -> &MarkdownConfig {
        &self.config
    }

    pub(crate) fn refresh_renderers(&self) -> bool {
        self.refresh_renderers
    }

    pub(crate) fn renderer_runtime(&self) -> &crate::renderer::RendererRuntime {
        &self.renderer_runtime
    }

    pub(crate) fn renderer_profiles(&self) -> &RendererProfiles {
        &self.renderer_profiles
    }

    pub(crate) fn allows_external_renderers(&self) -> bool {
        self.allow_external_renderers
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
        reason: PageReferenceFailure,
        span: Range<usize>,
    },
    #[error("could not highlight fenced code as {language:?}: {message}")]
    Highlight {
        language: String,
        message: String,
        span: Range<usize>,
    },
    #[error("{message}")]
    Citation { message: String, span: Range<usize> },
    #[error("footnote reference {label:?} has no definition")]
    Footnote { label: String, span: Range<usize> },
    #[error("could not render margin footnotes: {message}")]
    MarginFootnote { message: String, span: Range<usize> },
    #[error("{0}")]
    Picture(#[from] picture::Error),
    #[error("{0}")]
    RendererRequest(#[from] renderer_request::Error),
    #[error("external renderer permission is required")]
    RendererPermission { span: Range<usize> },
    #[error("renderer resource has no licensing declaration while licensing.required = true")]
    RendererLicensing { span: Range<usize> },
    #[error("a renderer resource requires its page to have rendered output")]
    RendererResourcePage { span: Range<usize> },
    #[error("renderer replacement offsets exceed the supported range")]
    RendererOffsets { span: Range<usize> },
    #[error("Markdown block component error: {message}")]
    Component { message: String, span: Range<usize> },
    #[error("Markdown block directives cannot appear inside a {container} yet")]
    DirectivePlacement {
        container: &'static str,
        span: Range<usize>,
    },
    #[error("renderer failed: {message}")]
    RendererProcess {
        message: String,
        diagnostics: Option<String>,
        staged_body_input: bool,
        span: Range<usize>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageReferenceFailure {
    Ordinary,
    ExcludedDraft,
}

#[derive(Debug, Diagnostic, Error)]
#[error("invalid {domain} {kind} in {path}")]
#[diagnostic(code(heine::markdown::invalid))]
pub(crate) struct SourceError {
    domain: &'static str,
    path: Box<Path>,
    kind: &'static str,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
    #[help]
    help: String,
}

/// Keeps parsed tables semantic while giving site CSS one scrollable owner.
///
/// Changing a table's own `display` can remove its table semantics for assistive
/// technology. Raw HTML remains authored input and deliberately bypasses this
/// Markdown-specific structure.
pub(super) fn wrap_tables(
    events: Vec<(Event<'static>, Range<usize>)>,
    table_scroll_label: &str,
) -> Vec<(Event<'static>, Range<usize>)> {
    let mut wrapped = Vec::with_capacity(events.len());

    for (event, span) in events {
        let starts_table = matches!(&event, Event::Start(Tag::Table(_)));
        let ends_table = matches!(&event, Event::End(TagEnd::Table));

        if starts_table {
            let mut wrapper = String::from(
                "<div class=\"heine-table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"",
            );
            push_html_attribute(table_scroll_label, &mut wrapper);
            wrapper.push_str("\">");
            wrapped.push((Event::Html(wrapper.into()), span.clone()));
        }
        if ends_table {
            wrapped.push((event, span.clone()));
            wrapped.push((Event::Html("</div>\n".into()), span));
        } else {
            wrapped.push((event, span));
        }
    }

    wrapped
}

/// The complete source-aware context for one declared Markdown input.
///
/// Template filters use a separate rendering API because their text has no
/// stable authored source span or page-level fact contract.
pub(crate) struct SourceAnalysisInputs<'a> {
    pub(crate) display_path: &'a Path,
    pub(crate) input: &'a str,
    pub(crate) renderer: &'a Renderer,
    pub(crate) assets: &'a AssetLinks,
    pub(crate) pages: &'a PageLinks,
    pub(crate) words_per_minute: Option<std::num::NonZeroU64>,
    pub(crate) fragments: Option<&'a [String]>,
    pub(crate) collect_search_sections: bool,
    pub(crate) renderer_sources: &'a BTreeMap<String, crate::renderer::Source>,
    pub(crate) citations: Option<&'a mut citation::Collector>,
}

/// Derives page facts and validates checked transformations while the original
/// content-file source remains available. Rendering stays template-driven, so
/// this build-time pass never creates HTML that the caller discards.
pub(crate) fn analyze_source_with_links(
    inputs: SourceAnalysisInputs<'_>,
) -> Result<analysis::MarkdownAnalysis, Box<SourceError>> {
    analysis::analyze_and_validate(
        inputs.input,
        analysis::ValidationInputs {
            renderer: inputs.renderer,
            assets: inputs.assets,
            pages: inputs.pages,
            renderer_sources: inputs.renderer_sources,
            citations: inputs.citations,
        },
        inputs.words_per_minute,
        inputs.fragments,
        inputs.collect_search_sections,
        #[cfg(test)]
        inputs.display_path,
    )
    .map_err(|error| Box::new(source_error(inputs.display_path, inputs.input, error)))
}

pub(crate) fn source_error(display_path: &Path, input: &str, error: RenderError) -> SourceError {
    let (kind, label, help, span) = match &error {
        RenderError::Math { span, .. } => (
            "math",
            "invalid TeX here".into(),
            "correct the TeX expression or set markdown.math = \"off\"".into(),
            span.clone(),
        ),
        RenderError::Asset { span, message, .. } => (
            "asset reference",
            format!("invalid asset reference here: {message}"),
            "use an existing asset ID relative to content/, without a leading '/', or '.' or '..' path segments".into(),
            span.clone(),
        ),
        RenderError::Page {
            span,
            message,
            reason: PageReferenceFailure::ExcludedDraft,
            ..
        } => (
            "page reference",
            "draft page referenced here".into(),
            format!(
                "{message}; remove this reference until the page is published, or use heine serve --drafts when the draft is intended to render in a local preview"
            ),
            span.clone(),
        ),
        RenderError::Page { span, .. } => (
            "page reference",
            "invalid page reference here".into(),
            "use page:<id> for the current locale or page:<locale>:<id> for an explicitly configured locale".into(),
            span.clone(),
        ),
        RenderError::Highlight { span, .. } => (
            "code fence",
            "invalid language here".into(),
            "use a bundled Giallo language name, omit the fence language, or set markdown.highlighting.missing_language = \"plain\"".into(),
            span.clone(),
        ),
        RenderError::MarginFootnote { span, .. } => (
            "margin footnotes",
            "margin-footnote rendering is inconsistent here".into(),
            "rebuild the site after correcting the selected Markdown source".into(),
            span.clone(),
        ),
        RenderError::Citation { message, span } => (
            "citation",
            message.clone(),
            "correct the citation or its bibliography; escape a literal @ as \\@".into(),
            span.clone(),
        ),
        RenderError::Footnote { span, .. } => (
            "footnote",
            "reference without a matching definition".into(),
            "add one [^label]: definition, or remove this reference".into(),
            span.clone(),
        ),
        RenderError::Picture(error) => (
            "picture",
            error.label().into(),
            error.help().into(),
            error.span(),
        ),
        RenderError::RendererRequest(error) => (
            "renderer request",
            error.message.clone(),
            "use a renderer name and TOML scalar options in the documented render syntax"
                .into(),
            error.span.clone(),
        ),
        RenderError::RendererPermission { span } => (
            "renderer request",
            "external renderer permission is required here".into(),
            "rerun with --allow-external-renderers after reviewing the selected renderer profile"
                .into(),
            span.clone(),
        ),
        RenderError::RendererLicensing { span } => (
            "renderer request",
            "resource request without a licensing declaration".into(),
            "set licensing_record on this request or its renderer profile".into(),
            span.clone(),
        ),
        RenderError::RendererResourcePage { span } => (
            "renderer request",
            "resource request on a page without rendered output".into(),
            "select a page template, or use embedded renderer output instead".into(),
            span.clone(),
        ),
        RenderError::RendererOffsets { span } => (
            "renderer request",
            "replacement offsets exceed the supported range".into(),
            "reduce the size of this Markdown source or its renderer replacements".into(),
            span.clone(),
        ),
        RenderError::Component { message, span } => (
            "Markdown block component",
            message.clone(),
            "use a top-level :::component[name key = value] block with a non-empty Markdown body".into(),
            span.clone(),
        ),
        RenderError::DirectivePlacement { container, span } => (
            "directive",
            format!("block directive in a {container}"),
            directive_placement_help(container),
            span.clone(),
        ),
        RenderError::RendererProcess {
            message,
            diagnostics,
            staged_body_input,
            span,
        } => (
            "renderer request",
            format!("renderer failed here: {message}"),
            renderer_process_help(diagnostics.as_deref(), *staged_body_input),
            span.clone(),
        ),
    };
    SourceError {
        domain: "Markdown",
        path: display_path.into(),
        kind,
        message: error.to_string(),
        source_code: crate::source_diagnostic::named_source_at_display_path(
            display_path,
            input,
            "Markdown",
        ),
        span: span.into(),
        label,
        help,
    }
}

pub(crate) fn citation_bibliography_error(
    path: &Path,
    source: &str,
    span: Range<usize>,
    message: String,
) -> SourceError {
    SourceError {
        domain: "citation",
        path: path.into(),
        kind: "bibliography",
        label: format!("invalid bibliography field: {message}"),
        message,
        source_code: crate::source_diagnostic::named_source_at_display_path(path, source, "BibTeX"),
        span: span.into(),
        help: "correct the bibliography field".into(),
    }
}

pub(crate) fn citation_configuration_error(
    path: &Path,
    source: &str,
    span: Range<usize>,
    message: String,
) -> SourceError {
    SourceError {
        domain: "citation",
        path: path.into(),
        kind: "configuration",
        label: format!("style selected here: {message}"),
        message,
        source_code: crate::source_diagnostic::named_source_at_display_path(path, source, "TOML"),
        span: span.into(),
        help: "choose a compatible local CSL style or adjust its selection".into(),
    }
}

fn directive_placement_help(container: &str) -> String {
    if container == "paragraph continuation" {
        "begin the directive after a paragraph break, or write it as inline code when documenting literal syntax".into()
    } else {
        "move the directive to an ordinary top-level block, or write it as inline code when documenting literal syntax".into()
    }
}

pub(crate) fn renderer_process_error(
    error: crate::renderer::RendererError,
    staged_body_input: bool,
    span: Range<usize>,
) -> RenderError {
    RenderError::RendererProcess {
        message: error.message,
        diagnostics: error.diagnostics,
        staged_body_input,
        span,
    }
}

fn renderer_process_help(diagnostics: Option<&str>, staged_body_input: bool) -> String {
    let Some(diagnostics) = diagnostics else {
        return "inspect the configured renderer profile and its diagnostic output".into();
    };
    let context = staged_body_input.then_some(
        "the renderer received the request body as an isolated staged input; paths and line numbers below refer to that input\n\n",
    );
    format!(
        "{}renderer output:\n{diagnostics}\ncorrect the renderer input or its configured invocation",
        context.unwrap_or_default(),
    )
}

/// Validates one parsed event that needs the resolved site relationship model.
pub(super) fn validate_event(
    event: &Event<'_>,
    span: Range<usize>,
    renderer: &Renderer,
    links: TransformLinks<'_>,
) -> Result<(), RenderError> {
    match event {
        Event::InlineMath(tex) if links.math == MathMode::Mathml => {
            validate_math(tex, "inline", span)?;
        }
        Event::DisplayMath(tex) if links.math == MathMode::Mathml => {
            validate_math(tex, "display", span)?;
        }
        Event::Start(Tag::Link { dest_url, .. }) => {
            resolve_link_destination(dest_url, span, links.assets, links.pages)?;
        }
        Event::Start(Tag::Image { dest_url, .. }) => {
            resolve_image_destination(dest_url, span, links.assets)?;
        }
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
            let language = info.split_ascii_whitespace().next().unwrap_or_default();
            if !language.is_empty()
                && let Some(highlighter) = renderer.highlighter.as_deref()
            {
                highlighter
                    .validate(language)
                    .map_err(|error| RenderError::Highlight {
                        language: language.into(),
                        message: error.to_string(),
                        span,
                    })?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn unresolved_footnote_reference(
    input: &str,
    config: &MarkdownConfig,
) -> Option<(String, Range<usize>)> {
    if !config.extensions().contains(&MarkdownExtension::Footnotes) {
        return None;
    }
    let mut scanner = UnresolvedFootnoteReferenceScanner::default();
    let mut in_code_block = false;
    let mut picture_paragraph_depth = 0usize;
    for (event, span) in Parser::new_ext(input, options(config)).into_offset_iter() {
        if picture_paragraph_depth > 0 {
            if matches!(event, Event::End(TagEnd::Paragraph)) {
                picture_paragraph_depth -= 1;
            }
            continue;
        }
        if matches!(event, Event::Start(Tag::Paragraph))
            && picture::starts_directive(&input[span.clone()])
        {
            picture_paragraph_depth += 1;
            scanner.clear();
            continue;
        }
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                let _ = scan_unresolved_footnote_event(
                    &mut scanner,
                    &event,
                    &span,
                    input,
                    in_code_block,
                );
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                let _ = scan_unresolved_footnote_event(
                    &mut scanner,
                    &event,
                    &span,
                    input,
                    in_code_block,
                );
                in_code_block = false;
            }
            _ => {
                if let Some(reference) = scan_unresolved_footnote_event(
                    &mut scanner,
                    &event,
                    &span,
                    input,
                    in_code_block,
                ) {
                    return Some(reference);
                }
            }
        }
    }
    None
}

const PLAIN_CODE_LANGUAGE: &str = "plain";

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
        let language = match &kind {
            CodeBlockKind::Fenced(info) => info
                .split_ascii_whitespace()
                .next()
                .unwrap_or(PLAIN_CODE_LANGUAGE),
            CodeBlockKind::Indented => PLAIN_CODE_LANGUAGE,
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
        let rendered =
            highlighter
                .render(language, &source)
                .map_err(|error| RenderError::Highlight {
                    language: language.to_owned(),
                    message: error.to_string(),
                    span,
                })?;
        output.push((Event::Html(rendered.into()), block[0].1.clone()));
    }
    Ok(output)
}

pub(super) fn options(config: &MarkdownConfig) -> Options {
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

#[derive(Clone, Copy)]
pub(super) struct TransformLinks<'a> {
    math: MathMode,
    assets: Option<&'a AssetLinks>,
    pages: Option<&'a PageLinks>,
}

pub(super) fn transform_event(
    (event, span): (Event<'_>, Range<usize>),
    links: TransformLinks<'_>,
    anchors: &mut std::slice::Iter<'_, String>,
    task_labels: &mut std::slice::Iter<'_, Option<String>>,
    anchors_enabled: bool,
    ordinary_footnotes: Option<&mut OrdinaryFootnotes>,
) -> Result<Event<'static>, RenderError> {
    match event {
        Event::Start(Tag::Heading {
            level,
            classes,
            attrs,
            ..
        }) => Ok(Event::Start(Tag::Heading {
            level,
            id: anchors_enabled
                .then(|| anchors.next().map(|value| value.clone().into()))
                .flatten(),
            classes: classes
                .into_iter()
                .map(pulldown_cmark::CowStr::into_static)
                .collect(),
            attrs: attrs
                .into_iter()
                .map(|(name, value)| {
                    (
                        name.into_static(),
                        value.map(pulldown_cmark::CowStr::into_static),
                    )
                })
                .collect(),
        })),
        Event::InlineMath(tex) => match links.math {
            MathMode::Mathml => render_math(&tex, DisplayMode::Inline, "inline", span)
                .map(|mathml| Event::InlineHtml(mathml.into())),
            MathMode::Tex => Ok(Event::Text(format!("${tex}$").into())),
            MathMode::Off => Ok(Event::Text(format!("${tex}$").into())),
        },
        Event::DisplayMath(tex) => match links.math {
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
        }) => resolve_link_destination(&dest_url, span, links.assets, links.pages).map(|url| {
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
        }) => resolve_image_destination(&dest_url, span, links.assets).map(|url| {
            Event::Start(Tag::Image {
                link_type,
                dest_url: url.into(),
                title: title.into_static(),
                id: id.into_static(),
            })
        }),
        Event::TaskListMarker(checked) => Ok(Event::Html(
            task_list_marker(
                checked,
                task_labels.next().and_then(|label| label.as_deref()),
            )
            .into(),
        )),
        Event::FootnoteReference(label) => match ordinary_footnotes {
            Some(footnotes) => Ok(Event::Html(
                ordinary_footnote_reference(footnotes.number(&label)).into(),
            )),
            None => Ok(Event::FootnoteReference(label.into_static())),
        },
        Event::Start(Tag::FootnoteDefinition(label)) => match ordinary_footnotes {
            Some(footnotes) => Ok(Event::Html(
                ordinary_footnote_definition(footnotes.number(&label)).into(),
            )),
            None => Ok(Event::Start(Tag::FootnoteDefinition(label.into_static()))),
        },
        Event::End(TagEnd::FootnoteDefinition) if ordinary_footnotes.is_some() => {
            Ok(Event::Html("</div>\n".into()))
        }
        event => Ok(event.into_static()),
    }
}

/// Keeps pulldown-cmark's encounter-order footnote numbering while reserving
/// generated definition fragments outside the heading-fragment namespace.
#[derive(Default)]
pub(super) struct OrdinaryFootnotes {
    numbers: BTreeMap<String, usize>,
}

impl OrdinaryFootnotes {
    fn number(&mut self, label: &str) -> usize {
        let next = self.numbers.len() + 1;
        *self.numbers.entry(label.into()).or_insert(next)
    }
}

fn ordinary_footnote_reference(number: usize) -> String {
    let target = ordinary_footnote_id(number);
    format!("<sup class=\"footnote-reference\"><a href=\"#{target}\">{number}</a></sup>")
}

fn ordinary_footnote_definition(number: usize) -> String {
    let id = ordinary_footnote_id(number);
    format!(
        "<div class=\"footnote-definition\" id=\"{id}\"><sup class=\"footnote-definition-label\">{number}</sup>"
    )
}

fn ordinary_footnote_id(number: usize) -> String {
    format!("{GENERATED_FOOTNOTE_ID_PREFIX}-definition-{number}")
}

#[derive(Default)]
struct ListItem {
    task: Option<(usize, usize)>,
}

/// Collects each task item's direct authored text for its native checkbox.
/// Nested list text is a separate task or list item, never part of its parent.
fn task_labels(input: &str, config: &MarkdownConfig) -> Vec<Option<String>> {
    let mut labels = Vec::new();
    let mut items = Vec::<ListItem>::new();
    let mut list_depth = 0;

    for event in Parser::new_ext(input, options(config)) {
        match event {
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::Item) => items.push(ListItem::default()),
            Event::End(TagEnd::Item) => {
                items.pop();
            }
            Event::TaskListMarker(_) => {
                let index = labels.len();
                labels.push(Some(String::new()));
                if let Some(item) = items.last_mut() {
                    item.task = Some((index, list_depth));
                }
            }
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text) => {
                append_task_label(&mut labels, &items, list_depth, &text);
            }
            Event::SoftBreak | Event::HardBreak => {
                append_task_label(&mut labels, &items, list_depth, " ");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                let text = raw_html_text(&html);
                append_task_label(&mut labels, &items, list_depth, &text);
            }
            _ => {}
        }
    }

    labels
        .into_iter()
        .map(|label| label.and_then(collapse_task_label))
        .collect()
}

fn append_task_label(
    labels: &mut [Option<String>],
    items: &[ListItem],
    list_depth: usize,
    text: &str,
) {
    let Some((index, task_depth)) = items.last().and_then(|item| item.task) else {
        return;
    };
    if task_depth == list_depth
        && let Some(label) = &mut labels[index]
    {
        label.push_str(text);
    }
}

fn collapse_task_label(label: String) -> Option<String> {
    let mut collapsed = String::with_capacity(label.len());
    for word in label.split_whitespace() {
        if !collapsed.is_empty() {
            collapsed.push(' ');
        }
        collapsed.push_str(word);
    }
    (!collapsed.is_empty()).then_some(collapsed)
}

fn task_list_marker(checked: bool, label: Option<&str>) -> String {
    let mut output = String::from("<input disabled=\"\" type=\"checkbox\"");
    if checked {
        output.push_str(" checked=\"\"");
    }
    if let Some(label) = label {
        output.push_str(" aria-label=\"");
        push_html_attribute(label, &mut output);
        output.push('"');
    }
    output.push('>');
    output
}

pub(super) fn push_html_text(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

fn push_html_attribute(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
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
                reason: if matches!(error, crate::site::PageResolveError::DraftExcluded { .. }) {
                    PageReferenceFailure::ExcludedDraft
                } else {
                    PageReferenceFailure::Ordinary
                },
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
            reason: PageReferenceFailure::Ordinary,
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

fn validate_math(tex: &str, kind: &'static str, span: Range<usize>) -> Result<(), RenderError> {
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
    fn fenced_picture_examples_remain_literal_markdown() {
        let renderer = Renderer::new(MarkdownConfig::default()).unwrap();
        let rendered = render(
            "```markdown\n:::picture\nlight = \"asset:images/light.png\"\n:::\n```\n",
            &renderer,
            None,
        )
        .unwrap();

        assert!(rendered.contains(":::picture"), "{rendered}");
        assert!(rendered.contains("asset:images/light.png"), "{rendered}");
    }

    #[test]
    fn unresolved_ordinary_footnotes_are_checked_without_rejecting_escaped_text() {
        let mut scanner = UnresolvedFootnoteReferenceScanner::default();

        assert_eq!(
            scanner.push("A missing note.[^missing]", 0),
            vec![("missing".into(), 15..25)]
        );
        scanner.clear();
        assert_eq!(scanner.push(r"A literal \[^missing].", 0), Vec::new());
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
