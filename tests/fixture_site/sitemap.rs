use super::*;

#[test]
fn generates_canonical_sitemap_entries_for_rendered_pages() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\nbase_path = '/guide/'\ntime_zone = 'Europe/Berlin'\n\n[sitemap]\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00\n",
    );
    fixture.write(
        "content/en/hidden.page",
        "template = 'page.tera'\nsitemap = false\n",
    );
    fixture.write("templates/page.tera", "page");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    assert!(
        sitemap.contains("https://example.org/guide/index.html"),
        "{sitemap}"
    );
    assert!(sitemap.contains("2026-08-14T07:30:00Z"), "{sitemap}");
    assert!(!sitemap.contains("hidden.html"), "{sitemap}");
}

#[test]
fn sitemap_requires_an_origin_and_page_opt_out_requires_sitemap() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[sitemap]\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("origin is required");
    assert!(format!("{error:?}").contains("requires site.origin"));

    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\nsitemap = false\n",
    );
    fixture.write("templates/page.tera", "page");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("sitemap is required");
    assert!(format!("{error:?}").contains("requires [sitemap]"));
}

#[test]
fn sitemap_includes_generated_taxonomy_pages_without_authored_lastmod() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[sitemap]\n\n[collections.posts]\norder = 'published-desc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 10\n",
    );
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00\ncollections = ['posts']\n[taxonomies]\ntags = ['Rust']\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "index");
    fixture.write("templates/taxonomy-term.tera", "term");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    assert!(
        sitemap.contains("https://example.org/tags/index.html"),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("https://example.org/tags/Rust.html"),
        "{sitemap}"
    );
    assert_eq!(sitemap.matches("<lastmod>").count(), 1, "{sitemap}");
}

#[test]
fn directory_sitemap_boundary_excludes_descendants_and_their_pagers() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[sitemap]\n\n[collections.posts]\norder = 'published-desc'\n",
    );
    fixture.write(
        "content/en/drafts/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\nsitemap = false\n",
    );
    fixture.write(
        "content/en/drafts/one.page",
        "published = 2026-08-14T09:30:00\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/drafts/two.page",
        "published = 2026-08-15T09:30:00\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/drafts/index.page",
        "[pagination]\ncollection = 'posts'\nper_page = 1\n",
    );
    fixture.write("templates/page.tera", "page");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    assert!(!sitemap.contains("drafts/one.html"), "{sitemap}");
    assert!(!sitemap.contains("drafts/two.html"), "{sitemap}");
    assert!(!sitemap.contains("drafts/index.html"), "{sitemap}");
    assert!(!sitemap.contains("drafts/page/2/index.html"), "{sitemap}");
}

#[test]
fn sitemap_boundary_reports_a_page_attempting_to_reverse_it() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\n\n[sitemap]\n",
    );
    fixture.write(
        "content/en/drafts/_directory.toml",
        "[pages]\nsitemap = false\n",
    );
    fixture.write(
        "content/en/drafts/preview.page",
        "template = 'page.tera'\nsitemap = true\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("override is rejected");
    let rendered = format!("{error:?}");
    assert!(rendered.contains("cannot override an excluded directory"));
    assert!(rendered.contains("preview.page"));
    assert!(rendered.contains("_directory.toml"));
    assert!(rendered.contains("recursive sitemap boundary"));
}

#[test]
fn sitemap_settings_accept_only_false() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\n\n[sitemap]\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\nsitemap = true\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("true is invalid");
    assert!(
        format!("{error:?}").contains("accepts only false"),
        "{error:?}"
    );

    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\n\n[sitemap]\n",
    );
    fixture.write("content/en/_directory.toml", "[pages]\nsitemap = true\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("true is invalid");
    assert!(
        format!("{error:?}").contains("accepts only false"),
        "{error:?}"
    );
}

#[test]
fn directory_sitemap_boundary_requires_global_sitemap_generation() {
    let fixture = Fixture::new();
    fixture.write("content/en/_directory.toml", "[pages]\nsitemap = false\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("sitemap is required");
    assert!(
        format!("{error:?}").contains("pages.sitemap` requires [sitemap]"),
        "{error:?}"
    );
}
