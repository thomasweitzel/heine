use super::*;

#[test]
fn builds_a_site_with_pages_segments_markdown_and_versioned_assets() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.html'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/article.page",
        "template = 'page.html'\ncontent.main = 'article.html'\n",
    );
    fixture.write("content/en/index.md", "# Home");
    fixture.write(
        "content/en/article.html",
        "<!-- segment: summary -->\nFirst\n<!-- segment: summary -->\nSecond",
    );
    fixture.write(
        "content/en/quiz/quiz-1.page",
        "template = 'page.html'\ncontent.main = 'quiz.html'\n",
    );
    fixture.write("content/en/quiz/quiz.html", "Quiz content");
    fixture.write("content/en/quiz/styles.v2.css", "quiz {}");
    fixture.write("content/en/style.v2.css", "body {}");
    fixture.write(
        "templates/page.html",
        "{{ page.id }}|{{ siblings() | length }}|{% if page.id == 'index' %}{{ page.content.main | markdown }}{% elif page.id == 'quiz/quiz-1' %}{{ 'styles.css' | latest }}{% else %}{{ page.content.main.summary[0] }}{% endif %}|{{ '/style.css' | latest }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("index output"),
        "index|1|<h1>Home</h1>\n|/style.v2.css"
    );
    assert!(
        fs::read_to_string(fixture.path().join("public/article.html"))
            .expect("article output")
            .contains("First")
    );
    assert!(
        fs::read_to_string(fixture.path().join("public/quiz/quiz-1.html"))
            .expect("quiz output")
            .contains("styles.v2.css")
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/style.v2.css")).expect("copied asset"),
        "body {}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_anywhere_in_content() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'linked.md'\n",
    );
    fixture.write("templates/page.tera", "page");
    let external = fixture.path().join("external.md");
    fs::write(&external, "outside content").expect("external content");
    symlink(&external, fixture.path().join("content/en/linked.md")).expect("content symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink must be rejected");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/linked.md"), "{diagnostic}");
    assert!(diagnostic.contains("symbolic link"), "{diagnostic}");
}

#[test]
fn reports_the_page_location_for_invalid_toml() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = [\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("page TOML should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("invalid TOML in content definition"),
        "{diagnostic}"
    );
}

#[test]
fn reports_the_content_location_for_invalid_json() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'data.json'\n",
    );
    fixture.write("content/en/data.json", "{ invalid }");
    fixture.write("templates/page.tera", "{{ page.content.main }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("JSON should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/data.json:1"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("invalid JSON content"), "{diagnostic}");
}

#[test]
fn reports_an_invalid_segment_identifier_at_its_marker() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'segments.html'\n",
    );
    fixture.write(
        "content/en/segments.html",
        "<!-- segment: valid -->\ntext\n<!-- segment: invalid-key -->\ntext",
    );
    fixture.write("templates/page.tera", "{{ page.content.main }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("segment is invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/segments.html:3"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("invalid-key"), "{diagnostic}");
}

#[test]
fn rejects_content_references_that_escape_the_content_directory() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/nested/index.page",
        "template = 'page.tera'\ncontent.main = '../../outside.md'\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main }}");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("reference should be rejected");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("index.page"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/nested/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("escapes the content directory"),
        "{diagnostic}"
    );
}

#[test]
fn reports_the_page_value_for_an_invalid_weight() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\nweight = true\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("weight should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("must be an integer"), "{diagnostic}");
}

#[test]
fn rejects_a_page_suffix_that_contains_output_path_syntax() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\nsuffix = 'html/../../outside'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("suffix should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("without path separators"),
        "{diagnostic}"
    );
    assert!(!fixture.path().join("outside").exists());
}

#[test]
fn reports_a_missing_declared_content_file_at_its_page_reference() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'missing.md'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("content file is missing");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("missing.md"), "{diagnostic}");
}

#[test]
fn sorts_page_subdirectories_deterministically() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/zeta/child.txt", "asset");
    fixture.write("content/en/alpha/child.txt", "asset");
    fixture.write("templates/page.tera", "{{ page.subdirs | join(sep=',') }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "alpha,zeta"
    );
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_content_paths_instead_of_changing_their_ids() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");
    let invalid = fixture
        .path()
        .join("content")
        .join("en")
        .join(OsString::from_vec(b"invalid-\xff.txt".to_vec()));
    fs::write(invalid, "asset").expect("non-UTF-8 fixture asset");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("path should be rejected");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("must use UTF-8") && diagnostic.contains("components"),
        "{diagnostic}"
    );
}

#[test]
fn excludes_declared_content_inputs_but_copies_adjacent_assets() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/blog/post.page",
        "template = 'page.tera'\ncontent.main = 'post/body.md'\n",
    );
    fixture.write("content/en/blog/post/body.md", "# Post");
    fixture.write("content/en/blog/post/images/sunset.svg", "<svg></svg>");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert!(!fixture.path().join("public/blog/post/body.md").exists());
    assert!(
        fixture
            .path()
            .join("public/blog/post/images/sunset.svg")
            .is_file()
    );
    assert!(fixture.path().join("public/blog/post.html").is_file());
}

#[test]
fn excludes_declared_content_of_a_page_without_rendered_output() {
    let fixture = Fixture::new();
    fixture.write("content/en/data.page", "content.items = 'items.json'\n");
    fixture.write("content/en/items.json", "[\"one\"]");
    fixture.write("content/en/images/logo.svg", "<svg></svg>");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "rendered");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert!(!fixture.path().join("public/items.json").exists());
    assert!(fixture.path().join("public/images/logo.svg").is_file());
}

#[test]
fn rejects_content_names_that_templates_cannot_address_with_dot_notation() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[content]\nmain-body = 'body.md'\n",
    );
    fixture.write("content/en/body.md", "Body");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("content key is invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:3"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("main-body"), "{diagnostic}");
    assert!(
        diagnostic.contains("simple template identifier"),
        "{diagnostic}"
    );
}

#[test]
fn copies_dotfiles_that_are_not_declared_content() {
    let fixture = Fixture::new();
    fixture.write("content/en/source/.metadata", "published metadata\n");
    fixture.write("content/en/source/asset.txt", "published asset\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "rendered");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert!(fixture.path().join("public/source/.metadata").is_file());
    assert!(fixture.path().join("public/source/asset.txt").is_file());
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_content_paths_that_differ_only_by_unicode_normalization() {
    let fixture = Fixture::new();
    fixture.write("content/en/café.page", "template = 'page.tera'\n");
    fixture.write("content/en/cafe\u{301}.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("normalization-equivalent content paths are ambiguous");
    let message = error.to_string();

    assert!(message.contains("café.page"), "{message}");
    assert!(message.contains("cafe\u{301}.page"), "{message}");
    assert!(message.contains("Unicode normalization"), "{message}");
}
