//! Deterministic, source-level Markdown facts.
//!
//! Rendering is intentionally not involved here. A fact must remain stable if
//! a template changes where, or how often, it renders a declared input.

use std::num::NonZeroU64;

#[cfg(test)]
thread_local! {
    static BUILD_PROBE: std::cell::RefCell<Option<BuildProbe>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct BuildProbe {
    path: std::path::PathBuf,
    pub(crate) walks: usize,
}

#[cfg(test)]
pub(crate) fn begin_build_probe(path: &std::path::Path) {
    BUILD_PROBE.with(|probe| {
        *probe.borrow_mut() = Some(BuildProbe {
            path: path.to_owned(),
            walks: 0,
        })
    });
}

#[cfg(test)]
pub(crate) fn take_build_probe() -> BuildProbe {
    BUILD_PROBE.with(|probe| probe.borrow_mut().take().expect("build probe was started"))
}

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    asset::AssetLinks,
    config::{MarkdownConfig, MarkdownExtension},
    site::PageLinks,
};

use super::{
    RenderError, Renderer, SearchTextSection, TransformLinks, UnresolvedFootnoteReferenceScanner,
    options, picture, renderer_request, scan_unresolved_footnote_event,
    search_text::{append_boundary, append_text, raw_html_text},
    validate_event,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct MarkdownFacts {
    word_count: u64,
    reading_time: Option<u64>,
}

/// Source-derived values used by independent features that read declared
/// Markdown.
/// One parser walk keeps word-count and search contracts independent of
/// template rendering without paying for the same structural traversal twice.
#[derive(Debug)]
pub(crate) struct MarkdownAnalysis {
    pub(crate) facts: MarkdownFacts,
    pub(crate) search_sections: Vec<SearchTextSection>,
    pub(crate) renderer_requests: Vec<renderer_request::ResolvedRequest>,
}

#[cfg(test)]
pub(crate) fn analyze(
    input: &str,
    config: &MarkdownConfig,
    words_per_minute: Option<NonZeroU64>,
    fragments: Option<&[String]>,
    collect_search_sections: bool,
) -> MarkdownAnalysis {
    analyze_inner(
        input,
        config,
        words_per_minute,
        (fragments, collect_search_sections),
        None,
        None,
    )
    .expect("analysis without checked relationships cannot fail")
}

#[cfg(test)]
pub(crate) fn observe_citations(
    input: &str,
    config: &MarkdownConfig,
    citations: &mut super::citation::Collector,
) -> Result<MarkdownAnalysis, RenderError> {
    analyze_inner(input, config, None, (None, false), None, Some(citations))
}

/// Derives source facts and validates checked Markdown transformations in one
/// parser walk. Rendering remains separate because template output has its own
/// lifetime and may render an input more than once.
pub(super) struct ValidationInputs<'a> {
    pub(super) renderer: &'a Renderer,
    pub(super) assets: &'a AssetLinks,
    pub(super) pages: &'a PageLinks,
    pub(super) renderer_sources: &'a std::collections::BTreeMap<String, crate::renderer::Source>,
    pub(super) citations: Option<&'a mut super::citation::Collector>,
}

pub(super) fn analyze_and_validate(
    input: &str,
    validation: ValidationInputs<'_>,
    words_per_minute: Option<NonZeroU64>,
    fragments: Option<&[String]>,
    collect_search_sections: bool,
    #[cfg(test)] display_path: &std::path::Path,
) -> Result<MarkdownAnalysis, RenderError> {
    #[cfg(test)]
    BUILD_PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        if let Some(probe) = probe.as_mut().filter(|probe| probe.path == display_path) {
            probe.walks += 1;
        }
    });
    analyze_inner(
        input,
        validation.renderer.config(),
        words_per_minute,
        (fragments, collect_search_sections),
        Some(Validation {
            renderer: validation.renderer,
            renderer_sources: validation.renderer_sources,
            links: TransformLinks {
                math: validation.renderer.config().math(),
                assets: Some(validation.assets),
                pages: Some(validation.pages),
            },
        }),
        validation.citations,
    )
}

#[derive(Clone, Copy)]
struct Validation<'a> {
    renderer: &'a Renderer,
    renderer_sources: &'a std::collections::BTreeMap<String, crate::renderer::Source>,
    links: TransformLinks<'a>,
}

fn analyze_inner(
    input: &str,
    config: &MarkdownConfig,
    words_per_minute: Option<NonZeroU64>,
    search: (Option<&[String]>, bool),
    validation: Option<Validation<'_>>,
    mut citations: Option<&mut super::citation::Collector>,
) -> Result<MarkdownAnalysis, RenderError> {
    let (fragments, collect_search_sections) = search;
    let renderer_requests = renderer_request::parse(input)?;
    let resolved_renderer_requests = validation
        .map(|validation| {
            renderer_request::validate(
                &renderer_requests,
                validation.renderer.renderer_profiles(),
                validation.renderer_sources,
            )
        })
        .transpose()?
        .unwrap_or_default();
    let mut text = CountedText::default();
    let mut sections = Vec::new();
    let mut section = SectionBuilder::default();
    let mut fragments = fragments.unwrap_or_default().iter();
    let mut code_block_depth = 0usize;
    let mut footnote_definition_depth = 0usize;
    let mut unresolved_footnotes = validation
        .filter(|_| config.extensions().contains(&MarkdownExtension::Footnotes))
        .map(|_| UnresolvedFootnoteReferenceScanner::default());
    let mut events = Parser::new_ext(input, options(config)).into_offset_iter();
    let mut request_index = 0;

    while let Some((event, span)) = events.next() {
        if let Some(collector) = &mut citations {
            while request_index < renderer_requests.len()
                && renderer_requests[request_index].span().end <= span.start
            {
                request_index += 1;
            }
            let inside_request = renderer_requests.get(request_index).is_some_and(|request| {
                let request = request.span();
                span.start >= request.start && span.end <= request.end
            });
            if inside_request {
                collector.clear_pending();
            } else {
                collector.observe(input, &event, &span);
            }
        }
        if matches!(event, Event::Start(Tag::Paragraph))
            && picture::starts_directive(&input[span.clone()])
        {
            consume_picture_paragraph(&mut events);
            let picture_text = match validation {
                Some(validation) => picture::validate_and_search_text(
                    &input[span.clone()],
                    span.start,
                    validation.links.assets,
                )
                .map_err(RenderError::Picture)?,
                None => picture::source_text(&input[span.clone()]),
            };
            if let Some(picture_text) = picture_text {
                text.push(&picture_text);
                if collect_search_sections {
                    append_text(&mut section.text, &picture_text);
                }
            }
            text.boundary();
            if collect_search_sections {
                append_boundary(&mut section.text);
            }
            if let Some(scanner) = &mut unresolved_footnotes {
                scanner.clear();
            }
            continue;
        }
        if let Some(validation) = validation {
            validate_event(&event, span.clone(), validation.renderer, validation.links)?;
        }
        if starts_source_text_structure(&event) {
            text.boundary();
            if collect_search_sections {
                append_boundary(&mut section.text);
            }
        }
        let raw_html = match &event {
            Event::Html(value) | Event::InlineHtml(value) => Some(raw_html_text(value)),
            _ => None,
        };
        if code_block_depth > 0 {
            if matches!(event, Event::End(TagEnd::CodeBlock)) {
                code_block_depth -= 1;
            }
        } else {
            match &event {
                Event::Start(Tag::CodeBlock(_)) => code_block_depth += 1,
                Event::Text(value) | Event::Code(value) => text.push(value),
                Event::Html(_) | Event::InlineHtml(_) => {
                    if let Some(raw_html) = raw_html.as_deref() {
                        text.push(raw_html);
                    }
                }
                Event::SoftBreak | Event::HardBreak => text.boundary(),
                Event::End(end) if ends_source_text_structure(*end) => text.boundary(),
                Event::InlineMath(_) | Event::DisplayMath(_) => {}
                _ => {}
            }
        }

        if !collect_search_sections {
            scan_unresolved_footnotes(
                &event,
                &span,
                input,
                &mut unresolved_footnotes,
                code_block_depth > 0,
            )?;
            continue;
        }
        match &event {
            Event::Start(Tag::FootnoteDefinition(_)) => footnote_definition_depth += 1,
            Event::End(TagEnd::FootnoteDefinition) => {
                footnote_definition_depth -= 1;
                append_boundary(&mut section.text);
            }
            Event::Start(Tag::Heading { .. }) if footnote_definition_depth == 0 => {
                section.finish_into(&mut sections);
                section.fragment = fragments.next().cloned();
            }
            Event::Text(value)
            | Event::Code(value)
            | Event::InlineMath(value)
            | Event::DisplayMath(value) => section.text.push_str(value),
            Event::SoftBreak | Event::HardBreak => section.text.push('\n'),
            Event::Html(_) | Event::InlineHtml(_) => {
                if let Some(raw_html) = raw_html.as_deref() {
                    append_text(&mut section.text, raw_html);
                }
            }
            Event::End(end) if ends_source_text_structure(*end) => {
                append_boundary(&mut section.text);
            }
            _ => {}
        }
        scan_unresolved_footnotes(
            &event,
            &span,
            input,
            &mut unresolved_footnotes,
            code_block_depth > 0,
        )?;
    }

    if let Some(collector) = &mut citations {
        collector.finish();
    }
    if collect_search_sections {
        section.finish_into(&mut sections);
    }
    let word_count = text.value.unicode_words().count() as u64;
    let reading_time = words_per_minute.map(|rate| word_count.div_ceil(rate.get()));
    Ok(MarkdownAnalysis {
        facts: MarkdownFacts {
            word_count,
            reading_time,
        },
        search_sections: sections,
        renderer_requests: resolved_renderer_requests,
    })
}

fn consume_picture_paragraph<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,
) {
    for (event, _) in events {
        if matches!(event, Event::End(TagEnd::Paragraph)) {
            break;
        }
    }
}

fn scan_unresolved_footnotes(
    event: &Event<'_>,
    span: &std::ops::Range<usize>,
    input: &str,
    scanner: &mut Option<UnresolvedFootnoteReferenceScanner>,
    in_code_block: bool,
) -> Result<(), RenderError> {
    let Some(scanner) = scanner else {
        return Ok(());
    };
    if let Some((label, span)) =
        scan_unresolved_footnote_event(scanner, event, span, input, in_code_block)
    {
        return Err(RenderError::Footnote { label, span });
    }
    Ok(())
}

#[derive(Default)]
struct CountedText {
    value: String,
}

impl CountedText {
    fn push(&mut self, value: &str) {
        self.value.push_str(value);
    }

    fn boundary(&mut self) {
        if !self.value.ends_with(char::is_whitespace) && !self.value.is_empty() {
            self.value.push(' ');
        }
    }
}

/// Markdown structure separates authored text even when a tight list omits
/// paragraph events around its items. Both source-derived text consumers use
/// this one policy so search text and word counts cannot disagree about words.
fn starts_source_text_structure(event: &Event<'_>) -> bool {
    matches!(event, Event::Rule)
        || matches!(event, Event::Start(tag) if is_source_text_structure_tag(tag))
}

fn is_source_text_structure_tag(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::Image { .. }
    )
}

fn ends_source_text_structure(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::Image
    )
}

#[derive(Default)]
struct SectionBuilder {
    fragment: Option<String>,
    text: String,
}

impl SectionBuilder {
    fn finish_into(&mut self, sections: &mut Vec<SearchTextSection>) {
        let text = std::mem::take(&mut self.text).trim().to_owned();
        let fragment = self.fragment.take();
        if !text.is_empty() {
            sections.push(SearchTextSection { fragment, text });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_citation_build_uses_one_shared_analysis_walk() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let root = dir.path();
        for path in ["content/en", "templates", "locales/en", "citations"] {
            std::fs::create_dir_all(root.join(path)).expect("fixture directory");
        }
        std::fs::write(root.join("heine.toml"), "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n[markdown]\nextensions=['footnotes']\n[markdown.margin_footnotes]\nmarker_message='footnote-marker'\n").unwrap();
        std::fs::write(
            root.join("locales/en/site.ftl"),
            "footnote-marker = Footnote { $number }\n",
        )
        .unwrap();
        std::fs::write(root.join("content/en/index.page"), "template='page.tera'\ncontent.main='main.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n[toc]\ncontent='main'\n[footnotes]\ncontent='main'\n").unwrap();
        std::fs::write(
            root.join("content/en/main.md"),
            "## Heading\n\nText [@work].[^n]\n\n[^n]: A note.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("citations/works.bib"),
            "@book{work,title={Example}}\n",
        )
        .unwrap();
        std::fs::write(root.join("citations/style.csl"), r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#).unwrap();
        std::fs::write(
            root.join("templates/page.tera"),
            "{{ markdown(content='main') }}{{ bibliography() }}",
        )
        .unwrap();
        begin_build_probe(std::path::Path::new("content/en/main.md"));
        let result = crate::build_at(root, crate::site::BuildMode::Full);
        let probe = take_build_probe();
        result.expect("citation fixture builds");
        assert_eq!(probe.walks, 1);
        let output = std::fs::read_to_string(root.join("public/index.html")).unwrap();
        assert!(
            output.contains("href=\"#__heine-bibliography\""),
            "{output}"
        );
    }

    #[test]
    fn counts_readable_markdown_but_not_code_or_math() {
        let source = "# Heading\n\nProse with [a link](https://example.test), `inline code`, and prose.\n\n```rust\nlet excluded = true;\n```\n";
        let counted = analyze(
            source,
            &MarkdownConfig::default(),
            NonZeroU64::new(5),
            None,
            false,
        )
        .facts;
        assert_eq!(counted.word_count, 9);
        assert_eq!(counted.reading_time, Some(2));
    }

    #[test]
    fn uses_unicode_word_boundaries_and_zero_stays_zero_minutes() {
        let counted = analyze(
            "Überblick, العربية, and 42.",
            &MarkdownConfig::default(),
            None,
            None,
            false,
        )
        .facts;
        assert_eq!(counted.word_count, 4);
        assert_eq!(counted.reading_time, None);

        let empty = analyze(
            "```text\nonly code\n```",
            &MarkdownConfig::default(),
            NonZeroU64::new(1),
            None,
            false,
        )
        .facts;
        assert_eq!(empty.word_count, 0);
        assert_eq!(empty.reading_time, Some(0));
    }

    #[test]
    fn analysis_combines_facts_with_fragment_aware_search_sections() {
        let fragments = vec!["overview".into()];
        let analysis = analyze(
            "Before.\n\n# Overview\n\nVisible `code`.\n\n```text\nexcluded from facts\n```\n",
            &MarkdownConfig::default(),
            NonZeroU64::new(3),
            Some(&fragments),
            true,
        );

        assert_eq!(analysis.facts.word_count, 4);
        assert_eq!(analysis.facts.reading_time, Some(2));
        assert_eq!(analysis.search_sections.len(), 2);
        assert_eq!(
            analysis.search_sections[1].fragment.as_deref(),
            Some("overview")
        );
        assert!(
            analysis.search_sections[1]
                .text
                .contains("excluded from facts")
        );
    }

    #[test]
    fn source_text_boundaries_separate_nested_blocks_and_image_alternatives() {
        let analysis = analyze(
            "* Development tools\n  * JetBrains IDEs\n* Parent\n  > Quoted child\n* Example\n  ```text\n  code child\n  ```\n\ntools![JetBrains](logo.svg)later\n",
            &MarkdownConfig::default(),
            NonZeroU64::new(20),
            None,
            true,
        );
        let text = analysis
            .search_sections
            .iter()
            .map(|section| section.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        assert_eq!(
            text,
            "Development tools\n\nJetBrains IDEs\n\nParent\n\nQuoted child\n\nExample\n\ncode child\n\ntools\n\nJetBrains\n\nlater"
        );
        assert_eq!(analysis.facts.word_count, 11);
        assert_eq!(analysis.facts.reading_time, Some(1));
    }

    #[test]
    fn source_text_boundaries_preserve_inline_authored_adjacency() {
        let analysis = analyze(
            "Jet[Brains](https://example.test) and <span>Code</span>base.",
            &MarkdownConfig::default(),
            None,
            None,
            true,
        );

        assert_eq!(
            analysis.search_sections,
            vec![SearchTextSection {
                fragment: None,
                text: "JetBrains and Codebase.".into(),
            }]
        );
        assert_eq!(analysis.facts.word_count, 3);
    }

    #[test]
    fn facts_only_analysis_does_not_retain_search_sections() {
        let analysis = analyze(
            "# Heading\n\nText.",
            &MarkdownConfig::default(),
            None,
            None,
            false,
        );

        assert_eq!(analysis.facts.word_count, 2);
        assert!(analysis.search_sections.is_empty());
    }
}
