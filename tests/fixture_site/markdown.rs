use super::*;

#[test]
fn source_aware_markdown_exposes_a_toc_and_matching_heading_ids() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 3\n",
    );
    fixture.write("content/en/index.page", "content.main = 'index.md'\n");
    fixture.write(
        "content/en/index.md",
        "# Title\n\n## Über **view**\n\n### Über view\n\n## Über view\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% for heading in page.toc.headings %}[{{ heading.level }},{{ heading.depth }},{{ heading.fragment }}:{{ heading.text }}]{% endfor %}|{{ markdown(content='main') }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("[2,1,über-view:Über view]"), "{output}");
    assert!(output.contains("[3,2,über-view-2:Über view]"), "{output}");
    assert!(output.contains("[2,1,über-view-3:Über view]"), "{output}");
    assert!(output.contains("<h1 id=\"title\">Title</h1>"), "{output}");
    assert!(
        output.contains("<h2 id=\"über-view\">Über <strong>view</strong></h2>"),
        "{output}"
    );
}

#[test]
fn rejects_the_plain_filter_for_selected_toc_content_at_its_call_site() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n",
    );
    fixture.write("content/en/index.md", "## Heading\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("filter use fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("markdown(content="), "{diagnostic}");
}

#[test]
fn page_toc_replaces_a_directory_default_and_ignores_raw_html_headings() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 2\n",
    );
    fixture.write(
        "content/en/index.page",
        "content.main = 'index.md'\n\n[toc]\ncontent = 'main'\nmin_level = 3\nmax_level = 3\n",
    );
    fixture.write(
        "content/en/index.md",
        "## Outside\n\n### Inside\n\n<h3>Raw heading</h3>\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% for heading in page.toc.headings %}[{{ heading.level }}:{{ heading.fragment }}]{% endfor %}{{ markdown(content='main') }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("[3:inside]"), "{output}");
    assert!(!output.contains("[2:outside]"), "{output}");
    assert!(
        output.contains("<h2 id=\"outside\">Outside</h2>"),
        "{output}"
    );
    assert!(output.contains("<h3 id=\"inside\">Inside</h3>"), "{output}");
    assert!(output.contains("<h3>Raw heading</h3>"), "{output}");
}

#[test]
fn nearest_directory_toc_default_wins() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 2\n",
    );
    fixture.write(
        "content/en/guide/_directory.toml",
        "[pages.toc]\ncontent = 'main'\nmin_level = 3\nmax_level = 3\n",
    );
    fixture.write("content/en/guide/index.page", "content.main = 'main.md'\n");
    fixture.write("content/en/guide/main.md", "## Outer\n\n### Inner\n");
    fixture.write(
        "templates/page.tera",
        "{% for heading in page.toc.headings %}[{{ heading.level }}:{{ heading.fragment }}]{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output =
        fs::read_to_string(fixture.path().join("public/guide/index.html")).expect("output");
    assert_eq!(output, "[3:inner]");
}

#[test]
fn toc_opt_out_requires_an_inherited_default() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntoc = false\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("inert opt-out fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("otherwise effective inherited table of contents"),
        "{diagnostic}"
    );
}

#[test]
fn toc_configuration_errors_point_to_the_authored_field() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n\n[toc]\ncontent = 'main'\nmin_level = 7\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid level fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:6"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("toc.min_level must be an integer"),
        "{diagnostic}"
    );
}

#[test]
fn toc_diagnostics_do_not_label_an_unrelated_extra_key() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n\n[extra]\nmin_level = 2\n\n[toc]\ncontent = 'main'\nmin_level = 7\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid level fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:9"),
        "{diagnostic}"
    );
}

#[test]
fn directory_toc_diagnostics_do_not_label_an_unrelated_extra_key() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[extra]\nmin_level = 2\n\n[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 7\n",
    );
    fixture.write("content/en/index.page", "content.main = 'main.md'\n");
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid level fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/_directory.toml:9"),
        "{diagnostic}"
    );
}

#[test]
fn toc_requires_one_unsegmented_markdown_content_file() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "<!-- segment: first -->\n## Heading\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("segmented source fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:5"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("one unsegmented Markdown document"),
        "{diagnostic}"
    );
}

#[test]
fn source_aware_markdown_requires_current_page_markdown_content() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.data = 'data.toml'\n",
    );
    fixture.write("content/en/data.toml", "name = 'Heine'\n");
    fixture.write("templates/page.tera", "{{ markdown(content='data') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("non-Markdown content fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("declared unsegmented .md content name"),
        "{diagnostic}"
    );
}

#[test]
fn source_aware_markdown_without_a_toc_renders_ordinary_markdown() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<h2>Heading</h2>"), "{output}");
    assert!(!output.contains("id=\"heading\""), "{output}");
}

#[test]
fn markdown_extensions_and_mathml_are_rendered_when_enabled() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['tables', 'strikethrough', 'task_lists', 'footnotes']\nmath = 'mathml'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "~~old~~\n\n| Name |\n| --- |\n| Heine |\n\n- [x] done\n\nA note[^one].\n\n[^one]: Footnote\n\n$x^2$\n\n$$\\frac{1}{2}$$\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");

    assert!(output.contains("<del>old</del>"), "{output}");
    assert!(output.contains("<table>"), "{output}");
    assert!(output.contains("type=\"checkbox\""), "{output}");
    assert!(output.contains("footnote-reference"), "{output}");
    assert!(output.contains("<math"), "{output}");
    assert!(output.contains("display=\"block\""), "{output}");
}

#[test]
fn tex_math_preserves_delimiters_without_markdown_or_html_interpretation() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'tex'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "Inline $a_b * c <tag>$.\n\n$$\\unknown{a_b} * <tag>$$\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds without TeX validation");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");

    assert!(output.contains("$a_b * c &lt;tag&gt;$"), "{output}");
    assert!(
        output.contains("$$\\unknown{a_b} * &lt;tag&gt;$$"),
        "{output}"
    );
    assert!(!output.contains("<math"), "{output}");
}

#[test]
fn reports_invalid_math_in_markdown_content_at_its_source() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'mathml'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "The prose also contains a^b^c. The invalid expression is $a^b^c$.",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("math should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.md:1:58"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("invalid TeX here"), "{diagnostic}");
    assert!(diagnostic.contains("invalid TeX here"), "{diagnostic}");
}

#[test]
fn reports_invalid_math_in_a_template_at_the_filter_call_site() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'mathml'\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", r"{{ '$a^b^c$' | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("math should be invalid");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("a^b^c"), "{diagnostic}");
}

#[test]
fn generates_one_preference_aware_stylesheet_for_class_based_code() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "```rust\nfn main() {}\n```\n");
    fixture.write(
        "templates/page.tera",
        "<link rel='stylesheet' href='{{ highlight_css() }}'>{{ page.content.main | markdown }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let html = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    let css = fs::read_to_string(fixture.path().join("public/styles/highlighting.css"))
        .expect("generated stylesheet");

    assert!(html.contains("/guide/styles/highlighting.css"), "{html}");
    assert!(html.contains("heine-l-code heine-d-code"), "{html}");
    assert!(!html.contains("<pre class=\"giallo\" style="), "{html}");
    assert!(css.contains("@media (prefers-color-scheme: dark)"), "{css}");
    assert!(
        css.contains(".giallo > code { background: transparent; }"),
        "{css}"
    );
    assert!(css.contains("heine-l-"), "{css}");
    assert!(css.contains("heine-d-"), "{css}");
}

#[test]
fn reports_unknown_fenced_languages_at_the_markdown_source() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "```not-a-language\ntext\n```\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown language fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md"), "{diagnostic}");
    assert!(diagnostic.contains("not-a-language"), "{diagnostic}");
}

#[test]
fn highlight_css_must_not_be_called_when_highlighting_is_disabled() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ highlight_css() }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("function is unavailable");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("syntax highlighting is not enabled"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_a_highlighting_stylesheet_that_conflicts_with_a_copied_asset() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/styles/highlighting.css", "authored CSS");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("syntax-highlighting") && diagnostic.contains("stylesheet"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("styles/highlighting.css"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_a_highlighting_stylesheet_that_would_be_an_asset_directory() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/en/styles/highlighting.css/override.css",
        "authored CSS",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("directory collision fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("syntax-highlighting"), "{diagnostic}");
    assert!(diagnostic.contains("stylesheet"), "{diagnostic}");
}

#[test]
fn reports_an_unsafe_highlighting_output_path_at_its_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting?.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("stylesheet is unsafe");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("portable filename components"),
        "{diagnostic}"
    );
}

#[test]
fn reports_both_configuration_and_page_locations_for_a_stylesheet_collision() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/styles/highlighting.page",
        "template = 'page.tera'\nsuffix = '.css'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/styles/highlighting.page:2"),
        "{diagnostic}"
    );
}

#[test]
fn plain_missing_language_policy_keeps_unknown_fences_renderable() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\nmissing_language = 'plain'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "```unknown\ntext\n```\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("plain fallback builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<pre class=\"giallo"), "{output}");
    assert!(!output.contains("style="), "{output}");
}

#[test]
fn reports_an_unknown_highlighting_theme_at_the_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'not-a-theme'\ndark = 'github-dark'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("theme is invalid");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:5"), "{diagnostic}");
    assert!(diagnostic.contains("not-a-theme"), "{diagnostic}");
}
