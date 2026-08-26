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
    for directory in ["content", "data", "locales", "templates"] {
        copy_tree(&source.join(directory), &root.join(directory));
    }

    build_at(root, BuildMode::Full).expect("starter site builds");

    let output = root.join("public/my-project");
    let index = fs::read_to_string(output.join("index.html")).expect("home page");
    assert!(
        index.contains("<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"en\" dir=\"ltr\">"),
        "{index}"
    );
    assert!(index.contains("Heine Starter Site"), "{index}");
    assert!(index.contains("Quiz"), "{index}");
    assert!(index.contains("class=\"language-navigation\""), "{index}");
    assert!(
        index.contains("href=\"/my-project/de/index.html\" hreflang=\"de\" aria-label=\"German\""),
        "{index}"
    );
    assert!(
        index.contains("src=\"/my-project/assets/images/flags/german.svg\""),
        "{index}"
    );
    assert!(
        index.contains("So far there are \u{2068}3\u{2069} posts."),
        "{index}"
    );

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

    let german_blog = fs::read_to_string(output.join("de/blog/index.html")).expect("German blog");
    assert!(
        german_blog.contains("Bisher gibt es \u{2068}2\u{2069} Beiträge auf Deutsch."),
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
    assert!(
        tags.contains("href=\"/my-project/tags/index.html\">Tags</a>"),
        "{tags}"
    );
    assert!(!tags.contains("<main>\n      <p><a"), "{tags}");
    assert!(
        tags.contains("A compact example site built with Heine."),
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
    assert!(rust_tag.contains("14th August 2026"), "{rust_tag}");

    let german_post =
        fs::read_to_string(output.join("de/blog/2026-08-15.html")).expect("German post");
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

    let post = fs::read_to_string(output.join("blog/2026-08-14.html")).expect("post page");
    assert!(post.contains("<img"), "{post}");
    assert!(post.contains("width="), "{post}");
    assert!(
        post.contains("<audio controls preload=\"metadata\">"),
        "{post}"
    );
    assert!(
        post.contains("src=\"md/audio/heine-introduction.mp3\""),
        "{post}"
    );
    assert!(
        post.contains("<blockquote lang=\"ar\" dir=\"rtl\">"),
        "{post}"
    );
    assert!(post.contains("لا يوجد طريقٌ ملوكيٌّ إلى الهندسة."), "{post}");
    assert!(
        output
            .join("blog/md/audio/heine-introduction.mp3")
            .is_file()
    );

    let quiz = fs::read_to_string(output.join("quiz/quiz-1.html")).expect("quiz page");
    assert!(quiz.contains("quiz"), "{quiz}");
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
