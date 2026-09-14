//! Deterministic, source-level Markdown facts.
//!
//! Rendering is intentionally not involved here. A fact must remain stable if
//! a template changes where, or how often, it renders a declared input.

use std::num::NonZeroU64;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::MarkdownConfig;

use super::{options, picture, search_text::raw_html_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct MarkdownFacts {
    word_count: u64,
    reading_time: Option<u64>,
}

pub(crate) fn facts(
    input: &str,
    config: &MarkdownConfig,
    words_per_minute: Option<NonZeroU64>,
) -> MarkdownFacts {
    let mut text = CountedText::default();
    let mut code_block_depth = 0usize;
    let mut picture_paragraph = None;

    for (event, span) in Parser::new_ext(input, options(config)).into_offset_iter() {
        if let Some(picture_span) = picture_paragraph.take() {
            if matches!(event, Event::End(TagEnd::Paragraph)) {
                if let Some(picture_text) = picture::search_text(&input[picture_span]) {
                    text.push(&picture_text);
                }
                text.boundary();
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
        if code_block_depth > 0 {
            if matches!(event, Event::End(TagEnd::CodeBlock)) {
                code_block_depth -= 1;
            }
            continue;
        }
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block_depth += 1,
            Event::Text(value) | Event::Code(value) => text.push(&value),
            Event::Html(value) | Event::InlineHtml(value) => text.push(&raw_html_text(&value)),
            Event::SoftBreak | Event::HardBreak => text.boundary(),
            Event::End(end) if is_block_end(end) => text.boundary(),
            Event::InlineMath(_) | Event::DisplayMath(_) => {}
            _ => {}
        }
    }

    let word_count = text.value.unicode_words().count() as u64;
    let reading_time = words_per_minute.map(|rate| word_count.div_ceil(rate.get()));
    MarkdownFacts {
        word_count,
        reading_time,
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

fn is_block_end(end: TagEnd) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_readable_markdown_but_not_code_or_math() {
        let source = "# Heading\n\nProse with [a link](https://example.test), `inline code`, and prose.\n\n```rust\nlet excluded = true;\n```\n";
        let counted = facts(source, &MarkdownConfig::default(), NonZeroU64::new(5));
        assert_eq!(counted.word_count, 9);
        assert_eq!(counted.reading_time, Some(2));
    }

    #[test]
    fn uses_unicode_word_boundaries_and_zero_stays_zero_minutes() {
        let counted = facts(
            "Überblick, العربية, and 42.",
            &MarkdownConfig::default(),
            None,
        );
        assert_eq!(counted.word_count, 4);
        assert_eq!(counted.reading_time, None);

        let empty = facts(
            "```text\nonly code\n```",
            &MarkdownConfig::default(),
            NonZeroU64::new(1),
        );
        assert_eq!(empty.word_count, 0);
        assert_eq!(empty.reading_time, Some(0));
    }
}
