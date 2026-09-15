use super::*;

#[test]
fn generates_locale_local_atom_feeds_with_checked_template_links() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\nbase_path = '/guide/'\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'title-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'feeds/posts.xml'\ntitle_key = 'feed-title'\nsubtitle_key = 'feed-subtitle'\nlimit = 1\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "feed-title = English posts\nfeed-subtitle = Recent writing\n",
    );
    fixture.write(
        "locales/de/site.ftl",
        "feed-title = Deutsche Beiträge\nfeed-subtitle = Neues Schreiben\n",
    );
    fixture.write(
        "content/en/blog/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\nauthors = ['Default Author']\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/blog/older.page",
        "title = 'A title'\npublished = 2026-08-14T09:30:00\nsummary = 'A & < summary'\n",
    );
    fixture.write(
        "content/en/blog/newer.page",
        "title = 'Z & title'\npublished = 2026-08-15T09:30:00\nsummary = 'New & < summary'\nauthors = ['Guest & Author']\n",
    );
    fixture.write(
        "content/de/blog/post.page",
        "template = 'page.tera'\ntitle = 'Deutsch'\npublished = 2026-08-16T09:30:00\nsummary = 'Zusammenfassung'\nauthors = ['Deutsche Autorin']\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% set current = feed(name='posts') %}{% set german = feed(name='posts', locale='de') %}{{ current.url }}|{{ current.relative_url }}|{{ current.absolute_url }}|{{ current.title }}|{{ german.url }}|{{ german.absolute_url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let english =
        fs::read_to_string(fixture.path().join("public/feeds/posts.xml")).expect("English feed");
    assert!(english.contains("xml:lang=\"en\""), "{english}");
    assert!(
        english.contains("<title type=\"text\">Z &amp; title</title>"),
        "{english}"
    );
    assert!(!english.contains("A title"), "{english}");
    assert!(
        english.contains("<name>Guest &amp; Author</name>"),
        "{english}"
    );
    assert!(
        english.contains("<summary type=\"text\">New &amp; &lt; summary</summary>"),
        "{english}"
    );
    assert!(
        english.contains("https://example.org/guide/blog/newer.html"),
        "{english}"
    );
    assert!(english.contains("2026-08-15T07:30:00Z"), "{english}");
    let german =
        fs::read_to_string(fixture.path().join("public/de/feeds/posts.xml")).expect("German feed");
    assert!(german.contains("xml:lang=\"de\""), "{german}");
    assert!(german.contains("Deutsche Beiträge"), "{german}");
    let page = fs::read_to_string(fixture.path().join("public/index.html")).expect("page output");
    assert_eq!(
        page,
        "/guide/feeds/posts.xml|./feeds/posts.xml|https://example.org/guide/feeds/posts.xml|English posts|/guide/de/feeds/posts.xml|https://example.org/guide/de/feeds/posts.xml"
    );
}

#[test]
fn empty_feed_requires_its_explicit_atom_fallbacks() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("empty fallback is required");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("empty_updated"), "{diagnostic}");
    assert!(diagnostic.contains("posts"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:8"), "{diagnostic}");
}

#[test]
fn feed_reports_missing_entry_authors_at_collection_membership() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts\n");
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post'\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("authors are required");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("has no authors"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/post.page:5"),
        "{diagnostic}"
    );
}

#[test]
fn feeds_check_configuration_and_escape_empty_feed_metadata() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['A & B']\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts & news\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% set unknown = feed(name='unknown') %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown feed is checked");
    assert!(format!("{error:?}").contains("feed \"unknown\" was not found"));

    fixture.write("templates/page.tera", "page");
    build_at(fixture.path(), BuildMode::Full).expect("empty feed builds with fallbacks");
    let feed = fs::read_to_string(fixture.path().join("public/atom.xml")).expect("feed output");
    assert!(feed.contains("Posts &amp; news"), "{feed}");
    assert!(feed.contains("A &amp; B"), "{feed}");
    assert!(
        feed.contains("<updated>2026-08-01T07:00:00Z</updated>"),
        "{feed}"
    );

    fixture.write(
        "content/en/atom.page",
        "template = 'page.tera'\ntitle = 'Collision'\nsuffix = '.xml'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("feed output collides");
    assert!(format!("{error:?}").contains("output path atom.xml is claimed"));
}

#[test]
fn feeds_require_an_origin_at_their_configuration() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("origin is required");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("requires site.origin"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:4"), "{diagnostic}");
}

#[test]
fn author_lists_report_empty_values_at_their_authored_fields() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\nauthors = []\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("authors must be non-empty");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/post.page:2"),
        "{diagnostic}"
    );
    assert!(
        error.to_string().contains("must contain at least one"),
        "{diagnostic}"
    );
}

#[test]
fn feeds_report_unavailable_locale_metadata_at_the_authored_key() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'missing-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("message is absent");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("missing-title"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:17"), "{diagnostic}");
}

#[test]
fn quick_build_keeps_the_last_complete_feed_by_design() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts\n");
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'First title'\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\nauthors = ['Author']\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "{{ page.title }}");
    build_at(fixture.path(), BuildMode::Full).expect("full build");
    let feed_path = fixture.path().join("public/atom.xml");
    let initial = fs::read_to_string(&feed_path).expect("initial feed");

    std::thread::sleep(Duration::from_millis(10));
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Changed title'\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\nauthors = ['Author']\ncollections = ['posts']\n",
    );
    build_at(fixture.path(), BuildMode::Quick).expect("quick build");
    assert_eq!(fs::read_to_string(&feed_path).expect("quick feed"), initial);

    build_at(fixture.path(), BuildMode::Full).expect("full rebuild");
    assert!(
        fs::read_to_string(feed_path)
            .expect("rebuilt feed")
            .contains("Changed title")
    );
}

#[test]
fn feeds_use_default_messages_and_empty_metadata_for_an_empty_locale() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts\n");
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post'\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\nauthors = ['Author']\ncollections = ['posts']\n",
    );
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "page");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let feed =
        fs::read_to_string(fixture.path().join("public/de/atom.xml")).expect("empty German feed");
    assert!(feed.contains("xml:lang=\"de\""), "{feed}");
    assert!(
        feed.contains("<title type=\"text\">Posts</title>"),
        "{feed}"
    );
    assert!(feed.contains("<name>Site Author</name>"), "{feed}");
    assert!(
        feed.contains("<updated>2026-08-01T07:00:00Z</updated>"),
        "{feed}"
    );
    assert!(!feed.contains("<entry>"), "{feed}");
}

#[test]
fn feeds_reject_xml_forbidden_entry_text_and_do_not_assume_html_entries() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts\n");
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\nsuffix = '.xml'\ntitle = \"Bad \\u0001 title\"\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\nauthors = ['Author']\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("XML controls are rejected");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("U+0001"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/post.page:3"),
        "{diagnostic}"
    );

    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\nsuffix = '.xml'\ntitle = 'Post'\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\nauthors = ['Author']\ncollections = ['posts']\n",
    );
    build_at(fixture.path(), BuildMode::Full).expect("non-HTML entry builds");
    let feed = fs::read_to_string(fixture.path().join("public/atom.xml")).expect("feed output");
    assert!(
        feed.contains("<link rel=\"alternate\" href=\"https://example.org/post.xml\"/>"),
        "{feed}"
    );
}

#[test]
fn feeds_report_inherited_author_text_at_the_descriptor_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\ntime_zone = 'Europe/Berlin'\n\n[collections.posts]\norder = 'id-asc'\n\n[feeds.posts]\ncollection = 'posts'\npath = 'atom.xml'\ntitle_key = 'feed-title'\nempty_updated = 2026-08-01T09:00:00\nempty_authors = ['Site Author']\n",
    );
    fixture.write("locales/en/site.ftl", "feed-title = Posts\n");
    fixture.write(
        "content/en/blog/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\nauthors = [\"Bad \\u0001 author\"]\n",
    );
    fixture.write(
        "content/en/blog/post.page",
        "title = 'Post'\npublished = 2026-08-14T09:30:00\nsummary = 'Summary'\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("XML controls are rejected");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/blog/_directory.toml:3"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("U+0001"), "{diagnostic}");
}
