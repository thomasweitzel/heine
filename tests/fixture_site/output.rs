use super::*;

use std::time::{Duration, SystemTime};

#[test]
fn a_full_build_replaces_stale_managed_output() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("content/obsolete.txt", "obsolete asset");
    fixture.write("templates/page.tera", "first build");

    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");
    assert!(fixture.path().join("public/obsolete.txt").is_file());
    assert!(fixture.path().join("public/.heine-output").is_file());

    fs::remove_file(fixture.path().join("content/obsolete.txt")).expect("remove source asset");
    fixture.write("templates/page.tera", "second build");
    build_at(fixture.path(), BuildMode::Full).expect("replacement site builds");

    assert!(!fixture.path().join("public/obsolete.txt").exists());
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "second build"
    );
}

#[test]
fn refuses_to_replace_unmanaged_output_without_explicit_adoption() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "generated");
    fixture.write("public/manual.txt", "keep me unless explicitly replaced");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output is unmanaged");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("unmanaged output"), "{diagnostic}");
    assert!(diagnostic.contains("--clean-output"), "{diagnostic}");
    assert!(fixture.path().join("public/manual.txt").is_file());

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).clean_output(),
    )
    .expect("explicit adoption succeeds");

    assert!(!fixture.path().join("public/manual.txt").exists());
    assert!(fixture.path().join("public/.heine-output").is_file());
    assert!(fixture.path().join("public/index.html").is_file());
}

#[test]
fn rejects_an_unrecognized_output_ownership_marker() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "generated");
    fixture.write("public/.heine-output", "not a Heine marker\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("marker is invalid");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("unrecognized ownership marker"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("--clean-output"), "{diagnostic}");

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).clean_output(),
    )
    .expect("explicit adoption replaces the invalid marker");
}

#[test]
fn a_failed_full_build_preserves_the_previous_published_site() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "published site");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    fixture.write("templates/page.tera", "{% extents 'base.tera' %}");
    build_at(fixture.path(), BuildMode::Full).expect_err("template is invalid");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("previous output"),
        "published site"
    );
    assert!(fixture.path().join("public/.heine-output").is_file());
}

#[test]
fn a_quick_build_keeps_stale_output_by_design() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("content/obsolete.txt", "obsolete asset");
    fixture.write("templates/page.tera", "rendered");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    fs::remove_file(fixture.path().join("content/obsolete.txt")).expect("remove source asset");
    build_at(fixture.path(), BuildMode::Quick).expect("quick build succeeds");

    assert!(fixture.path().join("public/obsolete.txt").is_file());
}

#[test]
fn a_quick_build_copies_assets_newer_than_their_output() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("content/app.js", "before");
    fixture.write("templates/page.tera", "rendered");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    fixture.write("content/app.js", "after");
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/app.js"))
        .expect("asset source")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set asset modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build succeeds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/app.js")).expect("copied asset"),
        "after"
    );
}

#[cfg(unix)]
#[test]
fn a_quick_build_refuses_a_symbolic_linked_asset_target() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("content/app.js", "before");
    fixture.write("templates/page.tera", "rendered");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    let external = fixture.path().join("outside.js");
    fs::write(&external, "protected").expect("external file");
    fs::remove_file(fixture.path().join("public/app.js")).expect("remove output asset");
    symlink(&external, fixture.path().join("public/app.js")).expect("asset symlink");
    fixture.write("content/app.js", "after");

    let error = build_at(fixture.path(), BuildMode::Quick).expect_err("symlink is rejected");
    assert!(format!("{error:?}").contains("symbolic link"));
    assert_eq!(
        fs::read_to_string(external).expect("external file"),
        "protected"
    );
}

#[cfg(unix)]
#[test]
fn a_quick_build_refuses_a_symbolic_linked_page_target() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "before");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    let external = fixture.path().join("outside.html");
    fs::write(&external, "protected").expect("external file");
    fs::remove_file(fixture.path().join("public/index.html")).expect("remove output page");
    symlink(&external, fixture.path().join("public/index.html")).expect("page symlink");
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/index.page"))
        .expect("page source")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set page modification time");

    let error = build_at(fixture.path(), BuildMode::Quick).expect_err("symlink is rejected");
    assert!(format!("{error:?}").contains("symbolic link"));
    assert_eq!(
        fs::read_to_string(external).expect("external file"),
        "protected"
    );
}

#[test]
fn reserves_the_output_marker_path() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("content/.heine-output", "not an output marker");
    fixture.write("templates/page.tera", "rendered");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("marker path is reserved");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("reserved for Heine internal use"),
        "{diagnostic}"
    );
}

#[test]
fn reserves_the_development_server_namespace_for_assets() {
    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("content/__heine/client.js", "not a server endpoint");
    fixture.write("templates/page.tera", "rendered");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("namespace is reserved");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("reserved for Heine internal use"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("__heine"), "{diagnostic}");
}

#[test]
fn reports_the_page_location_for_a_reserved_development_server_namespace() {
    let fixture = Fixture::new();
    fixture.write("content/__heine/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "rendered");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("namespace is reserved");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("reserved for Heine internal use"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("__heine/index.page"), "{diagnostic}");
    assert!(diagnostic.contains("output declared here"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn rejects_a_symbolic_linked_output_directory() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "rendered");
    fs::create_dir_all(fixture.path().join("outside")).expect("outside directory");
    symlink(
        fixture.path().join("outside"),
        fixture.path().join("public"),
    )
    .expect("output symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink must be rejected");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("symbolic"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn rejects_a_symbolic_linked_output_ancestor() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("heine.toml", "[build]\noutput_dir = 'redirect/public'\n");
    fixture.write("content/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "rendered");
    let outside = fixture.path().join("outside");
    fs::create_dir_all(&outside).expect("outside directory");
    symlink(&outside, fixture.path().join("redirect")).expect("output ancestor symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink must be rejected");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("ancestor"), "{diagnostic}");
    assert!(!outside.join("public").exists());
}

#[test]
fn reports_the_configuration_location_for_an_unsafe_output_directory() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[build]\noutput_dir = '../published'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output directory is unsafe");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("build.output_dir"), "{diagnostic}");
}

#[test]
fn rejects_an_output_directory_inside_global_data() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[build]\noutput_dir = 'data/public'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output path is unsafe");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("data/"), "{diagnostic}");
}

#[test]
fn rejects_a_page_output_that_would_overwrite_a_copied_asset() {
    let fixture = Fixture::new();
    fixture.write(
        "content/index.page",
        "template = 'page.tera'\nsuffix = '.html'\n",
    );
    fixture.write("content/index.html", "an asset");
    fixture.write("templates/page.tera", "a rendered page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output paths collide");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("index"), "{diagnostic}");
    assert!(diagnostic.contains("index.html"), "{diagnostic}");
    assert!(diagnostic.contains("content/index.page:2"), "{diagnostic}");
    assert!(diagnostic.contains("copied asset"), "{diagnostic}");
}

#[test]
fn reports_both_page_locations_for_an_output_collision() {
    let fixture = Fixture::new();
    fixture.write(
        "content/report.page",
        "template = 'page.tera'\nsuffix = '.a.xml'\n",
    );
    fixture.write(
        "content/report.a.page",
        "template = 'page.tera'\nsuffix = '.xml'\n",
    );
    fixture.write("templates/page.tera", "a rendered page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output paths collide");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/report.page:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/report.a.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("first output declaration"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("this output is also declared here"),
        "{diagnostic}"
    );
}
