use std::{fs, path::Path};

mod temp_dir;

use temp_dir::TestDirectory;

pub(crate) struct Fixture(TestDirectory);

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = TestDirectory::new();
        fs::create_dir_all(root.path().join("content")).expect("fixture content directory");
        fs::create_dir_all(root.path().join("templates")).expect("fixture template directory");
        Self(root)
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }

    pub(crate) fn write(&self, relative: impl AsRef<Path>, contents: &str) {
        let path = self.path().join(relative);
        fs::create_dir_all(path.parent().expect("fixture file has a parent"))
            .expect("fixture parent directory");
        fs::write(path, contents).expect("fixture file");
    }
}
