//! The starter site is a supported example, so it is built as an integration
//! fixture rather than merely kept as documentation.

use std::{fs, path::Path};

use heine::{BuildMode, BuildOptions, build_at, build_at_with_options};

#[path = "support/temp_dir.rs"]
mod temp_dir;

use temp_dir::TestDirectory;

#[test]
fn builds_the_documented_starter_site() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("starter-site");
    let temporary = TestDirectory::new();
    let root = temporary.path();

    copy_file(&source.join("heine.toml"), &root.join("heine.toml"));
    for directory in ["content", "data", "deploy", "LICENSES", "templates", "kits"] {
        copy_tree(&source.join(directory), &root.join(directory));
    }

    build_at(root, BuildMode::Full).expect("starter site builds");

    let output = root.join("public/my-project");
    assert!(output.join(".htaccess").is_file());
    assert_eq!(
        fs::read_to_string(output.join("robots.txt")).expect("robots.txt"),
        "User-agent: *\nAllow: /\nSitemap: https://example.org/my-project/sitemap.xml\n"
    );
    let sitemap = fs::read_to_string(output.join("sitemap.xml")).expect("sitemap");
    assert!(
        sitemap.contains("https://example.org/my-project/index.html"),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("xmlns:xhtml=\"http://www.w3.org/1999/xhtml\""),
        "{sitemap}"
    );
    assert!(
        sitemap.contains(
            "<url><loc>https://example.org/my-project/index.html</loc><xhtml:link rel=\"alternate\" hreflang=\"de\" href=\"https://example.org/my-project/de/index.html\"/><xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"https://example.org/my-project/index.html\"/></url>"
        ),
        "{sitemap}"
    );
    let index = fs::read_to_string(output.join("index.html")).expect("home page");
    assert!(
        index.contains("<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"en\" dir=\"ltr\">"),
        "{index}"
    );
    assert!(index.contains("Heine Starter Site"), "{index}");
    assert!(
        index.contains(
            "<link rel=\"canonical\" href=\"https://example.org/my-project/index.html\">"
        ),
        "{index}"
    );
    assert!(
        index.contains(
            "<meta name=\"description\" content=\"A small multilingual example site built with Heine, featuring posts and a short guide to Heinrich Heine.\">"
        ),
        "{index}"
    );
    assert!(!index.contains(">Quiz<"), "{index}");
    assert!(index.contains("<h2>Heinrich Heine</h2>"), "{index}");
    assert!(
        index.contains("A short, source-linked guide to the writer whose name this site borrows."),
        "{index}"
    );
    assert!(
        index.contains("href=\"/my-project/heine/index.html\">Heinrich Heine</a>"),
        "{index}"
    );
    assert!(
        !index.contains("href=\"/my-project/lesen/index.html\""),
        "{index}"
    );
    assert!(
        index.contains(
            "<a class=\"navigation-icon\" href=\"/my-project/search/index.html\"><img src=\"/my-project/assets/images/search.svg\" alt=\"Search\"></a>"
        ),
        "{index}"
    );
    let heine_navigation = index.find("Heinrich Heine</a>").expect("Heine navigation");
    let search_navigation = index.find("alt=\"Search\"").expect("search icon");
    assert!(heine_navigation < search_navigation, "{index}");
    assert!(!index.contains("<h2>Terms</h2>"), "{index}");
    assert!(
        index.contains("href=\"/my-project/about/index.html\">About Heine</a>"),
        "{index}"
    );
    assert!(
        index.contains("href=\"/my-project/legal/terms/index.html\">Terms</a>"),
        "{index}"
    );
    assert!(
        index.contains("type=\"application/atom+xml\" href=\"/my-project/atom.xml\""),
        "{index}"
    );
    assert!(index.contains("class=\"language-navigation\""), "{index}");
    assert!(
        index.contains("href=\"/my-project/de/index.html\" hreflang=\"de\" aria-label=\"German\""),
        "{index}"
    );
    assert!(
        index.contains("src=\"/my-project/assets/images/flags/german.svg\""),
        "{index}"
    );
    assert!(!index.contains("width=\"\""), "{index}");
    assert!(!index.contains("height=\"\""), "{index}");
    assert!(
        index.contains("So far there are \u{2068}3\u{2069} posts."),
        "{index}"
    );
    assert!(
        !output.join("blog/draft-publication/index.html").exists(),
        "ordinary starter-site output must not contain its draft post"
    );

    assert!(output.join("assets/css/styles.v2.css").is_file());
    assert!(!output.join("assets/css/styles.v1.css").exists());
    assert!(output.join("assets/js/search.js").is_file());
    let search_script =
        fs::read_to_string(output.join("assets/js/search.js")).expect("search script");
    assert!(
        search_script.contains("const pageSize = 10;"),
        "{search_script}"
    );
    assert!(
        search_script.contains("joinUnspacedLineBreaks"),
        "{search_script}"
    );
    assert!(
        search_script.contains("documentsPromise = undefined;"),
        "{search_script}"
    );
    let search_page = fs::read_to_string(output.join("search/index.html")).expect("search page");
    assert!(
        search_page.contains("data-search-url=\"/my-project/search/site."),
        "{search_page}"
    );
    assert!(
        search_page.contains("src=\"/my-project/assets/js/search.js\" integrity=\"sha384-"),
        "{search_page}"
    );
    assert!(
        search_page.contains("data-results-one=\"\u{2068}__count__\u{2069} result\""),
        "{search_page}"
    );
    assert!(
        search_page.contains("name=\"q\" type=\"search\" dir=\"auto\""),
        "{search_page}"
    );
    let search_index = generated_file_with_extension(&output.join("search"), "json");
    let search_json = fs::read_to_string(search_index).expect("search index");
    assert!(
        search_json.contains("Math, Markdown, and Media"),
        "{search_json}"
    );
    assert!(
        search_json.contains("لا يوجد طريقٌ ملوكيٌّ إلى الهندسة."),
        "{search_json}"
    );

    let german_home = fs::read_to_string(output.join("de/index.html")).expect("German home");
    assert!(
        german_home
            .contains("<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"de\" dir=\"ltr\">"),
        "{german_home}"
    );
    assert!(
        german_home.contains(
            "<meta name=\"description\" content=\"Eine deutschsprachige Ansicht der kleinen Heine-Beispielseite mit Blog, Lektüre, Schlagwörtern und Suche.\">"
        ),
        "{german_home}"
    );
    assert!(
        german_home.contains("Bisher gibt es \u{2068}2\u{2069} Beiträge auf Deutsch."),
        "{german_home}"
    );
    assert!(
        german_home
            .contains("href=\"/my-project/index.html\" hreflang=\"en\" aria-label=\"Englisch\""),
        "{german_home}"
    );
    assert!(
        german_home.contains("src=\"/my-project/assets/images/flags/english.svg\""),
        "{german_home}"
    );
    assert!(
        german_home.contains("src=\"/my-project/assets/images/search.svg\" alt=\"Suche\""),
        "{german_home}"
    );
    assert!(german_home.contains("<h2>Lektüre</h2>"), "{german_home}");
    assert!(
        german_home.contains("href=\"/my-project/de/lesen/index.html\""),
        "{german_home}"
    );
    assert!(
        !german_home.contains("href=\"/my-project/de/series/index.html\">Reihen</a>"),
        "{german_home}"
    );
    let german_series_index =
        fs::read_to_string(output.join("de/series/index.html")).expect("German Series index");
    assert!(
        german_series_index.contains("In dieser Sprache gibt es derzeit keine Reihen."),
        "{german_series_index}"
    );
    let german_search =
        fs::read_to_string(output.join("de/search/index.html")).expect("German search page");
    assert!(
        german_search.contains("data-search-url=\"/my-project/de/search/site."),
        "{german_search}"
    );
    assert!(
        german_search.contains("data-results-one=\"\u{2068}__count__\u{2069} Ergebnis\""),
        "{german_search}"
    );
    assert!(
        german_home.contains("type=\"application/atom+xml\" href=\"/my-project/de/atom.xml\""),
        "{german_home}"
    );

    let atom = fs::read_to_string(output.join("atom.xml")).expect("English Atom feed");
    assert!(atom.contains("xml:lang=\"en\""), "{atom}");
    assert!(atom.contains("<name>Heinrich Heine</name>"), "{atom}");
    let german_atom = fs::read_to_string(output.join("de/atom.xml")).expect("German Atom feed");
    assert!(german_atom.contains("xml:lang=\"de\""), "{german_atom}");

    let german_blog = fs::read_to_string(output.join("de/blog/index.html")).expect("German blog");
    assert!(
        german_blog.contains("Bisher gibt es \u{2068}2\u{2069} Beiträge auf Deutsch."),
        "{german_blog}"
    );
    assert!(
        german_blog.contains("href=\"/my-project/de/atom.xml\">Atom abonnieren</a>"),
        "{german_blog}"
    );
    assert!(output.join("blog/page/2.html").is_file());
    assert!(!output.join("de/blog/page/2.html").exists());
    let english_blog = fs::read_to_string(output.join("blog/index.html")).expect("English blog");
    assert!(
        english_blog.contains("class=\"page-navigation pagination\""),
        "{english_blog}"
    );
    assert!(
        english_blog.contains(
            "<link rel=\"canonical\" href=\"https://example.org/my-project/blog/index.html\">"
        ),
        "{english_blog}"
    );
    assert!(
        english_blog.contains("href=\"/my-project/atom.xml\">Subscribe with Atom</a>"),
        "{english_blog}"
    );
    assert!(
        english_blog.contains("<span aria-current=\"page\">1</span>"),
        "{english_blog}"
    );
    assert!(
        english_blog.contains(
            "href=\"/my-project/de/blog/index.html\" hreflang=\"de\" aria-label=\"German\""
        ),
        "{english_blog}"
    );
    let tags = fs::read_to_string(output.join("tags/index.html")).expect("English tag index");
    assert!(tags.contains("<h1>Tags</h1>"), "{tags}");
    assert!(
        tags.contains(
            "<link rel=\"canonical\" href=\"https://example.org/my-project/tags/index.html\">"
        ),
        "{tags}"
    );
    assert!(
        tags.contains(
            "<meta name=\"description\" content=\"Browse the tags used by posts in this Heine starter site.\">"
        ),
        "{tags}"
    );
    assert!(tags.contains("<ul class=\"compact-index\">"), "{tags}");
    assert!(tags.contains("site-navigation"), "{tags}");
    assert!(!tags.contains("width=\"\""), "{tags}");
    assert!(!tags.contains("height=\"\""), "{tags}");
    assert!(
        tags.contains("href=\"/my-project/tags/index.html\">Tags</a>"),
        "{tags}"
    );
    assert!(
        tags.contains("href=\"/my-project/de/tags/index.html\" hreflang=\"de\""),
        "{tags}"
    );
    assert!(!tags.contains("<main>\n      <p><a"), "{tags}");
    assert!(
        tags.contains("A compact example site built with <a href=\"https://heine.rs\">Heine</a>."),
        "{tags}"
    );
    assert!(
        tags.contains("href=\"/my-project/legal/licenses/index.html\">Attributions</a>"),
        "{tags}"
    );
    assert!(
        tags.contains(
            "<a href=\"../tags/MathML.html\">MathML</a> <span class=\"item-count\">1</span>"
        ),
        "{tags}"
    );
    assert!(output.join("de/tags/index.html").is_file());
    let series_index =
        fs::read_to_string(output.join("series/index.html")).expect("English Series index");
    assert!(series_index.contains("<h1>Series</h1>"), "{series_index}");
    assert!(
        series_index.contains(
            "<link rel=\"canonical\" href=\"https://example.org/my-project/series/index.html\">"
        ),
        "{series_index}"
    );
    assert!(
        series_index.contains(
            "<meta name=\"description\" content=\"Ordered reading paths through this example site.\">"
        ),
        "{series_index}"
    );
    assert!(
        series_index.contains("<ul class=\"compact-index\">"),
        "{series_index}"
    );
    assert!(
        series_index.contains(
            "<a href=\"../series/heinrich-heine/index.html\">Heinrich Heine</a> <span class=\"item-count\">3</span>"
        ),
        "{series_index}"
    );
    assert!(
        series_index.contains("href=\"/my-project/de/series/index.html\" hreflang=\"de\""),
        "{series_index}"
    );
    let rust_tag = fs::read_to_string(output.join("tags/Rust.html")).expect("Rust tag listing");
    assert!(rust_tag.contains("Math, Markdown, and Media"), "{rust_tag}");
    assert!(rust_tag.contains("<ul class=\"page-list\">"), "{rust_tag}");
    assert!(
        rust_tag.contains("href=\"/my-project/de/tags/Rust.html\" hreflang=\"de\""),
        "{rust_tag}"
    );
    assert!(!rust_tag.contains("width=\"\""), "{rust_tag}");
    assert!(!rust_tag.contains("height=\"\""), "{rust_tag}");
    let german_static_tag = fs::read_to_string(output.join("de/tags/Statische Seiten.html"))
        .expect("German tag listing");
    assert!(
        german_static_tag.contains("href=\"/my-project/tags/Static%20sites.html\" hreflang=\"en\""),
        "{german_static_tag}"
    );

    let german_post = fs::read_to_string(output.join("de/blog/explicit-composition/index.html"))
        .expect("German post");
    assert!(
        german_post.contains("aria-label=\"Sprache\""),
        "{german_post}"
    );
    assert!(german_post.contains("hreflang=\"en\""), "{german_post}");
    assert!(!german_post.contains(" lang=\"en\""), "{german_post}");
    assert!(
        german_post.contains("aria-label=\"Englisch\""),
        "{german_post}"
    );
    assert!(
        german_post.contains("<dt>Schlagwort</dt>")
            && german_post.contains("<a href=\"../../../de/tags/Rust.html\">Rust</a>"),
        "{german_post}"
    );

    let post = fs::read_to_string(output.join("blog/mathml-audio/index.html")).expect("post page");
    assert!(
        post.contains("href=\"../../blog/explicit-composition/index.html\""),
        "{post}"
    );
    assert!(post.contains("class=\"navigation-arrow\""), "{post}");
    let header_end = post.find("</header>").expect("global header ends");
    let local_navigation = post
        .find("class=\"page-local-navigation\" aria-label=\"Page navigation\"")
        .expect("page-local navigation renders");
    let post_metadata = post
        .find("class=\"post-meta\"")
        .expect("post metadata renders");
    assert!(
        header_end < local_navigation && local_navigation < post_metadata,
        "{post}"
    );
    let guide = fs::read_to_string(output.join("heine/index.html")).expect("Heine guide");
    assert!(guide.contains("class=\"table-of-contents\""), "{guide}");
    assert!(
        guide.contains(
            "<nav class=\"table-of-contents\" aria-label=\"Table of contents\">\n    <ul>"
        ),
        "{guide}"
    );
    assert!(guide.contains("href=\"#a-short-guide\""), "{guide}");
    assert!(guide.contains("<h2 id=\"a-short-guide\">"), "{guide}");
    assert!(
        guide.contains(
            "<source media=\"(prefers-color-scheme: dark)\" srcset=\"/my-project/assets/images/heine-dark.jpg\">"
        ),
        "{guide}"
    );
    assert!(
        guide.contains(
            "<img src=\"/my-project/assets/images/heine-light.jpg\" width=\"480\" height=\"540\" alt=\"Portrait of Heinrich Heine\">"
        ),
        "{guide}"
    );
    assert!(
        guide.contains("<h2 id=\"sources-and-image-attribution\">"),
        "{guide}"
    );
    let guide_life =
        fs::read_to_string(output.join("heine/life/index.html")).expect("Heine guide chapter");
    assert!(
        guide_life.contains("<main class=\"has-margin-footnotes\">"),
        "{guide_life}"
    );
    assert!(
        guide_life.contains("class=\"heine-annotated-block\""),
        "{guide_life}"
    );
    assert!(guide_life.contains("aria-label=\"Footnote"), "{guide_life}");
    assert!(
        guide_life.contains("class=\"heine-footnote-number\" aria-hidden=\"true\">1</span>"),
        "{guide_life}"
    );
    assert!(guide_life.contains("role=\"doc-footnote\""), "{guide_life}");
    let attributions =
        fs::read_to_string(output.join("legal/licenses/index.html")).expect("attributions page");
    assert!(
        attributions.contains("<ul class=\"compact-index attribution-list\">"),
        "{attributions}"
    );
    assert!(
        attributions.contains("STIXTwoMath-Regular.woff2"),
        "{attributions}"
    );
    assert!(
        attributions
            .contains("Public-domain portrait of Heinrich Heine by Moritz Daniel Oppenheim"),
        "{attributions}"
    );
    assert!(
        attributions.contains("Public-domain copperplate engraving of Heinrich Heine"),
        "{attributions}"
    );
    assert!(
        attributions.contains("<a href=\"../../LICENSES/OFL-1.1.txt\">OFL-1.1</a>"),
        "{attributions}"
    );
    let german_attributions = fs::read_to_string(output.join("de/legal/licenses/index.html"))
        .expect("German attributions page");
    assert!(
        german_attributions
            .contains("Gemeinfreies Porträt Heinrich Heines von Moritz Daniel Oppenheim"),
        "{german_attributions}"
    );
    assert!(
        german_attributions.contains("Gemeinfreier Kupferstich von Heinrich Heine"),
        "{german_attributions}"
    );
    assert!(output.join("LICENSES/OFL-1.1.txt").is_file());
    assert!(
        post.contains("<audio controls preload=\"metadata\">"),
        "{post}"
    );
    assert!(
        post.contains("src=\"audio/heine-introduction.mp3\""),
        "{post}"
    );
    assert!(
        post.contains("<blockquote lang=\"ar\" dir=\"rtl\">"),
        "{post}"
    );
    assert!(post.contains("لا يوجد طريقٌ ملوكيٌّ إلى الهندسة."), "{post}");
    assert!(post.contains("<del>unnecessary ceremony</del>"), "{post}");
    assert!(
        post.contains(
            "<input disabled=\"\" type=\"checkbox\" checked=\"\" aria-label=\"Keep the formula in the article.\"/>"
        ),
        "{post}"
    );
    assert!(post.contains("<table>"), "{post}");
    assert!(post.contains("class=\"footnote-reference\""), "{post}");
    assert!(post.contains("<math display=\"inline\">"), "{post}");
    assert!(post.contains("class=\"table-of-contents\""), "{post}");
    assert!(
        output
            .join("blog/mathml-audio/audio/heine-introduction.mp3")
            .is_file()
    );

    let terms = fs::read_to_string(output.join("legal/terms/index.html")).expect("terms page");
    assert!(
        terms.contains("href=\"/my-project/legal/privacy/index.html\""),
        "{terms}"
    );
    assert!(
        terms.contains("href=\"/my-project/de/legal/terms/index.html\" hreflang=\"de\""),
        "{terms}"
    );
    assert!(output.join("legal/privacy/index.html").is_file());
    assert!(output.join("legal/imprint/index.html").is_file());

    let about = fs::read_to_string(output.join("about/index.html")).expect("About page");
    assert!(
        about.contains("Heine is a small static-site generator written in Rust."),
        "{about}"
    );
    assert!(
        about.contains("href=\"/my-project/de/about/index.html\" hreflang=\"de\""),
        "{about}"
    );
    let german_about =
        fs::read_to_string(output.join("de/about/index.html")).expect("German About page");
    assert!(
        german_about.contains("Heine ist ein kleiner statischer Site-Generator"),
        "{german_about}"
    );
    assert!(
        german_about.contains("href=\"/my-project/heine/index.html\""),
        "{german_about}"
    );

    let reading =
        fs::read_to_string(output.join("de/lesen/nachtgedanken/index.html")).expect("reading");
    assert!(reading.contains("<h1>Nachtgedanken</h1>"), "{reading}");
    let sleep = reading.find("<h2>Schlaflos</h2>");
    let mother = reading.find("<h2>Die alte Frau</h2>");
    let remains = reading.find("<h2>Was bleibt, was vergeht</h2>");
    let paris = reading.find("<h2>Ein Morgen in Paris</h2>");
    assert!(
        matches!((sleep, mother, remains, paris), (Some(sleep), Some(mother), Some(remains), Some(paris)) if sleep < mother && mother < remains && remains < paris),
        "{reading}"
    );
    assert!(
        reading.contains("Denk ich an Deutschland in der Nacht"),
        "{reading}"
    );
    assert!(
        reading.contains("<h2>Quellen und Hinweise</h2>"),
        "{reading}"
    );
    assert!(
        reading.contains("href=\"https://de.wikisource.org/wiki/Nachtgedanken_(Heine)\""),
        "{reading}"
    );
    assert!(
        reading.contains("href=\"../../../de/lesen/index.html\""),
        "{reading}"
    );
    assert!(
        reading.contains("href=\"../../../de/authors/Heinrich%20Heine.html\""),
        "{reading}"
    );
    assert!(
        reading.contains("href=\"../../../de/authors/index.html\""),
        "{reading}"
    );
    assert!(!reading.contains("language-navigation"), "{reading}");
    let reading_index =
        fs::read_to_string(output.join("de/lesen/index.html")).expect("reading index");
    assert!(
        reading_index.contains("href=\"../../de/lesen/nachtgedanken/index.html\""),
        "{reading_index}"
    );
    assert!(
        reading_index.contains("Heinrich Heines Nachtgedanken"),
        "{reading_index}"
    );
    let authors = fs::read_to_string(output.join("de/authors/Heinrich Heine.html"))
        .expect("German author listing");
    assert!(
        authors.contains("<h1>Autor: Heinrich Heine</h1>"),
        "{authors}"
    );
    assert!(
        authors.contains("href=\"../../de/lesen/nachtgedanken/index.html\""),
        "{authors}"
    );
    assert!(output.join("authors/index.html").is_file());
    assert!(!output.join("quiz").exists());

    build_at_with_options(root, BuildOptions::new(BuildMode::Full).preview_drafts())
        .expect("starter-site draft preview builds");
    let draft = fs::read_to_string(output.join("blog/draft-publication/index.html"))
        .expect("draft post preview");
    assert!(
        draft.contains(
            "<h1><span class=\"page-title\"><span class=\"draft-marker\">DRAFT</span><span class=\"page-title-text\">A Page Before Publication</span></span></h1>"
        ),
        "{draft}"
    );
    let blog = fs::read_to_string(output.join("blog/index.html")).expect("preview blog index");
    assert!(
        blog.contains("<span class=\"draft-marker\">DRAFT</span>"),
        "{blog}"
    );
    let search = fs::read_to_string(generated_file_with_extension(
        &output.join("search"),
        "json",
    ))
    .expect("preview search index");
    assert!(search.contains("\"draft\":true"), "{search}");
}

#[test]
fn starter_kit_reference_site_materializes_as_an_ordinary_site() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("starter-site");
    let kit = source.join("kits/starter");
    let temporary = TestDirectory::new();
    let root = temporary.path();

    copy_tree(&kit.join("site"), root);
    copy_tree(&kit, &root.join("kits/starter"));

    build_at(root, BuildMode::Full).expect("starter kit reference site builds");

    let output = root.join("public/my-project");
    assert!(output.join("index.html").is_file());
    assert!(output.join("assets/css/styles.v2.css").is_file());
    assert!(output.join("LICENSES/OFL-1.1.txt").is_file());
}

fn generated_file_with_extension(directory: &Path, extension: &str) -> std::path::PathBuf {
    fs::read_dir(directory)
        .expect("generated resource directory")
        .map(|entry| entry.expect("generated resource entry").path())
        .find(|path| path.extension().is_some_and(|value| value == extension))
        .expect("generated resource")
}

fn copy_tree(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).expect("source directory") {
        let entry = entry.expect("source entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            copy_file(&source_path, &destination_path);
        }
    }
}

fn copy_file(source: &Path, destination: &Path) {
    fs::create_dir_all(destination.parent().expect("destination parent"))
        .expect("create destination parent");
    fs::copy(source, destination).expect("copy starter-site source");
}
