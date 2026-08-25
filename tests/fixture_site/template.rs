use super::*;

#[test]
fn reports_the_page_location_for_a_missing_template() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'missing.tera'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template should be missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/index.page:1"));
    assert!(diagnostic.contains("template referenced here"));
    assert!(diagnostic.contains("missing.tera"));
}

#[test]
fn reports_a_missing_templates_directory() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fs::remove_dir_all(fixture.path().join("templates")).expect("remove templates directory");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("templates are missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates"), "{diagnostic}");
    assert!(diagnostic.contains("could not walk"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_template_paths_instead_of_lossy_template_names() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
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

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_anywhere_in_templates() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
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
    fixture.write("content/index.page", "template = 'broken.tera'\n");
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
    fixture.write("content/index.page", "template = 'broken.tera'\n");
    fixture.write("templates/broken.tera", "{% extends 'missing.tera' %}");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("parent template should be missing");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("templates/broken.tera:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("referenced parent template here"),
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
