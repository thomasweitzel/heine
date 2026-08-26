use super::*;

#[test]
fn asset_helper_resolves_versioned_assets_and_integrity() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[assets]\nintegrity = 'sha256'\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/css/styles.v2.css", "body {}");
    fixture.write(
        "templates/page.tera",
        "{% set asset = asset(id='css/styles.css') %}{{ asset.url }}|{{ asset.integrity }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.starts_with("/css/styles.v2.css|sha256-"), "{output}");
}

#[test]
fn quick_builds_rerender_pages_when_asset_versions_change() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/css/styles.v2.css", "body { color: navy; }");
    fixture.write(
        "templates/page.tera",
        "{% set style = asset(id='css/styles.css') %}{{ style.url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");
    fixture.write("content/en/css/styles.v3.css", "body { color: teal; }");
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/en/css/styles.v3.css"))
        .expect("new versioned asset")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set asset modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build succeeds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "/css/styles.v3.css"
    );
}

#[test]
fn quick_builds_rerender_integrity_when_an_asset_changes() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[assets]\nintegrity = 'sha256'\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/app.js", "before");
    fixture.write(
        "templates/page.tera",
        "{% set script = asset(id='app.js') %}{{ script.integrity }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");
    let before = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    fixture.write("content/en/app.js", "after");
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/en/app.js"))
        .expect("asset")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set asset modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build succeeds");
    let after = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");

    assert!(after.starts_with("sha256-"), "{after}");
    assert_ne!(after, before);
}

#[test]
fn asset_helper_exposes_detected_image_dimensions() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    write_bytes(&fixture, "content/en/images/pixel.PNG", PIXEL_PNG);
    fixture.write(
        "templates/page.tera",
        "{% set image = asset(id='images/pixel.PNG') %}{{ image.url }}|{{ image.width }}|{{ image.height }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "/images/pixel.PNG|1|1"
    );
}

#[test]
fn resource_urls_encode_literal_source_filenames_once() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/über space#%.page", "template = 'page.tera'\n");
    fixture.write("content/en/images/logo #%.svg", "<svg />");
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='images/logo #%.svg') %}{% set page = page(id='über space#%') %}{{ logo.url }}|{{ logo.relative_url }}|{{ page.url }}|{{ page.relative_url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "/images/logo%20%23%25.svg|./images/logo%20%23%25.svg|/%C3%BCber%20space%23%25.html|./%C3%BCber%20space%23%25.html"
    );
    assert!(fixture.path().join("public/über space#%.html").is_file());
    assert!(fixture.path().join("public/images/logo #%.svg").is_file());
}

#[test]
fn resource_filters_accept_content_root_relative_paths() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[build]\noutput_dir = 'public/guide'\n",
    );
    fixture.write("content/en/blog/post.page", "template = 'page.tera'\n");
    fixture.write("content/en/css/styles.v2.css", "body {}");
    write_bytes(&fixture, "content/en/images/pixel.png", PIXEL_PNG);
    fixture.write(
        "templates/page.tera",
        "{{ '/css/styles.css' | latest }}|{% set size = '/images/pixel.png' | dimensions %}{{ size.width }}x{{ size.height }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/blog/post.html")).expect("output"),
        "/guide/css/styles.v2.css|1x1"
    );
}

#[test]
fn latest_uses_the_logical_asset_id_for_locale_relative_urls() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[build]\noutput_dir = 'public/guide'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/blog/post.page", "template = 'page.tera'\n");
    fixture.write("content/de/blog/styles.v2.css", "body {}\n");
    fixture.write(
        "templates/page.tera",
        "{% if page.dir == 'blog' %}{{ 'styles.css' | latest }}{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/de/blog/post.html")).expect("output"),
        "styles.v2.css"
    );
}

#[test]
fn reports_a_missing_latest_asset_at_the_filter_call_site() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ 'missing.css' | latest }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset is missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("missing.css"), "{diagnostic}");
}

#[test]
fn reports_a_missing_dimensions_asset_at_the_filter_call_site() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ 'missing.png' | dimensions }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset is missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("missing.png"), "{diagnostic}");
}

#[test]
fn markdown_asset_references_resolve_versioned_assets_at_the_content_root() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[build]\noutput_dir = 'public/guide'\n",
    );
    fixture.write(
        "content/en/blog/post.page",
        "template = 'page.tera'\ncontent.main = 'post/body.md'\n",
    );
    fixture.write(
        "content/en/blog/post/body.md",
        "![Sunset](asset:blog/post/images/sunset.jpg)\n\n[Local](images/relative.jpg)",
    );
    fixture.write(
        "content/en/blog/post/images/sunset.v2.jpg",
        "not decoded as an image",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output =
        fs::read_to_string(fixture.path().join("public/guide/blog/post.html")).expect("output");

    assert!(
        output.contains("<img src=\"/guide/blog/post/images/sunset.v2.jpg\" alt=\"Sunset\" />"),
        "{output}"
    );
    assert!(output.contains("href=\"images/relative.jpg\""), "{output}");
}

#[test]
fn reports_an_invalid_markdown_asset_at_the_content_source() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "![Missing](asset:images/missing.png)",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset should be missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("invalid asset reference here"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("images/missing.png"), "{diagnostic}");
}

#[test]
fn reports_an_invalid_markdown_asset_in_a_template_at_the_filter_call_site() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{{ '![Missing](asset:images/missing.png)' | markdown }}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset should be missing");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("images/missing.png"), "{diagnostic}");
}

#[test]
fn resource_helpers_honor_the_configured_base_path() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[build]\noutput_dir = 'public/guide'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/blog/post.page", "template = 'page.tera'\n");
    fixture.write("content/en/css/styles.v2.css", "body {}\n");
    fixture.write(
        "templates/page.tera",
        "{% set style = asset(id='css/styles.css') %}{% set home = page(id='index') %}{{ style.url }}|{{ style.relative_url }}|{{ home.url }}|{{ home.relative_url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/blog/post.html"))
            .expect("post output"),
        "/guide/css/styles.v2.css|../css/styles.v2.css|/guide/index.html|../index.html"
    );
    assert!(
        fixture
            .path()
            .join("public/guide/css/styles.v2.css")
            .is_file()
    );
}

#[test]
fn reports_the_template_location_for_an_invalid_asset_id() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set style = asset(id='/css/styles.css') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset ID should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("asset id must be"), "{diagnostic}");
}

#[test]
fn rejects_backslashes_in_asset_ids() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        r"{% set style = asset(id='css\\styles.css') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset ID is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("asset id must be"), "{diagnostic}");
}

#[test]
fn reports_the_template_location_for_an_unknown_page_id() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set missing = page(id='does-not-exist') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("page ID should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("does-not-exist"), "{diagnostic}");
}

#[test]
fn reports_the_template_location_for_an_invalid_page_id() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set missing = page(id='../outside') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("page ID should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("page id must be"), "{diagnostic}");
}

#[test]
fn page_exists_only_suppresses_a_missing_rendered_page() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% if page_exists(id='does-not-exist') %}present{% else %}missing{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("missing page is a normal false result");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "missing"
    );

    fixture.write(
        "templates/page.tera",
        "{% if page_exists(locale='not a locale', id='does-not-exist') %}present{% endif %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid locale must fail");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("not a locale"), "{diagnostic}");
}

#[test]
fn page_function_exposes_page_fields_and_links() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'index.tera'\n");
    fixture.write(
        "content/en/guide.page",
        "template = 'page.tera'\ntitle = 'Guide'\ncontent.main = 'guide.txt'\n",
    );
    fixture.write("content/en/guide.txt", "Useful details");
    fixture.write(
        "templates/index.tera",
        "{% set guide = page(id='guide') %}{{ guide.title }}|{{ guide.content.main }}|{{ guide.url }}|{{ guide.relative_url }}",
    );
    fixture.write("templates/page.tera", "guide");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "Guide|Useful details|/guide.html|./guide.html"
    );
}

#[test]
fn taxonomies_generate_locale_local_index_and_term_listings() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 1\n",
    );
    fixture.write("content/en/index.page", "template = 'index.tera'\n");
    fixture.write(
        "content/en/guide.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['news']\n",
    );
    fixture.write(
        "content/en/alpha.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['news']\n",
    );
    fixture.write(
        "templates/index.tera",
        "{% set alpha = page(id='alpha') %}{% for term in alpha.taxonomies.tags %}{{ term.name }}|{{ term.relative_url }}{% endfor %}",
    );
    fixture.write(
        "templates/page.tera",
        "{% for term in page.taxonomies.tags %}{{ term.name }}|{{ term.url }}|{{ term.relative_url }}{% endfor %}",
    );
    fixture.write(
        "templates/taxonomy-index.tera",
        "{% for term in taxonomy.terms %}{{ term.name }}|{{ term.count }}|{{ term.url }}|{{ term.relative_url }}{% endfor %}",
    );
    fixture.write(
        "templates/taxonomy-term.tera",
        "{{ taxonomy.term.name }}|{{ pagination.page_number }}|{% for page in pagination.pages %}{{ page.id }}{% endfor %}|{{ pagination_links() | length }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/alpha.html")).unwrap(),
        "news|/tags/news.html|./tags/news.html"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "news|./tags/news.html"
    );
    assert!(
        fs::read_to_string(fixture.path().join("public/tags/index.html"))
            .unwrap()
            .contains("news|2|/tags/news.html|../tags/news.html")
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/tags/news.html")).unwrap(),
        "news|1|alpha|2"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/tags/news/page/2.html")).unwrap(),
        "news|2|guide|2"
    );
}

#[test]
fn rejects_invalid_or_ambiguous_resource_function_arguments_at_the_call_site() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set page = page(id='index', local='en') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown keyword fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("local"), "{diagnostic}");
    assert!(diagnostic.contains("does not accept"), "{diagnostic}");

    fixture.write(
        "templates/page.tera",
        "{{ siblings(locale='de') | length }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("locale needs an ID");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("together with id"), "{diagnostic}");
}
