//! Test-only feasibility test for a structural Markdown block component.
//!
//! This is intentionally not a public directive parser. It establishes that a
//! variable-length colon fence and its body can retain source spans before
//! ordinary Markdown rendering begins.

use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
struct ComponentBlock {
    directive_span: Range<usize>,
    body_span: Range<usize>,
}

fn footnote_definition_span(input: &str, body_span: Range<usize>) -> Option<Range<usize>> {
    let body = &input[body_span.clone()];
    let mut offset = body_span.start;

    for line in body.split_inclusive('\n') {
        let trimmed = trim_cr(line.trim_end_matches('\n')).trim_start();
        if trimmed.starts_with("[^") && trimmed.contains("]:") {
            return Some(offset..offset + line.trim_end_matches('\n').len());
        }
        offset += line.len();
    }

    None
}

#[derive(Debug, PartialEq, Eq)]
struct ParseError {
    message: &'static str,
    span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
struct SegmentBoundary {
    opening_segment: usize,
    opening_span: Range<usize>,
    closing_segment: usize,
    closing_span: Range<usize>,
}

fn parse(input: &str) -> Result<Vec<ComponentBlock>, ParseError> {
    let mut blocks = Vec::new();
    let mut offset = 0;
    let mut fenced = None;

    while offset < input.len() {
        let line_end = input[offset..]
            .find('\n')
            .map_or(input.len(), |index| offset + index);
        let line = trim_cr(&input[offset..line_end]);

        if let Some((marker, length)) = fenced {
            if fence_run(line, marker).is_some_and(|run| run >= length) {
                fenced = None;
            }
            offset = next_line(input, line_end);
            continue;
        }
        if let Some(fence) = opening_fence(line) {
            fenced = Some(fence);
            offset = next_line(input, line_end);
            continue;
        }
        if let Some(fence_length) = component_header_fence_length(line) {
            let (block, next) = parse_block(input, offset, line_end, fence_length)?;
            blocks.push(block);
            offset = next;
            continue;
        }
        if contains_component_header(line) {
            return Err(ParseError {
                message: "Markdown block component directives need an ordinary top-level block",
                span: offset..line_end,
            });
        }
        offset = next_line(input, line_end);
    }

    Ok(blocks)
}

fn parse_block(
    input: &str,
    directive_start: usize,
    header_end: usize,
    fence_length: usize,
) -> Result<(ComponentBlock, usize), ParseError> {
    let body_start = next_line(input, header_end);
    let mut offset = body_start;
    let mut fenced = None;

    while offset < input.len() {
        let line_end = input[offset..]
            .find('\n')
            .map_or(input.len(), |index| offset + index);
        let line = trim_cr(&input[offset..line_end]);

        if let Some((marker, length)) = fenced {
            if fence_run(line, marker).is_some_and(|run| run >= length) {
                fenced = None;
            }
            offset = next_line(input, line_end);
            continue;
        }
        if let Some(fence) = opening_fence(line) {
            fenced = Some(fence);
            offset = next_line(input, line_end);
            continue;
        }
        if colon_fence_length(line).is_some_and(|length| length >= fence_length) {
            let end = next_line(input, line_end);
            if input[body_start..offset].trim().is_empty() {
                return Err(ParseError {
                    message: "Markdown block components require an authored Markdown body",
                    span: body_start..offset,
                });
            }
            return Ok((
                ComponentBlock {
                    directive_span: directive_start..end,
                    body_span: body_start..offset,
                },
                end,
            ));
        }
        if component_header_fence_length(line).is_some() {
            return Err(ParseError {
                message: "Markdown block component directives cannot nest",
                span: offset..line_end,
            });
        }
        offset = next_line(input, line_end);
    }

    Err(ParseError {
        message: "Markdown block component directive has no closing fence",
        span: directive_start..header_end,
    })
}

fn component_header_fence_length(line: &str) -> Option<usize> {
    let fence_length = leading_colon_length(line)?;
    (fence_length >= 3 && line[fence_length..].starts_with("component[") && line.ends_with(']'))
        .then_some(fence_length)
}

fn contains_component_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    component_header_fence_length(trimmed).is_some()
        || trimmed
            .strip_prefix('>')
            .is_some_and(|quoted| component_header_fence_length(quoted.trim_start()).is_some())
}

fn colon_fence_length(line: &str) -> Option<usize> {
    let length = leading_colon_length(line)?;
    (length > 0 && line[length..].trim().is_empty()).then_some(length)
}

fn leading_colon_length(line: &str) -> Option<usize> {
    let length = line
        .bytes()
        .take_while(|character| *character == b':')
        .count();
    (length > 0).then_some(length)
}

fn trim_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

fn next_line(input: &str, line_end: usize) -> usize {
    usize::min(line_end + 1, input.len())
}

fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed = (indentation <= 3).then(|| &line[indentation..])?;
    let marker = *trimmed.as_bytes().first()?;
    matches!(marker, b'`' | b'~').then_some(())?;
    let length = trimmed.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some((marker, length))
}

fn fence_run(line: &str, marker: u8) -> Option<usize> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed = (indentation <= 3).then(|| &line[indentation..])?;
    let length = trimmed
        .bytes()
        .take_while(|character| *character == marker)
        .count();
    (length > 0 && trimmed[length..].trim().is_empty()).then_some(length)
}

fn without_component_delimiters(input: &str, blocks: &[ComponentBlock]) -> String {
    let mut source = String::with_capacity(input.len());
    let mut previous_end = 0;
    for block in blocks {
        source.push_str(&input[previous_end..block.directive_span.start]);
        source.extend(
            input[block.directive_span.start..block.body_span.start]
                .chars()
                .map(|character| if character == '\n' { '\n' } else { ' ' }),
        );
        source.push_str(&input[block.body_span.clone()]);
        source.extend(
            input[block.body_span.end..block.directive_span.end]
                .chars()
                .map(|character| if character == '\n' { '\n' } else { ' ' }),
        );
        previous_end = block.directive_span.end;
    }
    source.push_str(&input[previous_end..]);
    source
}

fn source_with_component_markers(
    input: &str,
    block: &ComponentBlock,
    opening: &str,
    closing: &str,
) -> String {
    let mut source = String::with_capacity(input.len() + opening.len() + closing.len());
    source.push_str(&input[..block.directive_span.start]);
    source.push_str(opening);
    source.push('\n');
    source.push_str(&input[block.body_span.clone()]);
    source.push_str(closing);
    source.push('\n');
    source.push_str(&input[block.directive_span.end..]);
    source
}

fn private_marker_pair(source: &str) -> (String, String) {
    for nonce in 0_u64.. {
        let opening = format!("<!--heine-component-open-{nonce}-->");
        let closing = format!("<!--heine-component-close-{nonce}-->");
        if !source.contains(&opening) && !source.contains(&closing) {
            return (opening, closing);
        }
    }
    unreachable!("a finite source cannot contain every numbered marker pair")
}

fn replace_marked_component_body(
    rendered: &str,
    opening: &str,
    closing: &str,
    component_html: &str,
) -> Result<String, &'static str> {
    if rendered.matches(opening).count() != 1 || rendered.matches(closing).count() != 1 {
        return Err("the rendered Markdown did not preserve one component body location");
    }
    let start = rendered
        .find(opening)
        .ok_or("the rendered Markdown lost the component body opening marker")?;
    let body_start = start + opening.len();
    let end = rendered[body_start..]
        .find(closing)
        .map(|offset| body_start + offset)
        .ok_or("the rendered Markdown lost the component body closing marker")?;
    Ok(format!(
        "{}{}{}",
        &rendered[..start],
        component_html,
        &rendered[end + closing.len()..]
    ))
}

fn crossed_segment_boundary(segments: &[&str]) -> Option<SegmentBoundary> {
    let mut opening = None::<(usize, Range<usize>, usize)>;

    for (segment_index, segment) in segments.iter().enumerate() {
        let mut offset = 0;
        while offset < segment.len() {
            let line_end = segment[offset..]
                .find('\n')
                .map_or(segment.len(), |index| offset + index);
            let line = trim_cr(&segment[offset..line_end]);

            if let Some((opening_segment, opening_span, fence_length)) = &opening {
                if colon_fence_length(line).is_some_and(|length| length >= *fence_length) {
                    if *opening_segment != segment_index {
                        return Some(SegmentBoundary {
                            opening_segment: *opening_segment,
                            opening_span: opening_span.clone(),
                            closing_segment: segment_index,
                            closing_span: offset..line_end,
                        });
                    }
                    opening = None;
                }
            } else if let Some(fence_length) = component_header_fence_length(line) {
                opening = Some((segment_index, offset..line_end, fence_length));
            }
            offset = next_line(segment, line_end);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tera::Context;

    use super::{
        ParseError, SegmentBoundary, crossed_segment_boundary, footnote_definition_span, parse,
        private_marker_pair, replace_marked_component_body, source_with_component_markers,
        without_component_delimiters,
    };
    use crate::{
        config::{MarkdownConfig, MarkdownExtension},
        markdown,
    };

    #[test]
    fn component_feasibility_keeps_the_authored_body_span_for_ordinary_markdown() {
        let source = "```markdown\n:::component[site.note]\nliteral\n:::\n```\n\n:::component[site.note kind=\"warning\"]\n## Read this first\n\nBody with **ordinary Markdown**.\n:::\n";

        let blocks = parse(source).expect("the real top-level directive is recognized");
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(
            &source[block.body_span.clone()],
            "## Read this first\n\nBody with **ordinary Markdown**.\n"
        );

        let body = &source[block.body_span.clone()];
        let toc = markdown::toc_plan("main".into(), body, &MarkdownConfig::default(), 2, 3);
        let renderer = markdown::Renderer::new(MarkdownConfig::default())
            .expect("default Markdown renderer is valid");
        let rendered = markdown::render_with_links_anchors(
            body,
            &renderer,
            None,
            None,
            Some(toc.anchors()),
            None,
            &[],
        )
        .expect("component body follows the ordinary Markdown path");

        assert_eq!(toc.anchors(), ["read-this-first"]);
        assert!(rendered.contains(r#"<h2 id="read-this-first">Read this first</h2>"#));
        assert!(rendered.contains("<strong>ordinary Markdown</strong>"));
    }

    #[test]
    fn component_feasibility_can_keep_heading_ids_in_one_document_plan() {
        let source =
            "# Before\n\n:::component[site.note]\n## Shared heading\n:::\n\n## Shared heading\n";
        let blocks = parse(source).expect("the component directive is recognized");
        let markdown_source = without_component_delimiters(source, &blocks);
        let config = MarkdownConfig::default();
        let toc = markdown::toc_plan("main".into(), &markdown_source, &config, 1, 6);
        let renderer = markdown::Renderer::new(config).expect("default Markdown is valid");
        let rendered = markdown::render_with_links_anchors(
            &markdown_source,
            &renderer,
            None,
            None,
            Some(toc.anchors()),
            None,
            &[],
        )
        .expect("one document plan renders every authored heading");

        assert_eq!(toc.anchors().len(), 3);
        assert_eq!(toc.anchors()[0], "before");
        assert_eq!(toc.anchors()[1], "shared-heading");
        assert_ne!(toc.anchors()[1], toc.anchors()[2]);
        for anchor in &toc.anchors()[1..] {
            assert!(
                rendered.contains(&format!(r#"<h2 id="{anchor}">Shared heading</h2>"#)),
                "{rendered}"
            );
        }
    }

    #[test]
    fn component_feasibility_keeps_links_footnotes_and_headings_in_one_document() {
        let source = r#"# Before

Outside [inside link][inside] and note[^outside-note].

:::component[site.note]
## Inside

Inside [after link][after] and note[^inside-note].

[inside]: https://example.invalid/inside
:::

## After

After [inside link][inside] and note[^after-note].

[after]: https://example.invalid/after
[^outside-note]: The definition is outside the component body.
[^inside-note]: This definition remains outside the component body.
[^after-note]: The definition is after the component body.
"#;
        let blocks = parse(source).expect("the component directive is recognized");
        let markdown_source = without_component_delimiters(source, &blocks);
        let config = MarkdownConfig::with_extensions(vec![MarkdownExtension::Footnotes]);
        let renderer = markdown::Renderer::new(config.clone()).expect("Markdown is configured");
        let toc = markdown::toc_plan("main".into(), &markdown_source, &config, 1, 6);

        let ordinary = markdown::render_with_links_anchors(
            &markdown_source,
            &renderer,
            None,
            None,
            Some(toc.anchors()),
            None,
            &[],
        )
        .expect("one Markdown render resolves relationships across the component boundary");

        assert_eq!(toc.anchors(), ["before", "inside", "after"]);
        assert!(ordinary.contains(r#"href="https://example.invalid/inside""#));
        assert!(ordinary.contains(r#"href="https://example.invalid/after""#));
        assert!(ordinary.contains(r#"id="inside""#));
        assert!(ordinary.contains(r#"id="after""#));
        assert_eq!(
            ordinary.matches("footnote-reference").count(),
            3,
            "{ordinary}"
        );

        let plan = markdown::margin_footnotes::plan(
            Path::new("content/main.md"),
            "main".into(),
            &markdown_source,
            &config,
            "margin-footnote-marker".into(),
        )
        .expect("one margin-footnote plan resolves definitions across the boundary");
        let labels = (1..=plan.number_count())
            .map(|number| format!("Footnote {number}"))
            .collect::<Vec<_>>();
        let margin = markdown::margin_footnotes::render(
            &markdown_source,
            markdown::margin_footnotes::RenderContext {
                renderer: &renderer,
                assets: None,
                pages: None,
                anchors: Some(toc.anchors()),
                table_scroll_label: None,
                replacements: &[],
            },
            &plan,
            &labels,
        )
        .expect("margin footnotes preserve the same document relationships");

        assert_eq!(plan.number_count(), 3);
        assert_eq!(
            margin.matches("heine-footnote-marker").count(),
            3,
            "{margin}"
        );
        assert!(margin.contains("This definition remains outside the component body."));
        assert!(margin.contains("The definition is after the component body."));
    }

    #[test]
    fn component_feasibility_identifies_a_footnote_definition_in_a_body() {
        let source = ":::component[site.note]\nText.[^note]\n\n[^note]: This definition must remain outside the body.\n:::\n";
        let block = parse(source)
            .expect("the component directive is recognized")
            .pop()
            .expect("one component block");

        assert_eq!(
            footnote_definition_span(source, block.body_span),
            Some(38..92),
            "a public implementation can label the definition itself"
        );
    }

    #[test]
    fn component_feasibility_wraps_one_rendered_markdown_body_in_its_component() {
        let source = r#"# Before

Outside [inside link][inside] and note[^outside-note].

:::component[site.note kind="warning"]
## Inside

Inside [after link][after] and note[^inside-note].

[inside]: https://example.invalid/inside
:::

## After

After [inside link][inside] and note[^after-note].

[after]: https://example.invalid/after
[^outside-note]: The definition is outside the component body.
[^inside-note]: This definition remains outside the component body.
[^after-note]: The definition is after the component body.
"#;
        let blocks = parse(source).expect("the component directive is recognized");
        assert_eq!(blocks.len(), 1);
        let (opening, closing) = private_marker_pair(source);
        let markdown_source = source_with_component_markers(source, &blocks[0], &opening, &closing);
        let config = MarkdownConfig::with_extensions(vec![MarkdownExtension::Footnotes]);
        let renderer = markdown::Renderer::new(config.clone()).expect("Markdown is configured");
        let toc = markdown::toc_plan("main".into(), &markdown_source, &config, 1, 6);
        let rendered = markdown::render_with_links_anchors(
            &markdown_source,
            &renderer,
            None,
            None,
            Some(toc.anchors()),
            None,
            &[],
        )
        .expect("the complete Markdown document renders once");

        let body_start = rendered
            .find(&opening)
            .expect("the Markdown renderer preserves the opening marker")
            + opening.len();
        let body_end = rendered[body_start..]
            .find(&closing)
            .map(|offset| body_start + offset)
            .expect("the Markdown renderer preserves the closing marker");
        let body = &rendered[body_start..body_end];

        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "components.tera",
            r#"{% component site.note(kind: string) {"heine.body_visibility": "visible"} %}<aside class="note-{{ kind }}">{{ body }}</aside>{% endcomponent %}
{% component site.disclosure(kind: string) {"heine.body_visibility": "may-hide"} %}<details class="note-{{ kind }}">{{ body }}</details>{% endcomponent %}"#,
        )
        .expect("the probe component is valid");
        let mut arguments = Context::new();
        arguments.insert("kind", "warning");
        let component_html = tera
            .render_component_with_implicits(
                "site.note",
                &arguments,
                &Context::new(),
                Some(body),
                true,
            )
            .expect("the component wraps the rendered Markdown body");
        let output = replace_marked_component_body(&rendered, &opening, &closing, &component_html)
            .expect("the component replaces its one planned body location");

        assert_eq!(toc.anchors(), ["before", "inside", "after"]);
        assert!(
            output.contains("<aside class=\"note-warning\">\n<h2 id=\"inside\">Inside</h2>"),
            "{output}"
        );
        assert!(output.contains(r#"href="https://example.invalid/inside""#));
        assert!(output.contains(r#"href="https://example.invalid/after""#));
        assert_eq!(output.matches("footnote-reference").count(), 3, "{output}");
        assert!(output.contains("This definition remains outside the component body."));
        assert!(output.contains("The definition is after the component body."));
        assert!(!output.contains(&opening));
        assert!(!output.contains(&closing));

        let margin_plan = markdown::margin_footnotes::plan(
            Path::new("content/main.md"),
            "main".into(),
            &markdown_source,
            &config,
            "margin-footnote-marker".into(),
        )
        .expect("the complete source has one margin-footnote plan");
        let labels = (1..=margin_plan.number_count())
            .map(|number| format!("Footnote {number}"))
            .collect::<Vec<_>>();
        let margin_rendered = markdown::margin_footnotes::render(
            &markdown_source,
            markdown::margin_footnotes::RenderContext {
                renderer: &renderer,
                assets: None,
                pages: None,
                anchors: Some(toc.anchors()),
                table_scroll_label: None,
                replacements: &[],
            },
            &margin_plan,
            &labels,
        )
        .expect("margin footnotes render through the same marked source");
        let margin_body_start = margin_rendered
            .find(&opening)
            .expect("margin-footnote rendering preserves the opening marker")
            + opening.len();
        let margin_body_end = margin_rendered[margin_body_start..]
            .find(&closing)
            .map(|offset| margin_body_start + offset)
            .expect("margin-footnote rendering preserves the closing marker");
        let margin_component = tera
            .render_component_with_implicits(
                "site.note",
                &arguments,
                &Context::new(),
                Some(&margin_rendered[margin_body_start..margin_body_end]),
                true,
            )
            .expect("the component wraps the margin-footnote body");
        let margin_output =
            replace_marked_component_body(&margin_rendered, &opening, &closing, &margin_component)
                .expect("the component replaces the margin-footnote body location");
        assert!(margin_output.contains("<aside class=\"note-warning\">"));
        assert_eq!(
            margin_output.matches("heine-footnote-marker").count(),
            3,
            "{margin_output}"
        );
        assert!(!margin_output.contains(&opening));
        assert!(!margin_output.contains(&closing));

        let disclosure = tera
            .render_component_with_implicits(
                "site.disclosure",
                &arguments,
                &Context::new(),
                Some(&margin_rendered[margin_body_start..margin_body_end]),
                true,
            )
            .expect("the disclosure wraps the same rendered margin-footnote body");
        let disclosure_output =
            replace_marked_component_body(&margin_rendered, &opening, &closing, &disclosure)
                .expect("the disclosure replaces its one planned body location");
        let disclosure_start = disclosure_output
            .find("<details class=\"note-warning\">")
            .expect("the disclosure is present");
        let disclosure_end = disclosure_output[disclosure_start..]
            .find("</details>")
            .map(|offset| disclosure_start + offset)
            .expect("the disclosure closes");
        assert!(
            disclosure_output[disclosure_start..disclosure_end].contains("heine-footnote-marker"),
            "the local margin-footnote marker remains with its disclosure body"
        );
    }

    #[test]
    fn component_feasibility_rejects_an_empty_body() {
        let source = ":::component[site.note]\n\n:::\n";

        assert_eq!(
            parse(source),
            Err(ParseError {
                message: "Markdown block components require an authored Markdown body",
                span: 24..25,
            })
        );
    }

    #[test]
    fn component_feasibility_allows_a_longer_fence_around_a_shorter_one() {
        let source = "::::component[site.note]\nBefore the literal fence.\n:::\nAfter the literal fence.\n::::\n";

        let blocks = parse(source).expect("the longer directive fence is recognized");
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            &source[blocks[0].body_span.clone()],
            "Before the literal fence.\n:::\nAfter the literal fence.\n"
        );
    }

    #[test]
    fn component_feasibility_reports_nested_and_non_top_level_directives() {
        let nested = ":::component[site.note]\n:::component[site.warning]\n:::\n:::\n";
        assert_eq!(
            parse(nested).unwrap_err(),
            ParseError {
                message: "Markdown block component directives cannot nest",
                span: 24..50,
            }
        );

        let quoted = "> :::component[site.note]\n> body\n> :::\n";
        assert_eq!(
            parse(quoted).unwrap_err(),
            ParseError {
                message: "Markdown block component directives need an ordinary top-level block",
                span: 0..25,
            }
        );
    }

    #[test]
    fn component_feasibility_identifies_both_fences_across_segments() {
        let segments = [
            "Before\n\n:::component[site.note]\nA component cannot start here.\n",
            "It cannot finish in this segment either.\n:::\n\nAfter\n",
        ];

        assert_eq!(
            crossed_segment_boundary(&segments),
            Some(SegmentBoundary {
                opening_segment: 0,
                opening_span: 8..31,
                closing_segment: 1,
                closing_span: 41..44,
            })
        );
    }
}
