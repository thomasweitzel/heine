use super::*;

#[test]
fn copies_deployment_files_to_the_output_root() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    fixture.write(
        "deploy/.htaccess",
        "Header always set Content-Security-Policy \"default-src 'self'\"\n",
    );
    fixture.write(
        "deploy/google1234567890.html",
        "google-site-verification: google1234567890.html\n",
    );
    fixture.write(
        "deploy/server/headers.conf",
        "X-Content-Type-Options: nosniff\n",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/.htaccess")).expect(".htaccess"),
        "Header always set Content-Security-Policy \"default-src 'self'\"\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/google1234567890.html"))
            .expect("Google verification file"),
        "google-site-verification: google1234567890.html\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/server/headers.conf"))
            .expect("nested deployment file"),
        "X-Content-Type-Options: nosniff\n"
    );
}

#[test]
fn deployment_files_participate_in_output_collision_validation() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    fixture.write("deploy/index.html", "deployment file");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output collision");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("index.html"), "{diagnostic}");
    assert!(diagnostic.contains("deploy/index.html"), "{diagnostic}");
    assert!(diagnostic.contains("copied file"), "{diagnostic}");
}

#[test]
fn deployment_files_follow_full_and_quick_build_output_rules() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    fixture.write("deploy/robots.txt", "before\n");
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    let source = fixture.path().join("deploy/robots.txt");
    fixture.write("deploy/robots.txt", "after\n");
    fs::File::options()
        .write(true)
        .open(&source)
        .expect("deployment source")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set deployment modification time");
    build_at(fixture.path(), BuildMode::Quick).expect("quick build updates deployment file");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/robots.txt")).expect("deployment file"),
        "after\n"
    );

    fs::remove_file(&source).expect("remove deployment file");
    build_at(fixture.path(), BuildMode::Quick).expect("quick build preserves stale output");
    assert!(fixture.path().join("public/robots.txt").is_file());

    build_at(fixture.path(), BuildMode::Full).expect("full build removes stale output");
    assert!(!fixture.path().join("public/robots.txt").exists());
}

#[test]
fn rejects_deployment_files_that_claim_reserved_output_paths() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    fixture.write("deploy/__heine/reload.js", "not allowed");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("reserved path");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("reserved for Heine internal use"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("deploy/__heine/reload.js"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_in_the_deployment_tree() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    let external = fixture.path().join("outside.txt");
    fs::write(&external, "outside").expect("external file");
    fs::create_dir_all(fixture.path().join("deploy")).expect("deployment directory");
    symlink(&external, fixture.path().join("deploy/linked.txt")).expect("deployment link");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink is rejected");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("symbolic link"), "{diagnostic}");
    assert!(diagnostic.contains("deploy/linked.txt"), "{diagnostic}");
}
