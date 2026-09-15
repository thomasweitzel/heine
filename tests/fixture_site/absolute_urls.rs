use super::*;

#[test]
fn reports_a_missing_origin_when_a_template_uses_a_canonical_url() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ site.url }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("origin is required");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("canonical URLs require site.origin"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("site.origin is not configured"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
}

#[test]
fn reports_a_missing_origin_when_a_template_uses_a_resource_absolute_url() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ page.absolute_url }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("origin is required");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("canonical URLs require site.origin"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("page.tera:1"), "{diagnostic}");
}

#[test]
fn exposes_base_path_aware_absolute_urls_for_site_pages_and_assets() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org:8443'\nbase_path = '/guide/'\ntime_zone = 'Europe/Berlin'\n",
    );
    fixture.write("content/en/index.page", "template = 'index.tera'\n");
    fixture.write("content/en/post.page", "template = 'page.tera'\n");
    fixture.write("content/en/assets/logo.svg", "<svg></svg>");
    fixture.write(
        "templates/index.tera",
        "{% set post = page(id='post') %}{% set logo = asset(id='assets/logo.svg') %}{{ site.url }}|{{ page.absolute_url }}|{{ post.absolute_url }}|{{ logo.absolute_url }}",
    );
    fixture.write("templates/page.tera", "{{ page.absolute_url }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("index output"),
        "https://example.org:8443/guide/|https://example.org:8443/guide/index.html|https://example.org:8443/guide/post.html|https://example.org:8443/guide/assets/logo.svg"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/post.html")).expect("post output"),
        "https://example.org:8443/guide/post.html"
    );
}

#[test]
fn exposes_canonical_counterpart_urls_without_guessing_translation_paths() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\nbase_path = '/guide/'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntranslationid = 'home'\n",
    );
    fixture.write(
        "content/de/start.page",
        "template = 'page.tera'\ntranslationid = 'home'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ page.absolute_url }}|{% for locale, translation in page.translations %}{{ locale }}={{ translation.absolute_url }}{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("English output"),
        "https://example.org/guide/index.html|de=https://example.org/guide/de/start.html"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/start.html")).expect("German output"),
        "https://example.org/guide/de/start.html|en=https://example.org/guide/index.html"
    );
}
