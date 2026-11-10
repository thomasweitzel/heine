use std::{fs, process::Command};

#[test]
fn command_creates_a_buildable_site_and_rejects_existing_destinations() {
    let parent = tempfile::tempdir().expect("temporary parent");
    let site = parent.path().join("my site");
    let binary = env!("CARGO_BIN_EXE_heine");
    let output = Command::new(binary)
        .arg("new")
        .arg(&site)
        .current_dir(parent.path())
        .output()
        .expect("run new");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(site.join(".gitignore").is_file());
    let build = Command::new(binary)
        .current_dir(&site)
        .output()
        .expect("build site");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    for file in [
        "index.html",
        "about.html",
        "blog/index.html",
        "search/index.html",
        "tags/index.html",
    ] {
        assert!(site.join("public").join(file).is_file(), "missing {file}");
    }
    let before = fs::read(site.join("README.md")).expect("read README");
    let again = Command::new(binary)
        .arg("new")
        .arg(&site)
        .output()
        .expect("repeat new");
    assert!(!again.status.success());
    assert_eq!(
        before,
        fs::read(site.join("README.md")).expect("preserved README")
    );
}

#[test]
fn new_rejects_build_options_and_missing_parent() {
    let parent = tempfile::tempdir().expect("temporary parent");
    let binary = env!("CARGO_BIN_EXE_heine");
    for args in [
        vec!["--quick", "new", "site"],
        vec!["--root", ".", "new", "site"],
        vec!["new", "site", "--quick"],
        vec!["new", "missing/site"],
    ] {
        let output = Command::new(binary)
            .args(&args)
            .current_dir(parent.path())
            .output()
            .expect("run new");
        assert!(!output.status.success(), "unexpected success: {args:?}");
    }
    assert!(!parent.path().join("site").exists());
}

#[cfg(unix)]
#[test]
fn new_does_not_follow_an_existing_destination_symlink() {
    use std::os::unix::fs::symlink;
    let parent = tempfile::tempdir().expect("temporary parent");
    let target = parent.path().join("target");
    fs::create_dir(&target).expect("target directory");
    let destination = parent.path().join("site");
    symlink(&target, &destination).expect("destination symlink");
    let error = heine::create_new_site(&destination).expect_err("symlink must not be followed");
    assert!(
        error
            .to_string()
            .contains("destination that does not exist")
    );
    assert_eq!(fs::read_dir(&target).expect("target entries").count(), 0);
}

#[test]
fn prepared_templates_support_series_toc_math_and_pagination() {
    let temporary = tempfile::tempdir().expect("temporary parent");
    let site = temporary.path().join("site");
    heine::create_new_site(&site).expect("create site");

    let readme = fs::read_to_string(site.join("README.md")).expect("README");
    let code = |language: &str, index: usize| {
        readme
            .split(&format!("```{language}\n"))
            .nth(index + 1)
            .expect("documented example")
            .split("```")
            .next()
            .expect("example end")
            .to_owned()
    };
    let append = |path: &str, text: &str| {
        use std::io::Write;
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(site.join(path))
                .expect("open fixture"),
            "\n{text}"
        )
        .expect("append fixture");
    };
    append("heine.toml", &code("toml", 0));
    append(
        "heine.toml",
        "[series.definitions.more]\ntitle_key = 'series-more'",
    );
    append("locales/en/site.ftl", &code("ftl", 0));
    append("locales/en/site.ftl", "series-more = More notes");
    append("content/en/blog/first/index.page", &code("toml", 1));
    append(
        "content/en/blog/first/index.page",
        "[[series]]\nid = 'more'\nposition = 10",
    );
    append(
        "content/en/blog/first/index.page",
        "[toc]\ncontent = 'main'\nmin_level = 1\nmax_level = 6",
    );
    append(
        "content/en/blog/first/main.md",
        "# Overview\n\n## Details\n\nMath: $x^2$. A note.[^a]\n\n### Nested detail\n\n#### Level four\n\n##### Level five\n\n###### Level six\n\n| One | Two |\n| --- | --- |\n| A | B |\n\n[^a]: A footnote.",
    );
    let listing = site.join("content/en/blog/index.page");
    let input = fs::read_to_string(&listing).expect("listing");
    fs::write(listing, input.replace("per_page = 10", "per_page = 1")).expect("paginate");

    heine::build_at(&site, heine::BuildMode::Full).expect("build configured site");
    for path in [
        "series/index.html",
        "series/notes/index.html",
        "blog/page/2.html",
        "tags/Notes.html",
    ] {
        assert!(site.join("public").join(path).is_file(), "missing {path}");
    }
    let post = fs::read_to_string(site.join("public/blog/first/index.html")).expect("post");
    assert!(
        post.contains("table-of-contents") || post.contains("class=\"toc\""),
        "{post}"
    );
    assert!(post.contains("<math"), "{post}");
    assert!(post.contains("footnote"), "{post}");
    for depth in 1..=6 {
        assert!(post.contains(&format!("toc-depth-{depth}")), "{post}");
    }
    assert!(post.contains("heine-table-scroll"), "{post}");
    assert!(post.contains("tabindex=\"0\""), "{post}");
    assert!(post.contains("role=\"region\""), "{post}");
    assert!(post.contains("aria-label=\"Scrollable table\""), "{post}");
    assert_eq!(
        post.matches("class=\"series-navigation\"").count(),
        2,
        "{post}"
    );
    assert!(post.contains("Series: \u{2068}Notes\u{2069}"), "{post}");
    assert!(
        post.contains("Series: \u{2068}More notes\u{2069}"),
        "{post}"
    );
}
