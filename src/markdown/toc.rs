//! Table-of-contents planning and heading fragments.

use std::collections::HashSet;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde::Serialize;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::config::MarkdownConfig;

use super::options;

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
struct TocHeading {
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

pub(super) struct TocCollector {
    used: HashSet<String>,
    anchors: Vec<String>,
    view: Vec<TocHeading>,
    heading: Option<(u8, String)>,
    footnote_definition_depth: usize,
    min_level: u8,
    max_level: u8,
}

impl TocCollector {
    pub(super) fn new(min_level: u8, max_level: u8) -> Self {
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

    pub(super) fn push(&mut self, event: &Event<'_>) {
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

    pub(super) fn finish(self, content: String) -> TocPlan {
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

#[cfg(test)]
mod tests {
    use super::*;

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
