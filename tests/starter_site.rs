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
    for directory in ["content", "data", "templates"] {
        copy_tree(&source.join(directory), &root.join(directory));
    }

    build_at(root, BuildMode::Full).expect("starter site builds");

    let output = root.join("public/my-project");
    let index = fs::read_to_string(output.join("index.html")).expect("home page");
    assert!(index.contains("Heine Starter Site"), "{index}");
    assert!(index.contains("Quiz"), "{index}");

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
        output
            .join("blog/md/audio/heine-introduction.mp3")
            .is_file()
    );

    let quiz = fs::read_to_string(output.join("quiz/quiz-1.html")).expect("quiz page");
    assert!(quiz.contains("quiz"), "{quiz}");
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
