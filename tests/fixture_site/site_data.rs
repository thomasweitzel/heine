use super::*;

#[test]
fn exposes_structured_site_data_without_publishing_it() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.json", r#"{ "title": "Heine" }"#);
    fixture.write(
        "data/navigation/main.toml",
        "[[links]]\nlabel = 'Home'\npath = '/'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ data.site.title }}|{{ data.navigation.main.links[0].label }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "Heine|Home"
    );
    assert!(!fixture.path().join("public/data/site.json").exists());
    assert!(
        !fixture
            .path()
            .join("public/data/navigation/main.toml")
            .exists()
    );
}

#[test]
fn quick_builds_rerender_when_global_data_changes() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.json", r#"{ "title": "Before" }"#);
    fixture.write("templates/page.tera", "{{ data.site.title }}");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    let output = fixture.path().join("public/index.html");
    fs::File::options()
        .write(true)
        .open(&output)
        .expect("output file")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now()))
        .expect("set output modification time");
    fixture.write("data/site.json", r#"{ "title": "After" }"#);
    fs::File::options()
        .write(true)
        .open(fixture.path().join("data/site.json"))
        .expect("data file")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set data modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build rerenders");

    assert_eq!(fs::read_to_string(output).expect("page output"), "After");
}

#[test]
fn rejects_invalid_json_data_with_its_source_location() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.json", "{ invalid }");
    fixture.write("templates/page.tera", "{{ data.site }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("JSON data is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("data/site.json:1"), "{diagnostic}");
    assert!(diagnostic.contains("invalid JSON data"), "{diagnostic}");
}

#[test]
fn rejects_invalid_toml_data_with_its_source_location() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.toml", "title = [\n");
    fixture.write("templates/page.tera", "{{ data.site }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("TOML data is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("data/site.toml:1"), "{diagnostic}");
    assert!(diagnostic.contains("invalid TOML data"), "{diagnostic}");
}

#[test]
fn rejects_unsupported_data_files() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.yaml", "title: Heine\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("YAML data is unsupported");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("data/site.yaml"), "{diagnostic}");
    assert!(diagnostic.contains(".json or .toml"), "{diagnostic}");
}

#[test]
fn rejects_data_key_collisions() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/navigation.json", "[]");
    fixture.write("data/navigation/main.toml", "title = 'Home'\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("data keys collide");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("produce") && diagnostic.contains("same"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("navigation.json"), "{diagnostic}");
    assert!(diagnostic.contains("main.toml"), "{diagnostic}");
}

#[test]
fn rejects_data_files_that_map_to_the_same_key() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.json", "{}");
    fixture.write("data/site.toml", "title = 'Heine'\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("data keys collide");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("produce") && diagnostic.contains("same"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("site.json"), "{diagnostic}");
    assert!(diagnostic.contains("site.toml"), "{diagnostic}");
}

#[test]
fn rejects_data_paths_that_cannot_be_accessed_with_dot_notation() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("data/main-menu.json", "{}");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("data key is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("data/main-menu.json"), "{diagnostic}");
    assert!(diagnostic.contains("data.*"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_anywhere_in_global_data() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");
    let external = fixture.path().join("external.json");
    fs::write(&external, "{}").expect("external data");
    fs::create_dir_all(fixture.path().join("data")).expect("data directory");
    symlink(&external, fixture.path().join("data/site.json")).expect("data symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink is rejected");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("data/site.json"), "{diagnostic}");
    assert!(diagnostic.contains("symbolic link"), "{diagnostic}");
}
