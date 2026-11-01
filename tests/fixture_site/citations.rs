use super::*;

/// Page and directory settings both reach the published output.
#[test]
fn citation_settings_render_citations_and_bibliography() {
    for directory_setting in [false, true] {
        let fixture = Fixture::new();
        fixture.write(
            "content/en/index.page",
            "template='page.tera'\ncontent.main='index.md'\n",
        );
        fixture.write("content/en/index.md", "Previously published text.\n");
        fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
        build_at(fixture.path(), BuildMode::Full).expect("initial site builds");
        let published =
            fs::read_to_string(fixture.path().join("public/index.html")).expect("initial output");

        fixture.write("content/en/index.md", "New text [@work].\n");
        fixture.write(
            "citations/works.bib",
            "@book{work, title={Example}, author={Smith, Alice}, year={2024}}\n",
        );
        fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
        if directory_setting {
            fixture.write(
                "content/en/_directory.toml",
                "[pages.citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n",
            );
        } else {
            fixture.write(
                "content/en/index.page",
                "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n",
            );
        }
        fixture.write(
            "templates/page.tera",
            "{{ markdown(content='main') }}{{ bibliography() }}",
        );
        build_at(fixture.path(), BuildMode::Full).expect("citation build succeeds");
        let output =
            fs::read_to_string(fixture.path().join("public/index.html")).expect("citation output");
        assert!(
            output.contains(
                "New text <a href=\"#__heine-bibliography\" role=\"doc-biblioref\">Example</a>."
            ),
            "{output}"
        );
        assert!(output.contains("csl-bibliography"), "{output}");
        assert_ne!(output, published);
    }
}

#[test]
fn page_can_disable_inherited_citation_settings() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages.citations]\ncontent='main'\nbibliography=['absent.bib']\nstyle='absent.csl'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\ncitations=false\n",
    );
    fixture.write("content/en/index.md", "Literal [@work].\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    build_at(fixture.path(), BuildMode::Full)
        .expect("disabled inherited setting does not load inputs");
}

#[test]
fn missing_citation_input_reports_its_inherited_declaration() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages.citations]\ncontent='main'\nbibliography=['missing.bib']\nstyle='style.csl'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\n",
    );
    fixture.write("content/en/index.md", "[@work]\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    fixture.write("citations/placeholder", "");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing input must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("missing.bib"), "{diagnostic}");
    assert!(diagnostic.contains("_directory.toml"), "{diagnostic}");
}

#[test]
fn citation_keys_are_checked_in_the_selected_markdown_source() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n");
    fixture.write("content/en/index.md", "Before [@missing] after.\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    fixture.write("citations/works.bib", "@book{work, title={Example}}\n");
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown citation must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("unknown citation key"), "{diagnostic}");
    assert!(diagnostic.contains("index.md"), "{diagnostic}");
    assert!(diagnostic.contains("missing"), "{diagnostic}");
}

fn configured_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n");
    fixture.write("content/en/index.md", "Text [@work].\n");
    fixture.write("citations/works.bib", "@book{work, title={Example}}\n");
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture
}

#[test]
fn bibliography_and_selected_source_must_each_be_placed_once() {
    for template in [
        "{{ markdown(content='main') }}",
        "{{ markdown(content='main') }}{{ bibliography() }}{{ bibliography() }}",
        "{{ bibliography() }}",
        "{% set discarded = bibliography() %}{{ markdown(content='main') }}",
        "{{ markdown(content='main') }}{% set list = bibliography() %}{{ list }}{{ list }}",
        "{{ markdown(content='main') }}{{ bibliography() | upper }}",
    ] {
        let fixture = configured_fixture();
        fixture.write("templates/page.tera", template);
        let error =
            build_at(fixture.path(), BuildMode::Full).expect_err("invalid placement must fail");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("bibliography") || diagnostic.contains("citation"),
            "{diagnostic}"
        );
    }
}

#[test]
fn citations_render_with_a_markdown_component() {
    let fixture = configured_fixture();
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nInside [@work].\n:::\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}<aside>{{ body }}</aside>{% endcomponent %}",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("component citation builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<aside><p>Inside <a href=\"#__heine-bibliography\""),
        "{output}"
    );
}

#[test]
fn citations_render_in_an_ordinary_footnote() {
    let fixture = configured_fixture();
    fixture.write("heine.toml", "[markdown]\nextensions=['footnotes']\n");
    fixture.write("content/en/index.md", "A note.[^n]\n\n[^n]: See [@work].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("footnote citation builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("See <a href=\"#__heine-bibliography\""),
        "{output}"
    );
}

#[test]
fn citations_render_in_repeated_margin_notes() {
    let fixture = configured_fixture();
    fixture.write("heine.toml", "[markdown]\nextensions=['footnotes']\n[markdown.margin_footnotes]\nmarker_message='footnote-marker'\n");
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n[footnotes]\ncontent='main'\n");
    fixture.write(
        "content/en/index.md",
        "A note.[^n] Again.[^n]\n\n[^n]: See [@work].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("margin citation builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert_eq!(
        output
            .matches("See <a href=\"#__heine-bibliography\"")
            .count(),
        2,
        "{output}"
    );
}

#[test]
fn unsupported_position_conditions_report_an_error() {
    let fixture = configured_fixture();
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><choose><if position="ibid"><text value="ibid"/></if><else><text variable="title"/></else></choose></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("unsupported condition must fail");
    assert!(format!("{error:?}").contains("ibid"), "{error:?}");
}

#[test]
fn citation_sources_require_a_page_template() {
    let fixture = configured_fixture();
    fixture.write("content/en/index.page", "content.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template is required");
    assert!(format!("{error:?}").contains("page template"), "{error:?}");
}

#[test]
fn invalid_bibliography_url_is_not_silently_discarded() {
    let fixture = configured_fixture();
    fixture.write(
        "citations/works.bib",
        "@book{work,title={Example},url={not a url}}\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid URL must fail");
    let diagnostic = format!("{error:?}");
    assert!(
        error.to_string().contains("invalid bibliography URL"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("works.bib"), "{diagnostic}");
}

#[test]
fn escaped_bibliography_url_explains_the_literal_url_rule() {
    let fixture = configured_fixture();
    fixture.write(
        "citations/works.bib",
        "@book{work,title={Example},url={https://example.org/?a=1\\&b=2}}\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("escaped URL must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("works.bib"), "{diagnostic}");
    assert!(diagnostic.contains("URL characters"), "{diagnostic}");
    assert!(diagnostic.contains("literally"), "{diagnostic}");
}

#[test]
fn literal_bibliography_url_keeps_query_and_percent_encoding() {
    let fixture = configured_fixture();
    fixture.write(
        "citations/works.bib",
        "@book{work,title={Example},url={https://example.org/a%2D%2Db?a=1&b=2}}\n",
    );
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="URL"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("literal URL builds");
    let html = std::fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        html.contains("https://example.org/a%2D%2Db?a=1&amp;b=2"),
        "{html}"
    );
}

#[test]
fn bibliography_urls_with_consecutive_hyphens_are_not_silently_changed() {
    for bibliography in [
        "@book{work,title={Example},url={https://example.org/a--b}}\n",
        "@string{address = {https://example.org/a--b}}\n@book{work,title={Example},url=address}\n",
        "@xdata{parent,url={https://example.org/a--b}}\n@book{work,title={Example},xdata={parent}}\n",
    ] {
        let fixture = configured_fixture();
        fixture.write("citations/works.bib", bibliography);
        let error = build_at(fixture.path(), BuildMode::Full).expect_err("changed URL must fail");
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains("consecutive hyphens"), "{diagnostic}");
        assert!(diagnostic.contains("works.bib"), "{diagnostic}");
    }
}

#[test]
fn bibliography_aliases_resolve_to_the_same_cited_work() {
    let fixture = configured_fixture();
    fixture.write(
        "citations/works.bib",
        "@book{work,ids={alias},title={Example}}\n",
    );
    fixture.write("content/en/index.md", "First [@alias]. Again [@work].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("alias builds");
    let html = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert_eq!(html.matches("role=\"doc-biblioref\"").count(), 2, "{html}");
    assert_eq!(html.matches("class=\"csl-entry\"").count(), 1, "{html}");
}

#[test]
fn a_style_that_can_strip_locator_periods_is_rejected_for_dotted_locators() {
    let fixture = configured_fixture();
    fixture.write("content/en/index.md", "Text [@work, p. 2.3].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="locator" strip-periods="true"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("locator must not be changed silently");
    assert!(format!("{error:?}").contains("strip periods"), "{error:?}");
}

#[test]
fn numeric_groups_with_locators_expand_without_changing_other_citations() {
    let fixture = configured_fixture();
    fixture.write(
        "content/en/index.md",
        "First [@a]. Group [@a; @b, p. 5; @c]. Last [@c].\n",
    );
    fixture.write(
        "citations/works.bib",
        "@book{a,title={A}}\n@book{b,title={B}}\n@book{c,title={C}}\n",
    );
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Numbers</title><id>https://example.org/style</id></info><citation collapse="citation-number"><sort><key variable="citation-number"/></sort><layout prefix="[" suffix="]" delimiter=", "><group delimiter=":"><text variable="citation-number"/><text variable="locator"/></group></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("grouped locator builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(output.contains("[1, 2:5, 3]"), "{output}");
    assert!(output.contains(">[1]</a>"), "{output}");
    assert!(output.contains(">[3]</a>"), "{output}");
}

#[test]
fn duplicate_keys_report_both_bibliography_files() {
    let fixture = configured_fixture();
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib', 'other.bib']\nstyle='style.csl'\n");
    fixture.write("citations/other.bib", "@book{work,title={Duplicate}}\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate keys must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("works.bib"), "{diagnostic}");
    assert!(diagnostic.contains("other.bib"), "{diagnostic}");
    assert!(diagnostic.contains("first declaration"), "{diagnostic}");
}

#[test]
fn citations_and_renderer_directives_share_a_source() {
    let fixture = configured_fixture();
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    fixture.write("heine.toml", &format!("[renderers.copy]\nprotocol='stream'\nprogram='{program}'\ntimeout_seconds=10\ncache_identity='copy-1'\ninputs=['body']\nresult='html'\n[renderers.copy.block]\narguments={arguments}\n"));
    fixture.write(
        "content/en/index.md",
        "Before [@work].\n\n:::render[copy]\n<span>generated</span>\n:::\n\nMiddle [@work].\n\n:::render[copy]\n<span>second</span>\n:::\n\nAfter [@work].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("citations and renderer build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert_eq!(
        output.matches("href=\"#__heine-bibliography\"").count(),
        3,
        "{output}"
    );
    assert!(output.contains("<span>generated</span>"), "{output}");
    assert!(output.contains("<span>second</span>"), "{output}");
}

#[test]
fn citations_and_renderer_directives_work_with_margin_footnotes() {
    let fixture = configured_fixture();
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    fixture.write("heine.toml", &format!("[markdown]\nextensions=['footnotes']\n[markdown.margin_footnotes]\nmarker_message='footnote-marker'\n[renderers.copy]\nprotocol='stream'\nprogram='{program}'\ntimeout_seconds=10\ncache_identity='copy-1'\ninputs=['body']\nresult='html'\n[renderers.copy.block]\narguments={arguments}\n"));
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n[footnotes]\ncontent='main'\n");
    fixture.write("content/en/index.md", ":::render[copy]\n<span>generated</span>\n:::\n\nAfter [@work].[^n]\n\n:::render[copy]\n<span>second</span>\n:::\n\n[^n]: Note [@work].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("renderer, citations and margin notes build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert_eq!(
        output.matches("href=\"#__heine-bibliography\"").count(),
        2,
        "{output}"
    );
    assert!(output.contains("<span>generated</span>"), "{output}");
    assert!(output.contains("<span>second</span>"), "{output}");
}

#[test]
fn citations_and_renderer_directives_work_with_components() {
    let fixture = configured_fixture();
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    fixture.write("heine.toml", &format!("[renderers.copy]\nprotocol='stream'\nprogram='{program}'\ntimeout_seconds=10\ncache_identity='copy-1'\ninputs=['body']\nresult='html'\n[renderers.copy.block]\narguments={arguments}\n"));
    fixture.write("content/en/index.md", ":::render[copy]\n<span>generated</span>\n:::\n\n:::component[site.note]\nInside [@work].\n:::\n");
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}<aside>{{ body }}</aside>{% endcomponent %}",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("renderer, citations and component build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<aside><p>Inside <a href=\"#__heine-bibliography\""),
        "{output}"
    );
    assert!(output.contains("<span>generated</span>"), "{output}");
}

#[test]
fn heading_citations_match_the_table_of_contents_and_anchors() {
    let fixture = configured_fixture();
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n[toc]\ncontent='main'\n");
    fixture.write(
        "content/en/index.md",
        "## Reading [@work]\n\nText.\n\n### More [@work]\n",
    );
    fixture.write("templates/page.tera", "{% for h in page.toc.headings %}[{{ h.fragment }}:{{ h.text }}]{% endfor %}{{ markdown(content='main') }}{{ bibliography() }}");
    build_at(fixture.path(), BuildMode::Full).expect("heading citations build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("[reading-work:Reading Example]"),
        "{output}"
    );
    assert!(output.contains("[more-work:More Example]"), "{output}");
    assert!(
        output.contains("<h2 id=\"reading-work\">Reading <a href=\"#__heine-bibliography\""),
        "{output}"
    );
    assert!(
        output.contains("<h3 id=\"more-work\">More <a href=\"#__heine-bibliography\""),
        "{output}"
    );
}

#[test]
fn citations_keep_heading_component_and_margin_plans_after_renderer_output() {
    let fixture = configured_fixture();
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    fixture.write("heine.toml", &format!("[markdown]\nextensions=['footnotes']\n[markdown.margin_footnotes]\nmarker_message='footnote-marker'\n[renderers.copy]\nprotocol='stream'\nprogram='{program}'\ntimeout_seconds=10\ncache_identity='copy-1'\ninputs=['body']\nresult='html'\n[renderers.copy.block]\narguments={arguments}\n"));
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n[footnotes]\ncontent='main'\n[toc]\ncontent='main'\n");
    fixture.write("content/en/index.md", ":::render[copy]\n<span>generated</span>\n:::\n\n## Heading [@work]\n\n:::component[site.note]\nText [@work].[^n]\n:::\n\n[^n]: Note [@work].\n");
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}<aside>{{ body }}</aside>{% endcomponent %}",
    );
    fixture.write("templates/page.tera", "{% for h in page.toc.headings %}[{{ h.fragment }}:{{ h.text }}]{% endfor %}{{ markdown(content='main') }}{{ bibliography() }}");
    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("combined citation features build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("[heading-work:Heading Example]"),
        "{output}"
    );
    assert!(output.contains("<span>generated</span>"), "{output}");
    assert!(output.contains("<aside>"), "{output}");
    assert_eq!(
        output.matches("href=\"#__heine-bibliography\"").count(),
        3,
        "{output}"
    );
}

#[test]
fn unmatched_bracket_does_not_hide_a_later_citation() {
    let fixture = configured_fixture();
    fixture.write(
        "content/en/index.md",
        "[ordinary prose\n\nSecond [@work].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("later citation builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("Second <a href=\"#__heine-bibliography\""),
        "{output}"
    );
    assert!(
        output.contains("<span class=\"csl-content\">Example</span>"),
        "{output}"
    );
}

#[test]
fn inline_html_contents_remain_literal_without_nested_links() {
    let fixture = configured_fixture();
    fixture.write(
        "content/en/index.md",
        "<a href=\"https://example.org\"><strong>[@missing]</strong></a> After<br> [@work].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("raw HTML stays literal");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<a href=\"https://example.org\"><strong>[@missing]</strong></a>"),
        "{output}"
    );
    assert_eq!(
        output.matches("href=\"#__heine-bibliography\"").count(),
        1,
        "{output}"
    );
}

#[test]
fn renderer_body_is_not_a_citation_source() {
    let fixture = configured_fixture();
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    fixture.write("heine.toml", &format!("[renderers.copy]\nprotocol='stream'\nprogram='{program}'\ntimeout_seconds=10\ncache_identity='copy-1'\ninputs=['body']\nresult='html'\n[renderers.copy.block]\narguments={arguments}\n"));
    fixture.write(
        "content/en/index.md",
        ":::render[copy]\n<span>[@missing]</span>\n:::\n\nActual [@work].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("renderer body stays literal");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(output.contains("<span>[@missing]</span>"), "{output}");
    assert_eq!(
        output.matches("href=\"#__heine-bibliography\"").count(),
        1,
        "{output}"
    );
}

#[test]
fn every_page_checks_and_canonicalizes_its_citation_locale() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text term="and"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\nlocale='de-DE'\n");
    fixture.write("content/en/z.page", "template='page.tera'\ncontent.main='z.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\nlocale='not a locale'\n");
    fixture.write("content/en/z.md", "Text [@work].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("cached inputs must not bypass locale checks");
    assert!(
        format!("{error:?}").contains("invalid citation locale"),
        "{error:?}"
    );
    fixture.write("content/en/z.page", "template='page.tera'\ncontent.main='z.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\nlocale='DE-de'\n");
    build_at(fixture.path(), BuildMode::Full).expect("valid locale spellings build");
    for output in ["index.html", "z.html"] {
        let html = fs::read_to_string(fixture.path().join("public").join(output)).unwrap();
        assert!(html.contains(">und</a>"), "{output}: {html}");
    }
}

#[test]
fn invalid_style_points_to_its_selection() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", "<style>broken</style>");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid style must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("style='style.csl'"), "{diagnostic}");
    assert!(diagnostic.contains("index.page:6"), "{diagnostic}");
}

#[test]
fn locator_error_points_to_the_affected_citation() {
    let fixture = configured_fixture();
    fixture.write(
        "content/en/index.md",
        "Earlier [@work].\n\nSecond [@work, p. 2.3].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="locator" strip-periods="true"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("dotted locator must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("index.md:3"), "{diagnostic}");
    assert!(diagnostic.contains("2.3"), "{diagnostic}");
}

#[test]
fn unused_style_macros_do_not_reject_safe_citations() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="unused"><choose><if position="ibid"><text value="ibid"/></if></choose><text variable="title" strip-periods="true"/></macro><citation><layout><text variable="locator"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write("content/en/index.md", "Text [@work, p. 2.3].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("unused macro must not affect citation");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(output.contains(">2.3</a>"), "{output}");
}

#[test]
fn reachable_nested_position_condition_is_rejected_at_style_selection() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="outer"><text macro="inner"/></macro><macro name="inner"><choose><if position="ibid"><text value="ibid"/></if></choose></macro><citation><layout><text macro="outer"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("reachable ibid must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("ibid"), "{diagnostic}");
    assert!(diagnostic.contains("index.page:6"), "{diagnostic}");
    assert_eq!(
        diagnostic.matches("unsupported position condition").count(),
        1,
        "{diagnostic}"
    );
}

#[test]
fn recursive_citation_macros_are_rejected() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="a"><text macro="b"/></macro><macro name="b"><text macro="a"/></macro><citation><layout><text macro="a"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("recursive macros must fail");
    assert!(format!("{error:?}").contains("cycle"), "{error:?}");
}

#[test]
fn duplicate_csl_macro_names_are_rejected_before_rendering() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="same"><text macro="same"/></macro><macro name="same"><text variable="title"/></macro><citation><layout><text macro="same"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate macros must fail");
    assert!(
        format!("{error:?}").contains("duplicate CSL macro"),
        "{error:?}"
    );
}

#[test]
fn bibliography_only_position_conditions_do_not_reject_in_text_citations() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="bib"><choose><if position="ibid"><text value="ibid"/></if><else><text variable="title"/></else></choose></macro><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text macro="bib"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full)
        .expect("bibliography-only condition does not affect citations");
}

#[test]
fn recursive_bibliography_macros_are_rejected() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="a"><text macro="b"/></macro><macro name="b"><text macro="a"/></macro><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text macro="a"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("recursive bibliography macros must fail");
    assert!(format!("{error:?}").contains("cycle"), "{error:?}");
}

#[test]
fn representative_author_date_and_numeric_styles_render() {
    use hayagriva::archive::ArchivedStyle;
    for (style, expected) in [
        (ArchivedStyle::AmericanPsychologicalAssociation, "Smith"),
        (
            ArchivedStyle::InstituteOfElectricalAndElectronicsEngineers,
            "1",
        ),
    ] {
        let fixture = configured_fixture();
        fixture.write(
            "citations/style.csl",
            &style.get().to_xml().expect("bundled style serializes"),
        );
        fixture.write("citations/works.bib", "@article{work, author={Smith, Alice}, title={An Example}, journal={Journal of Examples}, year={2024}, volume={1}, pages={10--20}, doi={10.1234/example}}\n");
        fixture.write(
            "templates/page.tera",
            "{{ markdown(content='main') }}{{ bibliography() }}",
        );
        build_at(fixture.path(), BuildMode::Full).expect("representative style builds");
        let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
        assert!(output.contains(expected), "{style:?}: {output}");
        assert!(
            output.contains("An Example") || output.contains("An example"),
            "{style:?}: {output}"
        );
    }
}

#[test]
fn unrelated_reachable_period_stripping_does_not_reject_a_locator() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout delimiter=", "><text variable="title" strip-periods="true"/><text variable="locator"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write("content/en/index.md", "Text [@work, p. 2.3].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full)
        .expect("unrelated title stripping must not affect the locator");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(output.contains("2.3</a>"), "{output}");
}

#[test]
fn a_stripping_macro_wrapper_around_a_locator_is_rejected() {
    let fixture = configured_fixture();
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><macro name="locator"><number variable="locator"/></macro><citation><layout><text macro="locator" strip-periods="true"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#);
    fixture.write("content/en/index.md", "Text [@work, p. 2.3].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("a stripping locator wrapper must fail");
    assert!(format!("{error:?}").contains("strip periods"), "{error:?}");
}

#[test]
fn unmatched_bracket_in_the_same_paragraph_does_not_hide_a_citation() {
    let fixture = configured_fixture();
    fixture.write("content/en/index.md", "[ordinary text before [@work].\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("citation after unmatched bracket builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("before <a href=\"#__heine-bibliography\""),
        "{output}"
    );
}
