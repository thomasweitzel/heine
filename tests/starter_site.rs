//! The starter site is a supported example, so it is built as an integration
//! fixture rather than merely kept as documentation.

use std::{fs, path::Path};

use heine::{BuildMode, build_at};

#[path = "support/temp_dir.rs"]
mod temp_dir;

use temp_dir::TestDirectory;

#[test]
fn builds_the_documented_starter_site() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("starter-site");
    let temporary = TestDirectory::new();
    let root = temporary.path();

    copy_file(&source.join("heine.toml"), &root.join("heine.toml"));
    for directory in [
        "content",
        "data",
        "deploy",
        "LICENSES",
        "locales",
        "templates",
    ] {
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
    let index = fs::read_to_string(output.join("index.html")).expect("home page");
    assert!(
        index.contains("<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"en\" dir=\"ltr\">"),
        "{index}"
    );
    assert!(index.contains("Heine Starter Site"), "{index}");
    assert!(index.contains("Quiz"), "{index}");
    assert!(index.contains("<h2>Heinrich Heine</h2>"), "{index}");
    assert!(
        index.contains("A short, source-linked guide to the writer whose name this site borrows."),
        "{index}"
    );
    assert!(
        index.contains("href=\"/my-project/heine/index.html\">Heinrich Heine</a>"),
        "{index}"
    );
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
    assert!(output.join("assets/css/styles.v2.css").is_file());
    assert!(!output.join("assets/css/styles.v1.css").exists());

    let german_home = fs::read_to_string(output.join("de/index.html")).expect("German home");
    assert!(
        german_home
            .contains("<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"de\" dir=\"ltr\">"),
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
    assert!(!german_blog.contains("Quiz"), "{german_blog}");
    assert!(output.join("blog/page/2.html").is_file());
    assert!(!output.join("de/blog/page/2.html").exists());
    let english_blog = fs::read_to_string(output.join("blog/index.html")).expect("English blog");
    assert!(
        english_blog.contains("class=\"page-navigation pagination\""),
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
    assert!(tags.contains("<ul>"), "{tags}");
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
            "<a href=\"../tags/MathML.html\">MathML</a> <span class=\"taxonomy-count\">1</span>"
        ),
        "{tags}"
    );
    assert!(output.join("de/tags/index.html").is_file());
    let rust_tag = fs::read_to_string(output.join("tags/Rust.html")).expect("Rust tag listing");
    assert!(
        rust_tag.contains("MathML, Audio, and Direction"),
        "{rust_tag}"
    );
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
    assert!(german_post.contains("Schlagwort:"), "{german_post}");

    let post = fs::read_to_string(output.join("blog/mathml-audio/index.html")).expect("post page");
    assert!(
        post.contains(
            "href=\"../../blog/explicit-composition/index.html\"><span aria-hidden=\"true\">←</span><span>Composing Content Explicitly</span>"
        ),
        "{post}"
    );
    assert!(
        post.contains("class=\"page-navigation\" aria-label=\"Page navigation\""),
        "{post}"
    );
    let guide = fs::read_to_string(output.join("heine/index.html")).expect("Heine guide");
    assert!(guide.contains("class=\"table-of-contents\""), "{guide}");
    assert!(guide.contains("href=\"#a-short-guide\""), "{guide}");
    assert!(guide.contains("<h2 id=\"a-short-guide\">"), "{guide}");
    assert!(
        guide.contains("<h2 id=\"sources-and-image-attribution\">"),
        "{guide}"
    );
    let attributions =
        fs::read_to_string(output.join("legal/licenses/index.html")).expect("attributions page");
    assert!(
        attributions.contains("STIXTwoMath-Regular.woff2"),
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
        about.contains("Heinrich Heine was born in Düsseldorf in 1797"),
        "{about}"
    );
    assert!(
        about.contains("href=\"/my-project/de/about/index.html\" hreflang=\"de\""),
        "{about}"
    );

    let quiz = fs::read_to_string(output.join("quiz/quiz-1.html")).expect("quiz page");
    assert!(quiz.contains("quiz"), "{quiz}");
    assert!(
        quiz.contains("<button class=\"quiz-answer q-correct\" type=\"button\">Berlin</button>"),
        "{quiz}"
    );
    assert!(!quiz.contains("language-navigation"), "{quiz}");
    assert!(output.join("quiz/script.js").is_file());
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
