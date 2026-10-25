//! Shared source-text handling for Markdown analysis.
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
use serde::Serialize;

/// One source-ordered searchable region, optionally beginning at a generated
/// heading fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SearchTextSection {
    pub(crate) fragment: Option<String>,
    pub(crate) text: String,
}

pub(super) fn append_text(output: &mut String, text: &str) {
    if !text.is_empty() {
        output.push_str(text);
    }
}

pub(super) fn append_boundary(output: &mut String) {
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
                        let boundary = is_html_text_boundary_element(&name);
                        if state.excluded.is_empty() && boundary {
                            append_boundary(&mut state.text);
                        }
                        if state.excluded.is_empty()
                            && name == "img"
                            && let Some(alt) = tag
                                .attrs
                                .iter()
                                .find(|attribute| attribute.name.local.as_str() == "alt")
                        {
                            append_text(&mut state.text, &alt.value);
                        }
                        if state.excluded.is_empty()
                            && boundary
                            && (tag.self_closing || is_html_void_text_boundary_element(&name))
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
                        } else if state.excluded.is_empty() && is_html_text_boundary_element(&name)
                        {
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
            | "caption"
            | "div"
            | "dialog"
            | "details"
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
            | "legend"
            | "main"
            | "menu"
            | "nav"
            | "ol"
            | "option"
            | "optgroup"
            | "p"
            | "pre"
            | "section"
            | "search"
            | "summary"
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

fn is_html_text_boundary_element(name: &str) -> bool {
    is_html_block_element(name) || matches!(name, "br" | "img")
}

fn is_html_void_text_boundary_element(name: &str) -> bool {
    matches!(name, "br" | "hr" | "img")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MarkdownConfig;

    fn search_sections(
        input: &str,
        config: &MarkdownConfig,
        fragments: Option<&[String]>,
    ) -> Vec<SearchTextSection> {
        super::super::analysis::analyze(input, config, None, fragments, true).search_sections
    }

    #[test]
    fn keeps_semantic_markdown_and_raw_html_text() {
        let source = "# Heading\n\nA [label](https://example.org), `code`, and ![image text](image.svg).\n\n<blockquote lang=\"ar\" dir=\"rtl\"><p>نص عربي</p></blockquote>\n\n<script>not searchable</script>\n";
        let sections = search_sections(source, &MarkdownConfig::default(), None);
        let text = sections
            .iter()
            .map(|section| section.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        assert!(text.contains("Heading"), "{text}");
        assert!(text.contains("label"), "{text}");
        assert!(text.contains("code"), "{text}");
        assert!(text.contains("image text"), "{text}");
        assert!(text.contains("نص عربي"), "{text}");
        assert!(!text.contains("not searchable"), "{text}");
        assert!(!text.contains("https://example.org"), "{text}");
    }

    #[test]
    fn picture_blocks_contribute_their_alternative_text_and_caption() {
        let source = "Before.\n\n:::picture\nalt = \"Heine portrait\"\nlight = \"asset:images/light.png\"\ndark = \"asset:images/dark.png\"\ncaption = \"A portrait in two color schemes\"\n:::\n\nAfter.\n\n```markdown\n:::picture\nlight = \"literal example\"\n:::\n```\n";
        let sections = search_sections(source, &MarkdownConfig::default(), None);
        let text = sections
            .iter()
            .map(|section| section.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        assert!(text.contains("Before."), "{text}");
        assert!(text.contains("Heine portrait"), "{text}");
        assert!(text.contains("A portrait in two color schemes"), "{text}");
        assert!(text.contains("After."), "{text}");
        assert!(!text.contains("asset:images/light.png"), "{text}");
        assert!(text.contains("literal example"), "{text}");
    }

    #[test]
    fn headings_start_sections_with_their_generated_fragments() {
        let fragments = vec!["first".into(), "second".into()];
        let sections = search_sections(
            "Before the first heading.\n\n# First\n\nFirst section.\n\n## Second\n\nSecond section.\n",
            &MarkdownConfig::default(),
            Some(&fragments),
        );

        assert_eq!(
            sections,
            vec![
                SearchTextSection {
                    fragment: None,
                    text: "Before the first heading.".into(),
                },
                SearchTextSection {
                    fragment: Some("first".into()),
                    text: "First\n\nFirst section.".into(),
                },
                SearchTextSection {
                    fragment: Some("second".into()),
                    text: "Second\n\nSecond section.".into(),
                },
            ]
        );
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

    #[test]
    fn raw_html_text_separates_semantic_boundaries_but_not_inline_markup() {
        let text = raw_html_text(
            "before<br>after<img alt=\"JetBrains\">later<details>details<summary>summary</summary>body</details>end",
        );

        assert_eq!(
            text,
            "before\n\nafter\n\nJetBrains\n\nlater\n\ndetails\n\nsummary\n\nbody\n\nend"
        );
        assert_eq!(
            raw_html_text(
                "<fieldset><legend>Development tools</legend>JetBrains IDEs</fieldset><select><optgroup><option>One</option><option>Two</option></optgroup></select>",
            ),
            "Development tools\n\nJetBrains IDEs\n\nOne\n\nTwo\n\n"
        );
        assert_eq!(
            raw_html_text("<table><caption>Projects</caption><tr><td>Heine</td></tr></table>"),
            "Projects\n\nHeine\n\n"
        );
        assert_eq!(raw_html_text("Jet<span>Brains</span>"), "JetBrains");
    }
}
