use super::*;

fn listing_fixture(fixture: &Fixture, listing: &str) {
    fixture.write("content/en/blog/index.page", listing);
    for number in 1..=5 {
        fixture.write(
            format!("content/en/blog/post-{number}.page"),
            &format!("template = 'post.tera'\ntitle = 'Post {number}'\ncollections = ['posts']\n"),
        );
    }
    fixture.write("templates/post.tera", "{{ page.title }}");
}

#[test]
fn paginates_a_collection_with_generated_routes_and_navigation() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fixture.write(
        "templates/listing.tera",
        "{{ page.id }}|{{ pagination.page_number }}/{{ pagination.page_count }}|{{ pagination.item_count }}|{{ pagination.first_item_number }}:{% for post in pagination.pages %}{{ post.title }};{% endfor %}|{% for item in pagination.navigation %}{{ item.kind }}{% if item.kind == 'link' %}:{{ item.page_number }}{% endif %};{% endfor %}|{% for link in pagination_links() %}{{ link.page_number }}={{ link.current }};{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/index.html")).expect("pager one"),
        "blog/index|1/3|5|1:Post 1;Post 2;|link:1;link:2;link:3;|1=true;2=false;3=false;"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/page/2.html")).expect("pager two"),
        "blog/index|2/3|5|3:Post 3;Post 4;|link:1;link:2;link:3;|1=false;2=true;3=false;"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/page/3.html")).expect("pager three"),
        "blog/index|3/3|5|5:Post 5;|link:1;link:2;link:3;|1=false;2=false;3=true;"
    );
    assert!(!fixture.path().join("public/blog/page/1.html").exists());
}

#[test]
fn pagination_links_are_base_path_aware_and_relative_to_every_generated_page() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[site]\nbase_path = '/guide/'\n[build]\noutput_dir = 'public/guide'\n[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fixture.write(
        "templates/listing.tera",
        "{% if pagination.page_number == 2 %}{{ pagination.current.url }}|{{ pagination.current.relative_url }}|{{ pagination.next.url }}|{{ pagination.next.relative_url }}{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/blog/page/2.html"))
            .expect("pager two"),
        "/guide/blog/page/2.html|../../blog/page/2.html|/guide/blog/page/3.html|../../blog/page/3.html"
    );
}

#[test]
fn custom_segments_and_empty_collections_have_one_unambiguous_route() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\nsegment = 'older-posts'\n",
    );
    fixture.write("templates/listing.tera", "{{ pagination.page_count }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/index.html")).expect("listing"),
        "3"
    );
    assert!(
        fixture
            .path()
            .join("public/blog/older-posts/2.html")
            .is_file()
    );
    assert!(
        !fixture
            .path()
            .join("public/blog/older-posts/1.html")
            .exists()
    );

    let empty = Fixture::new();
    empty.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    empty.write(
        "content/en/index.page",
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 10\n",
    );
    empty.write(
        "templates/listing.tera",
        "{{ pagination.page_count }}|{{ pagination.item_count }}|{{ pagination.first_item_number }}",
    );
    build_at(empty.path(), BuildMode::Full).expect("empty listing builds");
    assert_eq!(
        fs::read_to_string(empty.path().join("public/index.html")).expect("empty listing"),
        "1|0|0"
    );
}

#[test]
fn ordinary_pages_receive_a_null_pagination_value() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% if pagination %}present{% else %}null{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("ordinary page builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("ordinary page"),
        "null"
    );
}

#[test]
fn navigation_items_and_adjacent_links_expose_their_current_pager_state() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fixture.write(
        "templates/listing.tera",
        "{% if pagination.page_number == 2 %}{% for item in pagination.navigation %}{% if item.kind == 'link' %}{{ item.page_number }}={{ item.current }};{% endif %}{% endfor %}|{{ pagination.previous.page_number }}|{{ pagination.next.page_number }}{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/page/2.html")).expect("pager two"),
        "1=false;2=true;3=false;|1|3"
    );
}

#[test]
fn validates_pagination_values_and_reports_their_authored_location() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[pagination]\ncollection = 'posts'\nper_page = 0\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("zero page size fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:4"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("positive integer"), "{diagnostic}");
}

#[test]
fn validates_pagination_route_and_navigation_policy_at_their_values() {
    let cases = [
        (
            "segment = 'page:two'\n",
            "must use a literal portable URL-path component",
            "content/en/index.page:5",
        ),
        (
            "segment = 'older%20posts'\n",
            "must use a literal portable URL-path component",
            "content/en/index.page:5",
        ),
        (
            "[pagination.navigation]\nwindow = 6\n",
            "integer from 1 through 5",
            "content/en/index.page:6",
        ),
    ];
    for (suffix, expected, location) in cases {
        let fixture = Fixture::new();
        fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
        let page = format!(
            "template = 'page.tera'\n[pagination]\ncollection = 'posts'\nper_page = 1\n{suffix}"
        );
        fixture.write("content/en/index.page", &page);
        fixture.write("templates/page.tera", "page");

        let error = build_at(fixture.path(), BuildMode::Full).expect_err("pagination is invalid");
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains(location), "{diagnostic}");
        assert!(diagnostic.contains(expected), "{diagnostic}");
    }
}

#[test]
fn pagination_requires_a_rendered_authored_listing_page() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write(
        "content/en/index.page",
        "template = false\n[pagination]\ncollection = 'posts'\nper_page = 10\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("pagination requires output");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:3"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("requires this page to have a template"),
        "{diagnostic}"
    );
}

#[test]
fn generated_route_collisions_label_a_custom_segment() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\nsegment = 'older'\n",
    );
    fixture.write(
        "content/en/blog/older/2.page",
        "template = 'post.tera'\nsuffix = 'html'\n",
    );
    fixture.write("templates/listing.tera", "listing");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/blog/index.page:6"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/blog/older/2.page:2"),
        "{diagnostic}"
    );
}

#[test]
fn pagination_routes_participate_in_output_collision_validation() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fixture.write("content/en/blog/page/2.page", "template = 'post.tera'\n");
    fixture.write("templates/listing.tera", "listing");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("public") || diagnostic.contains("blog/page/2.html"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/blog/index.page"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/en/blog/page/2.page"),
        "{diagnostic}"
    );
}

#[test]
fn localized_pager_urls_include_the_locale_and_base_path() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[build]\noutput_dir = 'public/guide'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'id-asc'\n",
    );
    fixture.write(
        "content/de/blog/index.page",
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    for number in 1..=3 {
        fixture.write(
            format!("content/de/blog/post-{number}.page"),
            "template = 'post.tera'\ncollections = ['posts']\n",
        );
    }
    fixture.write("templates/post.tera", "post");
    fixture.write(
        "templates/listing.tera",
        "{{ pagination.current.url }}|{{ pagination.current.relative_url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("localized site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/guide/de/blog/page/2.html"))
            .expect("German pager"),
        "/guide/de/blog/page/2.html|../../../de/blog/page/2.html"
    );
}

#[test]
fn non_index_listings_nest_later_pagers_below_their_output_stem() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fs::rename(
        fixture.path().join("content/en/blog/index.page"),
        fixture.path().join("content/en/archive.page"),
    )
    .expect("rename listing source");
    fixture.write("templates/listing.tera", "{{ pagination.page_number }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert!(fixture.path().join("public/archive.html").exists());
    assert!(fixture.path().join("public/archive/page/2.html").exists());
    assert!(!fixture.path().join("public/page/2.html").exists());
}

#[test]
fn pagination_links_fail_at_the_template_call_site_outside_a_listing() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ pagination_links() | length }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("function is unavailable");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("paginated listing"), "{diagnostic}");
}

#[test]
fn template_errors_name_the_generated_pager_being_rendered() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fixture.write(
        "templates/listing.tera",
        "{% if pagination.page_number == 2 %}{{ data.missing.value }}{% endif %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("template fails on pager two");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("templates/listing.tera:1"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("pagination page 2 of listing blog/index (en)"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("page/2.html"), "{diagnostic}");
}

#[test]
fn quick_build_rerenders_every_affected_pager_when_a_collection_member_changes() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    listing_fixture(
        &fixture,
        "template = 'listing.tera'\ncollections = []\n[pagination]\ncollection = 'posts'\nper_page = 2\n",
    );
    fixture.write(
        "templates/listing.tera",
        "{% for post in pagination.pages %}{{ post.title }};{% endfor %}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("initial build");

    fixture.write(
        "content/en/blog/post-3.page",
        "template = 'post.tera'\ntitle = 'Updated post'\ncollections = ['posts']\n",
    );
    fs::File::options()
        .write(true)
        .open(fixture.path().join("content/en/blog/post-3.page"))
        .expect("collection member")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick build");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/blog/page/2.html")).expect("pager two"),
        "Updated post;Post 4;"
    );
}
