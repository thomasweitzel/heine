//! Recognition of citation-shaped Markdown during source analysis.

use std::{collections::BTreeMap, ops::Range};

use pulldown_cmark::{Event, LinkType, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) span: Range<usize>,
    pub(crate) keys: Vec<Range<usize>>,
    pub(crate) locators: Vec<Option<Locator>>,
    pub(crate) footnote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Locator {
    pub(crate) kind: LocatorKind,
    pub(crate) value: String,
    pub(crate) span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocatorKind {
    Page,
    Chapter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) span: Range<usize>,
    pub(crate) message: &'static str,
}

#[derive(Default)]
pub(crate) struct Collector {
    opening: Option<usize>,
    structural: bool,
    excluded: usize,
    decoded: Vec<(Range<usize>, String)>,
    footnote: Option<String>,
    heading: bool,
    collect_headings: bool,
    raw_html: Vec<String>,
    pub(crate) headings: Vec<Vec<(Range<usize>, String)>>,
    pub(crate) footnote_references: BTreeMap<String, usize>,
    pub(crate) groups: Vec<Group>,
    pub(crate) problems: Vec<Problem>,
}

impl Collector {
    pub(crate) fn with_headings(collect_headings: bool) -> Self {
        Self {
            collect_headings,
            ..Self::default()
        }
    }

    pub(crate) fn clear_pending(&mut self) {
        self.opening = None;
        self.structural = false;
        self.decoded.clear();
    }

    pub(crate) fn finish(&mut self) {
        let references = &self.footnote_references;
        self.groups.sort_unstable_by_key(|group| {
            let position = group
                .footnote
                .as_ref()
                .and_then(|note| references.get(note))
                .copied()
                .unwrap_or(group.span.start);
            (position, group.span.start)
        });
    }

    pub(crate) fn observe(&mut self, source: &str, event: &Event<'_>, span: &Range<usize>) {
        match event {
            Event::Start(Tag::Heading { .. })
                if self.collect_headings && self.footnote.is_none() =>
            {
                self.heading = true;
                self.headings.push(Vec::new());
            }
            Event::End(TagEnd::Heading(_)) => self.heading = false,
            Event::Text(text) | Event::Code(text) if self.heading => {
                if let Some(parts) = self.headings.last_mut() {
                    parts.push((span.clone(), text.to_string()));
                }
            }
            Event::SoftBreak | Event::HardBreak if self.heading => {
                if let Some(parts) = self.headings.last_mut()
                    && parts.last().is_none_or(|(_, text)| !text.ends_with(' '))
                {
                    parts.push((span.clone(), " ".to_owned()));
                }
            }
            _ => {}
        }
        if let Event::InlineHtml(html) = event {
            update_raw_html(&mut self.raw_html, html);
            if self.opening.is_some() {
                self.structural = true;
            }
            return;
        }
        if !self.raw_html.is_empty() {
            return;
        }
        match event {
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::TableCell
                | TagEnd::BlockQuote(_),
            ) => {
                self.clear_pending();
            }
            Event::Start(Tag::FootnoteDefinition(label)) => self.footnote = Some(label.to_string()),
            Event::End(TagEnd::FootnoteDefinition) => self.footnote = None,
            Event::FootnoteReference(label) => {
                self.footnote_references
                    .entry(label.to_string())
                    .or_insert(span.start);
            }
            Event::Start(Tag::Link {
                link_type: LinkType::Shortcut,
                id,
                ..
            }) if self.excluded == 0 && citation_marker(id) => {
                self.problems.push(Problem {
                    span: span.clone(),
                    message: "citation conflicts with a reference link",
                });
                self.excluded += 1;
                self.opening = None;
            }
            Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)) => {
                self.excluded += 1;
                if self.opening.is_some() {
                    self.structural = true;
                }
            }
            Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) => {
                self.excluded = self.excluded.saturating_sub(1);
            }
            Event::Text(text)
                if self.excluded == 0
                    && text.as_ref() == "["
                    && source.get(span.clone()) == Some("[") =>
            {
                if let Some(start) = self.opening
                    && citation_marker(&source[start + 1..span.start])
                {
                    self.problems.push(Problem {
                        span: start..span.end,
                        message: "citation contains unsupported Markdown",
                    });
                }
                self.opening = Some(span.start);
                self.structural = false;
                self.decoded.clear();
            }
            Event::Text(text)
                if self.opening.is_some() && source.get(span.clone()) != Some(text.as_ref()) =>
            {
                self.decoded.push((span.clone(), text.to_string()));
            }
            Event::Text(text)
                if self.excluded == 0
                    && text.as_ref() == "]"
                    && source.get(span.clone()) == Some("]") =>
            {
                if let Some(start) = self.opening.take() {
                    self.group(source, start..span.end);
                }
            }
            Event::Start(_)
            | Event::End(_)
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::SoftBreak
            | Event::HardBreak
                if self.opening.is_some() =>
            {
                self.structural = true
            }
            Event::Html(_) | Event::InlineHtml(_) if self.opening.is_some() => {
                self.structural = true
            }
            _ => {}
        }
    }

    fn group(&mut self, source: &str, span: Range<usize>) {
        let body = &source[span.start + 1..span.end - 1];
        if !body.split(';').any(citation_marker) {
            return;
        }
        if self.structural {
            self.problems.push(Problem {
                span,
                message: "citation contains unsupported Markdown",
            });
            return;
        }
        let mut keys = Vec::new();
        let mut locators = Vec::new();
        let mut offset = span.start + 1;
        let separators = body
            .match_indices(';')
            .filter_map(|(index, _)| {
                let absolute = span.start + 1 + index;
                let encoded = self
                    .decoded
                    .iter()
                    .any(|(range, _)| range.contains(&absolute));
                let escaped = body[..index]
                    .bytes()
                    .rev()
                    .take_while(|byte| *byte == b'\\')
                    .count()
                    % 2
                    != 0;
                (!encoded && !escaped).then_some(index)
            })
            .chain(std::iter::once(body.len()));
        let mut part_start = 0;
        for part_end in separators {
            let part = &body[part_start..part_end];
            let trimmed = part.trim();
            let start = offset + part.len() - part.trim_start().len();
            let Some(key_and_locator) = trimmed.strip_prefix('@') else {
                self.problems.push(Problem {
                    span,
                    message: "citation item must begin with @",
                });
                return;
            };
            let key_len = key_and_locator
                .char_indices()
                .take_while(|(_, c)| {
                    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.' | '/' | '+')
                })
                .map(|(i, c)| i + c.len_utf8())
                .last()
                .unwrap_or(0);
            let remainder = key_and_locator[key_len..].trim();
            if key_len == 0
                || (!remainder.is_empty()
                    && (!remainder.starts_with(',') || remainder[1..].trim().is_empty()))
            {
                self.problems.push(Problem {
                    span,
                    message: "invalid citation key or locator",
                });
                return;
            }
            keys.push(start + 1..start + 1 + key_len);
            if let Some(locator) = remainder.strip_prefix(',') {
                let raw_start = start + 1 + key_len + key_and_locator[key_len..].len()
                    - key_and_locator[key_len..].trim_start().len()
                    + 1;
                let left = raw_start + locator.len() - locator.trim_start().len();
                let right = left + locator.trim().len();
                let raw = &source[left..right];
                let found = [
                    ("pp.", LocatorKind::Page),
                    ("p.", LocatorKind::Page),
                    ("chap.", LocatorKind::Chapter),
                    ("chapter", LocatorKind::Chapter),
                ]
                .into_iter()
                .find_map(|(label, kind)| {
                    raw.strip_prefix(label)
                        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
                        .map(|rest| (label, kind, rest))
                });
                let Some((label, kind, rest)) = found else {
                    self.problems.push(Problem {
                        span,
                        message: "unsupported citation locator",
                    });
                    return;
                };
                let value_start = left + label.len() + rest.len() - rest.trim_start().len();
                let value_span = value_start..right;
                if value_span.is_empty() {
                    self.problems.push(Problem {
                        span: left..right,
                        message: "citation locator requires a value",
                    });
                    return;
                }
                let mut value = String::new();
                let mut position = value_start;
                for (change, text) in &self.decoded {
                    if change.start >= value_start && change.end <= right {
                        value.push_str(&source[position..change.start]);
                        value.push_str(text);
                        position = change.end;
                    }
                }
                value.push_str(&source[position..right]);
                locators.push(Some(Locator {
                    kind,
                    value,
                    span: value_span,
                }));
            } else {
                locators.push(None);
            }
            offset += part.len() + 1;
            part_start = part_end + 1;
        }
        self.groups.push(Group {
            span,
            keys,
            locators,
            footnote: self.footnote.clone(),
        });
    }
}

/// Track raw inline element boundaries without interpreting their contents as
/// Markdown citations. HTML's void elements never open a content region.
fn update_raw_html(open: &mut Vec<String>, html: &str) {
    use html5ever::{
        tendril::StrTendril,
        tokenizer::{
            BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
        },
    };
    use std::cell::RefCell;
    struct Tags<'a>(RefCell<&'a mut Vec<String>>);
    impl TokenSink for Tags<'_> {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            if let Token::TagToken(tag) = token {
                let mut open = self.0.borrow_mut();
                let name = tag.name.as_ref();
                match tag.kind {
                    TagKind::StartTag
                        if !matches!(
                            name,
                            "area"
                                | "base"
                                | "br"
                                | "col"
                                | "embed"
                                | "hr"
                                | "img"
                                | "input"
                                | "link"
                                | "meta"
                                | "param"
                                | "source"
                                | "track"
                                | "wbr"
                        ) =>
                    {
                        open.push(name.to_owned());
                    }
                    TagKind::EndTag => {
                        if let Some(index) = open.iter().rposition(|opened| opened == name) {
                            open.truncate(index);
                        }
                    }
                    _ => {}
                }
            }
            TokenSinkResult::Continue
        }
    }
    let queue = BufferQueue::default();
    queue.push_back(StrTendril::from_slice(html));
    let tokenizer = Tokenizer::new(Tags(RefCell::new(open)), TokenizerOpts::default());
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
}

fn citation_marker(text: &str) -> bool {
    text.char_indices().any(|(position, character)| {
        character == '@'
            && (position == 0
                || text[..position]
                    .chars()
                    .last()
                    .is_some_and(char::is_whitespace))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::SiteConfig, source_diagnostic::DiagnosticPaths};

    #[test]
    fn recognizes_citations_in_the_existing_analysis_walk() {
        let root = tempfile::tempdir().unwrap();
        let config_file = root.path().join("heine.toml");
        std::fs::write(&config_file, "[site]\ntime_zone = 'Europe/Berlin'\n[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[markdown]\nextensions = ['footnotes']\n").unwrap();
        let config = SiteConfig::load(&config_file, &DiagnosticPaths::new(root.path())).unwrap();
        let source = "First [@alice; @bob, p. 2]. `[@code]` \\[@literal] [link](/test)\n\n[@defined]\n\n[@defined]: https://example.org\n";
        let mut collector = Collector::default();
        super::super::analysis::observe_citations(source, config.markdown(), &mut collector)
            .unwrap();
        assert_eq!(collector.groups.len(), 1);
        let group = &collector.groups[0];
        assert_eq!(&source[group.span.clone()], "[@alice; @bob, p. 2]");
        assert_eq!(
            group
                .keys
                .iter()
                .map(|span| &source[span.clone()])
                .collect::<Vec<_>>(),
            ["alice", "bob"]
        );
        assert_eq!(collector.problems.len(), 1);
        assert_eq!(&source[collector.problems[0].span.clone()], "[@defined]");
        assert_eq!(collector.groups[0].locators[1].as_ref().unwrap().value, "2");
        for (source, groups, problems) in [
            ("[@one]", 1, 0),
            ("An incomplete [@one", 0, 0),
            ("[@one; @two]", 1, 0),
            ("[@one,]", 0, 1),
            ("[@one, p.]", 0, 1),
            ("[@one, unknown]", 0, 1),
            ("[@one, p. *2*]", 0, 1),
            ("[@one, p. `2`]", 0, 1),
            ("[@one, p. &#49;]", 1, 0),
            ("[@one, chapter 2]", 1, 0),
            ("[see @one]", 0, 1),
            ("[alice@example.org]", 0, 0),
            ("`[@one]`", 0, 0),
            ("\\[@one]", 0, 0),
            ("```text\n[@one]\n```", 0, 0),
            ("<div>\n[@one]\n</div>", 0, 0),
            ("## Heading [@one]", 1, 0),
            ("- List [@one]", 1, 0),
        ] {
            let mut collector = Collector::default();
            super::super::analysis::observe_citations(source, config.markdown(), &mut collector)
                .unwrap();
            assert_eq!(
                (collector.groups.len(), collector.problems.len()),
                (groups, problems),
                "{source}"
            );
            if source == "[@one, p. &#49;]" {
                let locator = collector.groups[0].locators[0].as_ref().unwrap();
                assert_eq!(locator.value, "1");
                assert_eq!(&source[locator.span.clone()], "&#49;");
            }
        }
        let source = ":::component[site.note]\nInside [@inside].[^n]\n:::\n\n[^n]: Note [@note].\n";
        let component = super::super::component::ComponentPlan::parse(source)
            .unwrap()
            .unwrap();
        let mut collector = Collector::default();
        super::super::analysis::observe_citations(
            component.normalized(),
            config.markdown(),
            &mut collector,
        )
        .unwrap();
        assert_eq!(
            collector
                .groups
                .iter()
                .map(|group| &source[group.keys[0].clone()])
                .collect::<Vec<_>>(),
            ["inside", "note"]
        );
        assert_eq!(collector.groups[1].footnote.as_deref(), Some("n"));
        assert_eq!(
            collector.footnote_references.get("n"),
            source.find("[^n]").as_ref()
        );

        let source = "First [@first].[^n] Between [@middle]. Again[^n].\n\n[^n]: Note [@note1] and [@note2].\n\n[^unused]: Unreferenced [@unused].\n";
        let mut collector = Collector::default();
        super::super::analysis::observe_citations(source, config.markdown(), &mut collector)
            .unwrap();
        assert_eq!(
            collector
                .groups
                .iter()
                .map(|group| &source[group.keys[0].clone()])
                .collect::<Vec<_>>(),
            ["first", "note1", "note2", "middle", "unused"]
        );
        assert_eq!(
            collector
                .groups
                .iter()
                .filter(|group| group.footnote.as_deref() == Some("n"))
                .count(),
            2
        );
    }
}

/// A formatted citation tied to its original Markdown byte range.
#[derive(Debug, Clone)]
pub(crate) struct Replacement {
    pub(crate) span: Range<usize>,
    pub(crate) html: String,
}

/// Replace citation text events without reparsing or changing their enclosing
/// Markdown structures. Citation spans are sorted by source position because
/// processor order can differ for citations inside footnote definitions.
pub(crate) fn replace_events(
    events: Vec<(Event<'static>, Range<usize>)>,
    replacements: &[Replacement],
) -> Result<Vec<(Event<'static>, Range<usize>)>, super::RenderError> {
    if replacements.is_empty() {
        return Ok(events);
    }
    let mut ordered: Vec<_> = replacements.iter().collect();
    ordered.sort_unstable_by_key(|item| item.span.start);
    let mut output = Vec::with_capacity(events.len());
    let mut index = 0;
    let mut inserted = false;
    for (event, span) in events {
        if !matches!(event, Event::Text(_)) {
            output.push((event, span));
            continue;
        }
        while index < ordered.len() && span.start >= ordered[index].span.end {
            if !inserted {
                return Err(super::RenderError::Citation {
                    message: "citation could not be placed in the Markdown event stream".into(),
                    span: ordered[index].span.clone(),
                });
            }
            index += 1;
            inserted = false;
        }
        if let Some(replacement) = ordered.get(index)
            && span.start < replacement.span.end
            && span.end > replacement.span.start
        {
            if span.start < replacement.span.start || span.end > replacement.span.end {
                return Err(super::RenderError::Citation {
                    message: "citation crosses a Markdown event boundary".into(),
                    span: replacement.span.clone(),
                });
            }
            if !inserted {
                output.push((
                    Event::InlineHtml(replacement.html.clone().into()),
                    replacement.span.clone(),
                ));
                inserted = true;
            }
            continue;
        }
        output.push((event, span));
    }
    if index < ordered.len() && (!inserted || index + 1 != ordered.len()) {
        return Err(super::RenderError::Citation {
            message: "citation could not be placed in the Markdown event stream".into(),
            span: ordered[index].span.clone(),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    use pulldown_cmark::{Parser, html};

    #[test]
    fn replaces_only_citation_text_and_preserves_enclosing_markdown() {
        let source = "Before **bold** [@a] and [@b].";
        let events = Parser::new(source)
            .into_offset_iter()
            .map(|(event, span)| (event.into_static(), span))
            .collect();
        let replacements = [
            Replacement {
                span: 25..29,
                html: "<i>B</i>".into(),
            },
            Replacement {
                span: 16..20,
                html: "<i>A</i>".into(),
            },
        ];
        let events = replace_events(events, &replacements).unwrap();
        let mut result = String::new();
        html::push_html(&mut result, events.into_iter().map(|(event, _)| event));
        assert_eq!(
            result,
            "<p>Before <strong>bold</strong> <i>A</i> and <i>B</i>.</p>\n"
        );
    }
}
