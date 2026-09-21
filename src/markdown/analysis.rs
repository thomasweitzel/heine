//! Deterministic, source-level Markdown facts.
//!
//! Rendering is intentionally not involved here. A fact must remain stable if
//! a template changes where, or how often, it renders a declared input.

use std::num::NonZeroU64;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::MarkdownConfig;

use super::{
    SearchTextSection, options, picture,
    search_text::{append_boundary, append_text, raw_html_text},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct MarkdownFacts {
    word_count: u64,
    reading_time: Option<u64>,
}

/// Source-derived values used by independent consumers of declared Markdown.
/// One parser walk keeps word-count and search contracts independent of
/// template rendering without paying for the same structural traversal twice.
#[derive(Debug)]
pub(crate) struct MarkdownAnalysis {
    pub(crate) facts: MarkdownFacts,
    pub(crate) search_sections: Vec<SearchTextSection>,
}

pub(crate) fn analyze(
    input: &str,
    config: &MarkdownConfig,
    words_per_minute: Option<NonZeroU64>,
    fragments: Option<&[String]>,
    collect_search_sections: bool,
) -> MarkdownAnalysis {
    let mut text = CountedText::default();
    let mut sections = Vec::new();
    let mut section = SectionBuilder::default();
    let mut fragments = fragments.unwrap_or_default().iter();
    let mut code_block_depth = 0usize;
    let mut picture_paragraph = None;
    let mut footnote_definition_depth = 0usize;

    for (event, span) in Parser::new_ext(input, options(config)).into_offset_iter() {
        if let Some(picture_span) = picture_paragraph.take() {
            if matches!(event, Event::End(TagEnd::Paragraph)) {
                if let Some(picture_text) = picture::search_text(&input[picture_span]) {
                    text.push(&picture_text);
                    if collect_search_sections {
                        append_text(&mut section.text, &picture_text);
                    }
                }
                text.boundary();
                if collect_search_sections {
                    append_boundary(&mut section.text);
                }
            } else {
                picture_paragraph = Some(picture_span);
            }
            continue;
        }
        if matches!(event, Event::Start(Tag::Paragraph))
            && picture::starts_directive(&input[span.clone()])
        {
            picture_paragraph = Some(span);
            continue;
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
                Event::End(end) if is_fact_block_end(*end) => text.boundary(),
                Event::InlineMath(_) | Event::DisplayMath(_) => {}
                _ => {}
            }
        }

        if !collect_search_sections {
            continue;
        }
        match event {
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
            | Event::DisplayMath(value) => section.text.push_str(&value),
            Event::SoftBreak | Event::HardBreak => section.text.push('\n'),
            Event::Html(_) | Event::InlineHtml(_) => {
                if let Some(raw_html) = raw_html.as_deref() {
                    append_text(&mut section.text, raw_html);
                }
            }
            Event::End(end) if is_search_block_end(end) => append_boundary(&mut section.text),
            _ => {}
        }
    }

    if collect_search_sections {
        section.finish_into(&mut sections);
    }
    let word_count = text.value.unicode_words().count() as u64;
    let reading_time = words_per_minute.map(|rate| word_count.div_ceil(rate.get()));
    MarkdownAnalysis {
        facts: MarkdownFacts {
            word_count,
            reading_time,
        },
        search_sections: sections,
    }
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

fn is_fact_block_end(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
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

fn is_search_block_end(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
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
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
