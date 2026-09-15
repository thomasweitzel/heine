use super::*;
use std::path::{Path, PathBuf};

fn search_index_path(directory: &Path) -> PathBuf {
    let paths = fs::read_dir(directory)
        .expect("search directory")
        .map(|entry| entry.expect("search entry").path())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1, "expected one search index in {directory:?}");
    paths.into_iter().next().expect("one search index")
}

#[test]
fn generates_versioned_locale_local_search_documents_from_collections() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\norigin = 'https://example.org'\nbase_path = '/guide/'\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'id-asc'\n\n[collections.docs]\norder = 'id-asc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 10\n\n[search.site]\ncollections = ['posts', 'docs']\npath = 'search/site.json'\n",
    );
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ntitle = 'Post title'\nsummary = 'Post summary'\npublished = 2026-08-14T09:30:00\ncollections = ['posts', 'docs']\n[taxonomies]\ntags = ['Rust']\n[content]\nmain = 'post.md'\nnotes = 'notes.md'\n\n[toc]\ncontent = 'main'\nmin_level = 2\n",
    );
    fixture.write(
        "content/en/post.md",
        "# Post heading\n\nA [link label](https://example.org) and `code`.\n\n:::picture\nalt = \"Searchable portrait\"\nlight = \"asset:images/light.png\"\ndark = \"asset:images/dark.png\"\ncaption = \"Searchable image context\"\n:::\n\n<blockquote lang=\"ar\" dir=\"rtl\"><p>نص عربي</p></blockquote>\n",
    );
    write_bytes(&fixture, "content/en/images/light.png", PIXEL_PNG);
    write_bytes(&fixture, "content/en/images/dark.png", PIXEL_PNG);
    fixture.write("content/en/notes.md", "# Unanchored note\n\nExtra text.\n");
    fixture.write(
        "content/en/docs.page",
        "template = 'page.tera'\ntitle = 'Documentation'\ncollections = ['docs']\n[content]\nmain = 'docs.md'\n",
    );
    fixture.write("content/en/docs.md", "Documentation text\n");
    fixture.write(
        "content/de/post.page",
        "template = 'page.tera'\ntitle = 'Deutsch'\ncollections = ['posts']\n[content]\nmain = 'post.md'\n",
    );
    fixture.write("content/de/post.md", "Deutscher Text\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% if page.id == 'post' %}{{ markdown(content='main') }}{% endif %}{% set current = search(name='site') %}{% set german = search(name='site', locale='de') %}{{ current.url }}|{{ current.relative_url }}|{{ current.absolute_url }}|{{ current.count }}|{{ german.url }}|{{ german.absolute_url }}",
    );
    fixture.write("templates/taxonomy-index.tera", "index");
    fixture.write("templates/taxonomy-term.tera", "term");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let english_directory = fixture.path().join("public/search");
    let english_path = search_index_path(&english_directory);
    let english_name = english_path
        .file_name()
        .expect("filename")
        .to_string_lossy();
    assert!(english_name.starts_with("site."), "{english_name}");
    assert!(english_name.ends_with(".json"), "{english_name}");
    assert_eq!(english_name.len(), "site.".len() + 64 + ".json".len());
    let english = fs::read_to_string(&english_path).expect("English index");
    let json: serde_json::Value = serde_json::from_str(&english).expect("valid JSON");
    assert_eq!(json["format"], 3);
    assert_eq!(json["locale"], "en");
    assert_eq!(json["documents"].as_array().expect("documents").len(), 2);
    assert!(english.contains("Post heading"), "{english}");
    assert!(english.contains("link label"), "{english}");
    assert!(english.contains("Searchable portrait"), "{english}");
    assert!(english.contains("Searchable image context"), "{english}");
    assert!(!english.contains("asset:images/light.png"), "{english}");
    assert!(english.contains("نص عربي"), "{english}");
    assert!(english.contains("/guide/post.html"), "{english}");
    let post = json["documents"]
        .as_array()
        .expect("documents")
        .iter()
        .find(|document| document["id"] == "post")
        .expect("post document");
    assert!(post.get("text").is_none(), "{post}");
    assert_eq!(post["taxonomies"]["tags"], serde_json::json!(["Rust"]));
    assert_eq!(post["title"], "Post title");
    assert_eq!(post["draft"], false);
    assert!(post["description"].is_null());
    assert_eq!(post["summary"], "Post summary");
    assert_eq!(post["published"], "2026-08-14T07:30:00Z");
    assert!(post["updated"].is_null());
    let sections = post["sections"].as_array().expect("sections");
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0]["fragment"], "post-heading");
    assert!(
        sections[0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("link label")
    );
    assert_eq!(sections[1]["fragment"], serde_json::Value::Null);
    assert!(
        sections[1]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Extra text")
    );
    let post_html =
        fs::read_to_string(fixture.path().join("public/post.html")).expect("post output");
    assert!(
        post_html.contains("<h1 id=\"post-heading\">Post heading</h1>"),
        "{post_html}"
    );

    let german_directory = fixture.path().join("public/de/search");
    let german_path = search_index_path(&german_directory);
    let german = fs::read_to_string(german_path).expect("German index");
    assert!(german.contains("Deutscher Text"), "{german}");
    assert!(!german.contains("Post heading"), "{german}");

    let page = fs::read_to_string(fixture.path().join("public/index.html")).expect("page output");
    assert!(page.starts_with("/guide/search/site."), "{page}");
    assert!(page.contains("|./search/site."), "{page}");
    assert!(
        page.contains("|https://example.org/guide/search/site."),
        "{page}"
    );
    assert!(page.contains("|2|/guide/de/search/site."), "{page}");
    assert!(
        page.contains("|https://example.org/guide/de/search/site."),
        "{page}"
    );
}

#[test]
fn search_configuration_and_template_calls_are_checked() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts', 'posts']\npath = '__heine/site.json'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicates are rejected");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("more than once"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:5"), "{diagnostic}");

    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'search/site.json'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set value = search(name='missing') %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("lookup is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("search index \"missing\" was not"),
        "{diagnostic}"
    );
    assert!(
        error
            .to_string()
            .contains("found; available search indexes: site"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("templates/page.tera:1:16"),
        "{diagnostic}"
    );
}

#[test]
fn quick_build_keeps_the_complete_search_index() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'search/site.json'\n",
    );
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ncollections = ['posts']\n[content]\nmain = 'post.md'\n",
    );
    fixture.write("content/en/post.md", "first source\n");
    fixture.write("templates/page.tera", "page");
    build_at(fixture.path(), BuildMode::Full).expect("full build");
    let directory = fixture.path().join("public/search");
    let initial = search_index_path(&directory);
    let initial_bytes = fs::read(&initial).expect("initial index");

    std::thread::sleep(Duration::from_millis(10));
    fixture.write("content/en/post.md", "changed source\n");
    build_at(fixture.path(), BuildMode::Quick).expect("quick build");
    assert_eq!(fs::read(&initial).expect("unchanged index"), initial_bytes);

    build_at(fixture.path(), BuildMode::Full).expect("complete rebuild");
    assert!(!initial.exists(), "superseded versioned index is removed");
    let current = search_index_path(&directory);
    assert_ne!(fs::read(current).expect("current index"), initial_bytes);
}

#[test]
fn stable_search_indexes_retain_the_configured_json_path() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'search/site.json'\nversioned = false\n",
    );
    fixture.write(
        "content/en/post.page",
        "template = 'page.tera'\ncollections = ['posts']\n",
    );
    fixture.write("templates/page.tera", "page");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert!(fixture.path().join("public/search/site.json").is_file());
}

#[test]
fn search_configuration_reports_invalid_collection_and_path_values() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[search.site]\ncollections = []\npath = 'search/site.json'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("empty selection fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("must not be empty"), "{diagnostic}");

    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'search/site.txt'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("wrong extension fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:6"), "{diagnostic}");
    assert!(
        diagnostic.contains("lowercase .json suffix"),
        "{diagnostic}"
    );

    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'search/site.JsOn'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("mixed suffix fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:6"), "{diagnostic}");
    assert!(diagnostic.contains("lowercase .json"), "{diagnostic}");

    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['missing']\npath = 'search/site.json'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown collection fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:5"), "{diagnostic}");
    assert!(diagnostic.contains("not configured"), "{diagnostic}");
}

#[test]
fn search_text_has_stable_markdown_boundaries_and_empty_locale_indexes() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'search/site.json'\n",
    );
    fixture.write(
        "content/en/guide/index.page",
        "template = 'page.tera'\ntitle = 'Guide'\npublished = 2026-08-14T09:30:00\ncollections = ['posts']\n[content]\nalpha = 'alpha.md'\nbeta = 'beta.md'\nfacts = 'facts.json'\n",
    );
    fixture.write(
        "content/en/guide/alpha.md",
        "<!-- segment: body -->\nFirst segment.\n",
    );
    fixture.write("content/en/guide/beta.md", "Second source.\n");
    fixture.write(
        "content/en/guide/facts.json",
        "{\"hidden\": \"not searchable\"}",
    );
    fixture.write(
        "content/en/empty.page",
        "template = 'page.tera'\ntitle = 'Empty'\ncollections = ['posts']\n",
    );
    fixture.write("content/de/.keep", "");
    fixture.write(
        "templates/page.tera",
        "{% set index = search(name='site') %}{{ index.relative_url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let english = fs::read_to_string(search_index_path(&fixture.path().join("public/search")))
        .expect("English search index");
    let english: serde_json::Value = serde_json::from_str(&english).expect("valid JSON");
    let documents = english["documents"].as_array().expect("documents");
    let guide = documents
        .iter()
        .find(|document| document["id"] == "guide/index")
        .expect("guide document");
    assert_eq!(
        guide["sections"],
        serde_json::json!([
            {"fragment": null, "text": "First segment."},
            {"fragment": null, "text": "Second source."}
        ])
    );
    assert!(
        !guide["sections"]
            .as_array()
            .expect("sections")
            .iter()
            .any(|section| section["text"]
                .as_str()
                .unwrap_or_default()
                .contains("not searchable"))
    );
    let empty = documents
        .iter()
        .find(|document| document["id"] == "empty")
        .expect("empty document");
    assert_eq!(empty["sections"], serde_json::json!([]));

    let nested = fs::read_to_string(fixture.path().join("public/guide/index.html"))
        .expect("nested page output");
    assert!(nested.starts_with("../search/site."), "{nested}");
    let german = fs::read_to_string(search_index_path(&fixture.path().join("public/de/search")))
        .expect("German search index");
    let german: serde_json::Value = serde_json::from_str(&german).expect("valid JSON");
    assert_eq!(german["locale"], "de");
    assert_eq!(german["documents"], serde_json::json!([]));
}

#[test]
fn search_indexes_participate_in_checked_output_collisions() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[collections.posts]\norder = 'id-asc'\n\n[search.site]\ncollections = ['posts']\npath = 'index.json'\nversioned = false\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncollections = ['posts']\nsuffix = '.json'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("search index site"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:6"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/index.page:3"),
        "{diagnostic}"
    );
}
