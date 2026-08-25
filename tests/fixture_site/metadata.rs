use super::*;

use std::time::{Duration, SystemTime};

#[test]
fn inherits_directory_page_policy_and_exposes_explicit_metadata() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'default.tera'\n[pages.metadata]\nrequired = ['title']\n",
    );
    fixture.write(
        "content/en/blog/_directory.toml",
        "title = 'Blog'\n[pages]\ntemplate = 'post.tera'\n[pages.metadata]\nrequired = ['date', 'extra.author']\n",
    );
    fixture.write(
        "content/en/blog/post.page",
        "title = 'Post'\ndate = 2026-08-14\n[extra]\nauthor = 'Heine'\n",
    );
    fixture.write(
        "content/en/blog/nested/_directory.toml",
        "[pages]\ntemplate = 'nested.tera'\n",
    );
    fixture.write(
        "content/en/blog/nested/inherited.page",
        "title = 'Nested'\ndate = 2026-08-15\n[extra]\nauthor = 'Heine'\n",
    );
    fixture.write(
        "content/en/blog/nested/overridden.page",
        "template = 'override.tera'\ntitle = 'Override'\ndate = 2026-08-16\n[extra]\nauthor = 'Heine'\n",
    );
    fixture.write(
        "content/en/about.page",
        "title = 'About'\ntemplate = false\n",
    );
    fixture.write(
        "templates/post.tera",
        "{{ page.title }}|{{ page.date }}|{{ page.extra.author }}|{{ page.directory.title }}",
    );
    fixture.write("templates/default.tera", "{{ page.title }}");
    fixture.write("templates/nested.tera", "nested {{ page.title }}");
    fixture.write("templates/override.tera", "override {{ page.title }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/post.html")).expect("post output"),
        "Post|2026-08-14|Heine|Blog"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/nested/inherited.html"))
            .expect("nested output"),
        "nested Nested"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/nested/overridden.html"))
            .expect("overridden output"),
        "override Override"
    );
    assert!(!fixture.path().join("public/about.html").exists());
    assert!(!fixture.path().join("public/_directory.toml").exists());
    assert!(!fixture.path().join("public/blog/_directory.toml").exists());
}

#[test]
fn reports_missing_metadata_with_the_page_and_directory_policy_context() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages.metadata]\nrequired = ['title', 'extra.author']\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("metadata is required");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("extra.author"), "{diagnostic}");
}

#[test]
fn validates_inherited_directory_metadata_requirements() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "title = 'Root'\ndescription = 'Root directory'\n[metadata]\nrequired = ['title', 'description']\n",
    );
    fixture.write("content/en/blog/_directory.toml", "title = 'Blog'\n");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("directory metadata is required");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/blog/_directory.toml:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/_directory.toml:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("description"), "{diagnostic}");
}

#[test]
fn reports_invalid_directory_template_at_its_declaration() {
    let fixture = Fixture::new();
    fixture.write("content/en/_directory.toml", "[pages]\ntemplate = true\n");
    fixture.write("content/en/index.page", "title = 'Home'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template is invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("template name or `false`"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_ambiguous_local_date_times() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\ndate = 2026-08-14T09:30:00\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("date is ambiguous");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/index.page"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/index.page:3"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("offset-aware"), "{diagnostic}");
}

#[test]
fn renders_offset_date_times_in_rfc_3339_form() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\ndate = 2026-08-14T09:30:00+02:00\n",
    );
    fixture.write("templates/page.tera", "{{ page.date }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "2026-08-14T09:30:00+02:00"
    );
}

#[test]
fn rejects_extra_keys_that_templates_cannot_address_consistently() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n[extra]\nnot-valid = true\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("extra key is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/index.page"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/index.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("not-valid"), "{diagnostic}");
    assert!(
        diagnostic.contains("simple template identifiers"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_unknown_page_and_directory_policy_fields() {
    let fixture = Fixture::new();
    fixture.write("content/en/_directory.toml", "[pages]\nmetadta = {}\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("policy field is misspelled");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("unknown field `metadta`"),
        "{diagnostic}"
    );

    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[data]\ntitle = 'Old'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("old field is rejected");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("unknown field `data`"), "{diagnostic}");
}

#[test]
fn quick_builds_rerender_pages_after_a_descriptor_change() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "title = 'Before'\n[pages]\ntemplate = 'page.tera'\n",
    );
    fixture.write("content/en/index.page", "title = 'Home'\n");
    fixture.write("templates/page.tera", "{{ page.directory.title }}");
    build_at(fixture.path(), BuildMode::Full).expect("initial build");

    let output = fixture.path().join("public/index.html");
    fs::File::options()
        .write(true)
        .open(&output)
        .expect("output file")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now()))
        .expect("set output modification time");
    let descriptor = fixture.path().join("content/en/_directory.toml");
    fixture.write(
        "content/en/_directory.toml",
        "title = 'After'\n[pages]\ntemplate = 'page.tera'\n",
    );
    fs::File::options()
        .write(true)
        .open(descriptor)
        .expect("descriptor file")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set descriptor modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick rebuild");

    assert_eq!(fs::read_to_string(output).expect("output"), "After");
}

#[test]
fn reports_a_missing_inherited_template_at_its_directory_declaration() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'missing.tera'\n",
    );
    fixture.write("content/en/index.page", "title = 'Home'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template is missing");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("missing.tera"), "{diagnostic}");
}
