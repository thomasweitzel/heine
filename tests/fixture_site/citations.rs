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
fn citation_and_markdown_state_is_isolated_between_pages() {
    let fixture = configured_fixture();
    fixture.write(
        "citations/works.bib",
        "@book{work, title={First Work}}\n@book{other, title={Second Work}}\n",
    );
    fixture.write("content/en/index.md", "First page [@work].\n");
    fixture.write("content/en/second.page", "template='page.tera'\ncontent.main='second.md'\n[citations]\ncontent='main'\nbibliography=['works.bib']\nstyle='style.csl'\n");
    fixture.write("content/en/second.md", "Second page [@other].\n");
    fixture.write(
        "content/en/plain.page",
        "template='plain.tera'\ncontent.main='plain.md'\n",
    );
    fixture.write("content/en/plain.md", "Plain page.\n");
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    fixture.write("templates/plain.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("all pages build");
    let first = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    let second = fs::read_to_string(fixture.path().join("public/second.html")).unwrap();
    let plain = fs::read_to_string(fixture.path().join("public/plain.html")).unwrap();
    let first_bibliography = first
        .split_once("csl-bibliography")
        .expect("first bibliography")
        .1;
    let second_bibliography = second
        .split_once("csl-bibliography")
        .expect("second bibliography")
        .1;
    assert!(
        first.contains("First page") && first.contains("First Work"),
        "{first}"
    );
    assert!(
        !first.contains("Second Work") && !first.contains("Plain page"),
        "{first}"
    );
    assert!(
        second.contains("Second page") && second.contains("Second Work"),
        "{second}"
    );
    assert!(
        !second.contains("First Work") && !second.contains("Plain page"),
        "{second}"
    );
    assert!(first_bibliography.contains("First Work"), "{first}");
    assert!(!first_bibliography.contains("Second Work"), "{first}");
    assert!(second_bibliography.contains("Second Work"), "{second}");
    assert!(!second_bibliography.contains("First Work"), "{second}");
    assert!(plain.contains("Plain page"), "{plain}");
    assert!(
        !plain.contains("csl-bibliography")
            && !plain.contains("First Work")
            && !plain.contains("Second Work"),
        "{plain}"
    );
}

#[test]
fn publication_view_example_renders_partial_metadata() {
    let fixture = configured_fixture();
    fixture.write("data/publications.json", r#"{"items":[{"key":"paper","title":"A Study","authors":["Alice Smith","Bob García"],"editors":[],"date":{"text":"2022-07"},"journal":"Example Journal","book_title":null},{"key":"chapter","title":"A Chapter","authors":[],"editors":[{"role":"editor","names":["Morgan Jones"]}],"date":null,"journal":null,"book_title":"Collected Research"},{"key":"minimal","title":null,"authors":[],"editors":[],"date":null,"journal":null,"book_title":null}]}"#);
    fixture.write("templates/page.tera", r#"{{ markdown(content='main') }}{{ bibliography() }}{% for work in data.publications.items %}<p>{% if work.title %}{{ work.title }}{% else %}{{ work.key }}{% endif %}{% if work.authors %}: {{ work.authors | join(sep=', ') }}{% elif work.editors %}{% for group in work.editors %}: {{ group.names | join(sep=', ') }}{% endfor %}{% endif %}{% if work.date %} ({{ work.date.text }}){% endif %}{% if work.journal %}, {{ work.journal }}{% elif work.book_title %}, {{ work.book_title }}{% endif %}</p>{% endfor %}"#);
    build_at(fixture.path(), BuildMode::Full).expect("publication view example builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<p>A Study: Alice Smith, Bob García (2022-07), Example Journal</p>"),
        "{output}"
    );
    assert!(
        output.contains("<p>A Chapter: Morgan Jones, Collected Research</p>"),
        "{output}"
    );
    assert!(output.contains("<p>minimal</p>"), "{output}");
}

#[test]
fn publication_catalog_renders_without_page_citations() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[publications]\nbibliography=['works.bib']\nextra_fields=['code']\n",
    );
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\n",
    );
    fixture.write("content/en/index.md", "Publications.\n");
    fixture.write("citations/works.bib", "@article{paper,ids={alias},title={Example},author={Smith, Alice},year={2024},code={https://example.org/code}}\n@book{other,title={Other}}\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}{% for work in publications() %}{{ work.key }}:{{ work.title }}|{% endfor %}{% set selected = publication(key='alias') %}{{ selected.key }}:{{ selected.authors | join(sep=', ') }}:{{ selected.year }}:{{ selected.extra.code }}");
    build_at(fixture.path(), BuildMode::Full).expect("catalog builds independently of citations");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(output.contains("paper:Example|other:Other|"), "{output}");
    assert!(
        output.contains("paper:Alice Smith:2024:https://example.org/code"),
        "{output}"
    );
    fixture.write("templates/page.tera", "{{ publication(key='missing') }}");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown key must fail");
    assert!(format!("{error:?}").contains("missing"));
}

#[test]
fn publication_catalog_reports_configuration_and_input_errors() {
    for (config, expected) in [
        ("[publications]\nbibliography=[]\n", "at least one"),
        (
            "[publications]\nbibliography=['works.bib']\nextra_fields=['Code']\n",
            "lowercase",
        ),
        (
            "[publications]\nbibliography=['works.bib']\nextra_fields=['code','code']\n",
            "unique",
        ),
        (
            "[publications]\nbibliography=['missing.bib']\n",
            "missing.bib",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.write("heine.toml", config);
        fixture.write(
            "content/en/index.page",
            "template='page.tera'\ncontent.main='index.md'\n",
        );
        fixture.write("content/en/index.md", "Hello.\n");
        fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
        let error =
            build_at(fixture.path(), BuildMode::Full).expect_err("invalid configuration must fail");
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains(expected), "{diagnostic}");
        assert!(diagnostic.contains("heine.toml"), "{diagnostic}");
        if expected == "missing.bib" {
            assert!(diagnostic.contains("read citation input"), "{diagnostic}");
        }
    }
}

#[test]
fn publication_functions_require_configuration_and_check_arguments() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\n",
    );
    fixture.write("content/en/index.md", "Hello.\n");
    for template in ["{{ publications() }}", "{{ publication(key='work') }}"] {
        fixture.write("templates/page.tera", template);
        let error =
            build_at(fixture.path(), BuildMode::Full).expect_err("missing catalog must fail");
        assert!(format!("{error:?}").contains("configure [publications]"));
    }
    fixture.write("templates/page.tera", "{{ publications(unknown=true) }}");
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("unexpected argument must fail");
    assert!(format!("{error:?}").contains("unknown"));
}

#[test]
fn publication_catalog_maps_partial_fields_and_sorts_years() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[publications]\nbibliography=['works.bib']\nextra_fields=['abstract']\n",
    );
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\n",
    );
    fixture.write("content/en/index.md", "Publications.\n");
    fixture.write("citations/works.bib", "@custom{range,author={van der Waals, Johannes and {Research Group}},date={2020/2022},abstract={A study}}\n@book{later,editor={Smith, Alice},year={2024},publisher={First and Second}}\n@book{future,date={forthcoming}}\n");
    fixture.write("templates/page.tera", "{% for work in publications() | sort(attribute='year') | reverse %}{{ work.key }}={{ work.type }}{% if work.year is none %}:none{% else %}:{{ work.year }}{% endif %}{% if work.date %}:{{ work.date.text }}{% endif %}{% if work.authors %}:{{ work.authors | join(sep=',') }}{% endif %}{% for group in work.editors %}:{{ group.role }}={{ group.names | join(sep=',') }}{% endfor %}{% if work.publisher %}:{{ work.publisher | join(sep=',') }}{% endif %}{% if work.extra.abstract is defined %}:{{ work.extra.abstract }}{% endif %}|{% endfor %}");
    build_at(fixture.path(), BuildMode::Full).expect("mapped works build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output
            .contains("range=custom:none:2020/2022:Johannes van der Waals,Research Group:A study|"),
        "{output}"
    );
    assert!(
        output.contains("later=book:2024:2024:editor=Alice Smith:First,Second|"),
        "{output}"
    );
    assert!(output.contains("future=book:none:forthcoming|"), "{output}");
}

#[test]
fn publication_catalog_reports_invalid_legacy_year_at_source() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[publications]\nbibliography=['works.bib']\n");
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\n",
    );
    fixture.write("content/en/index.md", "Publications.\n");
    fixture.write(
        "citations/works.bib",
        "@book{work,\n title={Example},\n year={forthcoming}\n}\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% set work = publication(key='work') %}{{ work.title }}",
    );
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("invalid legacy year must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("works.bib"), "{diagnostic}");
    assert!(
        diagnostic.contains("invalid bibliography field"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("page.tera"), "{diagnostic}");
}

#[test]
fn publication_mapping_error_in_markdown_component_reports_all_sources() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[publications]\nbibliography=['works.bib']\n");
    fixture.write(
        "content/en/index.page",
        "template='page.tera'\ncontent.main='index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nA body.\n:::\n",
    );
    fixture.write(
        "citations/works.bib",
        "@book{work,\n title={Example},\n year={forthcoming}\n}\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    fixture.write("templates/components.tera", "{% component site.note() %}{% set work=publication(key='work') %}{{ work.title }}{{ body }}{% endcomponent site.note %}");
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("invalid legacy year must fail");
    let diagnostic = format!("{error:?}");
    for expected in [
        "components.tera",
        "index.md",
        "works.bib",
        "invalid bibliography field",
    ] {
        assert!(diagnostic.contains(expected), "{diagnostic}");
    }
}

#[test]
fn publication_link_shapes_resolve_only_assets_rendered_by_the_template() {
    for typed in [false, true] {
        let fixture = configured_fixture();
        fixture.write("locales/en/site.ftl", "pdf = PDF\ncode = Code\n");
        fixture.write("content/assets/paper.pdf", "paper");
        let data = if typed {
            r#"
[[works]]
title = "Visible"
selected = true
links = [{ kind = "asset", target = "assets/paper.pdf", label = "pdf" }, { kind = "external", target = "https://example.org/code", label = "code" }]

[[works]]
title = "Hidden"
selected = false
links = [{ kind = "asset", target = "assets/missing.pdf", label = "pdf" }]
"#
        } else {
            r#"
[[works]]
title = "Visible"
selected = true
pdf = "assets/paper.pdf"
code = "https://example.org/code"

[[works]]
title = "Hidden"
selected = false
pdf = "assets/missing.pdf"
code = "https://example.org/other"
"#
        };
        fixture.write("data/publications.toml", data);
        let links = if typed {
            r#"{% for link in work.links %}{% if link.kind == 'asset' %}{% set item = asset(id=link.target) %}<a href="{{ item.url }}">{{ t(id=link.label) }}</a>{% elif link.kind == 'external' %}<a href="{{ link.target }}">{{ t(id=link.label) }}</a>{% endif %}{% endfor %}"#
        } else {
            r#"{% set pdf = asset(id=work.pdf) %}<a href="{{ pdf.url }}">{{ t(id='pdf') }}</a><a href="{{ work.code }}">{{ t(id='code') }}</a>"#
        };
        let template = format!(
            "{{{{ markdown(content='main') }}}}{{{{ bibliography() }}}}{{% for work in data.publications.works %}}{{% if work.selected %}}<p>{{{{ work.title }}}}{links}</p>{{% endif %}}{{% endfor %}}"
        );
        fixture.write("templates/page.tera", &template);
        build_at(fixture.path(), BuildMode::Full).expect("only selected work is rendered");
        let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
        assert!(output.contains("assets/paper.pdf"), "{output}");
        assert!(output.contains("https://example.org/code"), "{output}");
        assert!(!output.contains("assets/missing.pdf"), "{output}");

        fixture.write(
            "data/publications.toml",
            &data.replace("https://example.org/code", "not a URL"),
        );
        build_at(fixture.path(), BuildMode::Full).expect("template external links are not checked");
        let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
        assert!(output.contains("href=\"not a URL\""), "{output}");

        if typed {
            fixture.write(
                "data/publications.toml",
                &data.replace("kind = \"external\"", "kind = \"unknown\""),
            );
            build_at(fixture.path(), BuildMode::Full)
                .expect("generic data has no link-kind schema");
            let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
            assert!(!output.contains("https://example.org/code"), "{output}");
        }
        fixture.write(
            "data/publications.toml",
            &data.replace("selected = false", "selected = true"),
        );
        let error =
            build_at(fixture.path(), BuildMode::Full).expect_err("rendered missing asset fails");
        assert!(format!("{error:?}").contains("assets/missing.pdf"));
    }
}

#[test]
fn publication_links_in_site_data_are_not_validated_as_urls() {
    let fixture = configured_fixture();
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}{{ bibliography() }}<a href=\"{{ data.publications.code }}\">Code</a>");
    for value in ["not a URL", "javascript:alert(1)"] {
        fixture.write("data/publications.toml", &format!("code='{value}'\n"));
        build_at(fixture.path(), BuildMode::Full).expect("a template link is rendered as authored");
        let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
        assert!(output.contains(&format!("href=\"{value}\"")), "{output}");
    }
}

#[test]
fn publication_data_can_be_sorted_by_year_in_a_template() {
    let fixture = configured_fixture();
    // This stands in for the proposed publications() result, not an implemented API.
    fixture.write("data/publications.json", r#"{"items":[{"key":"older","year":2020,"date":{"text":"2020"},"authors":["Johannes Diderik van der Waals","Jane Doe Jr"]},{"key":"undated","year":null,"date":null,"authors":[]},{"key":"range","year":null,"date":{"text":"2020/2022"},"authors":["Research Group"]},{"key":"forthcoming","year":null,"date":{"text":"forthcoming"},"authors":[]},{"key":"newer","year":2024,"date":{"text":"2024"},"authors":[]},{"key":"zero","year":0,"date":{"text":"0000"},"authors":[]}]}"#);
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}{{ bibliography() }}{% for work in data.publications.items | sort(attribute='year') | reverse %}{% if work.year is none %}{% else %}{{ work.key }}|{% endif %}{% endfor %}{% for work in data.publications.items %}{% if work.year is none %}{{ work.key }}|{% endif %}{% endfor %}");
    build_at(fixture.path(), BuildMode::Full).expect("publication years are sortable");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("newer|older|zero|undated|range|forthcoming|"),
        "{output}"
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}{{ bibliography() }}{% for work in data.publications.items %}{% if work.authors %}{{ work.authors | join(sep=', ') }}{% endif %}{% if work.date %}: {{ work.date.text }}{% endif %}|{% endfor %}");
    build_at(fixture.path(), BuildMode::Full).expect("publication names and dates render");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("Johannes Diderik van der Waals, Jane Doe Jr: 2020"),
        "{output}"
    );
    assert!(output.contains("Research Group: 2020/2022"), "{output}");
    assert!(output.contains(": forthcoming"), "{output}");
}

#[test]
fn curated_site_data_can_resolve_keys_with_template_lookup() {
    let fixture = configured_fixture();
    fixture.write(
        "data/publications.toml",
        "[works.\"ACISP:SKSI25\"]\ntitle='Example'\n\n[works.\"ACISP:LiSun25\"]\ntitle='Second'\n",
    );
    fixture.write(
        "data/curated.toml",
        "ids=['ACISP:LiSun25', 'ACISP:SKSI25']\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}{{ bibliography() }}<h2>Selected</h2>{% for id in data.curated.ids %}{{ data.publications.works[id].title }}|{% endfor %}");
    build_at(fixture.path(), BuildMode::Full).expect("curated keys resolve in site data");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<h2>Selected</h2>Second|Example|"),
        "{output}"
    );
    fixture.write("data/curated.toml", "ids=['ACISP:LiSun25', 'missing']\n");
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("missing key fails during rendering");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("missing") || diagnostic.contains("works[id]"),
        "{diagnostic}"
    );
}

#[test]
fn curated_publications_from_site_data_follow_selected_key_order() {
    let fixture = configured_fixture();
    fixture.write(
        "data/publications.toml",
        "[[items]]\nkey='work'\ntitle='Example'\n\n[[items]]\nkey='other'\ntitle='Second'\n",
    );
    fixture.write("data/curated.toml", "ids=['other', 'work', 'missing']\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}{{ bibliography() }}<h2>Selected</h2>{% for id in data.curated.ids %}{% for work in data.publications.items %}{% if work.key == id %}{{ work.title }}|{% endif %}{% endfor %}{% endfor %}");
    build_at(fixture.path(), BuildMode::Full).expect("site data selection builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<h2>Selected</h2>Second|Example|"),
        "{output}"
    );
}

#[test]
fn publication_list_from_site_data_is_independent_of_bibtex() {
    let fixture = configured_fixture();
    fixture.write("citations/works.bib", "@book{work,title={Example},author={Smith, Alice},year={2024}}\n@article{other,title={Second},author={Lee, Robin},year={2023}}\n");
    fixture.write(
        "data/publications.toml",
        r#"
[[items]]
key = "work"
title = "Example"
author = "Alice Smith"
year = 2024
kind = "book"
selected = true
pdf = "assets/work.pdf"

[[items]]
key = "other"
title = "Second"
author = "Robin Lee"
year = 2023
kind = "article"
selected = false
pdf = "assets/other.pdf"
"#,
    );
    fixture.write("content/assets/work.pdf", "work");
    fixture.write("content/assets/other.pdf", "other");
    fixture.write("templates/page.tera", r#"{{ markdown(content='main') }}{{ bibliography() }}
<h2>Selected</h2>{% for work in data.publications.items %}{% if work.selected %}<p>{{ work.title }}</p>{% endif %}{% endfor %}
<h2>All</h2>{% for work in data.publications.items %}{% set pdf = asset(id=work.pdf) %}<p>{{ work.author }}: {{ work.title }} ({{ work.year }}, {{ work.kind }}) <a href="{{ pdf.url }}">PDF</a></p>{% endfor %}"#);
    build_at(fixture.path(), BuildMode::Full).expect("data-backed publication page builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("<h2>Selected</h2><p>Example</p>"),
        "{output}"
    );
    assert!(
        output.contains("Alice Smith: Example (2024, book)"),
        "{output}"
    );
    assert!(
        output.contains("Robin Lee: Second (2023, article)"),
        "{output}"
    );
    assert!(output.contains("assets/work.pdf"), "{output}");
    fixture.write("citations/works.bib", "@book{work,title={Changed},author={Smith, Alice},year={2024}}\n@article{other,title={Second},author={Lee, Robin},year={2023}}\n");
    build_at(fixture.path(), BuildMode::Full).expect("independent data can become stale");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(output.contains(">Changed</a>"), "{output}");
    assert!(
        output.contains("Alice Smith: Example (2024, book)"),
        "{output}"
    );
    let data_path = fixture.path().join("data/publications.toml");
    let data = fs::read_to_string(data_path)
        .unwrap()
        .replace("assets/work.pdf", "assets/missing.pdf");
    fixture.write("data/publications.toml", &data);
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("missing publication asset fails");
    assert!(format!("{error:?}").contains("assets/missing.pdf"));
}

#[test]
fn citations_render_in_lists_tables_and_blockquotes() {
    let fixture = configured_fixture();
    fixture.write("heine.toml", "[markdown]\nextensions=['tables']\n");
    fixture.write(
        "content/en/index.md",
        "- List [@work].\n\n| Source |\n| --- |\n| Table [@work] |\n\n> Quote [@work].\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("citations in Markdown structures build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    for prefix in ["<li>List ", "<td>Table ", "<blockquote>\n<p>Quote "] {
        assert!(
            output.contains(&format!("{prefix}<a href=\"#__heine-bibliography\"")),
            "{output}"
        );
    }
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
fn assembled_url_ignores_hyphens_in_unrelated_bibliography_text() {
    let fixture = configured_fixture();
    fixture.write(
        "citations/works.bib",
        "@string{host={https://example.org}}\n@book{unrelated,title={A--B}}\n@book{work,title={Example},url=host # {/path}}\n",
    );
    fixture.write("citations/style.csl", r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Example</title><id>https://example.org/style</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="URL"/></layout></bibliography></style>"#);
    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ bibliography() }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("unrelated hyphens do not affect URL");
    let html = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(html.contains("https://example.org/path"), "{html}");
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
fn conflicting_aliases_report_both_bibliography_files() {
    let fixture = configured_fixture();
    fixture.write("content/en/index.page", "template='page.tera'\ncontent.main='index.md'\n[citations]\ncontent='main'\nbibliography=['works.bib', 'other.bib']\nstyle='style.csl'\n");
    fixture.write(
        "citations/other.bib",
        "@book{other,ids={work},title={Other}}\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("conflicting alias must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("works.bib"), "{diagnostic}");
    assert!(diagnostic.contains("other.bib"), "{diagnostic}");
    assert!(diagnostic.contains("first declaration"), "{diagnostic}");
    assert!(
        diagnostic.contains("declared more than once"),
        "{diagnostic}"
    );
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
