//! Source-level Markdown text extraction for search indexes.
//!
//! Search deliberately owns a plain-text contract independent of rendered
//! HTML, including a small HTML tokenizer for trusted raw Markdown fragments.

use std::cell::RefCell;

use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};
use pulldown_cmark::{Event, Parser, TagEnd};

use crate::config::MarkdownConfig;

use super::options;

/// Extracts stable source-level search text without depending on rendered
/// templates. Search owns a plain-text contract, not presentation output.
pub(crate) fn search_text(input: &str, config: &MarkdownConfig) -> String {
    let mut output = String::new();
    for event in Parser::new_ext(input, options(config)) {
        match event {
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text) => output.push_str(&text),
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::Html(html) | Event::InlineHtml(html) => {
                let text = raw_html_text(&html);
                append_text(&mut output, &text);
            }
            Event::End(end) if markdown_block_end(end) => append_boundary(&mut output),
            _ => {}
        }
    }
    output.trim().into()
}

fn markdown_block_end(end: TagEnd) -> bool {
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

fn append_text(output: &mut String, text: &str) {
    if !text.is_empty() {
        output.push_str(text);
    }
}

fn append_boundary(output: &mut String) {
    if !output.ends_with("\n\n") {
        if output.ends_with('\n') {
            output.push('\n');
        } else if !output.is_empty() {
            output.push_str("\n\n");
        }
    }
}

pub(super) fn raw_html_text(input: &str) -> String {
    let queue = BufferQueue::default();
    queue.push_back(StrTendril::from_slice(input));
    let tokenizer = Tokenizer::new(HtmlSearchTextSink::default(), TokenizerOpts::default());
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    tokenizer.sink.into_text()
}

#[derive(Default)]
struct HtmlSearchTextSink {
    state: RefCell<HtmlSearchTextState>,
}

#[derive(Default)]
struct HtmlSearchTextState {
    text: String,
    excluded: Vec<String>,
}

impl HtmlSearchTextSink {
    fn into_text(self) -> String {
        self.state.into_inner().text
    }
}

impl TokenSink for HtmlSearchTextSink {
    type Handle = ();

    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<Self::Handle> {
        let mut state = self.state.borrow_mut();
        match token {
            Token::TagToken(tag) => {
                let name = tag.name.to_string();
                match tag.kind {
                    TagKind::StartTag => {
                        let excluded = is_excluded_html_element(&name);
                        if excluded {
                            state.excluded.push(name.clone());
                        }
                        if state.excluded.is_empty()
                            && name == "img"
                            && let Some(alt) = tag
                                .attrs
                                .iter()
                                .find(|attribute| attribute.name.local.as_ref() == "alt")
                        {
                            append_text(&mut state.text, &alt.value);
                        }
                        if state.excluded.is_empty() && is_html_block_element(&name) {
                            append_boundary(&mut state.text);
                        }
                        if tag.self_closing
                            && state.excluded.is_empty()
                            && is_html_block_element(&name)
                        {
                            append_boundary(&mut state.text);
                        }
                        if tag.self_closing && excluded {
                            state.excluded.pop();
                        }
                    }
                    TagKind::EndTag => {
                        if state
                            .excluded
                            .last()
                            .is_some_and(|excluded| excluded == &name)
                        {
                            state.excluded.pop();
                        } else if state.excluded.is_empty() && is_html_block_element(&name) {
                            append_boundary(&mut state.text);
                        }
                    }
                }
            }
            Token::CharacterTokens(text) if state.excluded.is_empty() => {
                append_text(&mut state.text, &text);
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn is_excluded_html_element(name: &str) -> bool {
    matches!(name, "script" | "style" | "template")
}

fn is_html_block_element(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "div"
            | "dl"
            | "dt"
            | "dd"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hgroup"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "th"
            | "td"
            | "ul"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_semantic_markdown_and_raw_html_text() {
        let source = "# Heading\n\nA [label](https://example.org), `code`, and ![image text](image.svg).\n\n<blockquote lang=\"ar\" dir=\"rtl\"><p>نص عربي</p></blockquote>\n\n<script>not searchable</script>\n";
        let text = search_text(source, &MarkdownConfig::default());

        assert!(text.contains("Heading"), "{text}");
        assert!(text.contains("label"), "{text}");
        assert!(text.contains("code"), "{text}");
        assert!(text.contains("image text"), "{text}");
        assert!(text.contains("نص عربي"), "{text}");
        assert!(!text.contains("not searchable"), "{text}");
        assert!(!text.contains("https://example.org"), "{text}");
    }

    #[test]
    fn raw_html_events_are_independent_and_preserve_table_text() {
        let table = raw_html_text("<td>table text</td>");
        let unclosed = raw_html_text("<script>not searchable");
        let later = raw_html_text("<p>later text</p>");

        assert!(table.contains("table text"), "{table}");
        assert!(!unclosed.contains("not searchable"), "{unclosed}");
        assert!(later.contains("later text"), "{later}");
    }
}
