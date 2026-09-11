use super::*;

fn config() -> &'static str {
    "[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 2\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n"
}

fn resources(fixture: &Fixture) {
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\n",
    );
    fixture.write("templates/page.tera", "{{ page.title }}:{% for membership in page.series %}{{ membership.id }} {{ membership.number }} {{ membership.position }}/{{ membership.total }} {% if membership.previous %}{{ membership.previous.title }}{% endif %}{% if membership.next %}{{ membership.next.title }}{% endif %}{% endfor %}");
    fixture.write("templates/series-index.tera", "{{ series.title }}:{% for item in series.series %}{{ item.title }}={{ item.count }};{% endfor %}");
    fixture.write("templates/series.tera", "{{ series.title }}:{{ pagination.page_number }}/{{ pagination.page_count }}:{% for item in pagination.pages %}{{ item.title }};{% endfor %}");
}

#[test]
fn preserves_authored_membership_order_and_resolves_each_series_independently() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 10\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n\n[series.definitions.reference]\ntitle_key = 'series-reference-title'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\nseries-reference-title = Reference\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% for membership in page.series %}{{ membership.id }}={{ membership.number }}/{{ membership.total }}:{% if membership.previous %}{{ membership.previous.title }}{% endif %}:{% if membership.next %}{{ membership.next.title }}{% endif %};{% endfor %}",
    );
    fixture.write("templates/series-index.tera", "{{ series.title }}");
    fixture.write("templates/series.tera", "{{ series.title }}");
    fixture.write(
        "content/en/first.page",
        "template = 'page.tera'\ntitle = 'First'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "content/en/middle.page",
        "template = 'page.tera'\ntitle = 'Middle'\n[[series]]\nid = 'reference'\nposition = 30\n\n[[series]]\nid = 'guide'\nposition = 20\n",
    );
    fixture.write(
        "content/en/last.page",
        "template = 'page.tera'\ntitle = 'Last'\n[[series]]\nid = 'reference'\nposition = 10\n",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/middle.html")).expect("member page"),
        "reference=2/2:Last:;guide=2/2:First:;"
    );
}

#[test]
fn generates_checked_series_views_and_paginated_listings() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write(
        "content/en/one.page",
        "template = 'page.tera'\ntitle = 'One'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "content/en/two.page",
        "template = 'page.tera'\ntitle = 'Two'\n[[series]]\nid = 'guide'\nposition = 20\n",
    );
    fixture.write(
        "content/en/three.page",
        "template = 'page.tera'\ntitle = 'Three'\n[[series]]\nid = 'guide'\nposition = 30\n",
    );
    resources(&fixture);

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/series/index.html")).expect("index"),
        "Series:Guide=3;"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/series/guide/index.html")).expect("listing"),
        "Guide:1/2:One;Two;"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/series/guide/page/2.html")).expect("pager"),
        "Guide:2/2:Three;"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/two.html")).expect("member"),
        "Two:guide 2 20/3 OneThree"
    );
}

#[test]
fn rejects_duplicate_series_positions_at_both_memberships() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write(
        "content/en/one.page",
        "template = 'page.tera'\ntitle = 'One'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "content/en/two.page",
        "template = 'page.tera'\ntitle = 'Two'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    resources(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate position fails");
    let report = format!("{error:?}");
    assert!(report.contains("content/en/two.page:5"), "{report}");
    assert!(report.contains("content/en/one.page:5"), "{report}");
}

#[test]
fn checks_series_membership_at_its_authored_values() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post'\n[[series]]\nid = 'missing'\nposition = 0\n",
    );
    resources(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown Series fails");
    let report = format!("{error:?}");
    assert!(report.contains("content/en/post.page:4"), "{report}");
    assert!(report.contains("missing"), "{report}");

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post'\n[[series]]\nid = 'guide'\nposition = 0\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("zero position fails");
    let report = format!("{error:?}");
    assert!(report.contains("content/en/post.page:5"), "{report}");
    assert!(error.to_string().contains("positive integer"), "{report}");

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post'\n[[series]]\nid = 'guide'\nposition = -1\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("negative position fails");
    let report = format!("{error:?}");
    assert!(report.contains("content/en/post.page:5"), "{report}");
    assert!(error.to_string().contains("positive integer"), "{report}");
}

#[test]
fn checks_member_renderability_titles_and_definition_messages_at_their_sources() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write(
        "content/en/post.page",
        "[[series]]\nid = 'guide'\nposition = 10\n",
    );
    resources(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unrendered member fails");
    let report = format!("{error:?}");
    assert!(report.contains("content/en/post.page:2"), "{report}");
    assert!(report.contains("not rendered"), "{report}");

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("untitled member fails");
    let report = format!("{error:?}");
    assert!(report.contains("content/en/post.page:3"), "{report}");
    assert!(report.contains("has no title"), "{report}");

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write("locales/en/site.ftl", "series-index-title = Series\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing title message fails");
    let report = format!("{error:?}");
    assert!(report.contains("heine.toml:9"), "{report}");
    assert!(report.contains("series-guide-title"), "{report}");
}

#[test]
fn rejects_portably_colliding_series_definition_routes_before_membership() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 10\n\n[series.definitions.Guide]\ntitle_key = 'series-guide-title'\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
    );
    resources(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("colliding unused Series definitions fail");
    let report = format!("{error:?}");
    assert!(report.contains("heine.toml:8"), "{report}");
    assert!(report.contains("heine.toml:11"), "{report}");
    assert!(report.contains("portable filesystems"), "{report}");
}

#[test]
fn series_outputs_participate_in_checked_output_collisions() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write("content/en/series/index.html", "copied asset");
    resources(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("output collision fails");
    let report = format!("{error:?}");
    assert!(report.contains("heine.toml:2"), "{report}");
    assert!(report.contains("content/en/series/index.html"), "{report}");
}

#[test]
fn checks_series_listing_template_even_when_every_locale_is_empty() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\n",
    );
    fixture.write("templates/series-index.tera", "{{ series.title }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("missing listing template fails for an empty Series");
    let report = format!("{error:?}");
    assert!(report.contains("series.tera"), "{report}");
    assert!(report.contains("heine.toml:4"), "{report}");
}

#[test]
fn includes_generated_series_pages_in_the_sitemap() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[sitemap]\n\n[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 1\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
    );
    fixture.write(
        "content/en/one.page",
        "template = 'page.tera'\ntitle = 'One'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "content/en/two.page",
        "template = 'page.tera'\ntitle = 'Two'\n[[series]]\nid = 'guide'\nposition = 20\n",
    );
    resources(&fixture);

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let sitemap = fs::read_to_string(fixture.path().join("public/sitemap.xml")).expect("sitemap");
    assert!(
        sitemap.contains("https://example.org/series/index.html"),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("https://example.org/series/guide/index.html"),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("https://example.org/series/guide/page/2.html"),
        "{sitemap}"
    );
}

#[test]
fn quick_build_rerenders_series_listings_for_a_member_change() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", config());
    fixture.write(
        "content/en/one.page",
        "template = 'page.tera'\ntitle = 'Before'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/series-index.tera", "index");
    fixture.write(
        "templates/series.tera",
        "{% for page in pagination.pages %}{{ page.title }}{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("initial build");
    fixture.write(
        "content/en/one.page",
        "template = 'page.tera'\ntitle = 'After'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/en/one.page"))
        .expect("changed member")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set member modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/series/guide/index.html"))
            .expect("Series output"),
        "After"
    );
}

#[test]
fn checked_series_template_lookups_distinguish_empty_local_membership() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 10\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\n",
    );
    fixture.write(
        "locales/de/site.ftl",
        "series-index-title = Reihen\nseries-guide-title = Anleitung\n",
    );
    fixture.write(
        "content/en/one.page",
        "template = 'page.tera'\ntitle = 'One'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "content/de/one.page",
        "template = 'page.tera'\ntitle = 'Eins'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% if series_has_members(id='guide', locale='de') %}bad{% endif %}{% if locale.code == 'en' %}{% set current = series(id='guide') %}{{ current.title }}{% endif %}",
    );
    fixture.write("templates/series-index.tera", "{{ series.title }}");
    fixture.write("templates/series.tera", "{{ series.title }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/one.html")).expect("English page"),
        "Guide"
    );
    fixture.write(
        "templates/page.tera",
        "{% set current = series(id='guide') %}{{ current.title }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("German direct lookup fails");
    let report = format!("{error:?}");
    assert!(report.contains("has no members"), "{report}");
}
