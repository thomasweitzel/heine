use std::{
    fs,
    time::{Duration, SystemTime},
};

use super::*;

fn taxonomy_config() -> &'static str {
    "[collections.posts]\norder = 'id-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 20\n"
}

fn templates(fixture: &Fixture) {
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "index");
    fixture.write("templates/taxonomy-term.tera", "term");
}

#[test]
fn reports_unknown_taxonomies_and_duplicate_terms_at_their_authored_values() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\nunknown = ['news']\n",
    );
    templates(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown taxonomy fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/post.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("unknown"), "{diagnostic}");
    assert!(diagnostic.contains("not configured"), "{diagnostic}");

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['news', 'news']\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate term fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/post.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("duplicate term"), "{diagnostic}");
    assert!(diagnostic.contains("first term"), "{diagnostic}");

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = []\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("empty term list fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/post.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("at least one term"), "{diagnostic}");
}

#[test]
fn reports_membership_outside_the_taxonomys_source_collection() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\n[taxonomies]\ntags = ['news']\n",
    );
    templates(&fixture);

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("out of scope membership fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/post.page:3"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("not a member"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:5"), "{diagnostic}");
}

#[test]
fn reports_portably_colliding_literal_term_routes_with_both_locations() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write(
        "content/en/first.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['News']\n",
    );
    fixture.write(
        "content/en/second.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['news']\n",
    );
    templates(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("ambiguous routes fail");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/second.page:4"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/first.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("collide"), "{diagnostic}");
}

#[test]
fn quick_build_rerenders_the_affected_taxonomy_listing_for_a_member_change() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Before'\ncollections = ['posts']\n[taxonomies]\ntags = ['news']\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "index");
    fixture.write(
        "templates/taxonomy-term.tera",
        "{% for page in pagination.pages %}{{ page.title }}{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("initial build");
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'After'\ncollections = ['posts']\n[taxonomies]\ntags = ['news']\n",
    );
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/en/post.page"))
        .expect("changed member")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set member modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/tags/news.html")).expect("term output"),
        "After"
    );
}

#[test]
fn taxonomy_function_resolves_the_locale_local_index_and_reports_invalid_lookups() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    templates(&fixture);
    fixture.write(
        "templates/page.tera",
        "{% set tags = taxonomy(name='tags') %}{{ tags.name }}|{{ tags.url }}|{{ tags.relative_url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("taxonomy index lookup builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "tags|/tags/index.html|./tags/index.html"
    );

    fixture.write("templates/page.tera", "{{ taxonomy(name='unknown') }}");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown taxonomy fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("unknown"), "{diagnostic}");
    assert!(
        diagnostic.contains("available taxonomies: tags"),
        "{diagnostic}"
    );

    fixture.write("templates/page.tera", "{{ taxonomy(name='not/tags') }}");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid taxonomy name fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("simple template identifier"),
        "{diagnostic}"
    );
}

#[test]
fn validates_a_taxonomys_term_template_even_when_it_has_no_terms() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "index");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("an empty taxonomy still needs its term template");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:8"), "{diagnostic}");
    assert!(diagnostic.contains("taxonomy-term.tera"), "{diagnostic}");
    assert!(diagnostic.contains("add"), "{diagnostic}");
}

#[test]
fn reports_invalid_taxonomy_configuration_values_at_their_authored_spans() {
    let cases = [
        (
            taxonomy_config().replace("collection = 'posts'", "collection = 'missing'"),
            "heine.toml:5",
            "not configured",
        ),
        (
            taxonomy_config().replace("path = 'tags'", "path = '../tags'"),
            "heine.toml:6",
            "relative paths",
        ),
        (
            taxonomy_config().replace(
                "index_template = 'taxonomy-index.tera'",
                "index_template = '../index.tera'",
            ),
            "heine.toml:7",
            "template names",
        ),
        (
            taxonomy_config().replace(
                "term_template = 'taxonomy-term.tera'",
                "term_template = '../term.tera'",
            ),
            "heine.toml:8",
            "template names",
        ),
        (
            taxonomy_config().replace("per_page = 20", "per_page = 0"),
            "heine.toml:9",
            "positive integer",
        ),
        (
            taxonomy_config().replace("[taxonomies.tags]", "[taxonomies.blog-tags]"),
            "heine.toml:4",
            "simple template identifier",
        ),
    ];

    for (config, location, detail) in cases {
        let fixture = Fixture::new();
        fixture.write("heine.toml", &config);

        let error = build_at(fixture.path(), BuildMode::Full)
            .expect_err("invalid taxonomy configuration must fail");
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains(location), "{diagnostic}");
        assert!(diagnostic.contains(detail), "{diagnostic}");
    }
}

#[test]
fn taxonomy_routes_are_locale_and_base_path_aware_and_sort_terms_for_the_locale() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[build]\noutput_dir = 'public/guide'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'id-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 20\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/de/first.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['Ähre']\n",
    );
    fixture.write(
        "content/de/second.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['Apfel']\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% set tags = taxonomy(name='tags') %}{{ tags.url }}|{{ tags.relative_url }}",
    );
    fixture.write(
        "templates/taxonomy-index.tera",
        "{% for term in taxonomy.terms %}{{ term.name }}|{% endfor %}",
    );
    fixture.write("templates/taxonomy-term.tera", "term");

    build_at(fixture.path(), BuildMode::Full).expect("localized taxonomy site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/de/index.html"))
            .expect("German index"),
        "/guide/de/tags/index.html|../de/tags/index.html"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/de/tags/index.html"))
            .expect("German taxonomy index"),
        "Ähre|Apfel|"
    );
}

#[test]
fn taxonomy_outputs_participate_in_normal_output_collision_diagnostics() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        &taxonomy_config().replace("path = 'tags'", "path = 'blog'"),
    );
    fixture.write("content/en/blog/index.page", "template = 'page.tera'\n");
    templates(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("taxonomy index must not overwrite an authored page");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/blog/index.page:1"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:6"), "{diagnostic}");
    assert!(diagnostic.contains("claimed by taxonomy"), "{diagnostic}");
}

#[test]
fn quick_build_rerenders_a_taxonomy_index_for_changed_global_data() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("data/site.toml", "label = 'Before'\n");
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "{{ data.site.label }}");
    fixture.write("templates/taxonomy-term.tera", "term");

    build_at(fixture.path(), BuildMode::Full).expect("initial build");
    fixture.write("data/site.toml", "label = 'After'\n");
    fs::File::options()
        .write(true)
        .open(fixture.path().join("data/site.toml"))
        .expect("changed global data")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set global data modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/tags/index.html"))
            .expect("taxonomy index output"),
        "After"
    );
}

#[test]
fn reports_taxonomy_template_render_errors_at_the_template_and_generated_output() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", taxonomy_config());
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ncollections = ['posts']\n[taxonomies]\ntags = ['news']\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write("templates/taxonomy-index.tera", "index");
    fixture.write(
        "templates/taxonomy-term.tera",
        "{{ asset(id='missing.png') }}",
    );

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("a taxonomy template failure must be diagnosed");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("templates/taxonomy-term.tera:1"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("missing.png"), "{diagnostic}");
    assert!(
        diagnostic.contains("taxonomy tags (en) to tags/news.html"),
        "{diagnostic}"
    );
}
