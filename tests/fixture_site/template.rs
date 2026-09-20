use super::*;

#[test]
fn exposes_components_defined_in_any_template_without_imports() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/components.tera",
        "{% component ui.greeting(label: string) %}<p>{{ label }}</p>{% endcomponent ui.greeting %}",
    );
    fixture.write(
        "templates/page.tera",
        "{{ <ui.greeting label=\"Hello\" /> }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("component is globally available");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("rendered output"),
        "<p>Hello</p>"
    );
}

#[test]
fn rejects_unknown_keywords_for_fixed_template_function_contracts() {
    let cases = [
        ("{{ highlight_css(unknown=true) }}", true),
        ("{{ 'text' | markdown(unknown=true) }}", false),
        ("{{ 'missing.css' | latest(unknown=true) }}", false),
        ("{{ 'missing.png' | dimensions(unknown=true) }}", false),
        ("{{ format_date(value='2026-08-26', unknown=true) }}", false),
        ("{{ collection(name='news', unknown=true) }}", false),
        ("{{ taxonomy(name='tags', unknown=true) }}", false),
    ];

    for (template, highlighting) in cases {
        let fixture = Fixture::new();
        fixture.write("content/en/index.page", "template = 'page.tera'\n");
        if highlighting {
            fixture.write(
                "heine.toml",
                "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
            );
        }
        fixture.write("templates/page.tera", template);

        let error = build_at(fixture.path(), BuildMode::Full)
            .expect_err("unknown keyword must fail before function-specific work");
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
        assert!(diagnostic.contains("unknown"), "{diagnostic}");
        assert!(diagnostic.contains("does not accept"), "{diagnostic}");
        assert!(diagnostic.contains("keyword"), "{diagnostic}");
        assert!(diagnostic.contains("argument"), "{diagnostic}");
    }
}

#[test]
fn reports_the_page_location_for_a_missing_template() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'missing.tera'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template should be missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/index.page:1"));
    assert!(diagnostic.contains("template referenced here"));
    assert!(diagnostic.contains("missing.tera"));
}

#[test]
fn reports_a_missing_templates_directory() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::remove_dir_all(fixture.path().join("templates")).expect("remove templates directory");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("templates are missing");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("could not walk template directory templates"),
        "{diagnostic}"
    );
    assert!(
        !diagnostic.contains(&fixture.path().display().to_string()),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("could not walk"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_template_paths_instead_of_lossy_template_names() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");
    let invalid = fixture
        .path()
        .join("templates")
        .join(OsString::from_vec(b"invalid-\xff.tera".to_vec()));
    fs::write(invalid, "ignored").expect("non-UTF-8 fixture template");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("path should be rejected");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("must use UTF-8") && diagnostic.contains("components"),
        "{diagnostic}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_template_paths_that_differ_only_by_unicode_normalization() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'café.tera'\n");
    fixture.write("templates/café.tera", "composed");
    fixture.write("templates/cafe\u{301}.tera", "decomposed");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("normalization-equivalent template paths are ambiguous");
    let message = error.to_string();

    assert!(message.contains("café.tera"), "{message}");
    assert!(message.contains("cafe\u{301}.tera"), "{message}");
    assert!(message.contains("Unicode normalization"), "{message}");
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_anywhere_in_templates() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");
    let external = fixture.path().join("external.tera");
    fs::write(&external, "outside template").expect("external template");
    symlink(&external, fixture.path().join("templates/linked.tera")).expect("template symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink must be rejected");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("linked.tera"), "{diagnostic}");
    assert!(diagnostic.contains("symbolic link"), "{diagnostic}");
}

#[test]
fn reports_the_template_location_for_a_template_syntax_error() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'broken.tera'\n");
    fixture.write("templates/broken.tera", "{% extents 'base.tera' %}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template syntax should fail");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("templates/broken.tera:1"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("error here"), "{diagnostic}");
}

#[test]
fn reports_the_template_location_for_a_missing_parent_template() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'broken.tera'\n");
    fixture.write(
        "templates/broken.tera",
        "{# missing.tera is mentioned in a comment #}\n{% extends 'missing.tera' %}",
    );

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("parent template should be missing");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("templates/broken.tera:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("referenced") && diagnostic.contains("parent template"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("missing.tera"), "{diagnostic}");
}

#[test]
fn reports_the_configuration_value_for_an_invalid_enum_option() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[assets]\nintegrity = 'sha999'\n");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("configuration should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("sha999"), "{diagnostic}");
    assert!(diagnostic.contains("sha384"), "{diagnostic}");
}

#[test]
fn rejects_unknown_markdown_extensions_at_the_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['tables', 'alerts']\n",
    );

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("configuration should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("alerts"), "{diagnostic}");
    assert!(diagnostic.contains("task_lists"), "{diagnostic}");
}

#[test]
fn rejects_duplicate_markdown_extensions_at_the_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['tables', 'tables']\n",
    );

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("configuration should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("duplicate"), "{diagnostic}");
}

#[test]
fn responsive_table_configuration_requires_the_tables_extension() {
    let disabled = Fixture::new();
    disabled.write(
        "heine.toml",
        "[markdown.tables]\nscroll_message = 'table-scroll'\n",
    );

    let error = build_at(disabled.path(), BuildMode::Full)
        .expect_err("table configuration needs the tables extension");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("markdown.extensions to include \"tables\""),
        "{diagnostic}"
    );
}

#[test]
fn tables_without_responsive_configuration_keep_plain_markup() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nextensions = ['tables']\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "| Name |\n| --- |\n| Heine |\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("ordinary tables build without configuration");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<table>"), "{output}");
    assert!(!output.contains("heine-table-scroll"), "{output}");
}

#[test]
fn table_scroll_messages_are_checked_and_escaped() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[markdown]\nextensions = ['tables']\n\n[markdown.tables]\nscroll_message = 'table-scroll'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "| Name |\n| --- |\n| Heine |\n");
    fixture.write(
        "content/de/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/de/index.md", "| Name |\n| --- |\n| Heine |\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("the configured Fluent message must exist");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:14"), "{diagnostic}");
    assert!(diagnostic.contains("table-scroll message"), "{diagnostic}");

    fixture.write("locales/en/site.ftl", "table-scroll = Scroll & <table>\n");
    build_at(fixture.path(), BuildMode::Full).expect("localized table builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("role=\"region\" aria-label=\"Scroll &amp; &lt;table&gt;\""),
        "{output}"
    );
    let german =
        fs::read_to_string(fixture.path().join("public/de/index.html")).expect("German output");
    assert!(
        german.contains("role=\"region\" aria-label=\"Scroll &amp; &lt;table&gt;\""),
        "{german}"
    );
}

#[test]
fn rejects_an_unknown_markdown_math_mode_at_the_configuration_value() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'katex'\n");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("configuration should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("katex"), "{diagnostic}");
    assert!(diagnostic.contains("mathml"), "{diagnostic}");
    assert!(diagnostic.contains("tex"), "{diagnostic}");
}

#[test]
fn reports_the_configuration_location_for_an_invalid_base_path() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[site]\nbase_path = 'guide'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("base path should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("must start with '/'"), "{diagnostic}");
}

#[test]
fn reports_the_configuration_location_for_an_uncanonical_base_path() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[site]\nbase_path = '/guide notes/'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("base path should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("percent-encode spaces"), "{diagnostic}");
}

#[test]
fn rejects_an_unknown_configuration_key_at_its_source_location() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[site]\nbase_peth = '/guide/'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("configuration is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("base_peth"), "{diagnostic}");
    assert!(diagnostic.contains("unknown field"), "{diagnostic}");
}

#[test]
fn rejects_invalid_collection_configuration_at_the_authored_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.blog-posts]\norder = 'id-desc'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collection is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:1"), "{diagnostic}");
    assert!(diagnostic.contains("blog-posts"), "{diagnostic}");
    assert!(
        diagnostic.contains("simple template identifier"),
        "{diagnostic}"
    );

    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'date-desc'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("order is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("date-desc"), "{diagnostic}");
    assert!(diagnostic.contains("published-desc"), "{diagnostic}");
}
