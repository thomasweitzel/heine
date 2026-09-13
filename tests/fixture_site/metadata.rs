use super::*;

use std::time::{Duration, SystemTime};

#[test]
fn inherits_directory_page_policy_and_exposes_explicit_metadata() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'default.tera'\n[pages.metadata]\nrequired = ['title']\n",
    );
    fixture.write(
        "content/en/blog/_directory.toml",
        "title = 'Blog'\n[pages]\ntemplate = 'post.tera'\n[pages.metadata]\nrequired = ['published', 'extra.author']\n",
    );
    fixture.write(
        "content/en/blog/post.page",
        "title = 'Post'\npublished = 2026-08-14T09:30:00\n[extra]\nauthor = 'Heine'\n",
    );
    fixture.write(
        "content/en/blog/nested/_directory.toml",
        "[pages]\ntemplate = 'nested.tera'\n",
    );
    fixture.write(
        "content/en/blog/nested/inherited.page",
        "title = 'Nested'\npublished = 2026-08-15T09:30:00\n[extra]\nauthor = 'Heine'\n",
    );
    fixture.write(
        "content/en/blog/nested/overridden.page",
        "template = 'override.tera'\ntitle = 'Override'\npublished = 2026-08-16T09:30:00\n[extra]\nauthor = 'Heine'\n",
    );
    fixture.write(
        "content/en/about.page",
        "title = 'About'\ntemplate = false\n",
    );
    fixture.write(
        "templates/post.tera",
        "{{ page.title }}|{{ page.published }}|{{ page.extra.author }}|{{ page.directory.title }}",
    );
    fixture.write("templates/default.tera", "{{ page.title }}");
    fixture.write("templates/nested.tera", "nested {{ page.title }}");
    fixture.write("templates/override.tera", "override {{ page.title }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/post.html")).expect("post output"),
        "Post|2026-08-14T07:30:00Z|Heine|Blog"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/nested/inherited.html"))
            .expect("nested output"),
        "nested Nested"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/nested/overridden.html"))
            .expect("overridden output"),
        "override Override"
    );
    assert!(!fixture.path().join("public/about.html").exists());
    assert!(!fixture.path().join("public/_directory.toml").exists());
    assert!(!fixture.path().join("public/blog/_directory.toml").exists());
}

#[test]
fn reports_missing_metadata_with_the_page_and_directory_policy_context() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages.metadata]\nrequired = ['title', 'extra.author']\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("metadata is required");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("extra.author"), "{diagnostic}");
}

#[test]
fn validates_inherited_directory_metadata_requirements() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "title = 'Root'\ndescription = 'Root directory'\n[metadata]\nrequired = ['title', 'description']\n",
    );
    fixture.write("content/en/blog/_directory.toml", "title = 'Blog'\n");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("directory metadata is required");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/blog/_directory.toml:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/_directory.toml:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("description"), "{diagnostic}");
}

#[test]
fn rejects_page_only_metadata_requirements_for_directories() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[metadata]\nrequired = ['authors']\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("authors belong to pages");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(
        error
            .to_string()
            .contains("directory metadata requirement must name"),
        "{diagnostic}"
    );
}

#[test]
fn reports_invalid_directory_template_at_its_declaration() {
    let fixture = Fixture::new();
    fixture.write("content/en/_directory.toml", "[pages]\ntemplate = true\n");
    fixture.write("content/en/index.page", "title = 'Home'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template is invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("template name or `false`"),
        "{diagnostic}"
    );
    assert_eq!(
        diagnostic.matches("template name or `false`").count(),
        1,
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("this content setting is invalid"),
        "{diagnostic}"
    );
}

#[test]
fn reports_an_unsafe_template_name_at_its_authored_value() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = '../outside.tera'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template is invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:1"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("relative paths"), "{diagnostic}");
}

#[test]
fn resolves_explicit_collections_with_inherited_membership_and_page_opt_out() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'published-desc'\n",
    );
    fixture.write(
        "content/en/blog/_directory.toml",
        "[pages]\ntemplate = 'post.tera'\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/blog/index.page",
        "template = 'index.tera'\ncollections = []\n",
    );
    fixture.write(
        "content/en/blog/older.page",
        "title = 'Older'\npublished = 2026-08-14T09:30:00\n",
    );
    fixture.write(
        "content/en/blog/newer.page",
        "title = 'Newer'\npublished = 2026-08-16T09:30:00\n",
    );
    fixture.write("templates/post.tera", "{{ page.title }}");
    fixture.write(
        "templates/index.tera",
        "{% set posts = collection(name='posts') %}{{ posts.name }}:{{ posts.count }}:{% for id in posts.page_ids %}{% set post = page(id=id) %}{{ post.title }};{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/index.html")).expect("listing output"),
        "posts:2:Newer;Older;"
    );
}

#[test]
fn collection_neighbors_follow_locale_local_collection_order() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n[collections.posts]\norder = 'published-desc'\n",
    );
    fixture.write(
        "content/en/blog/older.page",
        "template = 'post.tera'\ntitle = 'Older'\npublished = 2026-08-14T09:30:00\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/blog/newer.page",
        "template = 'post.tera'\ntitle = 'Newer'\npublished = 2026-08-16T09:30:00\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/de/blog/eins.page",
        "template = 'post.tera'\ntitle = 'Eins'\npublished = 2026-08-14T09:30:00\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/de/blog/zwei.page",
        "template = 'post.tera'\ntitle = 'Zwei'\npublished = 2026-08-16T09:30:00\ncollections = ['posts']\n",
    );
    fixture.write(
        "templates/post.tera",
        "{% set neighbors = collection_neighbors(name='posts') %}{{ page.id }}|{% if neighbors.before %}{{ neighbors.before.id }}:{{ neighbors.before.url }}{% else %}none{% endif %}|{% if neighbors.after %}{{ neighbors.after.id }}:{{ neighbors.after.url }}{% else %}none{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/newer.html")).expect("newer output"),
        "blog/newer|none|blog/older:/blog/older.html"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/older.html")).expect("older output"),
        "blog/older|blog/newer:/blog/newer.html|none"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/blog/zwei.html"))
            .expect("German newer output"),
        "blog/zwei|none|blog/eins:/de/blog/eins.html"
    );
}

#[test]
fn collection_neighbors_report_unknown_collections_and_non_members_at_the_call_site() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set neighbors = collection_neighbors(name='missing') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown collection fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("collection \"missing\" was not found"),
        "{diagnostic}"
    );

    fixture.write(
        "templates/page.tera",
        "{% set neighbors = collection_neighbors(name='posts') %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("non-member fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("is not a member of collection \"posts\""),
        "{diagnostic}"
    );
}

#[test]
fn applies_each_collection_order_with_page_id_tie_breaking() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.published_asc]\norder = 'published-asc'\n[collections.published_desc]\norder = 'published-desc'\n[collections.title_asc]\norder = 'title-asc'\n[collections.title_desc]\norder = 'title-desc'\n[collections.weight_asc]\norder = 'weight-asc'\n[collections.weight_desc]\norder = 'weight-desc'\n[collections.id_asc]\norder = 'id-asc'\n[collections.id_desc]\norder = 'id-desc'\n",
    );
    fixture.write(
        "content/en/alpha.page",
        "template = 'page.tera'\ntitle = 'Same'\npublished = 2026-08-14T09:30:00\nweight = 2\ncollections = ['published_asc', 'published_desc', 'title_asc', 'title_desc', 'weight_asc', 'weight_desc', 'id_asc', 'id_desc']\n",
    );
    fixture.write(
        "content/en/beta.page",
        "template = 'page.tera'\ntitle = 'Same'\npublished = 2026-08-16T09:30:00\nweight = 1\ncollections = ['published_asc', 'published_desc', 'title_asc', 'title_desc', 'weight_asc', 'weight_desc', 'id_asc', 'id_desc']\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'index.tera'\ncollections = []\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write(
        "templates/index.tera",
        "{% for name in ['published_asc', 'published_desc', 'title_asc', 'title_desc', 'weight_asc', 'weight_desc', 'id_asc', 'id_desc'] %}{% set entries = collection(name=name) %}{% for id in entries.page_ids %}{{ id }};{% endfor %}{% if name != 'id_desc' %}|{% endif %}{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("listing output"),
        "alpha;beta;|beta;alpha;|alpha;beta;|alpha;beta;|beta;alpha;|alpha;beta;|alpha;beta;|beta;alpha;"
    );
}

#[test]
fn exposes_empty_collections_in_every_locale() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n[collections.posts]\norder = 'id-asc'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{{ locale.code }}:{% set posts = collection(name='posts') %}{{ posts.count }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("English output"),
        "en:0"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).expect("German output"),
        "de:0"
    );
}

#[test]
fn reports_an_unknown_collection_at_the_template_call_site() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set entries = collection(name='missing') %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collection must exist");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("missing"), "{diagnostic}");
    assert!(
        diagnostic.contains("no collections are configured"),
        "{diagnostic}"
    );
}

#[test]
fn reports_invalid_collection_membership_at_its_authored_value() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collection must exist");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(error.to_string().contains("posts"), "{diagnostic}");
    assert!(error.to_string().contains("not configured"), "{diagnostic}");
}

#[test]
fn reports_collection_ordering_requirements_at_membership() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'published-desc'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("published is required");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("orders by publication time"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("add a TOML local date-time"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
}

#[test]
fn title_ordering_uses_locale_collation_and_requires_titles() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'de'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n[collections.posts]\norder = 'title-asc'\n",
    );
    fixture.write(
        "content/de/a.page",
        "template = 'page.tera'\ntitle = 'Apfel'\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/de/b.page",
        "template = 'page.tera'\ntitle = 'Ähre'\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/de/index.page",
        "template = 'index.tera'\ncollections = []\n",
    );
    fixture.write("templates/page.tera", "page");
    fixture.write(
        "templates/index.tera",
        "{% set posts = collection(name='posts') %}{% for id in posts.page_ids %}{{ id }};{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("listing output"),
        "b;a;"
    );

    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'de'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n[collections.posts]\norder = 'title-desc'\n",
    );
    build_at(fixture.path(), BuildMode::Full).expect("site builds in descending title order");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("listing output"),
        "a;b;"
    );

    fixture.write(
        "content/de/b.page",
        "template = 'page.tera'\ncollections = ['posts']\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("title is required");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/de/b.page:2"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:8"), "{diagnostic}");
    assert!(diagnostic.contains("orders by title"), "{diagnostic}");
    assert!(diagnostic.contains("add a title"), "{diagnostic}");
}

#[test]
fn reports_a_non_numeric_weight_at_the_weight_and_collection_order() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'weight-asc'\n");
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\nweight = false\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("weight is required");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/post.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/post.page:3"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("numeric collection") && diagnostic.contains("weight"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_duplicate_collection_membership_with_both_declarations() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncollections = ['posts', 'posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate must fail");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("declared more than once"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("first membership"), "{diagnostic}");
}

#[test]
fn rejects_collection_membership_for_a_non_rendered_page() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write("content/en/index.page", "collections = ['posts']\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("member must render");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:1"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("is not rendered"), "{diagnostic}");
    assert!(
        diagnostic.contains("give this page a template"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_updated_values_before_publication() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/first.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00\nupdated = 2026-08-14T09:00:00\n",
    );
    fixture.write("content/en/second.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("updated must follow publication");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("updated = 2026-08-14T09:00:00"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("field `updated`"), "{diagnostic}");
}

#[test]
fn distinguishes_nonexistent_and_repeated_publication_times() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\npublished = 2026-03-29T02:30:00\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("time is in a gap");
    let diagnostic = format!("{error:?}");
    assert!(error.to_string().contains("does not exist"), "{diagnostic}");
    assert!(error.to_string().contains("Europe/Berlin"), "{diagnostic}");
    assert!(diagnostic.contains("02:30:00"), "{diagnostic}");

    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\npublished = 2026-10-25T02:30:00\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("time is repeated");
    let diagnostic = format!("{error:?}");
    assert!(error.to_string().contains("occurs twice"), "{diagnostic}");
    assert!(error.to_string().contains("Europe/Berlin"), "{diagnostic}");
    assert!(diagnostic.contains("02:30:00"), "{diagnostic}");
}

#[test]
fn quick_build_keeps_a_collection_listing_stale_after_membership_changes() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'index.tera'\ncollections = []\n",
    );
    fixture.write(
        "content/en/post.page",
        "template = 'post.tera'\ncollections = ['posts']\n",
    );
    fixture.write(
        "templates/index.tera",
        "{% set posts = collection(name='posts') %}{{ posts.count }}",
    );
    fixture.write("templates/post.tera", "post");
    build_at(fixture.path(), BuildMode::Full).expect("initial build");

    let post = fixture.path().join("content/en/post.page");
    fixture.write(
        "content/en/post.page",
        "template = 'post.tera'\ncollections = []\n",
    );
    fs::File::options()
        .write(true)
        .open(post)
        .expect("post source")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set post modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("listing output"),
        "1"
    );
    build_at(fixture.path(), BuildMode::Full).expect("full build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("listing output"),
        "0"
    );
}

#[test]
fn requires_a_time_zone_for_publication_times() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\npublished = 2026-08-14T09:30:00\n",
    );
    fixture.write("templates/page.tera", "page");

    fixture.write(
        "heine.toml",
        "[site]\n\n[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("time zone is required");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/index.page"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/index.page:3"),
        "{diagnostic}"
    );
    assert!(
        error.to_string().contains("requires site.time_zone"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_legacy_and_offset_bearing_publication_values_at_their_fields() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ndate = 2026-08-14\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("legacy date is invalid");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        error.to_string().contains("no longer supported"),
        "{diagnostic}"
    );
    assert!(error.to_string().contains("published"), "{diagnostic}");

    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\npublished = 2026-08-14T09:30:00Z\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("offset is invalid");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        error.to_string().contains("without a UTC offset"),
        "{diagnostic}"
    );
}

#[test]
fn renders_publication_times_as_utc_rfc_3339() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\npublished = 2026-08-14T09:30:00\n",
    );
    fixture.write("templates/page.tera", "{{ page.published }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "2026-08-14T07:30:00Z"
    );
}

#[test]
fn rejects_extra_keys_that_templates_cannot_address_consistently() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n[extra]\nnot-valid = true\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("extra key is invalid");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("content/en/index.page"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/index.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("not-valid"), "{diagnostic}");
    assert!(
        diagnostic.contains("simple template identifiers"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_unknown_page_and_directory_policy_fields() {
    let fixture = Fixture::new();
    fixture.write("content/en/_directory.toml", "[pages]\nmetadta = {}\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("policy field is misspelled");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("unknown field `metadta`"),
        "{diagnostic}"
    );

    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[data]\ntitle = 'Old'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("old field is rejected");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("unknown field `data`"), "{diagnostic}");
}

#[test]
fn quick_builds_rerender_pages_after_a_descriptor_change() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "title = 'Before'\n[pages]\ntemplate = 'page.tera'\n",
    );
    fixture.write("content/en/index.page", "title = 'Home'\n");
    fixture.write("templates/page.tera", "{{ page.directory.title }}");
    build_at(fixture.path(), BuildMode::Full).expect("initial build");

    let output = fixture.path().join("public/index.html");
    fs::File::options()
        .write(true)
        .open(&output)
        .expect("output file")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now()))
        .expect("set output modification time");
    let descriptor = fixture.path().join("content/en/_directory.toml");
    fixture.write(
        "content/en/_directory.toml",
        "title = 'After'\n[pages]\ntemplate = 'page.tera'\n",
    );
    fs::File::options()
        .write(true)
        .open(descriptor)
        .expect("descriptor file")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set descriptor modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick rebuild");

    assert_eq!(fs::read_to_string(output).expect("output"), "After");
}

#[test]
fn reports_a_missing_inherited_template_at_its_directory_declaration() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'missing.tera'\n",
    );
    fixture.write("content/en/index.page", "title = 'Home'\n");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template is missing");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/_directory.toml:2"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("missing.tera"), "{diagnostic}");
}
