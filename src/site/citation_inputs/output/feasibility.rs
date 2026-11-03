//! Processor behavior investigated for citation disambiguation and position handling.

use super::*;

// These tests record observed processor behavior. Revisit their expectations
// when Hayagriva changes rather than preserving an upstream defect.
fn citation_html(citation: &hayagriva::RenderedCitation) -> String {
    let mut events = Vec::new();
    nodes(&citation.citation.0, &mut events, false).unwrap();
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

#[test]
fn ibid_position_follows_previous_item_inside_and_across_groups() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, archive::locales,
        citationberg::Style,
    };
    let Style::Independent(style) = Style::from_xml(r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Position</title><id>https://example.org/position</id></info><citation><layout delimiter="; "><choose><if position="ibid"><text value="IBID"/></if><else><text variable="title"/></else></choose></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#).unwrap() else {
        panic!("independent style");
    };
    let bib = hayagriva::io::from_biblatex_str("@book{a,title={Alpha}}\n@book{b,title={Beta}}\n")
        .unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for group in [&["a", "b"][..], &["b"][..], &["a", "a"][..]] {
        let items = group
            .iter()
            .map(|key| CitationItem::with_entry(bib.get(key).unwrap()))
            .collect();
        driver.citation(CitationRequest::new(items, &style, None, &locales, None));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let values: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(values, ["Alpha; Beta", "IBID", "Alpha; IBID"]);
}

#[test]
fn ibid_single_item_locator_transitions() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, LocatorPayload,
        SpecificLocator,
        archive::locales,
        citationberg::{Style, taxonomy::Locator},
    };
    let Style::Independent(style) = Style::from_xml(r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Position</title><id>https://example.org/position</id></info><citation><layout><choose><if position="ibid-with-locator"><text value="changed"/></if><else-if position="ibid"><text value="same"/></else-if><else><text value="other"/></else></choose></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#).unwrap() else { panic!("style") };
    let bib = hayagriva::io::from_biblatex_str("@book{a,title={Alpha}}\n@book{b,title={Beta}}\n")
        .unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for (key, locator) in [
        ("a", None),
        ("a", None),
        ("a", Some((Locator::Page, "12"))),
        ("a", Some((Locator::Page, "12"))),
        ("a", Some((Locator::Page, "13"))),
        ("a", Some((Locator::Chapter, "13"))),
        ("a", None),
        ("b", None),
    ] {
        let locator =
            locator.map(|(kind, value)| SpecificLocator(kind, LocatorPayload::Str(value)));
        driver.citation(CitationRequest::new(
            vec![CitationItem::with_locator(bib.get(key).unwrap(), locator)],
            &style,
            None,
            &locales,
            None,
        ));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let values: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(
        values,
        [
            "other", "same", "changed", "same", "changed", "changed", "other", "other"
        ]
    );
}

#[test]
fn ibid_single_item_citation_with_position_dependent_sort() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, archive::locales,
        citationberg::Style,
    };
    let Style::Independent(style) = Style::from_xml(r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Sort position</title><id>https://example.org/sort</id></info><macro name="sort"><choose><if position="ibid"><text value="a"/></if><else><text value="b"/></else></choose></macro><citation><sort><key macro="sort"/></sort><layout><choose><if position="ibid"><text value="same"/></if><else><text value="other"/></else></choose></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#).unwrap() else { panic!("style") };
    let bib = hayagriva::io::from_biblatex_str("@book{a,title={Alpha}}\n").unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for _ in 0..2 {
        driver.citation(CitationRequest::new(
            vec![CitationItem::with_entry(bib.get("a").unwrap())],
            &style,
            None,
            &locales,
            None,
        ));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let values: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(values, ["other", "same"]);
}

#[test]
fn ibid_single_item_author_and_year_disambiguation() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, archive::locales,
        citationberg::Style,
    };
    let Style::Independent(style) = Style::from_xml(r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Disambiguation</title><id>https://example.org/disambiguation</id></info><citation disambiguate-add-givenname="true" disambiguate-add-year-suffix="true"><layout delimiter=" "><names variable="author"><name form="short" initialize-with=". "/></names><date variable="issued"><date-part name="year"/></date><choose><if position="ibid"><text value="REPEAT"/></if></choose></layout></citation><bibliography><sort><key variable="title"/></sort><layout delimiter=" "><names variable="author"><name/></names><date variable="issued"><date-part name="year"/></date><text variable="title"/></layout></bibliography></style>"#).unwrap() else { panic!("style") };
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Alice},title={Beta},year={2020}}\n@book{c,author={Smith, Bob},title={Gamma},year={2020}}\n").unwrap();
    let locales = locales();
    let render = |selected: &hayagriva::citationberg::IndependentStyle| {
        let mut driver = BibliographyDriver::new();
        for key in ["a", "a", "b", "b", "c", "c", "a"] {
            driver.citation(CitationRequest::new(
                vec![CitationItem::with_entry(bib.get(key).unwrap())],
                selected,
                None,
                &locales,
                None,
            ));
        }
        let result = driver.finish(BibliographyRequest::new(selected, None, &locales));
        result
            .citations
            .iter()
            .map(citation_html)
            .collect::<Vec<_>>()
    };
    let values = render(&style);
    assert_eq!(
        values,
        [
            "Smith2020a",
            "Smith2020aREPEAT",
            "Smith2020b",
            "Smith2020bREPEAT",
            "B. Smith2020",
            "B. Smith2020REPEAT",
            "Smith2020a"
        ]
    );
    let mut control = style.clone();
    control.citation.layout.elements.pop().unwrap();
    assert_eq!(
        render(&control),
        [
            "Smith2020a",
            "Smith2020a",
            "Smith2020b",
            "Smith2020b",
            "Smith2020c",
            "Smith2020c",
            "Smith2020a"
        ]
    );
}

#[test]
fn diagnostic_citation_requests_change_document_disambiguation() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
        LocatorPayload, SpecificLocator,
        archive::{ArchivedStyle, locales},
        citationberg::{Style, taxonomy::Locator},
    };
    let Style::Independent(style) = ArchivedStyle::AmericanPsychologicalAssociation.get() else {
        panic!("style");
    };
    let library = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Bob},title={Beta},year={2020}}\n").unwrap();
    let locales = locales();
    for mode in ["baseline", "interleaved", "appended", "appended-hidden"] {
        let mut driver = BibliographyDriver::new();
        for (key, page) in [("a", "12"), ("b", "13")] {
            let entry = library.get(key).unwrap();
            driver.citation(CitationRequest::new(
                vec![CitationItem::with_locator(
                    entry,
                    Some(SpecificLocator(Locator::Page, LocatorPayload::Str(page))),
                )],
                &style,
                None,
                &locales,
                None,
            ));
            if mode == "interleaved" {
                driver.citation(CitationRequest::new(
                    vec![CitationItem::with_entry(entry)],
                    &style,
                    None,
                    &locales,
                    None,
                ));
            }
        }
        if mode == "appended" || mode == "appended-hidden" {
            for key in ["a", "b"] {
                let mut item = CitationItem::with_entry(library.get(key).unwrap());
                item.hidden = mode == "appended-hidden";
                driver.citation(CitationRequest::new(
                    vec![item],
                    &style,
                    None,
                    &locales,
                    None,
                ));
            }
        }
        let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
        let rendered: Vec<_> = result
            .citations
            .iter()
            .map(|citation| {
                let mut plain = String::new();
                citation
                    .citation
                    .write_buf(&mut plain, BufWriteFormat::Plain)
                    .unwrap();
                plain
            })
            .collect();
        let expected: &[&str] = match mode {
            "baseline" => &["(Smith, 2020, p. 12)", "(Smith, 2020, p. 13)"],
            "interleaved" => &[
                "(Smith, 2020a, p. 12)",
                "(Smith, 2020a)",
                "(Smith, 2020b, p. 13)",
                "(Smith, 2020b)",
            ],
            "appended" => &[
                "(Smith, 2020a, p. 12)",
                "(Smith, 2020b, p. 13)",
                "(Smith, 2020a)",
                "(Smith, 2020b)",
            ],
            "appended-hidden" => &["(Smith, 2020a, p. 12)", "(Smith, 2020b, p. 13)", "", ""],
            _ => unreachable!(),
        };
        assert_eq!(rendered, expected, "{mode}");
    }
}

#[test]
fn apa_metadata_collision_probe() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
        ElemMeta,
        archive::{ArchivedStyle, locales},
        citationberg::Style,
    };
    fn fields(nodes: &[ElemChild], parts: &mut Vec<String>) {
        for node in nodes {
            if let ElemChild::Elem(elem) = node {
                if matches!(elem.meta, Some(ElemMeta::Name(_, _) | ElemMeta::Date)) {
                    let mut text = String::new();
                    elem.children
                        .write_buf(&mut text, BufWriteFormat::Plain)
                        .unwrap();
                    parts.push(text);
                } else {
                    fields(&elem.children.0, parts);
                }
            }
        }
    }
    let Style::Independent(style) = ArchivedStyle::AmericanPsychologicalAssociation.get() else {
        panic!("style");
    };
    let library = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Bob},title={Beta},year={2020}}\n@book{c,author={Smith, Alice},title={Gamma},year={2021}}\n@book{d,author={Smith, Alice},title={Delta},year={2020}}\n").unwrap();
    let locales = locales();
    for (groups, expected) in [
        (vec![vec!["a"], vec!["b"]], true),
        (vec![vec!["a"], vec!["c"]], false),
        (vec![vec!["a"], vec!["a"]], false),
        (vec![vec!["a"], vec!["d"]], true),
        (vec![vec!["a", "b"]], true),
    ] {
        let mut driver = BibliographyDriver::new();
        for group in &groups {
            driver.citation(CitationRequest::new(
                group
                    .iter()
                    .map(|key| CitationItem::with_entry(library.get(key).unwrap()))
                    .collect(),
                &style,
                None,
                &locales,
                None,
            ));
        }
        let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
        if groups == [vec!["a"], vec!["d"]] {
            let suffix = result.citations[0].citation.find_elem_by(&|elem| {
                elem.meta == Some(ElemMeta::Text)
                    && matches!(elem.children.0.as_slice(), [ElemChild::Text(text)] if text.text == "a")
            });
            assert!(suffix.is_some());
            let rendered: Vec<_> = result
                .citations
                .iter()
                .map(|cite| {
                    let mut plain = String::new();
                    cite.citation
                        .write_buf(&mut plain, BufWriteFormat::Plain)
                        .unwrap();
                    plain
                })
                .collect();
            assert_ne!(rendered[0], rendered[1]);
        }
        let mut seen = std::collections::BTreeMap::new();
        let mut collision = false;
        for (cite, group) in result.citations.iter().zip(&groups) {
            for (index, key) in group.iter().enumerate() {
                let entry = cite.citation.find_meta(&ElemMeta::Entry(index)).unwrap();
                let mut signature = Vec::new();
                fields(&entry.children.0, &mut signature);
                if let Some(previous) = seen.insert(signature, key) {
                    collision |= previous != key;
                }
            }
        }
        assert_eq!(collision, expected, "{groups:?}");
    }
}

#[test]
fn apa_different_locators_hide_ambiguous_authors() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, LocatorPayload,
        SpecificLocator,
        archive::{ArchivedStyle, locales},
        citationberg::{Style, taxonomy::Locator},
    };
    let Style::Independent(style) = ArchivedStyle::AmericanPsychologicalAssociation.get() else {
        panic!("independent style");
    };
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Bob},title={Beta},year={2020}}\n").unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for (key, page) in [("a", "12"), ("b", "13")] {
        driver.citation(CitationRequest::new(
            vec![CitationItem::with_locator(
                bib.get(key).unwrap(),
                Some(SpecificLocator(Locator::Page, LocatorPayload::Str(page))),
            )],
            &style,
            None,
            &locales,
            None,
        ));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let locator = result.citations[0].citation.find_elem_by(&|elem| {
        elem.meta == Some(hayagriva::ElemMeta::Text)
            && matches!(elem.children.0.as_slice(), [ElemChild::Text(text)] if text.text == "12")
    });
    assert!(locator.is_some());
    fn has_given_name(nodes: &[ElemChild], expected: &str) -> bool {
        nodes.iter().any(|node| match node {
            ElemChild::Elem(element) => {
                matches!(&element.meta, Some(hayagriva::ElemMeta::Names(names)) if names.iter().flat_map(|(people, _)| people).any(|person| person.given_name.as_deref() == Some(expected)))
                    || has_given_name(&element.children.0, expected)
            }
            _ => false,
        })
    }
    for (citation, author) in result.citations.iter().zip(["Alice", "Bob"]) {
        assert!(
            citation
                .citation
                .find_meta(&hayagriva::ElemMeta::Date)
                .is_some()
        );
        assert!(has_given_name(&citation.citation.0, author));
    }
    let citations: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(citations, ["(Smith, 2020, p. 12)", "(Smith, 2020, p. 13)"]);
    let bibliography = render_bibliography(result.bibliography.as_ref().unwrap()).unwrap();
    for title in ["Alpha", "Beta"] {
        assert!(
            bibliography.contains(&format!("Smith. (2020). <em>{title}</em>")),
            "{bibliography}"
        );
    }
}

#[test]
fn apa_disambiguation_with_distinct_citation_sequences() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
        archive::{ArchivedStyle, locales},
        citationberg::Style,
    };
    let Style::Independent(style) = ArchivedStyle::AmericanPsychologicalAssociation.get() else {
        panic!("independent style");
    };
    assert!(super::super::style::check(&style).is_ok());
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Alice},title={Beta},year={2020}}\n@book{c,author={Smith, Bob},title={Gamma},year={2020}}\n").unwrap();
    let locales = locales();
    for keys in [
        &["a", "b"][..],
        &["a", "c"][..],
        &["a", "b", "c"][..],
        &["a", "a", "b", "b", "c", "c", "a"][..],
    ] {
        let mut driver = BibliographyDriver::new();
        for key in keys {
            driver.citation(CitationRequest::new(
                vec![CitationItem::with_entry(bib.get(key).unwrap())],
                &style,
                None,
                &locales,
                None,
            ));
        }
        let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
        let citations: Vec<_> = result.citations.iter().map(citation_html).collect();
        let bibliography = render_bibliography(result.bibliography.as_ref().unwrap()).unwrap();
        match keys {
            ["a", "b"] => {
                assert_eq!(citations, ["(Smith, 2020a)", "(Smith, 2020b)"]);
                assert!(bibliography.contains("Smith, A. (2020a). <em>Alpha</em>"));
                assert!(bibliography.contains("Smith, A. (2020b). <em>Beta</em>"));
            }
            ["a", "c"] => {
                assert_eq!(citations, ["(Smith, 2020)", "(Smith, 2020)"]);
                assert!(bibliography.contains("Smith. (2020). <em>Alpha</em>"));
                assert!(bibliography.contains("Smith. (2020). <em>Gamma</em>"));
            }
            ["a", "b", "c"] => assert_eq!(citations, ["(Smith, 2020)"; 3]),
            _ => assert_eq!(
                citations,
                [
                    "(Smith, 2020a)",
                    "(Smith, 2020a)",
                    "(Smith, 2020b)",
                    "(Smith, 2020b)",
                    "(Smith, 2020c)",
                    "(Smith, 2020c)",
                    "(Smith, 2020a)"
                ]
            ),
        }
    }
}

#[test]
fn chicago_disambiguation_style_roundtrip_and_sequence() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
        archive::{ArchivedStyle, locales},
        citationberg::{LocaleCode, Style},
    };
    let original = ArchivedStyle::ChicagoAuthorDate.get();
    let roundtrip = Style::from_xml(&original.to_xml().unwrap()).unwrap();
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Alice},title={Beta},year={2020}}\n@book{c,author={Smith, Bob},title={Gamma},year={2020}}\n").unwrap();
    let locales = locales();
    for (name, style) in [("original", original), ("roundtrip", roundtrip)] {
        let Style::Independent(style) = style else {
            panic!("independent style");
        };
        for locale in [None, Some(LocaleCode("en".into()))] {
            for keys in [
                &["a", "b", "c"][..],
                &["a", "a", "b", "c"][..],
                &["a", "a", "b", "b", "c", "c", "a"][..],
            ] {
                let mut driver = BibliographyDriver::new();
                for &key in keys {
                    driver.citation(CitationRequest::new(
                        vec![CitationItem::with_entry(bib.get(key).unwrap())],
                        &style,
                        locale.clone(),
                        &locales,
                        None,
                    ));
                }
                let result =
                    driver.finish(BibliographyRequest::new(&style, locale.clone(), &locales));
                let bibliography =
                    render_bibliography(result.bibliography.as_ref().unwrap()).unwrap();
                let disambiguated = keys.len() == 7;
                for (title, year) in [("Alpha", "2020a"), ("Beta", "2020b"), ("Gamma", "2020c")] {
                    let expected = if disambiguated { year } else { "2020" };
                    assert!(
                        bibliography.contains(&format!("{expected}. <em>{title}</em>")),
                        "{name} {locale:?} {keys:?}: {bibliography}"
                    );
                }
            }
        }
    }
}

#[test]
fn chicago_author_date_disambiguation_matches_bibliography() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
        archive::{ArchivedStyle, locales},
        citationberg::Style,
    };
    let Style::Independent(style) = ArchivedStyle::ChicagoAuthorDate.get() else {
        panic!("independent style");
    };
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Alice},title={Beta},year={2020}}\n@book{c,author={Smith, Bob},title={Gamma},year={2020}}\n").unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for key in ["a", "a", "b", "b", "c", "c", "a"] {
        driver.citation(CitationRequest::new(
            vec![CitationItem::with_entry(bib.get(key).unwrap())],
            &style,
            None,
            &locales,
            None,
        ));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let citations: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(
        citations,
        [
            "(Smith 2020a)",
            "(Smith 2020a)",
            "(Smith 2020b)",
            "(Smith 2020b)",
            "(Smith 2020c)",
            "(Smith 2020c)",
            "(Smith 2020a)"
        ]
    );
    let bibliography = render_bibliography(result.bibliography.as_ref().unwrap()).unwrap();
    for entry in [
        "Smith, Alice. 2020a. <em>Alpha</em>",
        "Smith, Alice. 2020b. <em>Beta</em>",
        "Smith, Bob. 2020c. <em>Gamma</em>",
    ] {
        assert!(
            bibliography.contains(entry),
            "missing {entry} in {bibliography}"
        );
    }
}

#[test]
fn chicago_author_date_groups_and_locators() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, LocatorPayload,
        SpecificLocator,
        archive::{ArchivedStyle, locales},
        citationberg::{Style, taxonomy::Locator},
    };
    let Style::Independent(style) = ArchivedStyle::ChicagoAuthorDate.get() else {
        panic!("independent style");
    };
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Zed, Alice},title={Alpha},year={2020}}\n@book{b,author={Able, Bob},title={Beta},year={2021}}\n@incollection{c,author={Carl, Carol},title={Chapter},booktitle={Collected Works},type={Essay},year={2022}}\n").unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for group in [
        &[("a", None), ("b", None)][..],
        &[("b", None)][..],
        &[("a", None)][..],
        &[("b", None), ("c", None)][..],
        &[("c", None)][..],
        &[("b", Some("12"))][..],
        &[("b", Some("13"))][..],
        &[("b", None), ("a", None)][..],
        &[("a", None)][..],
    ] {
        let items = group
            .iter()
            .map(|(key, page)| {
                CitationItem::with_locator(
                    bib.get(key).unwrap(),
                    page.map(|page| SpecificLocator(Locator::Page, LocatorPayload::Str(page))),
                )
            })
            .collect();
        driver.citation(CitationRequest::new(items, &style, None, &locales, None));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let values: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(
        values,
        [
            "(Zed 2020; Able 2021)",
            "(Able 2021)",
            "(Zed 2020)",
            "(Able 2021; Carl 2022)",
            "(Carl 2022)",
            "(Able 2021, 12)",
            "(Able 2021, 13)",
            "(Able 2021; Zed 2020)",
            "(Zed 2020)"
        ]
    );
}

#[test]
fn chicago_author_date_groups_with_untitled_genre_entries() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
        archive::{ArchivedStyle, locales},
        citationberg::Style,
    };
    let Style::Independent(style) = ArchivedStyle::ChicagoAuthorDate.get() else {
        panic!("style")
    };
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Zed, Alice},title={Alpha},year={2020}}\n@incollection{c,booktitle={Collected Works},type={Essay}}\n@article{j,journaltitle={Journal},issuetitle={Special},type={Review}}\n").unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for group in [&["a", "c"][..], &["c"][..], &["a", "j"][..], &["j"][..]] {
        let items = group
            .iter()
            .map(|key| CitationItem::with_entry(bib.get(key).unwrap()))
            .collect();
        driver.citation(CitationRequest::new(items, &style, None, &locales, None));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let values: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert_eq!(
        values,
        [
            "(Zed 2020; Essay, n.d.)",
            "(Essay, n.d.)",
            "(Zed 2020; Review, n.d.)",
            "(Review, n.d.)"
        ]
    );
}

#[test]
fn instrumented_chicago_position_branch_for_genre_entries() {
    use hayagriva::{
        BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
        archive::{ArchivedStyle, locales},
        citationberg::Style,
    };
    let xml = ArchivedStyle::ChicagoAuthorDate.get().to_xml().unwrap();
    let original = "<choose><if position=\"ibid ibid-with-locator\" match=\"none\">";
    assert!(xml.contains(original));
    let instrumented = xml.replace(
        original,
        &format!("<text value=\"POSITION-TESTED\"/>{original}"),
    );
    let Style::Independent(style) = Style::from_xml(&instrumented).unwrap() else {
        panic!("style")
    };
    let bib = hayagriva::io::from_biblatex_str("@book{a,author={Zed, Alice},title={Alpha},year={2020}}\n@incollection{c,booktitle={Collected Works},type={Essay}}\n@article{j,journaltitle={Journal},issuetitle={Special},type={Review}}\n@legislation{l,title={Example Act},type={Statute},year={2020}}\n@jurisdiction{k,title={Example Case},type={Decision},year={2021}}\n").unwrap();
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    for group in [
        &["a", "c"][..],
        &["c"][..],
        &["a", "j"][..],
        &["j"][..],
        &["a", "l"][..],
        &["l"][..],
        &["a", "k"][..],
        &["k"][..],
    ] {
        let items = group
            .iter()
            .map(|key| CitationItem::with_entry(bib.get(key).unwrap()))
            .collect();
        driver.citation(CitationRequest::new(items, &style, None, &locales, None));
    }
    let result = driver.finish(BibliographyRequest::new(&style, None, &locales));
    let values: Vec<_> = result.citations.iter().map(citation_html).collect();
    assert!(
        values
            .iter()
            .all(|value| !value.contains("POSITION-TESTED"))
    );
    let start = instrumented.find("<citation ").unwrap();
    let end = instrumented.find("</citation>").unwrap() + "</citation>".len();
    let mut positive = instrumented.clone();
    positive.replace_range(
        start..end,
        "<citation><layout><text macro=\"description-format-short\"/></layout></citation>",
    );
    let Style::Independent(positive_style) = Style::from_xml(&positive).unwrap() else {
        panic!("style")
    };
    let mut driver = BibliographyDriver::new();
    for key in ["c", "j"] {
        driver.citation(CitationRequest::new(
            vec![CitationItem::with_entry(bib.get(key).unwrap())],
            &positive_style,
            None,
            &locales,
            None,
        ));
    }
    let positive_result = driver.finish(BibliographyRequest::new(&positive_style, None, &locales));
    let values: Vec<_> = positive_result
        .citations
        .iter()
        .map(citation_html)
        .collect();
    assert!(values[0].contains("POSITION-TESTED"), "{values:?}");
    assert!(!values[1].contains("POSITION-TESTED"), "{values:?}");
}
