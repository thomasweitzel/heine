use std::{
    fs,
    time::{Duration, SystemTime},
};

use heine::{BuildMode, BuildOptions, build_at, build_at_with_options};

use crate::Fixture;

fn multilingual_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[i18n.locales.pl]\ndirection = 'ltr'\n",
    );
    fixture.write("templates/page.tera", "{{ locale.code }}|{{ locale.direction }}|{{ page.id }}|{{ page.path }}|{{ data.site.name }}|{{ locale_data.navigation.label }}|{{ t(id='items', count=2) }}|{{ page.translations | length }}");
    fixture.write("data/site.toml", "name = 'Heine'\n");
    fixture.write("data/en/navigation.toml", "label = 'English'\n");
    fixture.write("data/de/navigation.toml", "label = 'Deutsch'\n");
    fixture.write("data/pl/navigation.toml", "label = 'Polski'\n");
    fixture.write(
        "locales/en/site.ftl",
        "items = { $count ->\n [one] one item\n*[other] { $count } items\n}\n",
    );
    fixture.write(
        "locales/de/site.ftl",
        "items = { $count ->\n [one] ein Element\n*[other] { $count } Elemente\n}\n",
    );
    fixture
}

#[test]
fn builds_explicit_locale_trees_with_data_fallback_and_translation_groups() {
    let fixture = multilingual_fixture();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntranslationid = 'home'\n",
    );
    fixture.write(
        "content/de/start.page",
        "template = 'page.tera'\ntranslationid = 'home'\n",
    );
    fixture.write("content/pl/index.page", "template = 'page.tera'\n");

    build_at(fixture.path(), BuildMode::Full).expect("localized build");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "en|ltr|index|index.html|Heine|English|\u{2068}2\u{2069} items|1"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/start.html")).unwrap(),
        "de|ltr|start|de/start.html|Heine|Deutsch|\u{2068}2\u{2069} Elemente|1"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/pl/index.html")).unwrap(),
        "pl|ltr|index|pl/index.html|Heine|Polski|\u{2068}2\u{2069} items|0"
    );
}

#[test]
fn renders_each_locale_with_its_configured_text_direction() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.ar]\ndirection = 'rtl'\n",
    );
    fixture.write(
        "templates/page.tera",
        "<html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"></html>",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/ar/index.page", "template = 'page.tera'\n");

    build_at(fixture.path(), BuildMode::Full).expect("localized build");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "<html lang=\"en\" dir=\"ltr\"></html>"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/ar/index.html")).unwrap(),
        "<html lang=\"ar\" dir=\"rtl\"></html>"
    );
}

#[test]
fn reports_an_invalid_locale_direction_at_its_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'sideways'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid direction");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("heine.toml:5:13")
            && diagnostic.contains("sideways")
            && diagnostic.contains("ltr")
            && diagnostic.contains("rtl"),
        "{diagnostic}"
    );
}

#[test]
fn reports_a_missing_locale_direction_at_its_locale_table() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing direction");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("heine.toml") && diagnostic.contains("missing field `direction`"),
        "{diagnostic}"
    );
}

#[test]
fn locale_assets_overlay_shared_assets_at_locale_urls() {
    let fixture = multilingual_fixture();
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='assets/images/logo.svg') %}{{ logo.url }}",
    );
    fixture.write("content/assets/images/logo.svg", "shared");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write("content/pl/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/assets/images/logo.svg", "deutsch");

    build_at(fixture.path(), BuildMode::Full).expect("localized build");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "/assets/images/logo.svg"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).unwrap(),
        "/de/assets/images/logo.svg"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/assets/images/logo.svg")).unwrap(),
        "shared"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/assets/images/logo.svg")).unwrap(),
        "deutsch"
    );
}

#[test]
fn asset_function_resolves_current_locale_assets() {
    let fixture = multilingual_fixture();
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='assets/logo.svg') %}{{ logo.url }}|{% if locale.code == 'en' %}{% set local = asset(id='english.svg') %}{{ local.url }}{% else %}{% set local = asset(id='german.svg') %}{{ local.url }}{% endif %}",
    );
    fixture.write("content/assets/logo.svg", "shared");
    fixture.write("content/en/english.svg", "english");
    fixture.write("content/de/assets/logo.svg", "deutsch logo");
    fixture.write("content/de/german.svg", "deutsch");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    build_at(fixture.path(), BuildMode::Full).expect("localized build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "/assets/logo.svg|/english.svg"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).unwrap(),
        "/de/assets/logo.svg|/de/german.svg"
    );
}

#[test]
fn locale_aware_resource_urls_honor_the_base_path() {
    let fixture = multilingual_fixture();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[i18n.locales.pl]\ndirection = 'ltr'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='assets/logo.svg') %}{% set guide = page(locale='de', id='guide') %}{{ logo.url }}|{{ guide.url }}",
    );
    fixture.write("content/assets/logo.svg", "shared");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/guide.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    build_at(fixture.path(), BuildMode::Full).expect("localized build with base path");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "/guide/assets/logo.svg|/guide/de/guide.html"
    );
}

#[test]
fn siblings_can_select_an_explicit_locale_for_an_explicit_page() {
    let fixture = multilingual_fixture();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();
    fixture.write(
        "content/en/blog/first.page",
        "template = 'page.tera'\nweight = 1\n",
    );
    fixture.write(
        "content/en/blog/second.page",
        "template = 'page.tera'\nweight = 2\n",
    );
    fixture.write(
        "content/de/blog/first.page",
        "template = 'page.tera'\nweight = 1\n",
    );
    fixture.write(
        "content/de/blog/second.page",
        "template = 'page.tera'\nweight = 2\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% if page.id == 'blog/first' %}{% for id in siblings(id='blog/first', locale='de') %}{{ id }}{% endfor %}{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("localized build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/first.html")).unwrap(),
        "blog/second"
    );
}

#[test]
fn explicit_template_page_links_select_the_requested_locale() {
    let fixture = multilingual_fixture();
    fixture.write(
        "templates/page.tera",
        "{% set install = page(locale='de', id='hilfe/installieren') %}{{ install.title }}|{{ install.url }}|{{ install.relative_url }}",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/de/hilfe/installieren.page",
        "template = 'page.tera'\ntitle = 'Installation'\n",
    );
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    build_at(fixture.path(), BuildMode::Full).expect("localized build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "Installation|/de/hilfe/installieren.html|./de/hilfe/installieren.html"
    );
}

#[test]
fn reports_an_unconfigured_explicit_page_locale_at_the_template_call_site() {
    let fixture = multilingual_fixture();
    fixture.write(
        "templates/page.tera",
        "{% set page = page(locale='fr', id='guide') %}{{ page.url }}",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unconfigured locale");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("templates/page.tera:1")
            && diagnostic.contains("locale \"fr\" is not configured"),
        "{diagnostic}"
    );
}

#[test]
fn markdown_page_links_are_resolved_with_the_current_or_explicit_locale() {
    let fixture = multilingual_fixture();
    fixture.write("templates/page.tera", "{{ page.content.body | markdown }}");
    fixture.write("templates/other.tera", "");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.body = 'body.md'\n",
    );
    fixture.write(
        "content/en/body.md",
        "[Current](page:guide) [German](page:de:hilfe/installieren#start)\n",
    );
    fixture.write("content/en/guide.page", "template = 'other.tera'\n");
    fixture.write(
        "content/de/hilfe/installieren.page",
        "template = 'other.tera'\n",
    );
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    build_at(fixture.path(), BuildMode::Full).expect("localized build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).unwrap();
    assert!(
        output.contains("href=\"/guide.html\"")
            && output.contains("href=\"/de/hilfe/installieren.html#start\""),
        "{output}"
    );
}

#[test]
fn duplicate_translationids_in_one_locale_report_the_second_declaration() {
    let fixture = multilingual_fixture();
    fixture.write("content/en/one.page", "translationid = 'same'\n");
    fixture.write("content/en/two.page", "translationid = 'same'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate group member");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/one.page")
            && diagnostic.contains("content/en/two.page")
            && diagnostic.contains("translation ID"),
        "{diagnostic}"
    );
}

#[test]
fn reports_invalid_fluent_syntax_at_the_localization_source() {
    let fixture = multilingual_fixture();
    fixture.write("locales/de/site.ftl", "items = { $count ->\n [one] one\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid Fluent source");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("locales/de/site.ftl:3")
            && diagnostic.contains("invalid Fluent resource"),
        "{diagnostic}"
    );
}

#[test]
fn reports_both_fluent_message_declarations() {
    let fixture = multilingual_fixture();
    fixture.write("locales/en/first.ftl", "welcome = First\n");
    fixture.write("locales/en/second.ftl", "welcome = Second\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate Fluent message");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("locales/en/first.ftl:1")
            && diagnostic.contains("locales/en/second.ftl:1")
            && diagnostic.contains("duplicate declaration")
            && diagnostic.contains("first declaration"),
        "{diagnostic}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn rejects_localization_paths_that_differ_only_by_unicode_normalization() {
    let fixture = multilingual_fixture();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("locales/en/café.ftl", "composed = composed\n");
    fixture.write("locales/en/cafe\u{301}.ftl", "decomposed = decomposed\n");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("normalization-equivalent localization paths are ambiguous");
    let message = error.to_string();

    assert!(message.contains("café.ftl"), "{message}");
    assert!(message.contains("cafe\u{301}.ftl"), "{message}");
    assert!(message.contains("Unicode normalization"), "{message}");
}

#[test]
fn reports_both_duplicate_declarations_in_one_fluent_resource() {
    let fixture = multilingual_fixture();
    fixture.write(
        "locales/en/site.ftl",
        "welcome = First\n\r\nwelcome = Second\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate Fluent message");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("locales/en/site.ftl:1")
            && diagnostic.contains("locales/en/site.ftl:3")
            && diagnostic.contains("duplicate declaration")
            && diagnostic.contains("first declaration"),
        "{diagnostic}"
    );
}

#[test]
fn reports_a_message_and_term_that_share_a_fluent_bundle_identity() {
    let fixture = multilingual_fixture();
    fixture.write("locales/en/site.ftl", "brand = Heine\n-brand = Heine\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("conflicting Fluent entries");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("locales/en/site.ftl:1")
            && diagnostic.contains("locales/en/site.ftl:2")
            && diagnostic.contains("duplicate declaration")
            && diagnostic.contains("first declaration"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_linked_content_and_localization_roots() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "page");
    let content = fixture.path().join("outside-content");
    fs::create_dir_all(content.join("en")).unwrap();
    fs::remove_dir_all(fixture.path().join("content")).unwrap();
    symlink(&content, fixture.path().join("content")).unwrap();
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("content root symlink");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content") && diagnostic.contains("symbolic link"),
        "{diagnostic}"
    );

    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");
    let locales = fixture.path().join("outside-locales");
    fs::create_dir_all(&locales).unwrap();
    symlink(&locales, fixture.path().join("locales")).unwrap();
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("locales root symlink");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("locales") && diagnostic.contains("symbolic link"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_broken_symbolic_links_for_shared_assets_and_locale_content() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "page");
    symlink(
        fixture.path().join("missing-assets"),
        fixture.path().join("content/assets"),
    )
    .expect("broken shared-assets symlink");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("shared assets symlink");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/assets") && diagnostic.contains("symbolic link"),
        "{diagnostic}"
    );

    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "page");
    fs::remove_dir_all(fixture.path().join("content/en")).expect("replace locale content");
    symlink(
        fixture.path().join("missing-locale-content"),
        fixture.path().join("content/en"),
    )
    .expect("broken locale-content symlink");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("locale content symlink");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en") && diagnostic.contains("symbolic link"),
        "{diagnostic}"
    );
}

#[test]
fn reports_the_first_unexpected_content_root_entry_deterministically() {
    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "page");
    fixture.write("content/z-unexpected", "z");
    fixture.write("content/a-unexpected", "a");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unexpected content root");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/a-unexpected") && !diagnostic.contains("z-unexpected"),
        "{diagnostic}"
    );
}

#[test]
fn does_not_hide_current_locale_formatting_errors_with_fallback() {
    let fixture = multilingual_fixture();
    fixture.write("templates/page.tera", "{{ t(id='items') }}");
    fs::create_dir_all(fixture.path().join("content/en")).unwrap();
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing Fluent argument");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("render error in template \"page.tera\"")
            && !diagnostic.contains("2 items"),
        "{diagnostic}"
    );
}

#[test]
fn formats_page_dates_for_the_locale_being_rendered() {
    let fixture = multilingual_fixture();
    fixture.write(
        "templates/page.tera",
        "{% set date = format_date(value=page.published, style='long') %}<time datetime=\"{{ page.published }}\">{{ date }}</time>",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00\n",
    );
    fixture.write(
        "content/de/index.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00\n",
    );
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    build_at(fixture.path(), BuildMode::Full).expect("localized dates");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).unwrap(),
        "<time datetime=\"2026-08-14T07:30:00Z\">August 14, 2026</time>"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).unwrap(),
        "<time datetime=\"2026-08-14T07:30:00Z\">14. August 2026</time>"
    );
}

#[test]
fn reports_invalid_date_formatting_options_at_the_template_call_site() {
    let fixture = multilingual_fixture();
    fixture.write(
        "templates/page.tera",
        "{{ format_date(value=page.published, style='full') }}",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00\n",
    );
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid date style");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("templates/page.tera:1")
            && diagnostic.contains("date style must be")
            && diagnostic.contains("short\"")
            && diagnostic.contains("medium\"")
            && diagnostic.contains("long\""),
        "{diagnostic}"
    );
}

#[test]
fn reports_an_invalid_locale_at_its_table_key() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de-at]\ndirection = 'ltr'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("non-canonical locale");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("heine.toml:7:15")
            && diagnostic.contains("de-at")
            && diagnostic.contains("canonical BCP-47 spelling"),
        "{diagnostic}"
    );
}

#[test]
fn reports_an_empty_locale_table_at_the_configuration_location() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales]\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("empty locale set");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("heine.toml:2:18")
            && diagnostic.contains("i18n.locales")
            && diagnostic.contains("locale table"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_for_locale_data_roots() {
    use std::os::unix::fs::symlink;

    let fixture = multilingual_fixture();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();
    let external = fixture.path().join("outside-data");
    fs::create_dir_all(&external).unwrap();
    fs::remove_dir_all(fixture.path().join("data/de")).expect("replace locale data directory");
    symlink(&external, fixture.path().join("data/de")).expect("locale data symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("symlink is rejected");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("data/de") && diagnostic.contains("symbolic link"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_a_broken_locale_directory_symbolic_link() {
    use std::os::unix::fs::symlink;

    let fixture = multilingual_fixture();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();
    fs::remove_dir_all(fixture.path().join("locales/de")).expect("replace locale directory");
    symlink(
        fixture.path().join("missing-locale-directory"),
        fixture.path().join("locales/de"),
    )
    .expect("broken locale symlink");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("locale symlink is rejected");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("locales/de") && diagnostic.contains("symbolic link"),
        "{diagnostic}"
    );
}

#[test]
fn quick_builds_rerender_only_the_locale_whose_locale_data_changed() {
    let fixture = multilingual_fixture();
    fixture.write("templates/page.tera", "{{ locale_data.navigation.label }}");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();
    build_at(fixture.path(), BuildMode::Full).expect("initial build");

    let output = fixture.path().join("public/de/index.html");
    fs::File::options()
        .write(true)
        .open(&output)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now()))
        .unwrap();
    let data = fixture.path().join("data/de/navigation.toml");
    fixture.write("data/de/navigation.toml", "label = 'Aktualisiert'\n");
    fs::File::options()
        .write(true)
        .open(&data)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .unwrap();

    build_at_with_options(fixture.path(), BuildOptions::new(BuildMode::Quick))
        .expect("quick build");
    assert_eq!(fs::read_to_string(output).unwrap(), "Aktualisiert");
}

#[test]
fn quick_builds_rerender_pages_for_changed_fluent_resources_and_fallbacks() {
    let fixture = multilingual_fixture();
    fixture.write("templates/page.tera", "{{ t(id='label') }}");
    fixture.write("locales/en/site.ftl", "label = English\n");
    fixture.write("locales/de/site.ftl", "label = Deutsch\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write("content/pl/index.page", "template = 'page.tera'\n");
    build_at(fixture.path(), BuildMode::Full).expect("initial build");

    let german = fixture.path().join("locales/de/site.ftl");
    fixture.write("locales/de/site.ftl", "label = Aktualisiert\n");
    fs::File::options()
        .write(true)
        .open(&german)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .unwrap();
    build_at_with_options(fixture.path(), BuildOptions::new(BuildMode::Quick))
        .expect("quick build after locale resource change");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).unwrap(),
        "Aktualisiert"
    );

    let english = fixture.path().join("locales/en/site.ftl");
    fixture.write("locales/en/site.ftl", "label = Updated English\n");
    fs::File::options()
        .write(true)
        .open(&english)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(2)))
        .unwrap();
    build_at_with_options(fixture.path(), BuildOptions::new(BuildMode::Quick))
        .expect("quick build after fallback resource change");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/pl/index.html")).unwrap(),
        "Updated English"
    );
}

#[test]
fn rejects_a_default_locale_asset_that_would_overlay_a_shared_asset() {
    let fixture = multilingual_fixture();
    fixture.write("content/assets/logo.svg", "shared");
    fixture.write("content/en/assets/logo.svg", "default locale");
    fs::create_dir_all(fixture.path().join("content/de")).unwrap();
    fs::create_dir_all(fixture.path().join("content/pl")).unwrap();

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("asset collision");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/assets/logo.svg")
            && diagnostic.contains("content/en/assets")
            && diagnostic.contains("logo.svg"),
        "{diagnostic}"
    );
}
