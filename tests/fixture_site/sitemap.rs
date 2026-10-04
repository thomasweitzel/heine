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
    assert!(!sitemap.contains("xmlns:xhtml"), "{sitemap}");
}

#[test]
fn sitemap_hreflang_uses_explicit_included_translation_groups() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[site]\norigin = 'https://example.org'\nbase_path = '/guide/'\n\n[sitemap]\nhreflang = true\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntranslationid = 'home'\n",
    );
    fixture.write(
        "content/de/index.page",
        "template = 'page.tera'\ntranslationid = 'home'\n",
    );
    fixture.write(
        "content/en/private.page",
        "template = 'page.tera'\ntranslationid = 'private'\nsitemap = false\n",
    );
    fixture.write(
        "content/de/private.page",
        "template = 'page.tera'\ntranslationid = 'private'\n",
    );
    fixture.write("templates/page.tera", "page");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    assert!(
        sitemap.contains("xmlns:xhtml=\"http://www.w3.org/1999/xhtml\""),
        "{sitemap}"
    );
    let english = "<url><loc>https://example.org/guide/index.html</loc><xhtml:link rel=\"alternate\" hreflang=\"de\" href=\"https://example.org/guide/de/index.html\"/><xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"https://example.org/guide/index.html\"/></url>";
    let german = "<url><loc>https://example.org/guide/de/index.html</loc><xhtml:link rel=\"alternate\" hreflang=\"de\" href=\"https://example.org/guide/de/index.html\"/><xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"https://example.org/guide/index.html\"/></url>";
    assert!(sitemap.contains(english), "{sitemap}");
    assert!(sitemap.contains(german), "{sitemap}");
    assert!(
        sitemap.contains("<url><loc>https://example.org/guide/de/private.html</loc></url>"),
        "{sitemap}"
    );
    assert!(!sitemap.contains("/private.html\"/>"), "{sitemap}");
}

#[test]
fn sitemap_hreflang_accepts_only_true() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\n\n[sitemap]\nhreflang = false\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("false is redundant");
    assert!(
        format!("{error:?}").contains("sitemap.hreflang accepts only true"),
        "{error:?}"
    );
}

#[test]
fn sitemap_hreflang_includes_only_the_first_translated_listing_page() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[site]\norigin = 'https://example.org'\n\n[sitemap]\nhreflang = true\n\n[collections.posts]\norder = 'title-asc'\n",
    );
    for (locale, title) in [("en", "One"), ("de", "Eins")] {
        fixture.write(
            format!("content/{locale}/blog/index.page"),
            "template = 'page.tera'\ntranslationid = 'blog'\n\n[pagination]\ncollection = 'posts'\nper_page = 1\n",
        );
        fixture.write(
            format!("content/{locale}/blog/one.page"),
            &format!("template = 'page.tera'\ntitle = '{title}'\ncollections = ['posts']\n"),
        );
        fixture.write(
            format!("content/{locale}/blog/two.page"),
            &format!("template = 'page.tera'\ntitle = '{title} two'\ncollections = ['posts']\n"),
        );
    }
    fixture.write("templates/page.tera", "page");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    assert!(
        sitemap.contains(
            "<url><loc>https://example.org/blog/index.html</loc><xhtml:link rel=\"alternate\" hreflang=\"de\" href=\"https://example.org/de/blog/index.html\"/><xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"https://example.org/blog/index.html\"/></url>"
        ),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("<url><loc>https://example.org/blog/page/2.html</loc></url>"),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("<url><loc>https://example.org/de/blog/page/2.html</loc></url>"),
        "{sitemap}"
    );
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
    assert!(error.to_string().contains("requires [sitemap]"));
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
fn taxonomy_sitemap_boundary_excludes_its_index_terms_and_pagers() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\n\n[sitemap]\n\n[collections.posts]\norder = 'title-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 1\nsitemap = false\n",
    );
    for (name, title) in [("one", "One"), ("two", "Two")] {
        fixture.write(
            format!("content/en/{name}.page"),
            &format!(
                "template = 'page.tera'\ntitle = '{title}'\ncollections = ['posts']\n[taxonomies]\ntags = ['Rust']\n"
            ),
        );
    }
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "index");
    fixture.write("templates/taxonomy-term.tera", "term");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    for path in [
        "public/tags/index.html",
        "public/tags/Rust.html",
        "public/tags/Rust/page/2.html",
    ] {
        assert!(fixture.path().join(path).is_file(), "{path}");
    }
    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    assert!(
        sitemap.contains("https://example.org/one.html"),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("https://example.org/two.html"),
        "{sitemap}"
    );
    assert!(!sitemap.contains("/tags/"), "{sitemap}");
}

#[test]
fn generated_listing_sitemap_settings_require_sitemap_and_accept_only_false() {
    let cases = [
        (
            "[collections.posts]\norder = 'title-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 10\nsitemap = false\n",
            "taxonomy tags sitemap requires [sitemap]",
        ),
        (
            "[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 10\nsitemap = false\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
            "series sitemap requires [sitemap]",
        ),
        (
            "[site]\norigin = 'https://example.org'\n\n[sitemap]\n\n[collections.posts]\norder = 'title-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 10\nsitemap = true\n",
            "taxonomy tags sitemap accepts only false",
        ),
        (
            "[site]\norigin = 'https://example.org'\n\n[sitemap]\n\n[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 10\nsitemap = true\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
            "series sitemap accepts only false",
        ),
        (
            "[site]\norigin = 'https://example.org'\n\n[sitemap]\ninclude_pagers = true\n",
            "sitemap.include_pagers accepts only false",
        ),
    ];

    for (config, message) in cases {
        let fixture = Fixture::new();
        fixture.write("heine.toml", config);
        let error = build_at(fixture.path(), BuildMode::Full).expect_err("configuration fails");
        let report = format!("{error:?}");
        assert!(report.contains(message), "{report}");
    }
}

#[test]
fn sitemap_can_exclude_derived_pagers_without_excluding_listing_families() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\n\n[sitemap]\ninclude_pagers = false\n\n[collections.posts]\norder = 'title-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 1\n\n[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 1\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
    );
    fixture.write(
        "content/en/blog/index.page",
        "template = 'listing.tera'\ntitle = 'Blog'\ncollections = []\n\n[pagination]\ncollection = 'posts'\nper_page = 1\n",
    );
    for (name, title, position) in [("one", "One", 10), ("two", "Two", 20)] {
        fixture.write(
            format!("content/en/{name}.page"),
            &format!(
                "template = 'page.tera'\ntitle = '{title}'\ncollections = ['posts']\n[taxonomies]\ntags = ['Rust']\n\n[[series]]\nid = 'guide'\nposition = {position}\n"
            ),
        );
    }
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/listing.tera", "listing");
    fixture.write("templates/taxonomy-index.tera", "taxonomy index");
    fixture.write("templates/taxonomy-term.tera", "taxonomy term");
    fixture.write("templates/series-index.tera", "series index");
    fixture.write("templates/series.tera", "series listing");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let sitemap =
        fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap output");
    for url in [
        "https://example.org/blog/index.html",
        "https://example.org/tags/index.html",
        "https://example.org/tags/Rust.html",
        "https://example.org/series/index.html",
        "https://example.org/series/guide/index.html",
    ] {
        assert!(sitemap.contains(url), "{url}: {sitemap}");
    }
    for url in [
        "https://example.org/blog/page/2.html",
        "https://example.org/tags/Rust/page/2.html",
        "https://example.org/series/guide/page/2.html",
    ] {
        assert!(!sitemap.contains(url), "{url}: {sitemap}");
    }
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
    assert!(
        error
            .to_string()
            .contains("cannot override an excluded directory")
    );
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
        error.to_string().contains("accepts only false"),
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
        error.to_string().contains("accepts only false"),
        "{error:?}"
    );
}

#[test]
fn directory_sitemap_boundary_requires_global_sitemap_generation() {
    let fixture = Fixture::new();
    fixture.write("content/en/_directory.toml", "[pages]\nsitemap = false\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("sitemap is required");
    assert!(
        error
            .to_string()
            .contains("pages.sitemap` requires [sitemap]"),
        "{error:?}"
    );
}
