//! Convert structured CSL output into escaped HTML events.
use hayagriva::{
    ElemChild, Formatted, RenderedBibliography,
    citationberg::{
        FontStyle, FontVariant, FontWeight, SecondFieldAlign, TextDecoration, VerticalAlign,
    },
};
use pulldown_cmark::{Event, LinkType, Tag, TagEnd, html};

fn formatted<'a>(value: &'a Formatted, events: &mut Vec<Event<'a>>) {
    let f = value.formatting;
    let italic = f.font_style == FontStyle::Italic;
    let bold = f.font_weight == FontWeight::Bold;
    let light = f.font_weight == FontWeight::Light;
    let small = f.font_variant == FontVariant::SmallCaps;
    let underline = f.text_decoration == TextDecoration::Underline;
    let vertical = match f.vertical_align {
        VerticalAlign::Sup => Some(("<sup>", "</sup>")),
        VerticalAlign::Sub => Some(("<sub>", "</sub>")),
        _ => None,
    };
    if italic {
        events.push(Event::Start(Tag::Emphasis));
    }
    if bold {
        events.push(Event::Start(Tag::Strong));
    }
    if light {
        events.push(Event::Html("<span class=\"csl-light\">".into()));
    }
    if small {
        events.push(Event::Html("<span class=\"csl-small-caps\">".into()));
    }
    if underline {
        events.push(Event::Html("<u>".into()));
    }
    if let Some((start, _)) = vertical {
        events.push(Event::Html(start.into()));
    }
    events.push(Event::Text(value.text.as_str().into()));
    if let Some((_, end)) = vertical {
        events.push(Event::Html(end.into()));
    }
    if underline {
        events.push(Event::Html("</u>".into()));
    }
    if small {
        events.push(Event::Html("</span>".into()));
    }
    if light {
        events.push(Event::Html("</span>".into()));
    }
    if bold {
        events.push(Event::End(TagEnd::Strong));
    }
    if italic {
        events.push(Event::End(TagEnd::Emphasis));
    }
}

pub(super) fn nodes<'a>(
    values: &'a [ElemChild],
    events: &mut Vec<Event<'a>>,
    in_link: bool,
) -> Result<(), &'static str> {
    for value in values {
        match value {
            ElemChild::Text(text) => formatted(text, events),
            ElemChild::Elem(elem) if elem.display.is_none() => {
                nodes(&elem.children.0, events, in_link)?
            }
            ElemChild::Link { text, url } if !in_link => {
                if url
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
                {
                    return Err("invalid URL");
                }
                let parsed = url::Url::parse(url).map_err(|_| "invalid URL")?;
                if !matches!(parsed.scheme(), "http" | "https")
                    || parsed.host_str().is_none()
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                {
                    return Err("unsupported URL");
                }
                events.push(Event::Start(Tag::Link {
                    link_type: LinkType::Inline,
                    dest_url: url.as_str().into(),
                    title: "".into(),
                    id: "".into(),
                }));
                formatted(text, events);
                events.push(Event::End(TagEnd::Link));
            }
            _ => return Err("unsupported CSL output"),
        }
    }
    Ok(())
}

pub(super) fn render_bibliography(bib: &RenderedBibliography) -> Result<String, &'static str> {
    let alignment = match bib.second_field_align {
        None => "none",
        Some(SecondFieldAlign::Flush) => "flush",
        Some(SecondFieldAlign::Margin) => "margin",
    };
    let mut events = vec![Event::Html(format!("<ul class=\"csl-bibliography\" role=\"list\" data-align=\"{alignment}\" data-hanging=\"{}\" data-line-spacing=\"{}\" data-entry-spacing=\"{}\">", bib.hanging_indent, bib.line_spacing, bib.entry_spacing).into())];
    for item in &bib.items {
        events.push(Event::Html("<li class=\"csl-entry\">".into()));
        if let Some(first) = &item.first_field {
            events.push(Event::Html("<span class=\"csl-first\">".into()));
            nodes(std::slice::from_ref(first), &mut events, false)?;
            events.push(Event::Html("</span>".into()));
        }
        events.push(Event::Html("<span class=\"csl-content\">".into()));
        nodes(&item.content.0, &mut events, false)?;
        events.push(Event::Html("</span></li>".into()));
    }
    events.push(Event::Html("</ul>".into()));
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    Ok(output)
}

#[test]
fn structured_formatting_and_hostile_values() {
    use hayagriva::Formatting;
    let value = Formatted {
        text: "<script>&\"".into(),
        formatting: Formatting {
            font_style: FontStyle::Italic,
            font_variant: FontVariant::SmallCaps,
            font_weight: FontWeight::Bold,
            text_decoration: TextDecoration::Underline,
            vertical_align: VerticalAlign::Sup,
        },
    };
    let mut events = Vec::new();
    let text = [ElemChild::Text(value.clone())];
    nodes(&text, &mut events, false).expect("format");
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    assert!(output.contains("&lt;script&gt;&amp;"), "{output}");
    assert!(
        output.contains("<em><strong><span class=\"csl-small-caps\"><u><sup>"),
        "{output}"
    );
    for url in [
        "javascript:alert(1)",
        "https://user@example.org",
        "https://example.org/\\evil",
        "https://example.org/a b",
    ] {
        assert!(
            nodes(
                &[ElemChild::Link {
                    text: value.clone(),
                    url: url.into()
                }],
                &mut Vec::new(),
                false
            )
            .is_err(),
            "{url}"
        );
    }
    assert!(
        nodes(
            &[ElemChild::Markup("<script>".into())],
            &mut Vec::new(),
            false
        )
        .is_err()
    );
    assert!(
        nodes(
            &[ElemChild::Transparent {
                cite_idx: 0,
                format: Default::default()
            }],
            &mut Vec::new(),
            false
        )
        .is_err()
    );
    let link = [ElemChild::Link {
        text: value,
        url: "https://example.org/?a=1&b=2".into(),
    }];
    let mut events = Vec::new();
    nodes(&link, &mut events, false).expect("safe URL");
    let mut html = String::new();
    html::push_html(&mut html, events.into_iter());
    assert!(
        html.contains("href=\"https://example.org/?a=1&amp;b=2\""),
        "{html}"
    );
    assert!(nodes(&link, &mut Vec::new(), true).is_err());
    let sub = [ElemChild::Text(Formatted {
        text: "sub".into(),
        formatting: Formatting {
            font_weight: FontWeight::Light,
            vertical_align: VerticalAlign::Sub,
            ..Default::default()
        },
    })];
    let mut events = Vec::new();
    nodes(&sub, &mut events, false).expect("subscript");
    let mut html = String::new();
    html::push_html(&mut html, events.into_iter());
    assert_eq!(html, "<span class=\"csl-light\"><sub>sub</sub></span>");
}

#[test]
fn real_style_layout_and_escaped_title() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, archive::locales,
        citationberg::Style,
    };
    let style = r#"<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0"><info><title>Layout</title><id>https://example.org/layout</id></info><citation><layout><text variable="citation-number"/></layout></citation><bibliography hanging-indent="true" second-field-align="flush" line-spacing="2" entry-spacing="1"><layout><text variable="citation-number"/><text variable="title" font-style="italic" text-decoration="underline"/></layout></bibliography></style>"#;
    let Style::Independent(style) = Style::from_xml(style).expect("style") else {
        panic!("independent")
    };
    let bib =
        hayagriva::io::from_biblatex_str("@book{a,title={Hello <script>}}").expect("bibliography");
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    driver.citation(CitationRequest::from_items(
        vec![CitationItem::with_entry(bib.get("a").expect("entry"))],
        &style,
        &locales,
    ));
    let output = driver.finish(BibliographyRequest {
        style: &style,
        locale: None,
        locale_files: &locales,
    });
    let html =
        render_bibliography(output.bibliography.as_ref().expect("bibliography")).expect("render");
    assert!(
        html.starts_with("<ul class=\"csl-bibliography\" role=\"list\""),
        "{html}"
    );
    assert!(html.contains("<li class=\"csl-entry\">"), "{html}");
    assert!(html.ends_with("</li></ul>"), "{html}");
    assert!(html.contains("data-align=\"flush\" data-hanging=\"true\" data-line-spacing=\"2\" data-entry-spacing=\"1\""), "{html}");
    assert!(
        html.contains("<span class=\"csl-first\">1</span>"),
        "{html}"
    );
    assert!(
        html.contains("<em><u>Hello &lt;script&gt;</u></em>"),
        "{html}"
    );
}

#[cfg(test)]
mod feasibility;

#[test]
fn four_bundled_styles_render_representative_entries() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
        archive::{ArchivedStyle, locales},
        citationberg::Style,
    };
    let bib = hayagriva::io::from_biblatex_str(r#"@article{paper, author={Smith, Jane and Jones, Alex}, title={An <example> study}, journaltitle={Journal of Examples}, year={2024}, volume={12}, number={2}, pages={10--20}, doi={10.1234/example}}
@book{book, author={Garcia, Ana}, title={The Research Book}, year={2021}, publisher={Example Press}, url={https://example.org/book}}"#).expect("bibliography");
    let locales = locales();
    for archived in [
        ArchivedStyle::AmericanPsychologicalAssociation,
        ArchivedStyle::InstituteOfElectricalAndElectronicsEngineers,
        ArchivedStyle::ChicagoAuthorDate,
        ArchivedStyle::ChicagoNotes,
    ] {
        let Style::Independent(style) = archived.get() else {
            panic!("independent style")
        };
        let mut driver = BibliographyDriver::new();
        driver.citation(CitationRequest::new(
            vec![
                CitationItem::with_entry(bib.get("paper").expect("entry")),
                CitationItem::with_entry(bib.get("book").expect("entry")),
            ],
            &style,
            None,
            &locales,
            Some(1),
        ));
        let result = driver.finish(BibliographyRequest {
            style: &style,
            locale: None,
            locale_files: &locales,
        });
        let mut events = Vec::new();
        nodes(&result.citations[0].citation.0, &mut events, false)
            .unwrap_or_else(|error| panic!("{archived:?} citation: {error}"));
        let mut citation = String::new();
        html::push_html(&mut citation, events.into_iter());
        assert!(!citation.is_empty(), "{archived:?}");
        if archived == ArchivedStyle::InstituteOfElectricalAndElectronicsEngineers {
            assert!(
                citation.contains('1') && citation.contains('2'),
                "{citation}"
            );
        } else {
            assert!(
                citation.contains("Smith") && citation.contains("Garcia"),
                "{archived:?}: {citation}"
            );
        }
        let rendered = render_bibliography(result.bibliography.as_ref().expect("bibliography"))
            .unwrap_or_else(|error| panic!("{archived:?} bibliography: {error}"));
        assert!(
            rendered.contains("&lt;example&gt;"),
            "{archived:?}: {rendered}"
        );
        assert!(!rendered.contains("<example>"), "{archived:?}: {rendered}");
        assert!(
            rendered.contains("Research Book"),
            "{archived:?}: {rendered}"
        );
    }
}
