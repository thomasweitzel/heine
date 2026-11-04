//! Check candidate publication data against the existing bibliography pipeline.

use super::*;

#[test]
fn concatenated_bibtex_preamble_is_not_supported() {
    let inputs = [Source {
        path: "preamble.bib".into(),
        text: "@preamble{\"first\" # \"second\"}\n@book{work,title={Example}}\n".into(),
    }];
    assert!(matches!(
        bibliography::resolve(&inputs),
        Err(bibliography::Error::Parse { .. })
    ));
}

#[test]
fn custom_link_fields_have_no_checked_url_or_asset_meaning() {
    let inputs = [Source {
        path: "publications.bib".into(),
        text: "@book{work,title={Example},pdf={missing.pdf},code={not a URL}}\n".into(),
    }];
    let resolved = bibliography::resolve(&inputs).unwrap();
    let work = resolved.get("work").unwrap();
    assert_eq!(work.get_as::<String>("pdf").unwrap(), "missing.pdf");
    assert_eq!(work.get_as::<String>("code").unwrap(), "not a URL");
}

#[test]
fn multiple_urls_in_one_field_are_not_a_single_checked_url() {
    let inputs = [Source {
        path: "publications.bib".into(),
        text: "@article{work,title={Example},url={https://example.org/article; https://example.org/paper.pdf}}\n".into(),
    }];
    assert!(matches!(
        bibliography::resolve(&inputs),
        Err(bibliography::Error::InvalidUrl { .. })
    ));
}

#[test]
fn publication_keys_and_selected_fields_use_the_resolved_bibliography() {
    use biblatex::ChunksExt;
    let inputs = [Source {
        path: "publications.bib".into(),
        text: r#"
@book{first,title={First},code={https://example.org/code},private={local/path}}
@article{second,title={Second},note={A {NASA} example with $x^2$}}
"#
        .into(),
    }];
    let bibliography = bibliography::resolve(&inputs).unwrap();
    let keys: Vec<_> = bibliography
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    assert_eq!(keys, ["first", "second"]);
    let selected: Vec<_> = ["second", "first", "second"]
        .into_iter()
        .map(|key| bibliography.get(key).map(|entry| entry.key.as_str()))
        .collect();
    assert_eq!(selected, [Some("second"), Some("first"), Some("second")]);
    assert!(bibliography.get("missing").is_none());

    let first = bibliography.get("first").unwrap();
    let selected_fields = ["code", "pdf"]
        .into_iter()
        .filter_map(|name| first.get(name).map(|value| (name, value.format_verbatim())))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        selected_fields.get("code").map(String::as_str),
        Some("https://example.org/code")
    );
    assert!(!selected_fields.contains_key("pdf"));
    assert!(!selected_fields.contains_key("private"));
    let note = bibliography
        .get("second")
        .unwrap()
        .get("note")
        .unwrap()
        .format_verbatim();
    assert!(note.contains("NASA"));
    assert!(note.contains("$x^2$"));
}

// This is a probe of existing library accessors, not a proposed public API.
#[derive(Debug)]
struct PublicationProbe<'a> {
    key: &'a str,
    kind: &'a biblatex::EntryType,
    title: Option<String>,
    authors: Option<Vec<biblatex::Person>>,
    date: Option<biblatex::PermissiveType<biblatex::Date>>,
    journal: Option<String>,
    book_title: Option<String>,
    doi: Option<String>,
    pdf: Option<String>,
    code: Option<String>,
}

fn publication_probe(entry: &biblatex::Entry) -> PublicationProbe<'_> {
    use biblatex::ChunksExt;
    let text = |key: &str| entry.get(key).map(|value| value.format_verbatim());
    PublicationProbe {
        key: &entry.key,
        kind: &entry.entry_type,
        title: text("title"),
        authors: entry.get("author").map(|_| entry.author().unwrap()),
        date: entry
            .get("date")
            .or_else(|| entry.get("year"))
            .map(|_| entry.date().unwrap()),
        journal: entry.journal().ok().map(|value| value.format_verbatim()),
        book_title: text("booktitle"),
        doi: text("doi"),
        pdf: text("pdf"),
        code: text("code"),
    }
}

#[test]
fn candidate_publication_view_uses_resolved_fields_without_csl() {
    use biblatex::{DateValue, EntryType, PermissiveType};
    let inputs = [Source {
        path: "publications.bib".into(),
        text: r#"
@proceedings{conference,title={Example Proceedings},year={2023}}
@inproceedings{paper,author={Smith, Alice and García, Bob},title={A {NASA} Study},crossref={conference},doi={10.1000/example},pdf={papers/study.pdf},code={https://example.org/code}}
@article{article,author={Lee, Robin},title={A Result},journaltitle={Example Journal},date={2022-07}}
@incollection{chapter,title={A Chapter},booktitle={Collected Research},editor={Jones, Morgan},year={2020}}
@misc{undated,title={Forthcoming Work},author={{Example Research Group}},date={forthcoming}}
@misc{minimal,title={Research Data}}
"#.into(),
    }];
    let resolved = bibliography::resolve(&inputs).unwrap();
    let paper = publication_probe(resolved.get("paper").unwrap());
    assert_eq!(paper.key, "paper");
    assert_eq!(*paper.kind, EntryType::InProceedings);
    assert_eq!(paper.title.as_deref(), Some("A NASA Study"));
    let names = paper.authors.as_ref().unwrap();
    assert_eq!(
        (names[0].given_name.as_str(), names[0].name.as_str()),
        ("Alice", "Smith")
    );
    assert_eq!(
        (names[1].given_name.as_str(), names[1].name.as_str()),
        ("Bob", "García")
    );
    assert_eq!(
        names.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["Alice Smith", "Bob García"]
    );
    assert!(
        matches!(paper.date, Some(PermissiveType::Typed(biblatex::Date { value: DateValue::At(date), .. })) if date.year == 2023)
    );
    assert_eq!(paper.book_title.as_deref(), Some("Example Proceedings"));
    assert_eq!(paper.doi.as_deref(), Some("10.1000/example"));
    assert_eq!(paper.pdf.as_deref(), Some("papers/study.pdf"));
    assert_eq!(paper.code.as_deref(), Some("https://example.org/code"));
    let article = publication_probe(resolved.get("article").unwrap());
    assert_eq!(article.journal.as_deref(), Some("Example Journal"));
    assert!(
        matches!(article.date, Some(PermissiveType::Typed(biblatex::Date { value: DateValue::At(date), .. })) if date.year == 2022 && date.month == Some(6))
    );
    let chapter = publication_probe(resolved.get("chapter").unwrap());
    assert!(chapter.authors.is_none());
    assert_eq!(resolved.get("chapter").unwrap().editors().unwrap().len(), 1);
    let undated = publication_probe(resolved.get("undated").unwrap());
    assert!(matches!(undated.date, Some(PermissiveType::Chunks(_))));
    assert_eq!(
        undated.authors.as_ref().unwrap()[0].to_string(),
        "Example Research Group"
    );
    let minimal = publication_probe(resolved.get("minimal").unwrap());
    assert!(minimal.authors.is_none() && minimal.date.is_none() && minimal.doi.is_none());
}

#[test]
fn publication_names_and_date_ranges_follow_biblatex() {
    use biblatex::{ChunksExt, DateValue, PermissiveType};
    let inputs = [Source {
        path: "publications.bib".into(),
        text: "@book{work,author={van der Waals, Johannes Diderik and Doe, Jr, Jane and {Research Group}},date={2020/2022}}\n@book{forthcoming,date={forthcoming}}".into(),
    }];
    let resolved = bibliography::resolve(&inputs).unwrap();
    let work = publication_probe(resolved.get("work").unwrap());
    let names = work.authors.unwrap();
    assert_eq!(names[0].prefix, "van der");
    assert_eq!(names[0].name, "Waals");
    assert_eq!(names[1].suffix, "Jr");
    assert_eq!(names[2].to_string(), "Research Group");
    assert_eq!(names[0].to_string(), "Johannes Diderik van der Waals");
    assert_eq!(names[1].to_string(), "Jane Doe Jr");
    assert!(
        matches!(work.date, Some(PermissiveType::Typed(biblatex::Date { value: DateValue::Between(start, end), .. })) if start.year == 2020 && end.year == 2022)
    );
    assert_eq!(
        resolved
            .get("work")
            .unwrap()
            .get("date")
            .unwrap()
            .format_verbatim(),
        "2020/2022"
    );
    assert_eq!(
        resolved
            .get("forthcoming")
            .unwrap()
            .get("date")
            .unwrap()
            .format_verbatim(),
        "forthcoming"
    );
}

#[test]
fn publication_types_editors_and_publishers_use_library_values() {
    use biblatex::ChunksExt;
    let inputs = [Source {
        path: "publications.bib".into(),
        text: "@conference{work,editor={Smith, Alice},editora={Lee, Robin},editoratype={curator},publisher={First Press and Second Press}}\n@CustomKind{other,title={Example}}".into(),
    }];
    let resolved = bibliography::resolve(&inputs).unwrap();
    let work = resolved.get("work").unwrap();
    assert_eq!(work.entry_type.to_string(), "inproceedings");
    let editors = work.editors().unwrap();
    assert_eq!(editors[0].1.to_string(), "editor");
    assert_eq!(editors[1].1.to_string(), "curator");
    assert_eq!(editors[1].0[0].to_string(), "Robin Lee");
    assert_eq!(
        work.publisher()
            .unwrap()
            .iter()
            .map(|v| v.format_verbatim())
            .collect::<Vec<_>>(),
        ["First Press", "Second Press"]
    );
    assert!(
        matches!(&resolved.get("other").unwrap().entry_type, biblatex::EntryType::Unknown(value) if value == "customkind")
    );
}

#[test]
fn export_shaped_fields_and_latex_text_follow_biblatex() {
    use biblatex::ChunksExt;
    let inputs = [Source {
        path: "export.bib".into(),
        text: r#"@article{sample,
  author = {M{\"u}ller, Anne and others},
  title = {An {\'e}tude of {NASA} \& research~tools -- \textit{example}},
  year = {2024}, doi = {10.1000/example},
  url = {https://example.org/article}, urldate = {2024-05-03},
  abstract = {An example abstract.}, keywords = {science, history},
  file = {Full Text:/home/researcher/paper.pdf:application/pdf},
  langid = {english}
}"#
        .into(),
    }];
    let resolved = bibliography::resolve(&inputs).unwrap();
    let entry = resolved.get("sample").unwrap();
    assert_eq!(
        entry.get("title").unwrap().format_verbatim(),
        "An étude of NASA & research~tools – \\textit{example}"
    );
    assert_eq!(
        entry
            .author()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["Anne Müller", "others"]
    );
    for (field, expected) in [
        ("abstract", "An example abstract."),
        ("keywords", "science, history"),
        (
            "file",
            "Full Text:/home/researcher/paper.pdf:application/pdf",
        ),
        ("langid", "english"),
        ("urldate", "2024-05-03"),
    ] {
        assert_eq!(entry.get(field).unwrap().format_verbatim(), expected);
    }
}

#[test]
fn publication_fields_survive_resolution_and_expose_conversion_boundaries() {
    let inputs = [Source {
        path: "publications.bib".into(),
        text: r#"
@string{venue = {Proceedings of Example Research}}
@proceedings{conference, title=venue, year={2023}}
@inproceedings{paper, author={Smith, Alice and García, Bob}, title={A Study}, crossref={conference}, doi={10.1000/example}, pdf={papers/study.pdf}, code={https://example.org/code}, selected={true}}
@article{article, author={Lee, Robin}, title={A Result}, journal={Example Journal}, year={2022}, volume={12}, number={3}, pages={1--12}, url={https://example.org/article}}
@book{book, author={Chen, Mei}, title={Example Book}, year={2021}, publisher={Example Press}, isbn={9780000000000}}
@incollection{chapter, author={Patel, Asha}, title={A Chapter}, booktitle={Collected Research}, year={2020}, editor={Jones, Morgan}, pages={21--30}}
@misc{preprint, author={Müller, Alex}, title={A Preprint}, year={2024}, eprinttype={arxiv}, eprint={2401.12345}}
@misc{dataset, title={Research Data}, year={2019}, doi={10.1000/data}, code={https://example.org/data}}
"#.into(),
    }];
    let resolved = bibliography::resolve(&inputs).unwrap();
    let paper = resolved.get("paper").unwrap();
    for (field, expected) in [
        ("pdf", "papers/study.pdf"),
        ("code", "https://example.org/code"),
        ("selected", "true"),
        ("date", "2023"),
        ("booktitle", "Proceedings of Example Research"),
    ] {
        assert_eq!(paper.get_as::<String>(field).unwrap(), expected);
    }
    assert_eq!(paper.get("date").unwrap()[0].span.start, usize::MAX);
    let converted = hayagriva::io::from_biblatex(&resolved).unwrap();
    let entry = converted.get("paper").unwrap();
    assert_eq!(entry.doi(), Some("10.1000/example"));
    assert!(entry.title().is_some());
    assert_eq!(entry.authors().unwrap().len(), 2);
    assert!(!entry.has("code"));
    assert!(!entry.has("pdf"));
    assert!(!entry.has("selected"));
    assert!(converted.get("article").unwrap().url_any().is_some());
    assert!(converted.get("book").unwrap().title().is_some());
    assert!(converted.get("chapter").unwrap().title().is_some());
    assert!(converted.get("preprint").unwrap().title().is_some());
    assert!(converted.get("dataset").unwrap().title().is_some());
}
